//! What the user typed to their coding agents (Claude Code, Codex, Oh My Pi), read from the
//! session files those tools keep on this computer, so tx can learn what they work on.
//!
//! Only the user's own prompts are kept. Agent replies, tool output, injected context, subagent
//! sessions and non-interactive runs (`claude -p`, `codex exec`, including tx's own) are skipped;
//! obvious secrets and email addresses are redacted before anything leaves this module.

use crate::paths;
use chrono::{DateTime, Utc};
use regex::{Captures, Regex};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::SystemTime;

/// Longest excerpt kept from one prompt; long prompts are mostly pasted logs or code.
const PROMPT_CHARS: usize = 400;
/// Prompts shorter than this ("ok", "continue") say nothing about the person.
const MIN_CHARS: usize = 20;
/// Excerpt text sent to the agent, about 15k tokens.
const BUDGET_CHARS: usize = 60_000;
/// Projects described with excerpts; the rest are listed by name.
const MAX_PROJECTS: usize = 25;
/// A prompt's weight halves every this many days, so current work outranks work from months ago
/// without crowding it out.
const HALF_LIFE_DAYS: f64 = 30.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    ClaudeCode,
    Codex,
    OhMyPi,
}

impl Source {
    pub const ALL: [Source; 3] = [Source::ClaudeCode, Source::Codex, Source::OhMyPi];

    pub fn name(self) -> &'static str {
        match self {
            Source::ClaudeCode => "Claude Code",
            Source::Codex => "Codex",
            Source::OhMyPi => "Oh My Pi",
        }
    }

    /// Where the tool keeps its sessions.
    pub fn dir(self) -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        let from = |var: &str, default: &str| {
            std::env::var_os(var)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(default))
        };
        Some(match self {
            Source::ClaudeCode => from("CLAUDE_CONFIG_DIR", ".claude").join("projects"),
            Source::Codex => from("CODEX_HOME", ".codex").join("sessions"),
            Source::OhMyPi => home.join(".omp").join("agent").join("sessions"),
        })
    }
}

/// One thing the user typed to an agent.
#[derive(Clone, Debug)]
pub struct Prompt {
    pub source: Source,
    /// The folder the agent ran in.
    pub project: PathBuf,
    pub at: DateTime<Utc>,
    pub text: String,
}

/// Prompts tx itself sends through an agent: never the user's own words.
const TX_PROMPTS: &[&str] = &[
    "Review the video for this user's question",
    "You are running unattended on a schedule", // scheduled screening before tx 0.20
    crate::learn::PROMPT_START,
];

static INJECTED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?s)<system-reminder>.*?</system-reminder>|<system-notice>.*?</system-notice>|<file\b[^>]*>.*?</file>|\[Image #\d+[^\]]*\]",
    )
    .unwrap()
});

/// (pattern, replacement); `$1`… keep the label in `key = value` style matches.
static SECRETS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----", "[private key]"),
        (r"\b(?:sk|rk|pk)[-_][A-Za-z0-9_-]{16,}", "[secret]"),
        (r"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{20,}|\bgithub_pat_[A-Za-z0-9_]{20,}|\bglpat-[A-Za-z0-9_-]{20,}", "[secret]"),
        (r"\bxox[abprs]-[A-Za-z0-9-]{10,}", "[secret]"),
        (r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b", "[secret]"),
        (r"\bAIza[0-9A-Za-z_-]{35}", "[secret]"),
        (r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}", "[secret]"),
        (r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{16,}", "Bearer [secret]"),
        (r#"(?i)\b([A-Za-z0-9_]*(?:api[_-]?key|secret|token|passw(?:or)?d|pwd)[A-Za-z0-9_]*)(\s*[:=]\s*)["']?[^\s"',;]{6,}"#, "$1$2[secret]"),
        (r"(?i)\b([a-z][a-z0-9+.-]*://)[^/\s:@]+:[^/\s@]+@", "$1[credentials]@"),
        (r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b", "[email]"),
    ]
    .into_iter()
    .map(|(p, r)| (Regex::new(p).unwrap(), r))
    .collect()
});

/// Long runs of letters and digits mixed: API keys, tokens, hashes.
static TOKEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z0-9+_-]{32,}={0,2}").unwrap());

