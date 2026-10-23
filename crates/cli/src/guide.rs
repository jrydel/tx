//! Guiding users through tx inside their AI app: where they stand and the next step (`status`),
//! turning on the daily brief, and built-in commands (MCP prompts) the app lists in its menu.

use crate::agents::Agent;
use crate::schedule;
use anyhow::{Result, bail};
use serde_json::{Value, json};
use tx_core::db::{self, Db};
use tx_core::details;
use tx_core::report;

/// Setup isn't done until tx knows what the user wants to learn and follows something.
pub fn needs_setup(db: &Db) -> Result<bool> {
    let conn = db.lock().unwrap();
    Ok(db::get_settings(&conn)?.profile.trim().is_empty()
        || !db::channels(&conn)?.iter().any(|c| c.enabled))
}

/// Where the user stands, and the single next step for the agent to guide them to.
pub fn status(db: &Db) -> Result<Value> {
    let conn = db.lock().unwrap();
    let profile = db::get_settings(&conn)?.profile;
    let channels = db::channels(&conn)?;
    let waiting = db::pending(&conn)?.len();
    drop(conn);
    let watched: Vec<&db::Channel> = channels.iter().filter(|c| c.enabled).collect();
    let shows = watched.iter().filter(|c| c.podcast).count();
    let imported = channels.len() - watched.len();
    let analyzed: i64 = channels.iter().map(|c| c.analyzed).sum();
    let daily = schedule::scheduled();
    let example = watched.first().map_or("Fireship", |c| c.title.as_str());

    let (next, say): (String, Vec<String>) = if profile.trim().is_empty() {
        (
            "Setup has just begun: show the setup panel with onboarding. It starts by asking who they are and what they want to learn; every score is against it. In apps without panels, ask in chat and save it with set_profile.".into(),
            vec!["I'm a backend engineer building LLM features. I want concrete agent and eval techniques and notable model launches; skip hype.".into()],
        )
    } else if watched.is_empty() && imported > 0 {
        (
            format!(
                "They imported {imported} channels or shows but follow none. Read list_channels (watching_only false), pick the ones that fit their profile, and show onboarding with `suggest` so they tick what to follow."
            ),
            vec!["Suggest which of my subscriptions to follow".into()],
        )
    } else if watched.is_empty() {
        (
            "They follow nothing yet: show onboarding at its follow step (import their YouTube subscriptions, follow podcasts by name, or name channels).".into(),
            vec![
                "Import my YouTube subscriptions from Chrome and suggest which to follow".into(),
                "Follow Fireship and Theo".into(),
                "Follow the Latent Space podcast".into(),
            ],
        )
    } else if waiting > 0 {
        (
            format!(
                "{waiting} new videos or episodes are waiting. Offer to screen them now, then show the brief with show_videos."
            ),
            vec!["Screen my new videos and show me the brief".into()],
        )
    } else if analyzed == 0 {
        (
            "Nothing new since they started following, so the first brief is still empty. Offer a TL;DR of the latest uploads of a channel they follow, so they see what tx does right away.".into(),
            vec![format!("TL;DR of the latest 3 videos from {example}")],
        )
    } else if !daily {
        (
            "All caught up. Offer the latest brief (show_videos) and a daily brief: with set_daily_brief, new uploads are screened every morning and a notification says what's worth their time.".into(),
            vec!["Show me my latest brief".into(), "Send me a daily brief at 8:00".into()],
        )
    } else {
        (
            "All caught up, and the daily brief is on. Offer the latest brief (show_videos), or more channels and podcasts that fit their profile.".into(),
            vec!["Show me my latest brief".into(), "Suggest more channels and podcasts for me".into()],
        )
    };
    Ok(json!({
        "profile": if profile.trim().is_empty() { Value::Null } else { json!(profile) },
        "following": { "youtube_channels": watched.len() - shows, "podcasts": shows, "imported_not_followed": imported },
        "waiting_for_analysis": waiting,
        "analyzed": analyzed,
        "daily_brief": daily,
        "next_step": next,
        "you_could_say": say,
    }))
}

