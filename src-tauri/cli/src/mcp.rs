//! `tx mcp`: an MCP server over stdio. tx fetches (channels, feeds, transcripts) on this computer;
//! the user's agent (Claude Code, Claude Desktop, Codex…) does the analysis on their subscription.

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};
use std::sync::Mutex;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tx_core::db::{self, Db};
use tx_core::report::{self, AgentAnalysis};
use tx_core::{captions, feeds, ytdlp};

/// Transcript parts stay under typical MCP tool-output limits (~25k tokens).
const PART_CHARS: usize = 60_000;

const INSTRUCTIONS: &str = "tx watches YouTube channels on this computer. To screen new uploads: get_profile, \
check_new_videos, then for each video get_transcript and save_analysis (or skip_video), then write_digest. \
The screen-videos skill has the full procedure and scoring rubric.";

fn tools() -> Value {
    let video_id =
        json!({ "type": "string", "description": "Video id from check_new_videos or feed" });
    let stamp = json!({ "type": "string", "description": "Timestamp mm:ss or h:mm:ss from the transcript" });
    json!([
        { "name": "get_profile", "description": "Who the user is and what they want to learn, plus the score that counts as worth their time.",
          "inputSchema": { "type": "object", "properties": {} } },
        { "name": "set_profile", "description": "Save the user's description of who they are and what they want to learn.",
          "inputSchema": { "type": "object", "properties": { "profile": { "type": "string" } }, "required": ["profile"] } },
        { "name": "watch_channel", "description": "Start watching a YouTube channel. Only uploads published from now on are analyzed automatically.",
          "inputSchema": { "type": "object", "properties": { "channel": { "type": "string", "description": "@handle, channel URL, channel id, or a URL of one of its videos" } }, "required": ["channel"] } },
        { "name": "unwatch_channel", "description": "Stop watching a channel.",
          "inputSchema": { "type": "object", "properties": { "channel_id": { "type": "string" } }, "required": ["channel_id"] } },
        { "name": "list_channels", "description": "Channels tx knows about.",
          "inputSchema": { "type": "object", "properties": { "watching_only": { "type": "boolean", "default": true } } } },
        { "name": "import_subscriptions", "description": "Import the YouTube subscriptions of a browser signed in to YouTube on this computer (not watched until enabled with watch_channel).",
          "inputSchema": { "type": "object", "properties": { "browser": { "type": "string", "description": "chrome, firefox, edge, safari, brave; another profile as chrome:Profile 2" } }, "required": ["browser"] } },
        { "name": "check_new_videos", "description": "Check watched channels for new uploads and list every video waiting for analysis.",
          "inputSchema": { "type": "object", "properties": {} } },
        { "name": "get_transcript", "description": "Timestamped transcript ([mm:ss] paragraphs) of a tx video id or any video URL (YouTube, X, Vimeo…). Long ones come in parts.",
          "inputSchema": { "type": "object", "properties": { "video": { "type": "string", "description": "Video id or URL" }, "part": { "type": "integer", "minimum": 1, "default": 1 } }, "required": ["video"] } },
        { "name": "save_analysis", "description": "Store your analysis of a video for the user.",
          "inputSchema": { "type": "object", "required": ["video_id", "relevance", "verdict", "tldr", "takeaways", "watch"], "properties": {
            "video_id": video_id,
            "relevance": { "type": "integer", "minimum": 0, "maximum": 5, "description": "0 irrelevant … 5 must-watch, for this user" },
            "verdict": { "type": "string", "description": "One sentence: why, for this user" },
            "tldr": { "type": "string", "description": "2-3 sentences: the core content or claim" },
            "takeaways": { "type": "array", "items": { "type": "object", "required": ["text"], "properties": { "at": stamp, "text": { "type": "string" } } } },
            "watch": { "type": "object", "required": ["advice", "reason"], "properties": {
                "advice": { "type": "string", "enum": ["skip", "skim", "watch"] }, "from": stamp, "to": stamp, "reason": { "type": "string" } } } } } },
        { "name": "skip_video", "description": "Give up on a video for now (e.g. no usable transcript). It is retried on later runs, up to 3 attempts.",
          "inputSchema": { "type": "object", "properties": { "video_id": video_id, "reason": { "type": "string" } }, "required": ["video_id", "reason"] } },
        { "name": "feed", "description": "Videos analyzed recently, best first within the time window.",
          "inputSchema": { "type": "object", "properties": { "min_relevance": { "type": "integer", "default": 0 }, "limit": { "type": "integer", "default": 20 } } } },
        { "name": "write_digest", "description": "Write a markdown digest of everything analyzed since the last digest, and notify the user if something is worth their time.",
          "inputSchema": { "type": "object", "properties": {} } }
    ])
}

