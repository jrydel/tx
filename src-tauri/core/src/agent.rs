//! The user's coding agent CLI (default: Claude Code) does the analysis, on their own subscription.

use crate::paths;
use anyhow::{Context, Result, bail};
use regex::Regex;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::LazyLock;
use tokio::io::AsyncWriteExt;

/// `$TX_AGENT` (default `claude -p`), first word resolved on the extended PATH.
pub fn command(env_key: &str, default: &str) -> Option<(PathBuf, Vec<String>)> {
    let spec = std::env::var(env_key).unwrap_or_else(|_| default.to_string());
    let mut words = spec.split_whitespace().map(String::from);
    let program = words.next()?;
    let path =
        paths::which(&program).or_else(|| Some(PathBuf::from(&program)).filter(|p| p.is_file()))?;
    Some((path, words.collect()))
}

pub fn available() -> bool {
    command("TX_AGENT", "claude -p").is_some()
}

/// One-shot `$TX_AGENT <prompt>` with `stdin` piped in; returns stdout.
pub async fn ask(prompt: &str, stdin: &str) -> Result<String> {
    let (program, args) =
        command("TX_AGENT", "claude -p").context("Claude Code (claude) is not installed")?;
    let mut child = paths::command(&program)
        .args(args)
        .arg(prompt)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("starting {}", program.display()))?;
    let mut input = child.stdin.take().unwrap();
    let body = stdin.to_string();
    let writer = tokio::spawn(async move {
        let _ = input.write_all(body.as_bytes()).await;
    });
    let out = tokio::time::timeout(
        std::time::Duration::from_secs(900),
        child.wait_with_output(),
    )
    .await
    .context("agent timed out after 15 minutes")??;
    let _ = writer.await;
    if !out.status.success() {
        let text = String::from_utf8_lossy(if out.stderr.is_empty() {
            &out.stdout
        } else {
            &out.stderr
        })
        .trim()
        .to_string();
        bail!(
            "agent exited with {}: {}",
            out.status,
            last_chars(&text, 500)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The end of a long error message, where agents put the actual cause.
fn last_chars(text: &str, n: usize) -> String {
    let skip = text.chars().count().saturating_sub(n);
    text.chars().skip(skip).collect()
}

/// Like `ask`, but calls `on_text` with each piece of the answer as it is generated. Claude Code
/// streams token deltas (`--output-format stream-json`); any other agent streams stdout lines.
pub async fn ask_stream(
    prompt: &str,
    stdin: &str,
    mut on_text: impl FnMut(&str) + Send,
) -> Result<String> {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
    let (program, mut args) =
        command("TX_AGENT", "claude -p").context("Claude Code (claude) is not installed")?;
    let claude = program
        .file_stem()
        .is_some_and(|s| s.to_string_lossy().starts_with("claude"));
    if claude {
        args.extend(
            [
                "--output-format",
                "stream-json",
                "--verbose",
                "--include-partial-messages",
            ]
            .map(String::from),
        );
    }
    let mut child = paths::command(&program)
        .args(args)
        .arg(prompt)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("starting {}", program.display()))?;
    let mut input = child.stdin.take().unwrap();
    let body = stdin.to_string();
    let writer = tokio::spawn(async move {
        let _ = input.write_all(body.as_bytes()).await;
    });
    let mut stderr = child.stderr.take().unwrap();
    let errors = tokio::spawn(async move {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s).await;
        s
    });
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let (mut streamed, mut result, mut failure) = (String::new(), None::<String>, None::<String>);
    let read = async {
        while let Some(line) = lines.next_line().await? {
            if !claude {
                let piece = format!("{line}\n");
                streamed.push_str(&piece);
                on_text(&piece);
                continue;
            }
            let Ok(e) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            match e["type"].as_str() {
                Some("stream_event") if e["event"]["delta"]["type"] == "text_delta" => {
                    let piece = e["event"]["delta"]["text"].as_str().unwrap_or_default();
                    streamed.push_str(piece);
                    on_text(piece);
                }
                Some("result") => {
                    let text = e["result"].as_str().unwrap_or_default().to_string();
                    if e["is_error"].as_bool() == Some(true) {
                        failure = Some(text)
                    } else {
                        result = Some(text)
                    }
                }
                _ => {}
            }
        }
        anyhow::Ok(())
    };
    tokio::time::timeout(std::time::Duration::from_secs(900), read)
        .await
        .context("agent timed out after 15 minutes")??;
    let status = child.wait().await?;
    let _ = writer.await;
    let stderr = errors.await.unwrap_or_default();
    if let Some(f) = failure {
        bail!("agent failed: {}", last_chars(&f, 500));
    }
    if !status.success() {
        bail!(
            "agent exited with {status}: {}",
            last_chars(stderr.trim(), 500)
        );
    }
    // The final result is canonical (streamed blocks can lack separators).
    Ok(result.unwrap_or(streamed).trim().to_string())
}