/// Turn the daily brief on or off, run with the agent the user is talking to when it has a CLI.
pub fn set_daily_brief(db: &Db, args: &Value, client: &str) -> Result<String> {
    if args["off"].as_bool().unwrap_or(false) {
        schedule::uninstall()?;
        return Ok("The daily brief is off.".into());
    }
    let Some(agent) = agent_for(client) else {
        bail!(
            "the daily brief runs Claude Code or Codex in the background, and neither is installed. \
             Tell the user: install either one (signed in with their subscription), then ask again."
        );
    };
    let when = schedule::install(agent, schedule::When::parse(args["at"].as_str(), None)?)?;
    let digests = report::digest_dir(&db.lock().unwrap())?;
    Ok(format!(
        "The daily brief is on: new uploads are screened {when} with {} on this computer, even when this app is closed. \
         The digest goes to {} and a notification says what's worth their time. They can also ask here for the brief any time.",
        agent.program(),
        digests.display()
    ))
}

/// The setup panel (an MCP App view): the step to show and everything its checklist needs.
pub fn onboarding(db: &Db, args: &Value, ui: bool) -> Result<Value> {
    let conn = db.lock().unwrap();
    let profile = db::get_settings(&conn)?.profile;
    let channels = db::channels(&conn)?;
    drop(conn);
    let daily = schedule::scheduled();
    let suggested: Vec<(&str, &str)> = args["suggest"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| Some((s["id"].as_str()?, s["why"].as_str().unwrap_or(""))))
        .collect();
    let watching = channels.iter().filter(|c| c.enabled).count();
    let step = match args["step"].as_str() {
        Some(s @ ("about" | "follow" | "daily" | "done")) => s,
        _ if profile.trim().is_empty() => "about",
        _ if watching == 0 => "follow",
        _ if !daily => "daily",
        _ => "done",
    };
    // Suggested first, then what they already follow, then the rest by name.
    let mut list: Vec<Value> = channels
        .iter()
        .map(|c| {
            let why = suggested
                .iter()
                .find(|(id, _)| *id == c.id)
                .map(|(_, why)| *why);
            json!({ "id": c.id, "title": c.title, "podcast": c.podcast, "watching": c.enabled,
                    "suggested": why.is_some(), "why": why, "thumbnail": c.image, "subtitle": c.subtitle })
        })
        .collect();
    list.sort_by_key(|c| {
        (
            !c["suggested"].as_bool().unwrap_or(false),
            !c["watching"].as_bool().unwrap_or(false),
        )
    });
    let can_schedule = Agent::Claude.find().is_some() || Agent::Codex.find().is_some();
    // Apps without MCP Apps show no panel: the agent asks the step's question in chat instead.
    let ask = match step {
        "about" => {
            "Ask who they are and what they want to learn (2-3 sentences, with an example), and save it with set_profile."
        }
        "follow" => {
            "Offer to import their YouTube subscriptions (which browser?), follow podcasts by name, or name channels; after an import, suggest the ones that fit their profile and watch the ones they confirm."
        }
        "daily" => {
            "Ask whether they want a brief every morning, and at what time; turn it on with set_daily_brief."
        }
        _ => "Setup is done: offer to show their brief (show_videos), or a TL;DR of any link.",
    };
    if !ui {
        return Ok(json!({ "content": [{ "type": "text", "text": format!(
            "Setup step \"{step}\" ({watching} followed, {} known channels and shows). This app shows no panel, so do it in chat: {ask} \
             Then go on to the next step.", channels.len()) }] }));
    }
    let text = format!(
        "The user now sees the tx setup panel at the \"{step}\" step ({watching} followed, {} known channels and shows). \
         Don't repeat it; invite them in one sentence to fill it in. When they tell you their choice or ask to continue, \
         act on it, then call onboarding again to show the next step.{}",
        channels.len(),
        if step == "follow" && channels.len() > watching && suggested.is_empty() {
            " They have imported channels: next time pass `suggest` with the ones that fit their profile."
        } else {
            ""
        }
    );
    Ok(json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": { "view": "onboarding", "step": step, "profile": profile, "channels": list,
                               "daily_brief": daily, "can_schedule": can_schedule },
    }))
}

