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
For a channel's latest videos use get_profile, then recent_videos. Every analysis is for the profile's owner. \
Show results to the user with show_videos. The screen-videos skill has the full procedure and scoring rubric.";

/// MCP Apps: hosts that render them (Claude Desktop, claude.ai, the Codex app) show `show_videos`
/// results as cards from this page; others get markdown.
const UI_EXTENSION: &str = "io.modelcontextprotocol/ui";
const CARDS_URI: &str = "ui://tx/videos.html";
const CARDS_MIME: &str = "text/html;profile=mcp-app";
const CARDS_HTML: &str = include_str!("videos.html");

fn tools() -> Value {
    let video_id = json!({ "type": "string", "description": "Video id from check_new_videos, recent_videos or feed" });
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
        { "name": "save_analysis", "description": "Store your analysis of a video in tx (any video from check_new_videos, recent_videos or feed), so it shows in feed, show_videos and the digest and is never redone.",
          "inputSchema": { "type": "object", "required": ["video_id", "relevance", "verdict", "tldr", "takeaways", "watch"], "properties": {
            "video_id": video_id,
            "relevance": { "type": "integer", "minimum": 0, "maximum": 5, "description": "0 irrelevant … 5 must-watch, for this user" },
            "verdict": { "type": "string", "description": "One sentence: why, for this user" },
            "tldr": { "type": "string", "description": "2-3 sentences: the core content or claim" },
            "takeaways": { "type": "array", "description": "3-5 points for this user: what they should know, try or change because of that moment, with the concrete number, tool or technique. Not a retelling of the video; the TL;DR does that.",
              "items": { "type": "object", "required": ["text"], "properties": { "at": stamp, "text": { "type": "string" } } } },
            "watch": { "type": "object", "required": ["advice", "reason"], "properties": {
                "advice": { "type": "string", "enum": ["skip", "skim", "watch"] }, "from": stamp, "to": stamp, "reason": { "type": "string" } } } } } },
        { "name": "skip_video", "description": "Give up on a video for now (e.g. no usable transcript). It is retried on later runs, up to 3 attempts.",
          "inputSchema": { "type": "object", "properties": { "video_id": video_id, "reason": { "type": "string" } }, "required": ["video_id", "reason"] } },
        { "name": "recent_videos", "description": "Latest uploads of channels tx knows, newest first, including ones published before watching started and whether each is analyzed. Use it for \"the latest videos from X\"; analyze any of them with get_transcript and save_analysis.",
          "inputSchema": { "type": "object", "properties": { "channel": { "type": "string", "description": "Channel id or part of its name (see list_channels); omit for all channels" }, "limit": { "type": "integer", "default": 10 } } } },
        { "name": "feed", "description": "Videos analyzed recently, best first within the time window.",
          "inputSchema": { "type": "object", "properties": { "min_relevance": { "type": "integer", "default": 0 }, "limit": { "type": "integer", "default": 20 } } } },
        { "name": "show_videos", "description": "Show videos to the user as cards: thumbnail, score, verdict, TL;DR, timestamped takeaways. Call it whenever you present analyzed videos to the user, with their ids in the order to show; without ids it shows the latest analyzed videos.",
          "inputSchema": { "type": "object", "properties": {
            "video_ids": { "type": "array", "items": { "type": "string" } },
            "min_relevance": { "type": "integer", "default": 0, "description": "Without video_ids: lowest score to include" },
            "limit": { "type": "integer", "default": 10, "description": "Without video_ids: how many" } } },
          "_meta": { "ui": { "resourceUri": CARDS_URI } } },
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
        "recent_videos" => {
            if let Err(e) = feeds::poll(db).await {
                eprintln!("tx: checking channels failed, listing known videos: {e:#}");
            }
            let channel = args["channel"]
                .as_str()
                .map(|s| s.trim().trim_start_matches('@'))
                .filter(|s| !s.is_empty());
            let limit = args["limit"].as_i64().unwrap_or(10).clamp(1, 50);
            let list: Vec<Value> = db::recent(&db.lock().unwrap(), channel, limit)?
                .iter()
                .map(video_json)
                .collect();
            if list.is_empty() {
                bail!("no videos for that channel in tx; watch it with watch_channel first");
            }
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

/// `show_videos`: cards (`structuredContent`, rendered by CARDS_HTML) for hosts with MCP Apps, and
/// the same content as markdown for the rest.
fn show_videos(db: &Db, args: &Value, ui: bool) -> Result<Value> {
    let conn = db.lock().unwrap();
    let worth_from = db::get_settings(&conn)?.notify_min;
    let videos: Vec<db::Video> = match args["video_ids"].as_array() {
        Some(ids) if !ids.is_empty() => ids
            .iter()
            .filter_map(Value::as_str)
            .filter_map(|id| db::video(&conn, id).transpose())
            .collect::<Result<_>>()?,
        _ => db::analyzed(
            &conn,
            None,
            args["min_relevance"].as_i64().unwrap_or(0),
            args["limit"].as_i64().unwrap_or(10).clamp(1, 50),
        )?,
    };
    drop(conn);
    if videos.is_empty() {
        bail!("none of these videos are in tx");
    }
    let mut cards = Vec::new();
    let mut md = String::new();
    for v in &videos {
        let p = report::parts(v.analysis.as_deref().unwrap_or_default());
        let at_url = |at: &Option<String>| {
            at.as_deref()
                .and_then(report::seconds)
                .map(|s| format!("{}&t={s}s", v.url))
        };
        let length = v.duration.map(captions::fmt_time);
        let title = v.title.replace(['[', ']'], "");
        md.push_str(&format!("### [{title}]({})\n", v.url));
        let mut meta = vec![
            v.channel.clone(),
            v.published.get(..10).unwrap_or_default().to_string(),
        ];
        meta.extend(length.clone());
        match v.relevance {
            Some(r) if v.status == "done" => {
                md.push_str(&format!(
                    "**{r}/5 · {}** · {}\n\n",
                    p.advice,
                    meta.join(" · ")
                ));
                md.push_str(&format!(
                    "{}\n\n{}\n\n",
                    v.verdict.as_deref().unwrap_or_default(),
                    p.tldr
                ));
                if !p.takeaways.is_empty() {
                    md.push_str("**For you:**\n");
                }
                for t in &p.takeaways {
                    match (&t.at, at_url(&t.at)) {
                        (Some(at), Some(u)) => md.push_str(&format!("- [{at}]({u}) {}\n", t.text)),
                        _ => md.push_str(&format!("- {}\n", t.text)),
                    }
                }
                md.push('\n');
            }
            _ => md.push_str(&format!("{} · not analyzed yet\n\n", meta.join(" · "))),
        }
        cards.push(json!({
            "id": v.id, "title": v.title, "channel": v.channel, "url": v.url, "published": v.published,
            "thumbnail": format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", v.id), "length": length,
            "analyzed": v.status == "done", "relevance": v.relevance, "verdict": v.verdict, "tldr": p.tldr,
            "takeaways": p.takeaways.iter().map(|t| json!({ "text": t.text, "at": t.at, "url": at_url(&t.at) })).collect::<Vec<_>>(),
            "advice": p.advice, "skim_from": p.skim_from, "skim_to": p.skim_to, "skim_url": at_url(&p.skim_from),
            "reason": p.reason,
        }));
    }
    let text = if ui {
        let list: Vec<String> = videos
            .iter()
            .map(|v| {
                format!(
                    "- {} ({}): {}",
                    v.title,
                    v.id,
                    v.relevance
                        .map_or("not analyzed".into(), |r| format!("{r}/5"))
                )
            })
            .collect();
        format!(
            "The user now sees these {} videos as cards with thumbnails, scores, TL;DRs and timestamped takeaways. \
             Don't repeat them; add at most two sentences (for example, which to watch first).\n\n{}",
            videos.len(),
            list.join("\n")
        )
    } else {
        format!("Show the user this markdown as is, then add at most two sentences:\n\n{md}")
    };
    Ok(json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": { "videos": cards, "worth_from": worth_from },
    }))
}

