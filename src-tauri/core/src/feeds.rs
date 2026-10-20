//! Channel discovery via yt-dlp; new-video detection via YouTube's public per-channel RSS
//! feeds (no login, no API quota).

use crate::db::{self, Db};
use crate::ytdlp;
use anyhow::{Context, Result, bail};
use chrono::{DateTime, FixedOffset};
use regex::Regex;
use rusqlite::params;
use std::sync::LazyLock;

pub struct Entry {
    pub id: String,
    pub title: String,
    pub link: String,
    pub published: String,
}

/// Add every channel the browser's logged-in YouTube account subscribes to (not watched yet).
pub async fn import_subscriptions(db: &Db, browser: &str) -> Result<usize> {
    let mut args = ytdlp::cookies_args(browser);
    args.extend([
        "--flat-playlist".into(),
        "-J".into(),
        "https://www.youtube.com/feed/channels".into(),
    ]);
    let info = ytdlp::json(args).await?;
    let found: Vec<(String, String)> = info["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let id = e["channel_id"].as_str().or(e["id"].as_str())?;
            let title = e["title"].as_str().or(e["channel"].as_str()).unwrap_or(id);
            id.starts_with("UC")
                .then(|| (id.to_string(), title.to_string()))
        })
        .collect();
    if found.is_empty() {
        bail!("no subscriptions found; is {browser} signed in to YouTube?");
    }
    let conn = db.lock().unwrap();
    for (id, title) in &found {
        db::upsert_channel(&conn, id, title)?;
    }
    Ok(found.len())
}

static CHANNEL_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^UC[\w-]{22}$").unwrap());

/// Watch a channel given @handle, channel id, channel URL, or any of its video URLs.
pub async fn add_channel(db: &Db, reference: &str) -> Result<String> {
    let r = reference.trim();
    let url = if r.starts_with('@') {
        format!("https://www.youtube.com/{r}")
    } else if CHANNEL_ID.is_match(r) {
        format!("https://www.youtube.com/channel/{r}")
    } else {
        r.to_string()
    };
    let info = ytdlp::json(vec![
        "--flat-playlist".into(),
        "--playlist-items".into(),
        "1".into(),
        "-J".into(),
        url.into(),
    ])
    .await?;
    let id = info["channel_id"]
        .as_str()
        .filter(|id| id.starts_with("UC"))
        .context("that is not a YouTube channel")?;
    let title = info["channel"]
        .as_str()
        .or(info["uploader"].as_str())
        .or(info["title"].as_str())
        .unwrap_or(id);
    let conn = db.lock().unwrap();
    db::upsert_channel(&conn, id, title)?;
    db::set_enabled(&conn, id, true)?;
    Ok(title.to_string())
}

pub fn parse_feed(xml: &str) -> Result<Vec<Entry>> {
    let doc = roxmltree::Document::parse(xml)?;
    Ok(doc
        .descendants()
        .filter(|n| n.has_tag_name("entry"))
        .map(|e| {
            let child = |name: &str| e.children().find(|c| c.tag_name().name() == name);
            let text = |name: &str| {
                child(name)
                    .and_then(|c| c.text())
                    .unwrap_or_default()
                    .to_string()
            };
            Entry {
                id: text("videoId"),
                title: text("title"),
                link: child("link")
                    .and_then(|l| l.attribute("href"))
                    .unwrap_or_default()
                    .to_string(),
                published: text("published"),
            }
        })
        .collect())
}

async fn feed(channel_id: &str) -> Result<Vec<Entry>> {
    let url = format!("https://www.youtube.com/feeds/videos.xml?channel_id={channel_id}");
    let xml = reqwest::Client::new()
        .get(url)
        .header("User-Agent", "Mozilla/5.0 tx")
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    parse_feed(&xml)
}

/// Status a newly seen video gets: analyzed automatically only if published after watching started.
pub fn initial_status(published: &str, watching_since: Option<&str>) -> &'static str {
    let parse = |s: &str| DateTime::<FixedOffset>::parse_from_rfc3339(s).ok();
    match (parse(published), watching_since.and_then(parse)) {
        (Some(p), Some(since)) if p >= since => "queued",
        _ => "skipped",
    }
}

/// Insert unseen videos of watched channels; returns how many were queued.
pub async fn poll(db: &Db) -> Result<usize> {
    let channels: Vec<(String, String, Option<String>)> = {
        let conn = db.lock().unwrap();
        conn.prepare("SELECT id, title, enabled_at FROM channels WHERE enabled = 1")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    let mut queued = 0;
    for (id, title, since) in channels {
        let entries = match feed(&id).await {
            Ok(e) => e,
            Err(e) => {
                log::warn!("feed failed for {title}: {e:#}");
                continue;
            }
        };
        let conn = db.lock().unwrap();
        for e in entries
            .iter()
            .filter(|e| !e.id.is_empty() && !e.link.contains("/shorts/"))
        {
            let status = initial_status(&e.published, since.as_deref());
            let n = conn.execute(
                "INSERT OR IGNORE INTO videos(id, channel_id, title, url, published, status) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                params![e.id, id, e.title, format!("https://www.youtube.com/watch?v={}", e.id), e.published, status],
            )?;
            queued += if status == "queued" { n } else { 0 };
        }
        conn.execute(
            "UPDATE channels SET checked_at = ?1 WHERE id = ?2",
            params![db::now(), id],
        )?;
    }
    Ok(queued)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns:yt="http://www.youtube.com/xml/schemas/2015" xmlns="http://www.w3.org/2005/Atom">
 <title>Theo</title>
 <entry>
  <yt:videoId>m-B3jIMMmUE</yt:videoId>
  <title>Your prompts kinda suck</title>
  <link rel="alternate" href="https://www.youtube.com/watch?v=m-B3jIMMmUE"/>
  <published>2026-10-18T23:57:04+00:00</published>
 </entry>
 <entry>
  <yt:videoId>short1</yt:videoId>
  <title>A short</title>
  <link rel="alternate" href="https://www.youtube.com/shorts/short1"/>
  <published>2026-10-19T10:00:00+00:00</published>
 </entry>
</feed>"#;

    #[test]
    fn parses_namespaced_atom_entries() {
        let entries = parse_feed(FEED).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "m-B3jIMMmUE");
        assert_eq!(entries[0].title, "Your prompts kinda suck");
        assert_eq!(entries[1].link, "https://www.youtube.com/shorts/short1");
        assert_eq!(entries[0].published, "2026-10-18T23:57:04+00:00");
    }

    #[test]
    fn only_uploads_after_watching_started_are_queued() {
        let since = Some("2026-10-19T00:00:00+00:00");
        assert_eq!(
            initial_status("2026-10-18T23:57:04+00:00", since),
            "skipped"
        );
        assert_eq!(initial_status("2026-10-19T00:00:00+00:00", since), "queued");
        assert_eq!(initial_status("2026-10-20T08:00:00+02:00", since), "queued");
        assert_eq!(initial_status("garbage", since), "skipped");
        assert_eq!(initial_status("2026-10-20T08:00:00+00:00", None), "skipped");
    }
}
