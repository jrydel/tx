//! The user's coding agent CLI (default: Claude Code), for one-shot questions (`tx URL -a`).

use crate::paths;
use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use std::process::Stdio;
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

/// The end of a long error message, where agents put the actual cause.
fn last_chars(text: &str, n: usize) -> String {
    let skip = text.chars().count().saturating_sub(n);
    text.chars().skip(skip).collect()
}

/// Run the resolved agent command with `stdin` piped in, calling `on_text` with each
/// piece of the answer. Claude Code streams token deltas (`--output-format stream-json`);
/// other agents stream stdout lines. Returns the full answer.
pub async fn ask_stream(
    prompt: &str,
    stdin: &str,
    selected: (PathBuf, Vec<String>),
    mut on_text: impl FnMut(&str) + Send,
) -> Result<String> {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
    let (program, mut args) = selected;
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