async fn handle(db: &Db, ui: &mut bool, msg: &Value) -> Option<Value> {
    let id = msg.get("id")?.clone(); // notifications get no reply
    let method = msg["method"].as_str().unwrap_or_default();
    let params = &msg["params"];
    let result = match method {
        "initialize" => {
            *ui = params["capabilities"]["extensions"]
                .get(UI_EXTENSION)
                .is_some();
            Ok(json!({
                "protocolVersion": params["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": { "tools": {}, "resources": {} },
                "serverInfo": { "name": "tx", "version": env!("CARGO_PKG_VERSION") },
                "instructions": INSTRUCTIONS,
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "resources/list" => Ok(json!({ "resources": [
            { "uri": CARDS_URI, "name": "Video cards", "mimeType": CARDS_MIME }
        ] })),
        "resources/read" if params["uri"] == CARDS_URI => Ok(json!({ "contents": [{
            "uri": CARDS_URI, "mimeType": CARDS_MIME, "text": CARDS_HTML,
            "_meta": { "ui": { "csp": { "resourceDomains": ["https://i.ytimg.com"] }, "prefersBorder": false } },
        }] })),
        "resources/read" => Err(json!({ "code": -32002, "message": "resource not found" })),
        "tools/call" => {
            let name = params["name"].as_str().unwrap_or_default();
            let empty = json!({});
            let args = params.get("arguments").unwrap_or(&empty);
            let result = if name == "show_videos" {
                show_videos(db, args, *ui)
            } else {
                call(db, name, args)
                    .await
                    .map(|text| json!({ "content": [{ "type": "text", "text": text }] }))
            };
            Ok(result.unwrap_or_else(|e| {
                json!({ "content": [{ "type": "text", "text": format!("{e:#}") }], "isError": true })
            }))
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
    let mut ui = false;
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut out = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle(&db, &mut ui, &msg).await,
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