/// Replace obvious secrets and email addresses.
pub fn redact(text: &str) -> String {
    let mut out = text.to_string();
    for (re, with) in SECRETS.iter() {
        out = re.replace_all(&out, *with).into_owned();
    }
    TOKEN
        .replace_all(&out, |c: &Captures| {
            let s = &c[0];
            let digits = s.bytes().filter(u8::is_ascii_digit).count();
            let letters = s.bytes().filter(u8::is_ascii_alphabetic).count();
            if digits >= 4 && letters >= 4 {
                "[secret]".to_string()
            } else {
                s.to_string()
            }
        })
        .into_owned()
}

/// The user's words from a raw message, or None if it's injected context, a tx prompt or too
/// short to mean anything. Redacted, whitespace collapsed, cut to an excerpt.
fn clean(raw: &str) -> Option<String> {
    let text = INJECTED.replace_all(raw, "");
    let text = text.trim();
    if text.is_empty()
        || text.starts_with('<')
        || text.starts_with("[Request interrupted")
        || text.starts_with("[Wait interrupted")
        || text.starts_with("Caveat:")
        || TX_PROMPTS.iter().any(|p| text.starts_with(p))
    {
        return None;
    }
    let text = redact(text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let count = text.chars().count();
    if count < MIN_CHARS {
        return None;
    }
    Some(if count > PROMPT_CHARS {
        let mut cut: String = text.chars().take(PROMPT_CHARS).collect();
        cut.push('…');
        cut
    } else {
        text
    })
}

fn timestamp(v: &Value) -> Option<DateTime<Utc>> {
    match v {
        Value::String(s) => DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|t| t.with_timezone(&Utc)),
        Value::Number(n) => DateTime::from_timestamp_millis(n.as_i64()?),
        _ => None,
    }
}

/// Text blocks of a message's content, or None if it carries tool results instead.
fn text_blocks(content: &Value) -> Option<String> {
    match content {
        Value::String(s) => Some(s.clone()),
        Value::Array(items) => {
            if items.iter().any(|i| i["type"] == "tool_result") {
                return None;
            }
            let parts: Vec<&str> = items
                .iter()
                .filter(|i| matches!(i["type"].as_str(), Some("text" | "input_text")))
                .filter_map(|i| i["text"].as_str())
                .collect();
            Some(parts.join("\n"))
        }
        _ => None,
    }
}

