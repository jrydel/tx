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
        let tail: String = text
            .chars()
            .rev()
            .take(500)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        bail!("agent exited with {}: {tail}", out.status);
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
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
