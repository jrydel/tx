//! Podcasts: find shows (Apple's podcast search), read their RSS feeds and OPML exports, and use
//! transcripts published in the feed. Episodes without one are transcribed from their audio like
//! any video, so a show is stored as a channel with a `feed_url` and an episode as a video whose
//! `url` is its audio file.

use crate::captions::{Seg, Transcript, parse_vtt};
use crate::db::{self, Db};
use anyhow::{Context, Result, bail};
use chrono::DateTime;
use regex::Regex;
use rusqlite::params;
use serde_json::Value;
use std::sync::LazyLock;

/// Newest episodes read from a feed per check; long-running shows list hundreds.
const EPISODES_PER_FEED: usize = 20;
const DESCRIPTION_CHARS: usize = 600;

#[derive(Debug, Clone, PartialEq)]
pub struct Show {
    pub title: String,
    pub image: Option<String>,
    pub episodes: Vec<Episode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Episode {
    /// Stable within the feed: the guid, else the audio URL.
    pub guid: String,
    pub title: String,
    /// The episode's web page, if the feed has one.
    pub link: Option<String>,
    pub audio_url: String,
    /// RFC 3339 when the feed's date parses, else as written.
    pub published: String,
    pub duration: Option<f64>,
    pub description: String,
    /// Best timestamped transcript the feed publishes: (URL, MIME type).
    pub transcript: Option<(String, String)>,
}

pub fn channel_id(feed_url: &str) -> String {
    format!(
        "pod-{}",
        &sha1_smol::Sha1::from(feed_url.trim()).digest().to_string()[..16]
    )
}

pub fn episode_id(feed_url: &str, guid: &str) -> String {
    format!(
        "ep-{}",
        &sha1_smol::Sha1::from(format!("{}|{guid}", feed_url.trim()))
            .digest()
            .to_string()[..16]
    )
}

static TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").unwrap());
static SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

/// Show notes as plain text, cut to a length that's enough to judge the episode.
fn plain(html: &str) -> String {
    let text = TAGS
        .replace_all(html, " ")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">");
    let text = SPACE.replace_all(&text, " ").trim().to_string();
    match text.char_indices().nth(DESCRIPTION_CHARS) {
        Some((i, _)) => format!("{}…", &text[..i]),
        None => text,
    }
}

/// "3723", "3723.5", "62:03" or "1:02:03" (itunes:duration).
fn duration(s: &str) -> Option<f64> {
    s.trim()
        .split(':')
        .try_fold(0.0, |acc, p| {
            Some(acc * 60.0 + p.trim().parse::<f64>().ok()?)
        })
        .filter(|d| *d > 0.0)
}

/// Timestamped formats first; plain text and HTML transcripts have no times, so local
/// transcription gives better takeaways.
fn transcript_rank(mime: &str) -> Option<u8> {
    let m = mime.to_ascii_lowercase();
    if m.contains("json") {
        Some(0)
    } else if m.contains("vtt") {
        Some(1)
    } else if m.contains("srt") || m.contains("subrip") {
        Some(2)
    } else {
        None
    }
}