/// The prompts in one session file.
fn parse(source: Source, lines: impl Iterator<Item = String>, since: DateTime<Utc>) -> Vec<Prompt> {
    let mut out = Vec::new();
    let mut cwd: Option<PathBuf> = None;
    let mut push = |cwd: Option<&Path>, at: Option<DateTime<Utc>>, raw: Option<String>| {
        if let (Some(project), Some(at), Some(raw)) = (cwd, at, raw)
            && at >= since
            && let Some(text) = clean(&raw)
        {
            out.push(Prompt {
                source,
                project: project.to_path_buf(),
                at,
                text,
            });
        }
    };
    for line in lines {
        // Most lines are agent output; skip them before paying for a JSON parse.
        if !(line.contains("user") || line.contains("session") || line.contains("turn_context")) {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        match source {
            Source::ClaudeCode => {
                if v["type"] != "user"
                    || v["isSidechain"] == true
                    || v["isMeta"] == true
                    || v["isCompactSummary"] == true
                    || v["entrypoint"]
                        .as_str()
                        .is_some_and(|e| e.starts_with("sdk"))
                {
                    continue;
                }
                let dir = v["cwd"].as_str().map(PathBuf::from);
                push(
                    dir.as_deref(),
                    timestamp(&v["timestamp"]),
                    text_blocks(&v["message"]["content"]),
                );
            }
            Source::Codex => {
                let p = &v["payload"];
                match v["type"].as_str() {
                    Some("session_meta") => {
                        // `codex exec` runs are scripts and tools (tx's own screening among them);
                        // subagents (spawned threads, the approval reviewer) get prompts from agents.
                        if p["originator"] == "codex_exec"
                            || p["source"] == "exec"
                            || !p["source"]["subagent"].is_null()
                        {
                            return Vec::new();
                        }
                        cwd = p["cwd"].as_str().map(PathBuf::from);
                    }
                    Some("turn_context") => {
                        if let Some(c) = p["cwd"].as_str() {
                            cwd = Some(PathBuf::from(c));
                        }
                    }
                    Some("response_item") if p["type"] == "message" && p["role"] == "user" => {
                        // The desktop app lists attached files first, then "## My request:".
                        let raw =
                            text_blocks(&p["content"]).map(|t| match t.find("## My request") {
                                Some(i) => {
                                    t[i..].split_once('\n').map_or("", |(_, r)| r).to_string()
                                }
                                None => t,
                            });
                        push(cwd.as_deref(), timestamp(&v["timestamp"]), raw);
                    }
                    _ => {}
                }
            }
            Source::OhMyPi => match v["type"].as_str() {
                Some("session") => cwd = v["cwd"].as_str().map(PathBuf::from),
                Some("message") if v["message"]["role"] == "user" => {
                    let at = timestamp(&v["timestamp"])
                        .or_else(|| timestamp(&v["message"]["timestamp"]));
                    push(cwd.as_deref(), at, text_blocks(&v["message"]["content"]));
                }
                _ => {}
            },
        }
    }
    out
}

/// Session files changed since `since`, skipping subagent transcripts.
fn session_files(source: Source, root: &Path, since: SystemTime) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                // Claude Code: <project>/<session>/subagents/; Oh My Pi: <project>/<session>/<agent>.jsonl.
                let subagents = match source {
                    Source::ClaudeCode => entry.file_name() == "subagents",
                    Source::OhMyPi => depth >= 1,
                    Source::Codex => false,
                };
                if !subagents {
                    stack.push((path, depth + 1));
                }
            } else if path.extension().is_some_and(|e| e == "jsonl")
                && entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .is_ok_and(|t| t >= since)
            {
                files.push(path);
            }
        }
    }
    files
}

/// The user's prompts from one tool's sessions under `root`, since `since`.
pub fn read(source: Source, root: &Path, since: DateTime<Utc>) -> Vec<Prompt> {
    let modified_after = SystemTime::from(since);
    session_files(source, root, modified_after)
        .into_iter()
        .flat_map(|path| match std::fs::File::open(&path) {
            Ok(f) => parse(
                source,
                std::io::BufReader::new(f).lines().map_while(Result::ok),
                since,
            ),
            Err(e) => {
                log::warn!("skipping {}: {e}", path.display());
                Vec::new()
            }
        })
        .collect()
}

/// The user's prompts from every installed tool over the last `days` days. Sessions that ran in
/// tx's data folder are tx's own and are left out.
pub fn collect(days: u32, now: DateTime<Utc>) -> Vec<Prompt> {
    let since = now - chrono::Duration::days(days.into());
    let data = paths::data_dir();
    Source::ALL
        .into_iter()
        .filter_map(|s| Some((s, s.dir()?)))
        .flat_map(|(s, dir)| read(s, &dir, since))
        .filter(|p| !p.project.starts_with(&data))
        .collect()
}

/// Whether any supported tool keeps sessions on this computer, so learning can be offered.
pub fn available() -> bool {
    Source::ALL
        .iter()
        .any(|s| s.dir().is_some_and(|d| d.is_dir()))
}

/// One folder the user worked in.
#[derive(Debug)]
pub struct Project {
    pub path: PathBuf,
    pub prompts: usize,
    pub days: usize,
    pub last: DateTime<Utc>,
    pub sources: BTreeSet<Source>,
    weight: f64,
    /// Newest first.
    excerpts: Vec<(DateTime<Utc>, String)>,
}