/// Commands the app lists in its menu, so users needn't know what to ask.
pub fn prompts() -> Value {
    json!([
        { "name": "get_started", "title": "Set up tx", "description": "Walk through what's left: your interests, what to follow, the daily brief" },
        { "name": "brief", "title": "Today's brief", "description": "Screen new videos and episodes and show what's worth your time" },
        { "name": "follow", "title": "Follow channels or podcasts", "description": "Add YouTube channels or podcasts by name",
          "arguments": [{ "name": "what", "description": "Channel names, @handles, podcast names or links", "required": true }] },
        { "name": "import", "title": "Import my subscriptions", "description": "Bring in your YouTube subscriptions and pick what to follow" },
        { "name": "tldr", "title": "TL;DR a video or episode", "description": "Summary, takeaways for you and whether to watch",
          "arguments": [{ "name": "link", "description": "Video or episode link", "required": true }] },
    ])
}

pub fn prompt(name: &str, args: &Value) -> Result<Value> {
    let arg = |k: &str| args[k].as_str().map(str::trim).filter(|s| !s.is_empty());
    let text = match name {
        "get_started" => "Use tx: check where I stand (status) and walk me through the next step.".to_string(),
        "brief" => "Use tx: check my channels and podcasts for new uploads, screen them for me, and show me the brief.".to_string(),
        "follow" => match arg("what") {
            Some(w) => format!("Use tx: follow {w}."),
            None => bail!("missing what"),
        },
        "import" => "Use tx: import my YouTube subscriptions and suggest which to follow for my profile.".to_string(),
        "tldr" => match arg("link") {
            Some(l) => format!("Use tx: TL;DR {l} for me."),
            None => bail!("missing link"),
        },
        other => bail!("unknown prompt {other}"),
    };
    Ok(json!({ "messages": [{ "role": "user", "content": { "type": "text", "text": text } }] }))
}

/// The agent to run in the background for the app the user is talking to: Codex from Codex,
/// Claude Code from Claude apps, else whichever is installed.
pub fn agent_for(client: &str) -> Option<Agent> {
    let client = client.to_ascii_lowercase();
    let preferred = if client.contains("codex") {
        Some(Agent::Codex)
    } else if client.contains("claude") {
        Some(Agent::Claude)
    } else {
        None
    };
    preferred
        .filter(|a| a.find().is_some())
        .or_else(|| Agent::detect().ok())
}

/// Most channels the setup panel may ask details for at once.
const MAX_DETAILS: usize = 30;

/// Avatars and "@handle · 4.3M subscribers" for channels the panel lists (imported ones only
/// have a title): from the database, else read from YouTube and stored.
pub async fn channel_details(db: &Db, args: &Value) -> Result<Value> {
    let ids: Vec<String> = args["ids"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|id| id.as_str().map(String::from))
        .collect();
    if ids.len() > MAX_DETAILS {
        bail!("ask for at most {MAX_DETAILS} channels at a time");
    }
    Ok(json!({ "channels": details::lookup(db, &ids).await? }))
}

