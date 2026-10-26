//! `tx`: a video's transcript in your terminal, or ask your agent about it.
#![recursion_limit = "256"]

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::io::{IsTerminal, Write};
use std::process::{ExitCode, Stdio};
use tx_core::{agent, captions, db, paths, report, ytdlp};

mod agents;
mod guide;
mod mcp;
mod onboard;
mod review;
mod schedule;
mod terminal;

#[derive(Parser)]
#[command(
    name = "tx",
    version,
    about = "Print a video's transcript (any yt-dlp-supported URL), or ask an agent about it.",
    args_conflicts_with_subcommands = true,
    group(clap::ArgGroup::new("analysis_mode").args(["ask", "review", "chat"]))
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Video URL (YouTube, X, Vimeo, TikTok, …)
    url: Option<String>,
    /// Stream a free-form answer from the selected or detected agent
    #[arg(short, long, value_name = "QUESTION", conflicts_with = "chat")]
    ask: Option<String>,
    /// Colored video review; omit QUESTION for the default worth-watching assessment
    #[arg(short = 'q', long, num_args = 0..=1, default_missing_value = "Is this video worth watching? Which parts should I watch, and why?", value_name = "QUESTION", requires = "url", conflicts_with_all = ["ask", "chat", "no_timestamps"])]
    review: Option<String>,
    /// Color for --review (auto respects NO_COLOR and redirected output)
    #[arg(short = 'C', long, value_enum, default_value_t = ReviewColor::Auto, requires = "review")]
    color: ReviewColor,
    /// Interactive session with the selected or detected agent on the transcript
    #[arg(short, long)]
    chat: bool,
    /// Agent and optional model: codex[:MODEL] or claude[:MODEL]; overrides TX_AGENT/TX_CHAT_AGENT
    #[arg(short = 'g', long, value_name = "AGENT[:MODEL]", requires_all = ["url", "analysis_mode"])]
    agent: Option<agents::Selection>,
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

#[derive(Clone, Copy, clap::ValueEnum)]
enum ReviewColor {
    Auto,
    Always,
    Never,
}

#[derive(Subcommand)]
enum Command {
    /// Set tx up step by step: your AI apps, what you want to learn, what to follow, a daily brief
    Onboard,
    /// Start over as a new user: forget your profile and what you follow, turn off the daily brief
    Reset {
        /// Also delete every analysis, digest and cached transcript
        #[arg(long)]
        all: bool,
        /// Don't ask for confirmation
        #[arg(long)]
        yes: bool,
    },
    /// MCP server over stdio, for Claude Code, Claude Desktop, Codex and other MCP clients
    Mcp {
        /// Data folder to use (default: the OS data folder, or $TX_DATA_DIR)
        #[arg(long)]
        data_dir: Option<std::path::PathBuf>,
    },
    /// Screen new videos once with your agent (uses your Claude or ChatGPT subscription)
    Run {
        /// Which agent does the analysis (default: whichever is installed)
        #[arg(long, value_enum)]
        agent: Option<agents::Agent>,
    },
    /// Make tx available inside an agent's normal conversations
    Setup {
        #[arg(value_enum)]
        target: agents::Target,
    },
    /// Run `tx run` automatically (daily by default) with the OS scheduler
    Schedule {
        #[arg(long, value_enum)]
        agent: Option<agents::Agent>,
        /// Daily time, HH:MM (default 08:00)
        #[arg(long, conflicts_with = "every")]
        at: Option<String>,
        /// Every N hours instead, e.g. 3h
        #[arg(long)]
        every: Option<String>,
        /// Remove the schedule
        #[arg(long, conflicts_with_all = ["at", "every", "agent"])]
        off: bool,
        /// Show the current schedule
        #[arg(long, conflicts_with_all = ["at", "every", "agent", "off"])]
        status: bool,
    },
    /// Write a markdown digest of videos analyzed since the last one
    Digest {
        /// Save digests in this folder from now on (e.g. an iCloud, OneDrive or Drive folder)
        #[arg(long)]
        dir: Option<std::path::PathBuf>,
    },
}