/// What the user worked on, ready to show them and to send to their agent.
#[derive(Debug)]
pub struct Activity {
    pub days: u32,
    pub until: DateTime<Utc>,
    /// Most active first (recent prompts count more).
    pub projects: Vec<Project>,
    pub prompts: usize,
    pub by_source: BTreeMap<Source, usize>,
}

impl Activity {
    pub fn summarize(prompts: Vec<Prompt>, days: u32, until: DateTime<Utc>) -> Activity {
        let mut by_source = BTreeMap::new();
        let mut projects: BTreeMap<PathBuf, Project> = BTreeMap::new();
        let total = prompts.len();
        let mut active: BTreeMap<PathBuf, HashSet<chrono::NaiveDate>> = BTreeMap::new();
        for p in prompts {
            *by_source.entry(p.source).or_insert(0) += 1;
            let age = (until - p.at).num_seconds().max(0) as f64 / 86_400.0;
            active
                .entry(p.project.clone())
                .or_default()
                .insert(p.at.date_naive());
            let e = projects
                .entry(p.project.clone())
                .or_insert_with(|| Project {
                    path: p.project,
                    prompts: 0,
                    days: 0,
                    last: p.at,
                    sources: BTreeSet::new(),
                    weight: 0.0,
                    excerpts: Vec::new(),
                });
            e.prompts += 1;
            e.last = e.last.max(p.at);
            e.sources.insert(p.source);
            e.weight += 0.5f64.powf(age / HALF_LIFE_DAYS);
            e.excerpts.push((p.at, p.text));
        }
        let mut projects: Vec<Project> = projects
            .into_values()
            .map(|mut p| {
                p.days = active.get(&p.path).map_or(0, HashSet::len);
                p.excerpts.sort_by(|a, b| b.0.cmp(&a.0));
                let mut seen = HashSet::new();
                p.excerpts.retain(|(_, t)| seen.insert(t.clone()));
                p
            })
            .collect();
        projects.sort_by(|a, b| b.weight.total_cmp(&a.weight));
        // Share the excerpt budget by weight, so busy projects get more but none get nothing.
        let described = projects.len().min(MAX_PROJECTS);
        let weights: f64 = projects[..described].iter().map(|p| p.weight).sum();
        for (i, p) in projects.iter_mut().enumerate() {
            let share = if i < described && weights > 0.0 {
                ((BUDGET_CHARS as f64 * p.weight / weights) as usize).clamp(800, 6_000)
            } else {
                0
            };
            // Thin out evenly (newest always kept) instead of cutting off the oldest, so the
            // whole period is represented, not just the latest weeks.
            let total: usize = p.excerpts.iter().map(|(_, t)| t.chars().count()).sum();
            if total > share && total > 0 {
                let keep = share as f64 / total as f64;
                let mut i = 0.0f64;
                p.excerpts.retain(|_| {
                    let before = i.ceil();
                    i += keep;
                    i.ceil() > before
                });
            }
            let mut used = 0;
            p.excerpts.retain(|(_, t)| {
                used += t.chars().count();
                used <= share
            });
        }
        Activity {
            days,
            until,
            projects,
            prompts: total,
            by_source,
        }
    }

    /// Excerpts that would be sent.
    pub fn excerpts(&self) -> usize {
        self.projects.iter().map(|p| p.excerpts.len()).sum()
    }

    /// The material for the agent: activity per project, then the user's own words.
    pub fn render(&self) -> String {
        let mut s = format!(
            "Coding-agent sessions, last {} days up to {}: {} prompts by the user, most active projects first.\n",
            self.days,
            self.until.format("%Y-%m-%d"),
            self.prompts
        );
        let mut rest = Vec::new();
        for p in &self.projects {
            if p.excerpts.is_empty() {
                rest.push(format!("{} ({} prompts)", display(&p.path), p.prompts));
                continue;
            }
            let tools: Vec<&str> = p.sources.iter().map(|s| s.name()).collect();
            s.push_str(&format!(
                "\n## {}\n{} · {} prompts · active on {} days · last {}\n",
                display(&p.path),
                tools.join(", "),
                p.prompts,
                p.days,
                p.last.format("%Y-%m-%d")
            ));
            for (at, text) in &p.excerpts {
                s.push_str(&format!("- {}: {text}\n", at.format("%Y-%m-%d")));
            }
        }
        if !rest.is_empty() {
            s.push_str(&format!("\nOther projects: {}\n", rest.join(", ")));
        }
        s
    }
}