fn str_arg<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args[key]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow!("missing {key}"))
}

fn video_json(v: &db::Video) -> Value {
    let tldr = v
        .analysis
        .as_deref()
        .map(|a| report::section(a, "TL;DR"))
        .unwrap_or_default();
    json!({ "id": v.id, "title": v.title, "channel": v.channel, "published": v.published, "url": v.url,
            "status": v.status, "relevance": v.relevance, "verdict": v.verdict, "tldr": tldr,
            "length": v.duration.map(captions::fmt_time), "error": v.error })
}

/// The transcript URL for a tx video id, or the argument itself if it is a URL.
fn resolve(db: &Db, video: &str) -> Result<String> {
    if video.contains("://") {
        return Ok(video.to_string());
    }
    let conn = db.lock().unwrap();
    let v = db::video(&conn, video)?
        .with_context(|| format!("no video {video} in tx; pass a URL instead"))?;
    Ok(v.url)
}

async fn call(db: &Db, name: &str, args: &Value) -> Result<String> {
    let silent: ytdlp::Progress = Box::new(|_| {});
    Ok(match name {
        "get_profile" => {
            let s = db::get_settings(&db.lock().unwrap())?;
            serde_json::to_string_pretty(
                &json!({ "profile": s.profile, "worth_your_time_from_score": s.notify_min }),
            )?
        }
        "set_profile" => {
            let conn = db.lock().unwrap();
            let mut s = db::get_settings(&conn)?;
            s.profile = str_arg(args, "profile")?.trim().to_string();
            db::save_settings(&conn, &s)?;
            "Saved.".into()
        }
        "watch_channel" => format!(
            "Watching {}.",
            feeds::add_channel(db, str_arg(args, "channel")?).await?
        ),
        "unwatch_channel" => {
            if !db::set_enabled(&db.lock().unwrap(), str_arg(args, "channel_id")?, false)? {
                bail!("no such channel");
            }
            "Stopped watching.".into()
        }
        "list_channels" => {
            let only = args["watching_only"].as_bool().unwrap_or(true);
            let list: Vec<Value> = db::channels(&db.lock().unwrap())?
                .into_iter()
                .filter(|c| !only || c.enabled)
                .map(|c| json!({ "id": c.id, "title": c.title, "watching": c.enabled, "analyzed": c.analyzed, "checked_at": c.checked_at }))
                .collect();
            serde_json::to_string_pretty(&list)?
        }
        "import_subscriptions" => {
            let n = feeds::import_subscriptions(db, str_arg(args, "browser")?).await?;
            format!(
                "Imported {n} subscriptions. Watch the ones the user wants with watch_channel (use list_channels with watching_only=false for ids)."
            )
        }
        "check_new_videos" => {
            let queued = feeds::poll(db).await?;
            let conn = db.lock().unwrap();
            let watched = db::channels(&conn)?.iter().filter(|c| c.enabled).count();
            let pending: Vec<Value> = db::pending(&conn)?.iter().map(video_json).collect();
            serde_json::to_string_pretty(
                &json!({ "watched_channels": watched, "new_since_last_check": queued, "waiting_for_analysis": pending }),
            )?
        }
        "get_transcript" => {
            let url = resolve(db, str_arg(args, "video")?)?;
            let model = db::get_settings(&db.lock().unwrap())?.model;
            let (t, _) = ytdlp::load(&url, None, false, &model, false, &silent).await?;
            let text = captions::render(&t, true);
            // Split at paragraph boundaries.
            let mut parts: Vec<String> = vec![String::new()];
            for para in text.split_inclusive("\n\n") {
                if parts.last().unwrap().len() + para.len() > PART_CHARS
                    && !parts.last().unwrap().is_empty()
                {
                    parts.push(String::new());
                }
                parts.last_mut().unwrap().push_str(para);
            }
            let n = parts.len();
            let part = (args["part"].as_u64().unwrap_or(1) as usize).clamp(1, n);
            let note = if n > 1 {
                format!(
                    "[Part {part} of {n}. Call get_transcript with part={} for more.]\n\n",
                    (part % n) + 1
                )
            } else {
                String::new()
            };
            format!("{note}{}", parts.swap_remove(part - 1))
        }
        "save_analysis" => {
            let id = str_arg(args, "video_id")?;
            let a: AgentAnalysis =
                serde_json::from_value(args.clone()).context("invalid analysis")?;
            let md = report::compose(&a)?;
            let url = resolve(db, id)?;
            let model = db::get_settings(&db.lock().unwrap())?.model;
            let duration = ytdlp::load(&url, None, false, &model, false, &silent)
                .await
                .ok()
                .and_then(|(t, _)| t.duration);
            if !db::save_analysis(
                &db.lock().unwrap(),
                id,
                a.relevance,
                a.verdict.trim(),
                &md,
                duration,
            )? {
                bail!("no video {id} in tx");
            }
            "Saved.".into()
        }
        "skip_video" => {
            if !db::mark_failed(
                &db.lock().unwrap(),
                str_arg(args, "video_id")?,
                str_arg(args, "reason")?,
            )? {
                bail!("no such video");
            }
            "Skipped; it will be retried on a later run.".into()
        }
        "feed" => {
            let min = args["min_relevance"].as_i64().unwrap_or(0);
            let limit = args["limit"].as_i64().unwrap_or(20).clamp(1, 100);
            let list: Vec<Value> = db::analyzed(&db.lock().unwrap(), None, min, limit)?
                .iter()
                .map(video_json)
                .collect();
            serde_json::to_string_pretty(&list)?
        }
        "write_digest" => {
            let d = report::write_digest(&db.lock().unwrap())?;
            crate::notify_digest(&d);
            match &d.path {
                Some(p) => format!("Digest written to {}\n\n{}", p.display(), d.markdown),
                None => d.markdown,
            }
        }
        other => bail!("unknown tool {other}"),
    })
}