pub fn parse_feed(xml: &str) -> Result<Show> {
    let doc = roxmltree::Document::parse(xml).context("not an RSS feed")?;
    let channel = doc
        .descendants()
        .find(|n| n.has_tag_name("channel"))
        .context("not a podcast feed (no <channel>)")?;
    let is_itunes = |n: &roxmltree::Node| {
        n.tag_name()
            .namespace()
            .is_some_and(|ns| ns.contains("itunes"))
    };
    // First non-empty child with that name; feeds mix in empty namesakes like <atom:link/>.
    let text = |parent: roxmltree::Node, name: &str, itunes: bool| {
        parent
            .children()
            .filter(|c| c.tag_name().name() == name && is_itunes(c) == itunes)
            .find_map(|c| {
                c.text()
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
            })
    };
    let image = channel
        .children()
        .find(|c| c.tag_name().name() == "image" && is_itunes(c))
        .and_then(|c| c.attribute("href"))
        .map(String::from)
        .or_else(|| {
            channel
                .children()
                .find(|c| c.tag_name().name() == "image" && !is_itunes(c))
                .and_then(|i| text(i, "url", false))
        });
    let mut episodes: Vec<Episode> = channel
        .children()
        .filter(|c| c.has_tag_name("item"))
        .filter_map(|item| {
            let audio_url = item
                .children()
                .find(|c| c.has_tag_name("enclosure"))
                .and_then(|e| e.attribute("url"))?
                .trim()
                .to_string();
            let raw_date = text(item, "pubDate", false).unwrap_or_default();
            let published = DateTime::parse_from_rfc2822(&raw_date)
                .or_else(|_| DateTime::parse_from_rfc3339(&raw_date))
                .map(|d| {
                    d.to_utc()
                        .to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
                })
                .unwrap_or(raw_date);
            let transcript = item
                .children()
                .filter(|c| c.tag_name().name() == "transcript")
                .filter_map(|c| {
                    let mime = c.attribute("type")?;
                    Some((
                        transcript_rank(mime)?,
                        c.attribute("url")?.to_string(),
                        mime.to_string(),
                    ))
                })
                .min_by_key(|(rank, ..)| *rank)
                .map(|(_, url, mime)| (url, mime));
            let description = item
                .children()
                .find(|c| c.tag_name().name() == "encoded" || c.has_tag_name("description"))
                .and_then(|c| c.text())
                .or_else(|| {
                    item.children()
                        .find(|c| c.tag_name().name() == "summary" && is_itunes(c))
                        .and_then(|c| c.text())
                })
                .map(plain)
                .unwrap_or_default();
            Some(Episode {
                guid: text(item, "guid", false).unwrap_or_else(|| audio_url.clone()),
                title: text(item, "title", false).unwrap_or_else(|| audio_url.clone()),
                link: text(item, "link", false),
                published,
                duration: text(item, "duration", true).as_deref().and_then(duration),
                description,
                transcript,
                audio_url,
            })
        })
        .collect();
    // Newest first; RFC 3339 strings in UTC sort by time.
    episodes.sort_by(|a, b| b.published.cmp(&a.published));
    episodes.truncate(EPISODES_PER_FEED);
    Ok(Show {
        title: text(channel, "title", false).context("podcast feed has no title")?,
        image,
        episodes,
    })
}

/// (feed URL, title) of every podcast in an OPML export from a podcast app.
pub fn parse_opml(xml: &str) -> Result<Vec<(String, String)>> {
    let doc = roxmltree::Document::parse(xml).context("not an OPML file")?;
    Ok(doc
        .descendants()
        .filter(|n| n.has_tag_name("outline"))
        .filter_map(|o| {
            let url = o.attribute("xmlUrl")?.trim().to_string();
            let title = o
                .attribute("text")
                .or(o.attribute("title"))
                .unwrap_or(&url)
                .trim()
                .to_string();
            Some((url, title))
        })
        .collect())
}

/// A transcript published with the episode: Podcasting 2.0 JSON, WebVTT or SRT.
pub fn parse_transcript(body: &str, mime: &str) -> Vec<Seg> {
    if !mime.to_ascii_lowercase().contains("json") {
        return parse_vtt(body);
    }
    let Ok(data) = serde_json::from_str::<Value>(body) else {
        return vec![];
    };
    data["segments"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let text = s["body"].as_str()?.trim();
            // Hosts write times as numbers or as strings ("1.76").
            let start = s["startTime"]
                .as_f64()
                .or_else(|| s["startTime"].as_str()?.trim().parse().ok())?;
            (!text.is_empty()).then(|| Seg {
                start,
                // Diarized transcripts name the speaker; keep it, it matters in conversations.
                text: match s["speaker"]
                    .as_str()
                    .map(str::trim)
                    .filter(|sp| !sp.is_empty())
                {
                    Some(sp) => format!("{sp}: {text}"),
                    None => text.to_string(),
                },
            })
        })
        .collect()
}

async fn get(url: &str) -> Result<String> {
    Ok(reqwest::Client::new()
        .get(url)
        .header("User-Agent", "Mozilla/5.0 tx")
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?)
}

pub async fn fetch_feed(feed_url: &str) -> Result<Show> {
    parse_feed(
        &get(feed_url)
            .await
            .with_context(|| format!("fetching {feed_url}"))?,
    )
}