/// A path as the user sees it: `~/Projects/tx`.
pub fn display(path: &Path) -> String {
    match dirs::home_dir().and_then(|h| path.strip_prefix(&h).ok().map(Path::to_path_buf)) {
        Some(rel) if rel.as_os_str().is_empty() => "~".into(),
        Some(rel) => format!("~/{}", rel.display()),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn since() -> DateTime<Utc> {
        "2026-10-01T00:00:00Z".parse().unwrap()
    }

    fn lines(items: &[Value]) -> impl Iterator<Item = String> {
        items
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn texts(p: &[Prompt]) -> Vec<&str> {
        p.iter().map(|p| p.text.as_str()).collect()
    }

    #[test]
    fn claude_keeps_only_what_the_user_typed_interactively() {
        let user = |extra: Value, content: Value| {
            let mut v = serde_json::json!({"type": "user", "cwd": "/home/u/Projects/tx", "timestamp": "2026-10-20T10:00:00Z",
                "entrypoint": "cli", "message": {"role": "user", "content": content}});
            v.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            v
        };
        let p = parse(
            Source::ClaudeCode,
            lines(&[
                user(
                    serde_json::json!({}),
                    "Add a dry-run flag to the learn command please".into(),
                ),
                user(
                    serde_json::json!({}),
                    serde_json::json!([{"type": "text", "text": "<system-reminder>ignore me</system-reminder> Why does the digest skip podcasts?"}]),
                ),
                user(
                    serde_json::json!({}),
                    serde_json::json!([{"type": "tool_result", "content": "a very long tool output here"}]),
                ),
                user(
                    serde_json::json!({"isSidechain": true}),
                    "subagent task prompt from the orchestrator".into(),
                ),
                user(
                    serde_json::json!({"isMeta": true}),
                    "Caveat: the messages below were generated".into(),
                ),
                user(
                    serde_json::json!({"entrypoint": "sdk-cli"}),
                    "a scripted claude -p prompt, not the person".into(),
                ),
                user(
                    serde_json::json!({}),
                    "<command-name>/clear</command-name>".into(),
                ),
                user(
                    serde_json::json!({"timestamp": "2026-09-01T10:00:00Z"}),
                    "too old to count for this window".into(),
                ),
                serde_json::json!({"type": "assistant", "cwd": "/x", "message": {"content": "agent reply mentioning user"}}),
            ]),
            since(),
        );
        assert_eq!(
            texts(&p),
            [
                "Add a dry-run flag to the learn command please",
                "Why does the digest skip podcasts?"
            ]
        );
        assert_eq!(p[0].project, PathBuf::from("/home/u/Projects/tx"));
    }

    #[test]
    fn codex_tracks_cwd_and_skips_exec_sessions_and_context() {
        let ts = "2026-10-20T10:00:00Z";
        let msg = |text: &str| {
            serde_json::json!({"timestamp": ts, "type": "response_item",
            "payload": {"type": "message", "role": "user", "content": [{"type": "input_text", "text": text}]}})
        };
        let p = parse(
            Source::Codex,
            lines(&[
                serde_json::json!({"timestamp": ts, "type": "session_meta", "payload": {"cwd": "/w/a", "originator": "Codex Desktop"}}),
                msg("<environment_context><cwd>/w/a</cwd></environment_context>"),
                msg(
                    "# Files mentioned by the user:\n## shot.png: /tmp/shot.png\n## My request:\nMake the timeline bar clickable",
                ),
                serde_json::json!({"timestamp": ts, "type": "turn_context", "payload": {"cwd": "/w/b"}}),
                msg("Now port the same change to the podcast view"),
            ]),
            since(),
        );
        assert_eq!(
            texts(&p),
            [
                "Make the timeline bar clickable",
                "Now port the same change to the podcast view"
            ]
        );
        assert_eq!(p[0].project, PathBuf::from("/w/a"));
        assert_eq!(p[1].project, PathBuf::from("/w/b"));

        for meta in [
            serde_json::json!({"cwd": "/w/a", "originator": "codex_exec", "source": "exec"}),
            serde_json::json!({"cwd": "/w/a", "originator": "Codex Desktop", "source": {"subagent": {"other": "guardian"}}}),
            serde_json::json!({"cwd": "/w/a", "originator": "Codex Desktop", "source": {"subagent": {"thread_spawn": {"parent_thread_id": "x"}}}}),
        ] {
            let skipped = parse(
                Source::Codex,
                lines(&[
                    serde_json::json!({"timestamp": ts, "type": "session_meta", "payload": meta}),
                    msg("A prompt written by another agent, not by the person"),
                ]),
                since(),
            );
            assert!(skipped.is_empty(), "{meta}");
        }
    }

    #[test]
    fn oh_my_pi_uses_the_session_cwd_and_drops_attachments_and_agent_messages() {
        let ts = "2026-10-20T10:00:00Z";
        let user = |text: &str| {
            serde_json::json!({"type": "message", "timestamp": ts,
            "message": {"role": "user", "content": [{"type": "text", "text": text}, {"type": "image"}]}})
        };
        let p = parse(
            Source::OhMyPi,
            lines(&[
                serde_json::json!({"type": "title", "title": "x"}),
                serde_json::json!({"type": "session", "cwd": "/home/u/Projects/me", "timestamp": ts}),
                user("[Image #1, 1568x980] remove the footer text from the video"),
                user("[Wait interrupted by message] <irc from=\"Worker\">done</irc>"),
                serde_json::json!({"type": "message", "timestamp": ts, "message": {"role": "assistant", "content": "user asked for x"}}),
            ]),
            since(),
        );
        assert_eq!(texts(&p), ["remove the footer text from the video"]);
        assert_eq!(p[0].project, PathBuf::from("/home/u/Projects/me"));
    }

    #[test]
    fn secrets_and_emails_are_redacted_but_ordinary_text_is_not() {
        let cases = [
            (
                "use sk-ant-api03-abcdefghijklmnopqrstu for now",
                "use [secret] for now",
            ),
            (
                "token ghp_abcdefghijklmnopqrstuvwxyz0123 leaked",
                "token [secret] leaked",
            ),
            (
                "OPENAI_API_KEY=abcdef123456 in .env",
                "OPENAI_API_KEY=[secret] in .env",
            ),
            ("password: hunter2hunter2", "password: [secret]"),
            (
                "connect to postgres://admin:s3cret@db.local/app",
                "connect to postgres://[credentials]@db.local/app",
            ),
            ("mail jiri@example.com about it", "mail [email] about it"),
            (
                "Authorization: Bearer abcdefghijklmnop1234",
                "Authorization: Bearer [secret]",
            ),
            (
                "-----BEGIN RSA PRIVATE KEY-----\nMIIE\n-----END RSA PRIVATE KEY-----",
                "[private key]",
            ),
            (
                "commit 9f8e7d6c5b4a39281706f5e4d3c2b1a098765432 broke it",
                "commit [secret] broke it",
            ),
        ];
        for (input, want) in cases {
            assert_eq!(redact(input), want, "{input}");
        }
        let plain = "Refactor ~/Projects/tx/crates/core/src/sessions.rs and keep the very-long-kebab-case-identifier-names as they are";
        assert_eq!(redact(plain), plain);
    }

    #[test]
    fn tx_prompts_short_replies_and_long_pastes_are_handled() {
        assert_eq!(clean("ok"), None);
        assert_eq!(
            clean("Review the video for this user's question. Video metadata: …"),
            None
        );
        let long = "a ".repeat(1000);
        let cut = clean(&long).unwrap();
        assert_eq!(cut.chars().count(), PROMPT_CHARS + 1);
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn subagent_transcripts_are_not_read() {
        let root = tempfile::tempdir().unwrap();
        let session = serde_json::json!({"type": "session", "cwd": "/p", "timestamp": "2026-10-20T10:00:00Z"});
        let user = serde_json::json!({"type": "message", "timestamp": "2026-10-20T10:00:00Z",
            "message": {"role": "user", "content": "a prompt typed by the person themselves"}});
        let write = |rel: &str, text: &str| {
            let p = root.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        let body = format!("{session}\n{user}\n");
        write("-p/2026-10-20_one.jsonl", &body);
        write(
            "-p/2026-10-20_one/Reviewer.jsonl",
            &body.replace("themselves", "subagent"),
        );
        let p = read(Source::OhMyPi, root.path(), since());
        assert_eq!(texts(&p), ["a prompt typed by the person themselves"]);

        let claude = serde_json::json!({"type": "user", "cwd": "/p", "timestamp": "2026-10-20T10:00:00Z",
            "message": {"content": "a prompt typed by the person themselves"}});
        write("claude/-p/s.jsonl", &format!("{claude}\n"));
        write(
            "claude/-p/s/subagents/agent-1.jsonl",
            &format!("{claude}\n"),
        );
        assert_eq!(
            read(Source::ClaudeCode, &root.path().join("claude"), since()).len(),
            1
        );
    }

    #[test]
    fn recent_busy_projects_rank_first_and_every_described_project_gets_excerpts() {
        let now: DateTime<Utc> = "2026-10-27T00:00:00Z".parse().unwrap();
        let mk = |project: &str, days_ago: i64, text: String| Prompt {
            source: Source::ClaudeCode,
            project: project.into(),
            at: now - chrono::Duration::days(days_ago),
            text,
        };
        // 100 prompts three months ago weigh 12.5; 40 from yesterday weigh about 39.
        let mut prompts: Vec<Prompt> = (0..100)
            .map(|i| {
                mk(
                    "/old",
                    90,
                    format!("old project prompt number {i} with enough words to count"),
                )
            })
            .collect();
        prompts.extend((0..40).map(|i| {
            mk(
                "/now",
                1,
                format!("current project prompt {i} about the new feature"),
            )
        }));
        prompts.push(mk("/tiny", 2, "one prompt in a small side project".into()));
        let a = Activity::summarize(prompts, 30, now);
        let order: Vec<&str> = a
            .projects
            .iter()
            .map(|p| p.path.to_str().unwrap())
            .collect();
        assert_eq!(order, ["/now", "/old", "/tiny"]);
        assert!(a.projects.iter().all(|p| !p.excerpts.is_empty()));
        assert_eq!(
            a.projects[0].excerpts[0].1,
            "current project prompt 0 about the new feature"
        );
        assert!(a.render().chars().count() < BUDGET_CHARS + 6_000);
    }

    #[test]
    fn a_busy_project_keeps_excerpts_from_the_whole_period_not_just_the_latest_weeks() {
        let now: DateTime<Utc> = "2026-10-27T00:00:00Z".parse().unwrap();
        let prompts: Vec<Prompt> = (0..600)
            .map(|i| Prompt {
                source: Source::OhMyPi,
                project: "/busy".into(),
                at: now - chrono::Duration::hours(i * 6),
                text: format!("prompt {i} about the same busy project with enough words in it"),
            })
            .collect();
        let a = Activity::summarize(prompts, 180, now);
        let kept = &a.projects[0].excerpts;
        assert!(kept.len() < 600 && kept.len() > 20, "{}", kept.len());
        assert_eq!(
            kept[0].1,
            "prompt 0 about the same busy project with enough words in it"
        );
        let oldest = kept.last().unwrap().0;
        assert!(
            now - oldest > chrono::Duration::days(120),
            "oldest kept excerpt is only {} days old",
            (now - oldest).num_days()
        );
    }
}
