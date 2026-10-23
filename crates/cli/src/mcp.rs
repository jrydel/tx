//! `tx mcp`: an MCP server over stdio. tx fetches (channels, feeds, transcripts) on this computer;
//! the user's agent (Claude Code, Claude Desktop, Codex…) does the analysis on their subscription.

use crate::guide;
use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tx_core::captions::Transcript;
use tx_core::db::{self, Db};
use tx_core::debrief;
use tx_core::report::{self, AgentAnalysis};
use tx_core::{captions, feeds, podcasts, ytdlp};

/// Transcript parts stay under typical MCP tool-output limits (~25k tokens).
const PART_CHARS: usize = 60_000;

const INSTRUCTIONS: &str = "tx watches YouTube channels and podcasts on this computer. To screen new uploads: get_profile, \
check_new_videos, then for each video or episode get_transcript and save_analysis (or skip_video, or pass_video for a \
podcast episode that clearly doesn't fit from its title and notes), then write_digest, then save_debrief when it asks for \
one. For a channel's or show's latest use get_profile, then recent_videos. Every analysis is for the profile's owner. Show \
results to the user with show_videos: without ids it is their brief for the last 7 days, with the debrief on top; pass ids \
only to show specific videos. At the start of a conversation about tx, call status and guide the user to its next step; \
while setup isn't finished, show the onboarding panel. The screen-videos skill has the full procedure and scoring rubric.";

/// How long get_transcript waits for a transcription before telling the agent to come back; under
/// the tool-call timeouts of common hosts, while the transcription keeps running.
const TRANSCRIPT_WAIT: Duration = Duration::from_secs(40);

/// MCP Apps: hosts that render them (Claude Desktop, claude.ai, the Codex app) show `show_videos`
/// results as cards from this page; others get markdown.
const UI_EXTENSION: &str = "io.modelcontextprotocol/ui";
const CARDS_URI: &str = "ui://tx/videos.html";
const CARDS_MIME: &str = "text/html;profile=mcp-app";
const CARDS_HTML: &str = include_str!("videos.html");