/// The episode's published transcript, if the feed has one and it has timestamps.
pub async fn published_transcript(v: &db::Video) -> Result<Option<Transcript>> {
    let (Some(url), Some(mime)) = (&v.transcript_url, &v.transcript_type) else {
        return Ok(None);
    };
    let segs = parse_transcript(&get(url).await?, mime);
    Ok((!segs.is_empty()).then(|| Transcript {
        title: v.title.clone(),
        url: v.url.clone(),
        uploader: Some(v.channel.clone()),
        duration: v.duration,
        source: format!("published transcript ({mime})"),
        segs,
    }))
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Podcasts matching `query` in Apple's podcast directory (no account or key needed).
pub async fn search(query: &str) -> Result<Vec<Value>> {
    let url = format!(
        "https://itunes.apple.com/search?media=podcast&entity=podcast&limit=8&term={}",
        urlencode(query.trim())
    );
    let data: Value = serde_json::from_str(&get(&url).await.context("searching Apple Podcasts")?)?;
    Ok(data["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            Some(serde_json::json!({
                "title": r["collectionName"].as_str()?,
                "author": r["artistName"],
                "feed_url": r["feedUrl"].as_str()?,
                "episodes": r["trackCount"],
                "genre": r["primaryGenreName"],
                "artwork": r["artworkUrl100"],
            }))
        })
        .collect())
}

/// Store a show (not watched) and return its channel id.
pub fn save_show(
    conn: &rusqlite::Connection,
    feed_url: &str,
    title: &str,
    image: Option<&str>,
) -> Result<String> {
    let id = channel_id(feed_url);
    db::upsert_channel(conn, &id, title)?;
    conn.execute(
        "UPDATE channels SET feed_url = ?1, image = COALESCE(?2, image) WHERE id = ?3",
        params![feed_url.trim(), image, id],
    )?;
    Ok(id)
}

/// Add the shows of an OPML export (not watched yet). Returns (shows found, of which new to tx).
pub fn import_opml(db: &Db, path: &std::path::Path) -> Result<(usize, usize)> {
    let xml =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let shows = parse_opml(&xml)?;
    if shows.is_empty() {
        bail!(
            "no podcasts in {}; export an OPML file from your podcast app",
            path.display()
        );
    }
    let conn = db.lock().unwrap();
    let known = db::channels(&conn)?.len();
    for (url, title) in &shows {
        save_show(&conn, url, title, None)?;
    }
    Ok((shows.len(), db::channels(&conn)?.len() - known))
}

/// Watch a podcast by its feed URL; returns the show's title.
pub async fn add(db: &Db, feed_url: &str) -> Result<String> {
    let show = fetch_feed(feed_url).await?;
    if show.episodes.is_empty() {
        bail!("{feed_url} has no episodes with audio");
    }
    let conn = db.lock().unwrap();
    let id = save_show(&conn, feed_url, &show.title, show.image.as_deref())?;
    db::set_enabled(&conn, &id, true)?;
    Ok(show.title)
}

/// Insert unseen episodes of a watched show; returns how many were queued.
pub async fn poll_show(db: &Db, id: &str, feed_url: &str, since: Option<&str>) -> Result<usize> {
    let show = fetch_feed(feed_url).await?;
    let conn = db.lock().unwrap();
    let mut queued = 0;
    for e in &show.episodes {
        let status = crate::feeds::initial_status(&e.published, since);
        let (transcript_url, transcript_type) = e.transcript.clone().unzip();
        let n = conn.execute(
            "INSERT OR IGNORE INTO videos(id, channel_id, title, url, published, status, duration, link, description, transcript_url, transcript_type) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                episode_id(feed_url, &e.guid), id, e.title, e.audio_url, e.published, status,
                e.duration, e.link, e.description, transcript_url, transcript_type
            ],
        )?;
        queued += if status == "queued" { n } else { 0 };
    }
    conn.execute(
        "UPDATE channels SET checked_at = ?1, image = COALESCE(image, ?2) WHERE id = ?3",
        params![db::now(), show.image, id],
    )?;
    Ok(queued)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd" xmlns:podcast="https://podcastindex.org/namespace/1.0" xmlns:content="http://purl.org/rss/1.0/modules/content/">
