//! Analyses written by the user's own agent (MCP flow), and the markdown digest of results.

use crate::captions::fmt_time;
use crate::db::{self, Video};
use anyhow::{Result, bail};
use rusqlite::Connection;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Takeaway {
    /// Where it's said, "mm:ss" or "h:mm:ss".
    pub at: Option<String>,
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct Watch {
    /// skip | skim | watch
    pub advice: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct AgentAnalysis {
    pub relevance: i64,
    pub verdict: String,
    pub tldr: String,
    pub takeaways: Vec<Takeaway>,
    pub watch: Watch,
}

/// "[12:34]" from "12:34", "[12:34]" or "1:02:03"; None if it isn't a timestamp.
fn stamp(t: &str) -> Option<String> {
    let t = t
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();
    let parts: Vec<&str> = t.split(':').collect();
    let ok = (2..=3).contains(&parts.len())
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
    ok.then(|| format!("[{t}]"))
}

/// Markdown in the shape the app renders: TL;DR, takeaways ending in [mm:ss], worth watching.
pub fn compose(a: &AgentAnalysis) -> Result<String> {
    if !(0..=5).contains(&a.relevance) {
        bail!("relevance must be 0-5");
    }
    let advice = match a.watch.advice.trim().to_lowercase().as_str() {
        "skip" => "skip".to_string(),
        "watch" | "watch fully" => "watch fully".to_string(),
        "skim" => match (
            a.watch.from.as_deref().and_then(stamp),
            a.watch.to.as_deref().and_then(stamp),
        ) {
            (Some(f), Some(t)) => format!("skim {f}–{t}"),
            _ => "skim".to_string(),
        },
        other => bail!("watch.advice must be skip, skim or watch, not {other:?}"),
    };
    let mut md = format!("## TL;DR\n{}\n## Takeaways for you\n", a.tldr.trim());
    for t in &a.takeaways {
        match t.at.as_deref().and_then(stamp) {
            Some(s) => md.push_str(&format!("- {} {s}\n", t.text.trim())),
            None => md.push_str(&format!("- {}\n", t.text.trim())),
        }
    }
    md.push_str(&format!(
        "## Worth watching?\n{advice} — {}\n",
        a.watch.reason.trim()
    ));
    Ok(md)
}

/// Text of one `## Heading` section of an analysis.
pub fn section<'a>(analysis: &'a str, heading: &str) -> &'a str {
    let Some(start) = analysis.lines().position(|l| {
        l.trim_start_matches('#')
            .trim()
            .eq_ignore_ascii_case(heading)
    }) else {
        return "";
    };
    let offset: usize = analysis.lines().take(start + 1).map(|l| l.len() + 1).sum();
    let rest = analysis.get(offset.min(analysis.len())..).unwrap_or("");
    let end = rest.find("\n#").map(|i| i + 1).unwrap_or(rest.len());
    rest[..end].trim()
}

pub fn digest_dir(conn: &Connection) -> Result<PathBuf> {
    Ok(match db::get_value(conn, "digest_dir")? {
        Some(d) if !d.trim().is_empty() => PathBuf::from(d.trim()),
        _ => crate::paths::data_dir().join("digests"),
    })
}

fn entry(v: &Video, full: bool) -> String {
    let score = v
        .relevance
        .map(|r| format!("{r}/5"))
        .unwrap_or_else(|| "?/5".into());
    let length = v
        .duration
        .map(|d| format!(" ({})", fmt_time(d)))
        .unwrap_or_default();
    if !full {
        return format!(
            "- {score} [{}]({}) — {}{length}: {}\n",
            v.title,
            v.url,
            v.channel,
            v.verdict.as_deref().unwrap_or("")
        );
    }
    let a = v.analysis.as_deref().unwrap_or("");
    let mut s = format!(
        "## {score} · {}\n{}{length} · {}\n\n",
        v.title, v.channel, v.url
    );
    if let Some(verdict) = v.verdict.as_deref().filter(|x| !x.is_empty()) {
        s.push_str(&format!("**{verdict}**\n\n"));
    }
    for (heading, label) in [
        ("TL;DR", ""),
        ("Takeaways for you", ""),
        ("Worth watching?", "Worth watching: "),
    ] {
        let body = section(a, heading);
        if !body.is_empty() {
            s.push_str(&format!("{label}{body}\n\n"));
        }
    }
    s
}

pub struct Digest {
    pub path: Option<PathBuf>,
    pub markdown: String,
    pub worth: usize,
    pub total: usize,
}

/// Everything analyzed since the previous digest, written as one markdown file.
pub fn write_digest(conn: &Connection) -> Result<Digest> {
    let since = db::get_value(conn, "digest_at")?;
    let min = db::get_settings(conn)?.notify_min as i64;
    let videos = db::analyzed(conn, since.as_deref(), 0, 200)?;
    let (worth, rest): (Vec<&Video>, Vec<&Video>) =
        videos.iter().partition(|v| v.relevance.unwrap_or(0) >= min);
    let date = chrono::Local::now();
    let mut md = format!("# tx digest · {}\n\n", date.format("%Y-%m-%d %H:%M"));
    if videos.is_empty() {
        md.push_str("No new videos analyzed since the last digest.\n");
        return Ok(Digest {
            path: None,
            markdown: md,
            worth: 0,
            total: 0,
        });
    }
    md.push_str(&format!(
        "{} new video(s) analyzed, {} worth your time (score {min}+).\n\n",
        videos.len(),
        worth.len()
    ));
    for v in &worth {
        md.push_str(&entry(v, true));
    }
    if !rest.is_empty() {
        md.push_str("## Also analyzed\n");
        for v in &rest {
            md.push_str(&entry(v, false));
        }
    }
    let dir = digest_dir(conn)?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("tx-{}.md", date.format("%Y-%m-%d-%H%M")));
    std::fs::write(&path, &md)?;
    db::set_value(conn, "digest_at", &db::now())?;
    Ok(Digest {
        path: Some(path),
        markdown: md,
        worth: worth.len(),
        total: videos.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analysis(advice: &str) -> AgentAnalysis {
        AgentAnalysis {
            relevance: 4,
            verdict: "Concrete eval tips".into(),
            tldr: "He tests models.".into(),
            takeaways: vec![
                Takeaway {
                    at: Some("04:25".into()),
                    text: "Check real token cost".into(),
                },
                Takeaway {
                    at: Some("[1:02:03]".into()),
                    text: "Late point".into(),
                },
                Takeaway {
                    at: Some("soon".into()),
                    text: "No time".into(),
                },
            ],
            watch: Watch {
                advice: advice.into(),
                from: Some("22:03".into()),
                to: Some("33:19".into()),
                reason: "the audits".into(),
            },
        }
    }

    #[test]
    fn composes_markdown_the_app_parses() {
        let md = compose(&analysis("skim")).unwrap();
        assert!(md.starts_with("## TL;DR\nHe tests models.\n## Takeaways for you\n"));
        assert!(
            md.contains("- Check real token cost [04:25]\n- Late point [1:02:03]\n- No time\n")
        );
        assert!(md.ends_with("## Worth watching?\nskim [22:03]–[33:19] — the audits\n"));
        assert_eq!(section(&md, "TL;DR"), "He tests models.");
        assert_eq!(
            section(&md, "Worth watching?"),
            "skim [22:03]–[33:19] — the audits"
        );
    }

    #[test]
    fn rejects_unknown_advice_and_bad_scores() {
        assert!(compose(&analysis("maybe")).is_err());
        let mut a = analysis("watch");
        a.relevance = 7;
        assert!(compose(&a).is_err());
    }
}