fn tools() -> Value {
    let video_id = json!({ "type": "string", "description": "Video or episode id from check_new_videos, recent_videos or feed" });
    let stamp = json!({ "type": "string", "description": "Timestamp mm:ss or h:mm:ss from the transcript" });
    let clip = json!({ "type": "object", "required": ["video_id", "from", "to", "say"], "properties": {
        "video_id": { "type": "string", "description": "An analyzed YouTube video of the week (not a podcast episode)" },
        "say": { "type": "string", "description": "One punchy sentence, 120 characters at most (140 is the cut): why this clip matters to this user. No hype, no invented facts, in the profile's language" },
        "moment_from": { "type": "string", "description": "mm:ss or h:mm:ss where the key point starts, inside from..to, taken from the analyses' takeaway timestamps. The clip plays moment_from..moment_to at normal speed with sound" },
        "moment_to": { "type": "string", "description": "mm:ss or h:mm:ss where the key point ends; 8-15 seconds after moment_from (4 at least, 25 at most). An invalid moment is dropped, the clip stays" },
        "from": { "type": "string", "description": "mm:ss or h:mm:ss where the surrounding stretch starts; it plays fast and muted behind the line" },
        "to": { "type": "string", "description": "mm:ss or h:mm:ss where it ends; 1-6 minutes after from, 10 at most" } } });
    json!([
        { "name": "status", "description": "Where the user stands with tx (profile, what they follow, what's waiting, daily brief) and the next step to guide them to, with things they could say. Call it at the start of a conversation about tx and whenever the user seems unsure what to do.",
          "inputSchema": { "type": "object", "properties": {} } },
        { "name": "set_daily_brief", "description": "Turn the daily brief on or off: every day at a set time, new uploads are screened with the user's agent, a digest is written and a notification shows what's worth their time. Needs Claude Code or Codex installed.",
          "inputSchema": { "type": "object", "properties": {
            "at": { "type": "string", "description": "Local time HH:MM", "default": "08:00" },
            "off": { "type": "boolean", "default": false } } } },
        { "name": "onboarding", "description": "Show the setup panel: the user writes what they want to learn, picks what to follow (import buttons, then a checklist of their channels and podcasts) and turns on the daily brief, step by step. Use it when status says setup isn't finished, or when the user asks to set up tx or continue onboarding. Once they've imported channels, pass `suggest` with the ids that fit their profile so they see your picks pre-ticked.",
          "inputSchema": { "type": "object", "properties": {
            "suggest": { "type": "array", "description": "Channel or show ids (from list_channels) that fit the profile", "items": { "type": "object", "required": ["id"], "properties": {
                "id": { "type": "string" }, "why": { "type": "string", "description": "A few words: why it fits" } } } },
            "step": { "type": "string", "enum": ["about", "follow", "daily", "done"], "description": "Open this step instead of the first unfinished one" } } },
          "_meta": { "ui": { "resourceUri": CARDS_URI }, "openai/ui": { "preferredModelDisplayMode": "fullscreen" } } },
        { "name": "search_sources", "description": "Search YouTube channels and podcasts by name for the setup panel's autocomplete.",
          "inputSchema": { "type": "object", "properties": { "query": { "type": "string" } }, "required": ["query"] },
          "_meta": { "ui": { "visibility": ["app"] } } },
        { "name": "suggest_channels", "description": "Have the user's AI pick which imported channels fit their profile (for the setup panel).",
          "inputSchema": { "type": "object", "properties": {} },
          "_meta": { "ui": { "visibility": ["app"] } } },
        { "name": "screen_now", "description": "Check for new uploads and have the user's AI analyze them in the background (for the setup panel's first brief).",
          "inputSchema": { "type": "object", "properties": {} },
          "_meta": { "ui": { "visibility": ["app"] } } },
        { "name": "get_profile", "description": "Who the user is and what they want to learn, plus the score that counts as worth their time.",
          "inputSchema": { "type": "object", "properties": {} } },
        { "name": "set_profile", "description": "Save the user's description of who they are and what they want to learn.",
          "inputSchema": { "type": "object", "properties": { "profile": { "type": "string" } }, "required": ["profile"] } },
        { "name": "watch_channel", "description": "Start watching YouTube channels and podcasts. Only uploads published from now on are analyzed automatically; recent_videos still lists earlier ones.",
          "inputSchema": { "type": "object", "properties": { "channels": { "type": "array", "minItems": 1, "items": { "type": "string" }, "description": "Each an @handle, channel URL, channel or show id (from list_channels), a URL of one of its videos, or a podcast feed URL (from find_podcasts)" } }, "required": ["channels"] } },
        { "name": "find_podcasts", "description": "Search Apple's podcast directory by name or topic; returns shows with their feed URLs for watch_channel.",
          "inputSchema": { "type": "object", "properties": { "query": { "type": "string" } }, "required": ["query"] } },
        { "name": "unwatch_channel", "description": "Stop watching a channel.",
          "inputSchema": { "type": "object", "properties": { "channel_id": { "type": "string" } }, "required": ["channel_id"] } },
        { "name": "list_channels", "description": "YouTube channels and podcasts tx knows about.",
          "inputSchema": { "type": "object", "properties": { "watching_only": { "type": "boolean", "default": true } } } },
        { "name": "import_subscriptions", "description": "Import the user's YouTube subscriptions (from a browser signed in to YouTube on this computer, or a Google Takeout export) or podcast subscriptions (an OPML file exported from their podcast app). Imported channels and shows are not watched until the user picks them.",
          "inputSchema": { "type": "object", "properties": {
            "browser": { "type": "string", "description": "chrome, firefox, edge, safari, brave; another profile as chrome:Profile 2" },
            "file": { "type": "string", "description": "Path to a Google Takeout .zip or its subscriptions.csv, or a podcast app's .opml export" } } } },
        { "name": "check_new_videos", "description": "Check watched channels and podcasts for new uploads and list everything waiting for analysis. Podcast episodes include their show notes and whether a transcript is published.",
          "inputSchema": { "type": "object", "properties": {} } },
        { "name": "get_transcript", "description": "Timestamped transcript ([mm:ss] paragraphs) of a tx video or episode id, or any video URL (YouTube, X, Vimeo…). Long ones come in parts. Audio without captions or a published transcript is transcribed on this computer, which can take minutes: if it isn't ready, the reply says so and you call again later.",
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
        { "name": "pass_video", "description": "Pass on a podcast episode whose title and show notes clearly don't fit the profile, without transcribing it. It isn't retried or shown.",
          "inputSchema": { "type": "object", "properties": { "video_id": video_id, "reason": { "type": "string", "description": "One line: why it doesn't fit" } }, "required": ["video_id", "reason"] } },
        { "name": "recent_videos", "description": "Latest uploads of channels tx knows, newest first, including ones published before watching started and whether each is analyzed. Use it for \"the latest videos from X\"; analyze any of them with get_transcript and save_analysis.",
          "inputSchema": { "type": "object", "properties": { "channel": { "type": "string", "description": "Channel id or part of its name (see list_channels); omit for all channels" }, "limit": { "type": "integer", "default": 10 } } } },
        { "name": "feed", "description": "Videos analyzed recently, best first within the time window.",
          "inputSchema": { "type": "object", "properties": { "min_relevance": { "type": "integer", "default": 0 }, "limit": { "type": "integer", "default": 20 } } } },
        { "name": "show_videos", "description": "Show videos to the user as cards: thumbnail, score, verdict, TL;DR, timestamped takeaways. Without ids it is the user's brief for the last 7 days (what their channels and shows uploaded, a headline, the debrief across the week's videos, the picks, a page per video): call it that way whenever you talk with the user about their videos. With ids it shows exactly those videos (shown Watch first, then Skim, then Skip, best score first).",
          "inputSchema": { "type": "object", "properties": {
            "video_ids": { "type": "array", "items": { "type": "string" }, "description": "Only these videos; omit for the last 7 days' brief" },
            "min_relevance": { "type": "integer", "default": 0, "description": "Without video_ids: lowest score to include" },
            "limit": { "type": "integer", "description": "Without video_ids: how many (default 60 for the week; 10 for the latest analyzed when the week has none)" } } },
          "_meta": { "ui": { "resourceUri": CARDS_URI }, "openai/ui": { "preferredModelDisplayMode": "fullscreen" } } },
        { "name": "write_digest", "description": "Write a markdown digest of everything analyzed since the last digest, and notify the user if something is worth their time. When the last 7 days' analyzed videos changed since the last debrief, the reply asks you to write one with save_debrief.",
          "inputSchema": { "type": "object", "properties": {} } },
        { "name": "save_debrief", "description": "Store your debrief across the last 7 days' analyzed videos: what they add up to for this user, what to watch first, and the themes that run through two or more of them, plus a script for a reel of about a minute over the week's best YouTube moments (a short intro, then per clip a punchy line, the 8-15 second moment where the key point is said and the stretch around it, then a short outro). Call it when write_digest asks for it. It shows on top of the user's brief; the reel plays from its Watch your debrief button.",
          "inputSchema": { "type": "object", "required": ["headline", "summary"], "properties": {
            "headline": { "type": "string", "description": "One sentence, 200 characters at most" },
            "summary": { "type": "string", "description": "2-4 sentences, 800 characters at most: what the week adds up to for this user and what to watch first" },
            "threads": { "type": "array", "maxItems": 3, "description": "Themes that run through two or more videos", "items": { "type": "object", "required": ["title", "text", "video_ids"], "properties": {
                "title": { "type": "string", "description": "A few words" },
                "text": { "type": "string", "description": "1-2 sentences" },
                "video_ids": { "type": "array", "minItems": 1, "items": { "type": "string" }, "description": "Ids from the analyses write_digest listed" } } } },
            "script": { "type": "object", "required": ["intro", "clips"], "description": "A reel of about a minute for this user: per clip the stretch plays muted and fast behind the line, then the key moment plays with sound. Best clip first. Invalid clips are dropped.", "properties": {
                "intro": { "type": "string", "description": "One short sentence framing the week, 160 characters at most" },
                "clips": { "type": "array", "minItems": 3, "maxItems": 6, "items": clip },
                "outro": { "type": "string", "description": "One short closing line, 120 characters at most" } } } } } }
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
    let mut j = json!({ "id": v.id, "title": v.title, "channel": v.channel, "published": v.published, "url": v.url,
            "status": v.status, "relevance": v.relevance, "verdict": v.verdict, "tldr": tldr,
            "length": v.duration.map(captions::fmt_time), "error": v.error });
    if v.podcast {
        // What the agent needs to judge an episode before spending minutes transcribing it.
        j["kind"] = json!("podcast");
        j["show_notes"] = json!(v.description);
        j["transcript_published"] = json!(v.transcript_url.is_some());
    }
    j
}

type Job = tokio::sync::watch::Receiver<Option<Result<Transcript, String>>>;
/// Transcripts being fetched or transcribed, by video id or URL. A job outlives the tool call that
/// started it, so a long local transcription never hits the host's tool-call timeout.
static JOBS: LazyLock<Mutex<HashMap<String, Job>>> = LazyLock::new(Default::default);

/// Transcript of a tx video or episode id, or of a URL; None while it's still being made after
/// TRANSCRIPT_WAIT. Episodes use the transcript their feed publishes when there is one.
async fn transcript(db: &Db, video: &str) -> Result<Option<Transcript>> {
    let model = db::get_settings(&db.lock().unwrap())?.model;
    let v = if video.contains("://") {
        None
    } else {
        Some(
            db::video(&db.lock().unwrap(), video)?
                .with_context(|| format!("no video {video} in tx; pass a URL instead"))?,
        )
    };
    let url = v
        .as_ref()
        .map_or_else(|| video.to_string(), |v| v.url.clone());
    // An episode's audio URL tells yt-dlp nothing about it; the feed does.
    let names = v
        .as_ref()
        .filter(|v| v.podcast)
        .map(|v| (v.title.clone(), v.channel.clone(), v.duration));
    let mut rx = JOBS
        .lock()
        .unwrap()
        .entry(video.to_string())
        .or_insert_with(|| {
            let (tx, rx) = tokio::sync::watch::channel(None);
            tokio::spawn(async move {
                let got = async {
                    let cached = ytdlp::cache_path(&url, None, false, &model);
                    if let Some(v) = v.filter(|v| v.transcript_url.is_some() && !cached.exists()) {
                        match podcasts::published_transcript(&v).await {
                            Ok(Some(t)) => {
                                tokio::fs::write(&cached, serde_json::to_vec(&t)?).await?;
                                return Ok(t);
                            }
                            Ok(None) => {}
                            Err(e) => eprintln!(
                                "tx: published transcript failed, transcribing instead: {e:#}"
                            ),
                        }
                    }
                    let silent: ytdlp::Progress = Box::new(|_| {});
                    anyhow::Ok(
                        ytdlp::load(&url, None, false, &model, false, &silent)
                            .await?
                            .0,
                    )
                }
                .await;
                let _ = tx.send(Some(got.map_err(|e| format!("{e:#}"))));
            });
            rx
        })
        .clone();
    let Ok(done) = tokio::time::timeout(TRANSCRIPT_WAIT, rx.wait_for(Option::is_some)).await else {
        return Ok(None);
    };
    let got = done.map_err(|_| anyhow!("transcription stopped"))?.clone();
    JOBS.lock().unwrap().remove(video);
    let mut t = got.expect("waited for Some").map_err(|e| anyhow!(e))?;
    if let Some((title, show, length)) = names {
        t.title = title;
        t.uploader = Some(show);
        t.duration = t.duration.or(length);
    }
    Ok(Some(t))
}

async fn call(db: &Db, name: &str, args: &Value) -> Result<String> {
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
        "watch_channel" => {
            let refs: Vec<&str> = args["channels"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            if refs.is_empty() {
                bail!("missing channels");
            }
            let (mut watching, mut failed) = (Vec::new(), Vec::new());
            for r in refs {
                match feeds::add_channel(db, r).await {
                    Ok(title) => watching.push(title),
                    Err(e) => failed.push(format!("{r}: {e:#}")),
                }
            }
            let mut text = format!("Watching {}.", watching.join(", "));
            if !failed.is_empty() {
                text.push_str(&format!("\nNot found:\n{}", failed.join("\n")));
            }
            text
        }
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
                .map(|c| json!({ "id": c.id, "title": c.title, "kind": if c.podcast { "podcast" } else { "youtube" },
                                 "watching": c.enabled, "analyzed": c.analyzed, "checked_at": c.checked_at }))
                .collect();
            serde_json::to_string_pretty(&list)?
        }
        "import_subscriptions" => {
            let (found, new) = match (args["file"].as_str(), args["browser"].as_str()) {
                (Some(f), _) if !f.trim().is_empty() => {
                    let f = f.trim();
                    let path: std::path::PathBuf = f
                        .strip_prefix("~/")
                        .and_then(|rest| dirs::home_dir().map(|h| h.join(rest)))
                        .unwrap_or_else(|| f.into());
                    let opml = path.extension().is_some_and(|e| {
                        e.eq_ignore_ascii_case("opml") || e.eq_ignore_ascii_case("xml")
                    });
                    if opml {
                        podcasts::import_opml(db, &path)?
                    } else {
                        feeds::import_takeout(db, &path)?
                    }
                }
                (_, Some(b)) if !b.trim().is_empty() => {
                    feeds::import_subscriptions(db, b.trim()).await?
                }
                _ => bail!("pass browser or file.\n\n{}", feeds::TAKEOUT_HINT),
            };
            format!(
                "Imported {found} subscriptions ({new} new to tx); imported channels aren't watched until chosen. Next: read the profile with \
                 get_profile, list the channels with list_channels (watching_only=false), and suggest the ones that fit \
                 the profile, with a few words on why. Watch them with watch_channel only after the user confirms or \
                 changes the list; don't watch all of them unless the user asks, since every watched channel's new \
                 videos get analyzed."
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
            let Some(t) = transcript(db, str_arg(args, "video")?).await? else {
                return Ok("Still transcribing this one on the user's computer (long audio takes a few minutes). \
                           Analyze other videos meanwhile, then call get_transcript again for it."
                    .into());
            };
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
            let v = db::video(&db.lock().unwrap(), id)?
                .with_context(|| format!("no video {id} in tx"))?;
            // YouTube's feed has no lengths; the transcript does. Podcast feeds list them.
            let duration = if v.duration.is_some() || v.podcast {
                None
            } else {
                let model = db::get_settings(&db.lock().unwrap())?.model;
                let silent: ytdlp::Progress = Box::new(|_| {});
                ytdlp::load(&v.url, None, false, &model, false, &silent)
                    .await
                    .ok()
                    .and_then(|(t, _)| t.duration)
            };
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
        "pass_video" => {
            if !db::mark_passed(
                &db.lock().unwrap(),
                str_arg(args, "video_id")?,
                str_arg(args, "reason")?,
            )? {
                bail!("no such video");
            }
            "Passed; it won't be transcribed or shown.".into()
        }
        "find_podcasts" => {
            let found = podcasts::search(str_arg(args, "query")?).await?;
            if found.is_empty() {
                bail!(
                    "no podcasts found; try other words, or watch_channel with the show's RSS feed URL"
                );
            }
            serde_json::to_string_pretty(&found)?
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
            let conn = db.lock().unwrap();
            let d = report::write_digest(&conn)?;
            crate::notify_digest(&d);
            let digest = match &d.path {
                Some(p) => format!("Digest written to {}\n\n{}", p.display(), d.markdown),
                None => d.markdown,
            };
            format!("{digest}\n\n---\n\n{}", debrief::request(&conn)?)
        }
        "save_debrief" => {
            let d = debrief::save(&db.lock().unwrap(), args)?;
            format!(
                "Saved: \"{}\", {} threads over {} videos, {} clips. Next, if you're talking with the user, show_videos without ids.",
                d.headline,
                d.threads.len(),
                d.video_ids.len(),
                d.script.as_ref().map_or(0, |s| s.clips.len())
            )
        }
        other => bail!("unknown tool {other}"),
    })
}

fn card_state(status: &str) -> &'static str {
    match status {
        "done" => "done",
        "error" => "failed",
        "passed" => "passed",
        _ => "new",
    }
}

/// The debrief as markdown, with the titles of the videos each thread runs through.
fn debrief_markdown(d: &debrief::Debrief, videos: &[db::Video]) -> String {
    let mut md = format!("## {}\n\n{}\n\n", d.headline, d.summary);
    for t in &d.threads {
        let titles: Vec<&str> = t
            .video_ids
            .iter()
            .filter_map(|id| videos.iter().find(|v| &v.id == id))
            .map(|v| v.title.as_str())
            .collect();
        md.push_str(&format!("**{}** {}", t.title, t.text));
        if !titles.is_empty() {
            md.push_str(&format!(" ({})", titles.join("; ")));
        }
        md.push_str("\n\n");
    }
    md
}

/// `show_videos`: cards (`structuredContent`, rendered by CARDS_HTML) for hosts with MCP Apps, and
/// the same content as markdown for the rest. Without ids it's the last week's brief, or the
/// latest analyzed videos when the week has none.
fn show_videos(db: &Db, args: &Value, ui: bool) -> Result<Value> {
    let conn = db.lock().unwrap();
    let settings = db::get_settings(&conn)?;
    let min_relevance = args["min_relevance"].as_i64().unwrap_or(0);
    let mut days = None;
    let mut videos: Vec<db::Video> = match args["video_ids"].as_array() {
        Some(ids) if !ids.is_empty() => ids
            .iter()
            .filter_map(Value::as_str)
            .filter_map(|id| db::video(&conn, id).transpose())
            .collect::<Result<_>>()?,
        _ => {
            let mut week = db::week(
                &conn,
                debrief::DAYS,
                false,
                args["limit"]
                    .as_i64()
                    .unwrap_or(debrief::MAX_VIDEOS)
                    .clamp(1, debrief::MAX_VIDEOS),
            )?;
            week.retain(|v| v.relevance.unwrap_or(0) >= min_relevance);
            if week.is_empty() {
                db::analyzed(
                    &conn,
                    None,
                    min_relevance,
                    args["limit"].as_i64().unwrap_or(10).clamp(1, 50),
                )?
            } else {
                days = Some(debrief::DAYS);
                week
            }
        }
    };
    let debrief =
        debrief::stored(&conn)?.filter(|d| videos.iter().any(|v| d.video_ids.contains(&v.id)));
    drop(conn);
    if videos.is_empty() {
        bail!("none of these videos are in tx");
    }
    // Watch first, then skim, skip, not analyzed; best score, then newest, first within each.
    let rank = |v: &db::Video| match v.analysis.as_deref().map(|a| report::parts(a).advice) {
        _ if v.status != "done" => 3,
        Some(a) if a == "watch" => 0,
        Some(a) if a == "skim" => 1,
        _ => 2,
    };
    videos.sort_by_cached_key(|v| {
        (
            rank(v),
            std::cmp::Reverse((v.relevance, v.published.clone())),
        )
    });
    let mut cards = Vec::new();
    let mut md = String::new();
    for v in &videos {
        let state = card_state(&v.status);
        let p = report::parts(v.analysis.as_deref().unwrap_or_default());
        // Podcast timestamps open the audio file at that second (a media fragment).
        let at_url = |at: &Option<String>| {
            at.as_deref().and_then(report::seconds).map(|s| {
                if v.podcast {
                    format!("{}#t={s}", v.url)
                } else {
                    format!("{}&t={s}s", v.url)
                }
            })
        };
        let page = if v.podcast {
            v.link.as_deref().unwrap_or(&v.url)
        } else {
            &v.url
        };
        let length = v.duration.map(captions::fmt_time);
        let title = v.title.replace(['[', ']'], "");
        md.push_str(&format!("### [{title}]({page})\n"));
        let mut meta = vec![
            v.channel.clone(),
            chrono::DateTime::parse_from_rfc3339(&v.published)
                .map(|d| {
                    d.with_timezone(&chrono::Local)
                        .format("%Y-%m-%d %H:%M")
                        .to_string()
                })
                .unwrap_or_else(|_| v.published.clone()),
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
            _ => md.push_str(&format!(
                "{} · {}\n\n",
                meta.join(" · "),
                match (state, &v.error) {
                    ("failed", Some(e)) => format!("couldn't be analyzed: {e}"),
                    ("passed", Some(e)) => format!("passed: {e}"),
                    _ => "not analyzed yet".into(),
                }
            )),
        }
        let mut card = json!({
            "id": v.id, "title": v.title, "channel": v.channel, "url": page, "published": v.published,
            "podcast": v.podcast, "length": length, "state": state,
            "thumbnail": if v.podcast { v.image.clone() } else { Some(format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", v.id)) },
            "analyzed": v.status == "done", "relevance": v.relevance, "verdict": v.verdict, "tldr": p.tldr,
            "takeaways": p.takeaways.iter().map(|t| json!({ "text": t.text, "at": t.at, "url": at_url(&t.at) })).collect::<Vec<_>>(),
            "advice": p.advice, "skim_from": p.skim_from, "skim_to": p.skim_to, "skim_url": at_url(&p.skim_from),
            "reason": p.reason,
        });
        if matches!(state, "failed" | "passed") {
            card["error"] = json!(v.error);
        }
        cards.push(card);
    }
    let debrief_md = debrief.as_ref().map(|d| debrief_markdown(d, &videos));
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
        let shown = match &debrief {
            Some(d) => format!(
                " with the debrief on top (\"{}\"), so don't restate it either",
                d.headline
            ),
            None => String::new(),
        };
        format!(
            "The user now sees these {} videos as cards with thumbnails, scores, TL;DRs and timestamped takeaways{shown}. \
             Don't repeat them; add at most two sentences (for example, which to watch first).\n\n{}",
            videos.len(),
            list.join("\n")
        )
    } else {
        format!(
            "Show the user this markdown as is, then add at most two sentences:\n\n{}{md}",
            debrief_md.unwrap_or_default()
        )
    };
    let mut structured =
        json!({ "videos": cards, "profile": settings.profile, "debrief": debrief });
    if let Some(days) = days {
        structured["days"] = json!(days);
    }
    Ok(json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": structured,
    }))
}

/// What the connected app told us about itself at `initialize`.
#[derive(Default)]
struct Session {
    /// It renders MCP Apps (the brief and the setup panel).
    ui: bool,
    /// Its name (clientInfo), to run the daily brief with the same agent.
    client: String,
}

async fn handle(db: &Db, session: &mut Session, msg: &Value) -> Option<Value> {
    let id = msg.get("id")?.clone(); // notifications get no reply
    let method = msg["method"].as_str().unwrap_or_default();
    let params = &msg["params"];
    let result = match method {
        "initialize" => {
            session.ui = params["capabilities"]["extensions"]
                .get(UI_EXTENSION)
                .is_some();
            session.client = params["clientInfo"]["name"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            Ok(json!({
                "protocolVersion": params["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": { "tools": {}, "resources": {}, "prompts": {} },
                "serverInfo": { "name": "tx", "version": env!("CARGO_PKG_VERSION") },
                "instructions": INSTRUCTIONS,
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "prompts/list" => Ok(json!({ "prompts": guide::prompts() })),
        "prompts/get" => guide::prompt(
            params["name"].as_str().unwrap_or_default(),
            &params["arguments"],
        )
        .map_err(|e| json!({ "code": -32602, "message": format!("{e:#}") })),
        "resources/list" => Ok(json!({ "resources": [
            { "uri": CARDS_URI, "name": "Video cards", "mimeType": CARDS_MIME }
        ] })),
        "resources/read" if params["uri"] == CARDS_URI => {
            // Podcast artwork lives on each show's own host; allow the hosts of watched shows.
            // Video thumbnails, channel avatars (setup search), and Apple's podcast artwork.
            let mut hosts: Vec<String> = [
                "https://i.ytimg.com",
                "https://yt3.ggpht.com",
                "https://yt3.googleusercontent.com",
                "https://*.mzstatic.com",
                // The YouTube IFrame API script and player assets.
                "https://www.youtube.com",
                "https://s.ytimg.com",
            ]
            .map(String::from)
            .to_vec();
            for c in db::channels(&db.lock().unwrap()).unwrap_or_default() {
                let origin = c.image.as_deref().and_then(|u| {
                    let (scheme, rest) = u.split_once("://")?;
                    Some(format!("{scheme}://{}", rest.split('/').next()?))
                });
                if let Some(o) = origin.filter(|o| o.starts_with("https://") && !hosts.contains(o))
                {
                    hosts.push(o);
                }
            }
            Ok(json!({ "contents": [{
                "uri": CARDS_URI, "mimeType": CARDS_MIME, "text": CARDS_HTML,
                "_meta": { "ui": { "csp": {
                    "resourceDomains": hosts,
                    "frameDomains": ["https://www.youtube.com", "https://www.youtube-nocookie.com"],
                }, "prefersBorder": false } },
            }] }))
        }
        "resources/read" => Err(json!({ "code": -32002, "message": "resource not found" })),
        "tools/call" => {
            let name = params["name"].as_str().unwrap_or_default();
            let empty = json!({});
            let args = params.get("arguments").unwrap_or(&empty);
            let text = |t: String| json!({ "content": [{ "type": "text", "text": t }] });
            let result = match name {
                "show_videos" => show_videos(db, args, session.ui),
                "onboarding" => guide::onboarding(db, args, session.ui),
                "status" => {
                    guide::status(db).and_then(|s| Ok(text(serde_json::to_string_pretty(&s)?)))
                }
                "set_daily_brief" => guide::set_daily_brief(db, args, &session.client).map(text),
                // The setup panel's own tools: structured results for the view, plus text for any agent.
                "search_sources" | "suggest_channels" | "screen_now" => {
                    let out = match name {
                        "search_sources" => {
                            guide::search_sources(db, args["query"].as_str().unwrap_or_default())
                                .await
                        }
                        "suggest_channels" => guide::suggest_channels(db, &session.client).await,
                        _ => guide::screen_now(db, &session.client).await,
                    };
                    out.and_then(|v| Ok(json!({ "content": [{ "type": "text", "text": serde_json::to_string(&v)? }], "structuredContent": v })))
                }
                _ => call(db, name, args).await.map(text),
            };
            // First use in an app: until setup is done, steer the agent to the setup panel.
            const SETUP: [&str; 13] = [
                "status",
                "onboarding",
                "set_profile",
                "get_profile",
                "watch_channel",
                "unwatch_channel",
                "import_subscriptions",
                "find_podcasts",
                "list_channels",
                "set_daily_brief",
                "search_sources",
                "suggest_channels",
                "screen_now",
            ];
            let result = match result {
                Ok(mut r) if !SETUP.contains(&name) && guide::needs_setup(db).unwrap_or(false) => {
                    let how = if session.ui {
                        "show the onboarding panel so they can set it up in a minute"
                    } else {
                        "call onboarding and walk them through setup in chat"
                    };
                    let note = format!(
                        "Note: tx isn't set up yet (no profile, or nothing followed). Before going on, {how}; then come back to this."
                    );
                    if let Some(c) = r["content"].as_array_mut() {
                        c.insert(0, json!({ "type": "text", "text": note }));
                    }
                    Ok(r)
                }
                other => other,
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
    let mut session = Session::default();
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut out = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle(&db, &mut session, &msg).await,
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