pub const CHAT_PROMPT: &str = r#"You are answering follow-up questions about the video "{title}" by {channel} for one specific person:
<profile>
{profile}
</profile>

stdin is the full transcript; paragraphs start with [mm:ss] timestamps. Your earlier analysis of it:
<analysis>
{analysis}
</analysis>

The conversation so far:
<conversation>
{conversation}
</conversation>

Answer the latest question below. Ground claims in the transcript and cite [mm:ss] timestamps. If the transcript doesn't cover something, say so, then answer from general knowledge and label it as such. Be concise and specific: short paragraphs or bullets in markdown, no preamble. Reply in the language of the question.

Question: {question}"#;

/// Prompt for one chat turn; `history` is (role, text) pairs before this question.
pub fn chat_prompt(
    profile: &str,
    title: &str,
    channel: &str,
    analysis: &str,
    history: &[(String, String)],
    question: &str,
) -> String {
    let profile = if profile.trim().is_empty() {
        NO_PROFILE
    } else {
        profile.trim()
    };
    let conversation = if history.is_empty() {
        "(none yet)".to_string()
    } else {
        history
            .iter()
            .map(|(role, text)| format!("{}: {text}", if role == "user" { "User" } else { "You" }))
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    CHAT_PROMPT
        .replace("{title}", title)
        .replace("{channel}", channel)
        .replace("{profile}", profile)
        .replace("{analysis}", analysis)
        .replace("{conversation}", &conversation)
        .replace("{question}", question)
}

pub const PROMPT: &str = r#"You are screening a YouTube video for one specific person. Who they are and what knowledge they want:
<profile>
{profile}
</profile>

stdin is the transcript of "{title}" by {channel}. Paragraphs start with [mm:ss] timestamps.

Reply in markdown with exactly this structure and nothing before it:
RELEVANCE: <0-5>/5 — <one sentence: why, for this person>
## TL;DR
<2-3 sentences: the core content or claim>
## Takeaways for you
- <3-7 concrete, specific points this person should know or act on, each ending with its [mm:ss]>
## Worth watching?
<skip | skim [mm:ss]–[mm:ss] | watch fully> — <one sentence why>

Use only the transcript. Write in the language the profile is written in. If the video is irrelevant to the profile, give only the 1-2 most generally useful takeaways."#;

pub const NO_PROFILE: &str =
    "(No profile set. Judge usefulness for a curious technical generalist.)";

pub fn prompt(profile: &str, title: &str, channel: &str) -> String {
    let profile = if profile.trim().is_empty() {
        NO_PROFILE
    } else {
        profile.trim()
    };
    PROMPT
        .replace("{profile}", profile)
        .replace("{title}", title)
        .replace("{channel}", channel)
}

static RELEVANCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^\W*relevance\W*(\d)\s*(?:/\s*5)?\s*[—–:-]*\s*(.*)$").unwrap()
});

pub struct Analysis {
    pub relevance: Option<i64>,
    pub verdict: String,
    pub body: String,
}

/// Split agent output into relevance 0-5, one-line verdict, and the markdown body.
pub fn parse_analysis(out: &str) -> Analysis {
    let Some(c) = RELEVANCE.captures(out) else {
        return Analysis {
            relevance: None,
            verdict: String::new(),
            body: out.trim().to_string(),
        };
    };
    let whole = c.get(0).unwrap();
    let relevance = c[1].parse::<i64>().ok().map(|r| r.min(5));
    let verdict = c[2].trim().trim_matches('*').trim().to_string();
    let body = format!("{}{}", &out[..whole.start()], &out[whole.end()..])
        .trim()
        .to_string();
    Analysis {
        relevance,
        verdict,
        body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_score_and_verdict_and_strips_header() {
        let a = parse_analysis(
            "**RELEVANCE: 4/5 — covers eval harness design**\n## TL;DR\nText.\n## Takeaways for you\n- point [01:02]",
        );
        assert_eq!(
            (a.relevance, a.verdict.as_str()),
            (Some(4), "covers eval harness design")
        );
        assert!(a.body.starts_with("## TL;DR") && !a.body.contains("RELEVANCE"));
    }

    #[test]
    fn without_header_keeps_everything() {
        let a = parse_analysis("## TL;DR\nno score given");
        assert_eq!(
            (a.relevance, a.verdict.as_str(), a.body.as_str()),
            (None, "", "## TL;DR\nno score given")
        );
    }

    #[test]
    fn clamps_out_of_range_score() {
        assert_eq!(parse_analysis("Relevance: 9 - wow").relevance, Some(5));
    }

    #[test]
    fn empty_profile_falls_back_to_generalist() {
        assert!(prompt("  ", "T", "C").contains(NO_PROFILE));
        assert!(prompt("Rust dev", "T", "C").contains("<profile>\nRust dev\n</profile>"));
    }
}
