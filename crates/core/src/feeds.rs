//! Channel discovery via yt-dlp; new-video detection via YouTube's public per-channel RSS
//! feeds (no login, no API quota).

use crate::db::{self, Db};
use crate::ytdlp;
use anyhow::{Context, Result, bail};
use chrono::{DateTime, FixedOffset};
use regex::Regex;
use rusqlite::params;
use std::io::Read;
use std::path::Path;
use std::sync::LazyLock;

pub struct Entry {
    pub id: String,
    pub title: String,
    pub link: String,
    pub published: String,
}

/// How to get subscriptions without browser access; shown when reading the browser fails.
pub const TAKEOUT_HINT: &str = "Alternative that works on every computer: at https://takeout.google.com click \
\"Deselect all\", tick \"YouTube and YouTube Music\", click \"All YouTube data included\" and keep only \
\"subscriptions\", then export and download. Import the downloaded .zip (or the subscriptions.csv inside it) with \
import_subscriptions `file`.";

/// Add every channel the browser's logged-in YouTube account subscribes to (not watched yet).
/// Returns (channels found, of which new to tx).
pub async fn import_subscriptions(db: &Db, browser: &str) -> Result<(usize, usize)> {
    let mut args = ytdlp::cookies_args(browser);
    args.extend([
        "--flat-playlist".into(),
        "-J".into(),
        "https://www.youtube.com/feed/channels".into(),
    ]);
    let info = ytdlp::json(args).await.map_err(|e| {
        anyhow::anyhow!(
            "couldn't read YouTube subscriptions from {browser} ({e:#}); is it signed in to YouTube?\n\n{TAKEOUT_HINT}"
        )
    })?;
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
        bail!("no subscriptions found; is {browser} signed in to YouTube?\n\n{TAKEOUT_HINT}");
    }
    save_channels(db, &found)
}

/// Add the channels of a Google Takeout export: the downloaded .zip or its subscriptions.csv.
/// Returns (channels found, of which new to tx).
pub fn import_takeout(db: &Db, path: &Path) -> Result<(usize, usize)> {
    let is_zip = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"));
    let found = if is_zip {
        // Folder and file names are localized, so look for the CSV that lists channels.
        let mut zip = zip::ZipArchive::new(
            std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?,
        )?;
        let mut found = Vec::new();
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            if !entry.name()?.to_lowercase().ends_with(".csv") {
                continue;
            }
            let mut data = Vec::new();
            entry.read_to_end(&mut data)?;
            found = takeout_rows(&data);
            if !found.is_empty() {
                break;
            }
        }
        found
    } else {
        takeout_rows(&std::fs::read(path).with_context(|| format!("reading {}", path.display()))?)
    };
    if found.is_empty() {
        bail!(
            "no YouTube subscriptions in {}; it should be a Google Takeout export.\n\n{TAKEOUT_HINT}",
            path.display()
        );
    }
    save_channels(db, &found)
}

/// (channel id, title) rows of a Takeout subscriptions CSV: id, URL, title (headers are localized).
fn takeout_rows(data: &[u8]) -> Vec<(String, String)> {
    csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(data)
        .records()
        .filter_map(Result::ok)
        .filter_map(|r| {
            let id = r.get(0)?.trim();
            CHANNEL_ID.is_match(id).then(|| {
                let title = r
                    .get(2)
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .unwrap_or(id);
                (id.to_string(), title.to_string())
            })
        })
        .collect()
}

fn save_channels(db: &Db, found: &[(String, String)]) -> Result<(usize, usize)> {
    let conn = db.lock().unwrap();
    let known = db::channels(&conn)?.len();
    for (id, title) in found {
        db::upsert_channel(&conn, id, title)?;
    }
    Ok((found.len(), db::channels(&conn)?.len() - known))
}

static CHANNEL_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^UC[\w-]{22}$").unwrap());

/// Watch a channel given @handle, channel id, channel URL, or any of its video URLs.
pub async fn add_channel(db: &Db, reference: &str) -> Result<String> {
    let r = reference.trim();
    if CHANNEL_ID.is_match(r) {
        // Already imported: no lookup needed.
        let conn = db.lock().unwrap();
        if db::set_enabled(&conn, r, true)? {
            return Ok(db::channels(&conn)?
                .into_iter()
                .find(|c| c.id == r)
                .map_or_else(|| r.to_string(), |c| c.title));
        }
    }
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

    #[test]
    fn takeout_csv_rows_with_localized_headers_and_quoted_titles() {
        let csv = "ID kanálu,Adresa URL kanálu,Název kanálu\n\
                   UCsBjURrPoezykLs9EqgamOA,http://www.youtube.com/channel/UCsBjURrPoezykLs9EqgamOA,Fireship\n\
                   UCbRP3c757lWg9M-U7TyEkXA,http://www.youtube.com/channel/UCbRP3c757lWg9M-U7TyEkXA,\"Theo - t3.gg, live\"\n\
                   UCshortid,http://www.youtube.com/channel/UCshortid,Broken\n\
                   UCX6OQ3DkcsbYNE6H8uQQuVA,http://www.youtube.com/channel/UCX6OQ3DkcsbYNE6H8uQQuVA,\n\n";
        assert_eq!(
            takeout_rows(csv.as_bytes()),
            vec![
                (
                    "UCsBjURrPoezykLs9EqgamOA".to_string(),
                    "Fireship".to_string()
                ),
                (
                    "UCbRP3c757lWg9M-U7TyEkXA".to_string(),
                    "Theo - t3.gg, live".to_string()
                ),
                (
                    "UCX6OQ3DkcsbYNE6H8uQQuVA".to_string(),
                    "UCX6OQ3DkcsbYNE6H8uQQuVA".to_string()
                ),
            ]
        );
    }
}