async fn handle(db: &Db, msg: &Value) -> Option<Value> {
    let id = msg.get("id")?.clone(); // notifications get no reply
    let method = msg["method"].as_str().unwrap_or_default();
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": msg["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "tx", "version": env!("CARGO_PKG_VERSION") },
            "instructions": INSTRUCTIONS,
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => {
            let name = msg["params"]["name"].as_str().unwrap_or_default();
            let empty = json!({});
            let args = msg["params"].get("arguments").unwrap_or(&empty);
            Ok(match call(db, name, args).await {
                Ok(text) => json!({ "content": [{ "type": "text", "text": text }] }),
                Err(e) => {
                    json!({ "content": [{ "type": "text", "text": format!("{e:#}") }], "isError": true })
                }
            })
        }
        _ => Err(json!({ "code": -32601, "message": format!("method not found: {method}") })),
    };
    Some(match result {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": e }),
    })
}

pub async fn serve() -> Result<()> {
    let db: Db = Mutex::new(db::connect()?);
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut out = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle(&db, &msg).await,
            Err(e) => Some(
                json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": e.to_string() } }),
            ),
        };
        if let Some(r) = reply {
            out.write_all(format!("{r}\n").as_bytes()).await?;
            out.flush().await?;
        }
    }
    Ok(())
}
