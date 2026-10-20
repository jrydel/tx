//! `tx`: a video's transcript in your terminal, or ask your agent about it.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::io::{IsTerminal, Write};
use std::process::{ExitCode, Stdio};
use tx_core::{agent, captions, engine::Engine, paths, ytdlp};

#[derive(Parser)]
#[command(
    name = "tx",
    version,
    about = "Print a video's transcript (any yt-dlp-supported URL), or ask an agent about it.",
    args_conflicts_with_subcommands = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Video URL (YouTube, X, Vimeo, TikTok, …)
    url: Option<String>,
    /// One-shot question to $TX_AGENT (default: claude -p)
    #[arg(short, long, value_name = "QUESTION", conflicts_with = "chat")]
    ask: Option<String>,
    /// Interactive $TX_CHAT_AGENT (default: claude) session on the transcript
    #[arg(short, long)]
    chat: bool,
    /// Caption/audio language code, e.g. en, cs (default: video language, then en)
    #[arg(short, long)]
    lang: Option<String>,
    /// Skip platform captions; transcribe the audio locally
    #[arg(short, long)]
    whisper: bool,
    /// Whisper model: tiny, base, small, medium, large-v3-turbo
    #[arg(short, long, env = "TX_WHISPER_MODEL", default_value = "small")]
    model: String,
    /// Omit [mm:ss] paragraph markers
    #[arg(short = 'T', long)]
    no_timestamps: bool,
    /// Ignore the cached transcript
    #[arg(short, long)]
    refresh: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Check watched channels and analyze new videos once (what the desktop app does in the background)
    Check,
}

fn page(text: &str) -> Result<()> {
    let pager = std::env::var("PAGER").unwrap_or_else(|_| "less -FRX".into());
    let mut words = pager.split_whitespace();
    let spawned = std::io::stdout()
        .is_terminal()
        .then(|| {
            std::process::Command::new(words.next()?)
                .args(words)
                .stdin(Stdio::piped())
                .spawn()
                .ok()
        })
        .flatten();
    match spawned {
        Some(mut child) => {
            let _ = child.stdin.take().unwrap().write_all(text.as_bytes()); // broken pipe = user quit the pager
            child.wait()?;
        }
        None => std::io::stdout().write_all(text.as_bytes())?,
    }
    Ok(())
}

async fn run(cli: Cli) -> Result<i32> {
    if let Some(Command::Check) = cli.command {
        let engine =
            Engine::open(|n| eprintln!("tx: {} — {}", n.title, n.body.replace('\n', " · ")))?;
        let (queued, analyzed) = engine.check().await?;
        eprintln!("tx: {queued} new video(s) queued, {analyzed} analyzed");
        return Ok(0);
    }
    let url = cli.url.context("pass a video URL (see tx --help)")?;
    let progress: ytdlp::Progress = Box::new(|m| eprintln!("tx: {m}"));
    let (t, cache_path) = ytdlp::load(
        &url,
        cli.lang.as_deref(),
        cli.whisper,
        &cli.model,
        cli.refresh,
        &progress,
    )
    .await?;
    let text = captions::render(&t, !cli.no_timestamps);

    if let Some(question) = cli.ask {
        let (program, args) = agent::command("TX_AGENT", "claude -p")
            .context("Claude Code (claude) is not installed")?;
        let prompt = format!(
            "stdin is the transcript of the video \"{}\" ({}). Answer from the transcript; cite [mm:ss] timestamps where useful.\n\nQuestion: {question}",
            t.title, t.url
        );
        let mut child = std::process::Command::new(program)
            .args(args)
            .arg(prompt)
            .env("PATH", paths::search_path())
            .stdin(Stdio::piped())
            .spawn()?;
        child.stdin.take().unwrap().write_all(text.as_bytes())?;
        return Ok(child.wait()?.code().unwrap_or(1));
    }
    if cli.chat {
        let md = cache_path.with_extension("md");
        std::fs::write(&md, &text)?;
        let (program, args) = agent::command("TX_CHAT_AGENT", "claude")
            .context("Claude Code (claude) is not installed")?;
        let prompt = format!(
            "Read the transcript of the video \"{}\" at {}. Reply with a 5-bullet summary, then answer my questions from it, citing [mm:ss] timestamps.",
            t.title,
            md.display()
        );
        let status = std::process::Command::new(program)
            .args(args)
            .arg(prompt)
            .env("PATH", paths::search_path())
            .status()?;
        return Ok(status.code().unwrap_or(1));
    }
    page(&text)?;
    Ok(0)
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Cli::parse()).await {
        Ok(code) => ExitCode::from(code.clamp(0, 255) as u8),
        Err(e) => {
            eprintln!("tx: {e:#}");
            ExitCode::FAILURE
        }
    }
}