/// YouTube channels and podcasts matching what the user types, for the setup panel's search:
/// what tx already knows first (instant), then YouTube's channel search and Apple's podcasts.
pub async fn search_sources(db: &Db, query: &str) -> Result<Value> {
    let q = query.trim();
    if q.is_empty() {
        bail!("type a channel or podcast name");
    }
    let channels = db::channels(&db.lock().unwrap())?;
    let watching = |id: &str| channels.iter().any(|c| c.id == id && c.enabled);
    let lower = q.to_lowercase();
    let mut results: Vec<Value> = channels
        .iter()
        .filter(|c| c.title.to_lowercase().contains(&lower))
        .take(4)
        .map(|c| {
            json!({ "kind": if c.podcast { "podcast" } else { "youtube" }, "ref": c.id, "title": c.title,
                    "subtitle": if c.enabled { "Following" } else { "In your subscriptions" },
                    "thumbnail": c.image, "watching": c.enabled })
        })
        .collect();
    // A pasted link or @handle is followed as it is.
    if q.starts_with('@') || q.contains("://") {
        results.push(json!({ "kind": if q.contains("youtu") || q.starts_with('@') { "youtube" } else { "podcast" },
                             "ref": q, "title": q, "subtitle": "Follow this link", "thumbnail": null, "watching": false }));
        return Ok(json!({ "results": results }));
    }
    let url = format!(
        "https://www.youtube.com/results?search_query={}&sp=EgIQAg%3D%3D",
        q.split_whitespace().collect::<Vec<_>>().join("+")
    );
    let youtube = tx_core::ytdlp::json(vec![
        "--flat-playlist".into(),
        "-J".into(),
        "--playlist-items".into(),
        "1:5".into(),
        url.into(),
    ]);
    let (youtube, shows) = tokio::join!(youtube, tx_core::podcasts::search(q));
    let known: Vec<String> = results
        .iter()
        .filter_map(|r| r["ref"].as_str().map(String::from))
        .collect();
    for e in youtube
        .ok()
        .and_then(|d| d["entries"].as_array().cloned())
        .unwrap_or_default()
    {
        let Some(id) = e["channel_id"]
            .as_str()
            .or(e["id"].as_str())
            .filter(|id| id.starts_with("UC"))
        else {
            continue;
        };
        if known.iter().any(|k| k == id) {
            continue;
        }
        let followers = e["channel_follower_count"]
            .as_f64()
            .map(details::subscribers_label);
        let handle = e["uploader_id"].as_str().filter(|h| h.starts_with('@'));
        // Avatars come as protocol-relative 88 px URLs; ask for a sharper size.
        let thumbnail = e["thumbnails"][0]["url"].as_str().map(|u| {
            format!("https:{}", u.trim_start_matches("https:")).replace("=s88-", "=s176-")
        });
        let subtitle = details::subtitle(handle, followers.as_deref()).unwrap_or_default();
        let title = e["title"].as_str().or(e["channel"].as_str()).unwrap_or(id);
        results.push(
            json!({ "kind": "youtube", "ref": id, "title": title, "subtitle": subtitle,
                             "thumbnail": thumbnail, "watching": watching(id) }),
        );
    }
    for s in shows.unwrap_or_default().into_iter().take(4) {
        let Some(feed) = s["feed_url"].as_str() else {
            continue;
        };
        let id = tx_core::podcasts::channel_id(feed);
        if known.contains(&id) {
            continue;
        }
        results.push(
            json!({ "kind": "podcast", "ref": feed, "title": s["title"], "subtitle": s["author"],
                             "thumbnail": s["artwork"], "watching": watching(&id) }),
        );
    }
    Ok(json!({ "results": results }))
}