/// Desktop notification for a digest with something worth the user's time.
pub fn notify_digest(d: &report::Digest) {
    if d.worth == 0 {
        return;
    }
    let body = match &d.path {
        Some(p) => format!(
            "{} of {} new videos are worth your time.\n{}",
            d.worth,
            d.total,
            p.display()
        ),
        None => format!("{} new videos are worth your time.", d.worth),
    };
    if let Err(e) = notify_rust::Notification::new()
        .appname("tx")
        .summary("tx digest")
        .body(&body)
        .show()
    {
        eprintln!("tx: notification failed: {e}");
    }
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
    match cli.command {
        Some(Command::Reset { all, yes }) => {
            onboard::reset(all, yes)?;
            return Ok(0);
        }
        Some(Command::Onboard) => {
            onboard::run().await?;
            return Ok(0);
        }
        Some(Command::Mcp { data_dir }) => {
            if let Some(dir) = data_dir {
                paths::set_data_dir(dir);
            }
            mcp::serve().await?;
            return Ok(0);
        }
        Some(Command::Run { agent }) => {
            // YouTube breaks old yt-dlp releases; refresh tx's copy at most once a day.
            {
                let conn = db::connect()?;
                let last = db::get_value(&conn, "ytdlp_checked_at")?;
                let stale = last
                    .and_then(|t| chrono::DateTime::parse_from_rfc3339(&t).ok())
                    .is_none_or(|t| chrono::Utc::now().signed_duration_since(t).num_hours() >= 24);
                if stale {
                    match ytdlp::keep_current().await {
                        Ok(Some(v)) => eprintln!("tx: updated yt-dlp {v}"),
                        Ok(None) => {}
                        Err(e) => eprintln!("tx: yt-dlp update check failed: {e:#}"),
                    }
                    db::set_value(&conn, "ytdlp_checked_at", &db::now())?;
                }
            }
            let agent = match agent {
                Some(a) => a,
                None => agents::Agent::detect()?,
            };
            let code = tokio::task::spawn_blocking(move || agents::run(agent)).await??;
            // Catch anything the agent analyzed but didn't put in a digest.
            let d = report::write_digest(&db::connect()?)?;
            notify_digest(&d);
            if let Some(p) = &d.path {
                eprintln!("tx: digest written to {}", p.display());
            }
            return Ok(code);
        }
        Some(Command::Setup { target }) => {
            agents::setup(target)?;
            return Ok(0);
        }
        Some(Command::Schedule {
            agent,
            at,
            every,
            off,
            status,
        }) => {
            if status {
                let s = schedule::status()?;
                println!(
                    "{}",
                    if s.is_empty() {
                        "tx run is not scheduled.".to_string()
                    } else {
                        s
                    }
                );
            } else if off {
                schedule::uninstall()?;
                println!("tx run is no longer scheduled.");
            } else {
                let agent = match agent {
                    Some(a) => a,
                    None => agents::Agent::detect()?,
                };
                let when = schedule::install(
                    agent,
                    schedule::When::parse(at.as_deref(), every.as_deref())?,
                )?;
                println!(
                    "tx run --agent {agent:?} scheduled {when}. Check with `tx schedule --status`."
                );
            }
            return Ok(0);
        }
        Some(Command::Digest { dir }) => {
            let conn = db::connect()?;
            if let Some(dir) = dir {
                std::fs::create_dir_all(&dir)?;
                db::set_value(
                    &conn,
                    "digest_dir",
                    &dir.canonicalize()?.display().to_string(),
                )?;
            }
            let d = report::write_digest(&conn)?;
            notify_digest(&d);
            match &d.path {
                Some(p) => println!("{}\n(saved to {})", d.markdown, p.display()),
                None => println!("{}", d.markdown),
            }
            println!(
                "Digests are saved in {}",
                report::digest_dir(&conn)?.display()
            );
            return Ok(0);
        }
        None => {}
    }
    let Some(url) = cli.url else {
        // A bare `tx` walks through setup until it's done, then shows where you stand.
        use std::io::IsTerminal;
        let needs_setup = guide::needs_setup(&std::sync::Mutex::new(db::connect()?))?;
        if needs_setup && !std::io::stdin().is_terminal() {
            anyhow::bail!("tx isn't set up yet; run `tx` in a terminal to set it up");
        }
        if needs_setup {
            onboard::run().await?;
        } else {
            onboard::home()?;
        }
        return Ok(0);
    };
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

    if let Some(question) = cli.review {
        eprintln!("tx: transcript ready, reviewing with your agent…");
        let selected = agents::Selection::resolve(cli.agent.as_ref(), false)?;
        let analysis = review::analyze(&t, &text, &question, selected).await?;
        let term = dialoguer::console::Term::stdout();
        let width = term
            .size_checked()
            .map_or(80, |(_, columns)| usize::from(columns));
        let color = match cli.color {
            ReviewColor::Always => true,
            ReviewColor::Never => false,
            ReviewColor::Auto => {
                std::io::stdout().is_terminal()
                    && std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty())
                    && std::env::var("TERM").as_deref() != Ok("dumb")
            }
        };
        std::io::stdout().write_all(terminal::render(&t, &analysis, width, color).as_bytes())?;
        return Ok(0);
    }

    if let Some(question) = cli.ask {
        let prompt = format!(
            "stdin is the transcript of the video \"{}\" ({}). Answer from the transcript; cite [mm:ss] timestamps where useful. Match the length the question asks for.\n\nQuestion: {question}",
            t.title, t.url
        );
        if std::io::stderr().is_terminal() {
            eprintln!("tx: transcript ready ({}), asking the agent…", t.source);
        }
        // Print the answer as it is written instead of all at once at the end.
        let mut out = std::io::stdout();
        let mut printed = false;
        let selected = agents::Selection::resolve(cli.agent.as_ref(), false)?;
        let answer = agent::ask_stream(&prompt, &text, selected, |piece| {
            printed = true;
            let _ = out.write_all(piece.as_bytes());
            let _ = out.flush();
        })
        .await?;
        if !printed {
            print!("{answer}");
        }
        println!();
        return Ok(0);
    }
    if cli.chat {
        let md = cache_path.with_extension("md");
        std::fs::write(&md, &text)?;
        let (program, args) = agents::Selection::resolve(cli.agent.as_ref(), true)?;
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

fn main() -> ExitCode {
    #[cfg(target_os = "linux")]
    paths::restore_session_env();
    let result = tokio::runtime::Runtime::new()
        .map_err(anyhow::Error::from)
        .and_then(|rt| rt.block_on(run(Cli::parse())));
    match result {
        Ok(code) => ExitCode::from(code.clamp(0, 255) as u8),
        Err(e) => {
            eprintln!("tx: {e:#}");
            ExitCode::FAILURE
        }
    }
}