<channel>
  <title>Latent Space</title>
  <image><url>https://example.com/small.jpg</url></image>
  <itunes:image href="https://example.com/art.jpg"/>
  <item>
    <title>Older episode</title>
    <pubDate>Mon, 05 Oct 2026 09:00:00 GMT</pubDate>
    <enclosure url="https://cdn.example.com/old.mp3" type="audio/mpeg"/>
    <itunes:duration>62:03</itunes:duration>
    <description>Plain notes</description>
  </item>
  <item>
    <title>Agents in production</title>
    <guid isPermaLink="false">abc-123</guid>
    <link>https://example.com/ep/agents</link>
    <pubDate>Thu, 22 Oct 2026 14:05:00 +0200</pubDate>
    <enclosure url="https://cdn.example.com/agents.mp3" length="1" type="audio/mpeg"/>
    <itunes:duration>1:02:03</itunes:duration>
    <content:encoded><![CDATA[<p>We talk <b>evals</b> &amp; tracing.</p>]]></content:encoded>
    <podcast:transcript url="https://example.com/t.html" type="text/html"/>
    <podcast:transcript url="https://example.com/t.srt" type="application/x-subrip"/>
    <podcast:transcript url="https://example.com/t.vtt" type="text/vtt"/>
  </item>
  <item><title>Trailer without audio</title></item>
</channel>
</rss>"#;

    #[test]
    fn feed_episodes_newest_first_with_best_transcript_and_parsed_fields() {
        let show = parse_feed(FEED).unwrap();
        assert_eq!(show.title, "Latent Space");
        assert_eq!(
            show.image.as_deref(),
            Some("https://example.com/art.jpg"),
            "itunes:image wins"
        );
        assert_eq!(show.episodes.len(), 2, "items without audio are left out");
        let e = &show.episodes[0];
        assert_eq!(e.title, "Agents in production");
        assert_eq!(
            e.published, "2026-10-22T12:05:00+00:00",
            "RFC 2822 date normalized to UTC"
        );
        assert_eq!(e.duration, Some(3723.0));
        assert_eq!(e.description, "We talk evals & tracing.");
        assert_eq!(e.guid, "abc-123");
        assert_eq!(e.link.as_deref(), Some("https://example.com/ep/agents"));
        assert_eq!(
            e.transcript,
            Some((
                "https://example.com/t.vtt".to_string(),
                "text/vtt".to_string()
            )),
            "timestamped VTT beats SRT and HTML"
        );
        let old = &show.episodes[1];
        assert_eq!(
            (old.duration, old.guid.as_str()),
            (Some(3723.0), "https://cdn.example.com/old.mp3")
        );
        assert_eq!(old.transcript, None);
    }

    #[test]
    fn opml_and_json_transcripts() {
        let opml = r#"<opml version="2.0"><body><outline text="feeds">
            <outline type="rss" text="Latent Space" xmlUrl="https://example.com/feed.xml"/>
            <outline type="rss" title="Dwarkesh" xmlUrl=" https://example.com/d.xml "/>
            <outline text="folder without a feed"/>
        </outline></body></opml>"#;
        assert_eq!(
            parse_opml(opml).unwrap(),
            vec![
                (
                    "https://example.com/feed.xml".to_string(),
                    "Latent Space".to_string()
                ),
                (
                    "https://example.com/d.xml".to_string(),
                    "Dwarkesh".to_string()
                ),
            ]
        );
        let json = r#"{"version":"1.0.0","segments":[
            {"speaker":"Alessio","startTime":0.5,"endTime":3.1,"body":"Welcome back."},
            {"startTime":3.2,"endTime":4.0,"body":"  "},
            {"startTime":"4.20","body":"Today: evals."}]}"#;
        assert_eq!(
            parse_transcript(json, "application/json"),
            vec![
                Seg {
                    start: 0.5,
                    text: "Alessio: Welcome back.".into()
                },
                Seg {
                    start: 4.2,
                    text: "Today: evals.".into()
                },
            ]
        );
    }
}