/// Ask the user's own agent which channels fit their profile: (id, short reason) pairs.
pub fn suggest(
    agent: Agent,
    profile: &str,
    channels: &[db::Channel],
) -> Result<Vec<(String, String)>> {
    use anyhow::Context;
    use std::process::{Command, Stdio};
    let list: String = channels
        .iter()
        .map(|c| {
            format!(
                "{} | {} | {}\n",
                c.id,
                c.title,
                if c.podcast { "podcast" } else { "YouTube" }
            )
        })
        .collect();
    let prompt = format!(
        "A person describes what they want to learn. Below are the YouTube channels and podcasts they subscribe to. \
         Pick the ones whose new uploads fit that, usually 5 to 20. Reply with only lines like `<id> | <why, under 10 words>`, \
         nothing else.\n\nPerson: {profile}\n\nChannels (id | title | kind):\n{list}"
    );
    let program = agent.find().context("agent not installed")?;
    let text = match agent {
        Agent::Claude => {
            let out = Command::new(program)
                .args(["-p", &prompt])
                .stdin(Stdio::null())
                .output()?;
            String::from_utf8_lossy(&out.stdout).into_owned()
        }
        Agent::Codex => {
            let answer = tempfile::NamedTempFile::new()?;
            Command::new(program)
                .args([
                    "exec",
                    "--skip-git-repo-check",
                    "--sandbox",
                    "read-only",
                    "-o",
                ])
                .arg(answer.path())
                .arg(&prompt)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?;
            std::fs::read_to_string(answer.path())?
        }
    };
    let picks: Vec<(String, String)> = text
        .lines()
        .filter_map(|l| {
            // `id | why`, though agents sometimes echo the title in between: the reason is the last part.
            let (id, rest) = l.trim().trim_matches('`').split_once('|')?;
            let id = id.trim();
            let why = rest.rsplit('|').next().unwrap_or(rest).trim();
            channels
                .iter()
                .any(|c| c.id == id)
                .then(|| (id.to_string(), why.to_string()))
        })
        .collect();
    if picks.is_empty() {
        bail!("no usable answer");
    }
    Ok(picks)
}

/// The setup panel's "Let your AI pick": the user's agent picks from everything tx knows.
pub async fn suggest_channels(db: &Db, client: &str) -> Result<Value> {
    let (profile, channels) = {
        let conn = db.lock().unwrap();
        (db::get_settings(&conn)?.profile, db::channels(&conn)?)
    };
    if profile.trim().is_empty() {
        bail!("save what you want to learn first; the picks are for that");
    }
    if channels.is_empty() {
        bail!("import your subscriptions first, then your AI picks from them");
    }
    let Some(agent) = agent_for(client) else {
        bail!("picking needs Claude Code or Codex installed");
    };
    let picks = tokio::task::spawn_blocking(move || suggest(agent, &profile, &channels)).await??;
    Ok(
        json!({ "picks": picks.iter().map(|(id, why)| json!({ "id": id, "why": why })).collect::<Vec<_>>() }),
    )
}

/// The background screening started from the setup panel, so a second click doesn't start another.
static SCREENING: std::sync::Mutex<Option<std::process::Child>> = std::sync::Mutex::new(None);

/// "Show my first brief": check for new uploads, and if any are waiting, have the user's agent
/// analyze them in the background (`tx run`), the way the daily brief does. The panel polls
/// `status` and shows the brief when nothing is left waiting.
pub async fn screen_now(db: &Db, client: &str) -> Result<Value> {
    if let Err(e) = tx_core::feeds::poll(db).await {
        eprintln!("tx: checking for new uploads failed: {e:#}");
    }
    let waiting = db::pending(&db.lock().unwrap())?.len();
    let mut running = SCREENING.lock().unwrap();
    if let Some(child) = running.as_mut()
        && child.try_wait()?.is_none()
    {
        return Ok(
            json!({ "started": true, "waiting": waiting, "agent": null, "reason": "already screening" }),
        );
    }
    if waiting == 0 {
        return Ok(
            json!({ "started": false, "waiting": 0, "agent": null, "reason": "nothing new to screen" }),
        );
    }
    let Some(agent) = agent_for(client) else {
        return Ok(json!({ "started": false, "waiting": waiting, "agent": null,
                          "reason": "screening needs Claude Code or Codex installed" }));
    };
    let data = tx_core::paths::data_dir();
    let log = std::fs::File::create(data.join("screening.log"))?;
    let child = std::process::Command::new(std::env::current_exe()?)
        .args(["run", "--agent", agent.program()])
        .env("TX_DATA_DIR", &data)
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?;
    *running = Some(child);
    Ok(json!({ "started": true, "waiting": waiting, "agent": agent.program() }))
}
