//! SQLite store for watched channels, discovered videos, and settings.
//! Schema is shared with tx 0.1 (Python), so existing databases keep working.

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS channels (
    id         TEXT PRIMARY KEY,           -- YouTube channel id (UC…) or pod-<hash of feed_url>
    title      TEXT NOT NULL,
    enabled    INTEGER NOT NULL DEFAULT 0,
    enabled_at TEXT,                       -- videos published before this are not auto-analyzed
    checked_at TEXT,
    feed_url   TEXT,                       -- podcasts: the show's RSS feed
    image      TEXT,                       -- avatar or podcast artwork URL
    subtitle   TEXT                        -- @handle · 4.3M subscribers, or a podcast's author
);
CREATE TABLE IF NOT EXISTS videos (
    id          TEXT PRIMARY KEY,          -- YouTube video id, or ep-<hash> for a podcast episode
    channel_id  TEXT NOT NULL REFERENCES channels(id),
    title       TEXT NOT NULL,
    url         TEXT NOT NULL,             -- the video, or the episode's audio file
    published   TEXT NOT NULL,
    status      TEXT NOT NULL,             -- skipped | queued | working | done | error | passed
    attempts    INTEGER NOT NULL DEFAULT 0,
    relevance   INTEGER,                   -- 0-5 against the user's profile
    verdict     TEXT,                      -- one-line reason for the relevance score
    analysis    TEXT,                      -- markdown
    error       TEXT,                      -- why it failed, or why the agent passed on it
    analyzed_at TEXT,
    duration    REAL,                      -- seconds
    link        TEXT,                      -- podcasts: the episode's web page
    description TEXT,                      -- podcasts: show notes, to judge before transcribing
    transcript_url  TEXT,                  -- podcasts: transcript published in the feed
    transcript_type TEXT
);
CREATE INDEX IF NOT EXISTS videos_published ON videos(published DESC);
CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
";

pub const MAX_ATTEMPTS: i64 = 3;

/// The engine's one connection. Lock only around synchronous work; never across an `.await`.
pub type Db = std::sync::Mutex<Connection>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub profile: String,
    /// Notify (and count as worth your time in digests) when relevance >= this.
    pub notify_min: u8,
    /// Whisper model for videos without captions.
    pub model: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            profile: String::new(),
            notify_min: 3,
            model: "small".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub url: String,
    pub channel_id: String,
    pub channel: String,
    pub published: String,
    pub status: String,
    pub relevance: Option<i64>,
    pub verdict: Option<String>,
    pub analysis: Option<String>,
    pub error: Option<String>,
    pub duration: Option<f64>,
    pub analyzed_at: Option<String>,
    /// A podcast episode (its `url` is the audio file) rather than a YouTube video.
    pub podcast: bool,
    pub link: Option<String>,
    pub description: Option<String>,
    pub transcript_url: Option<String>,
    pub transcript_type: Option<String>,
    /// The show's artwork (podcasts).
    pub image: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Channel {
    pub id: String,
    pub title: String,
    pub enabled: bool,
    pub checked_at: Option<String>,
    pub analyzed: i64,
    pub podcast: bool,
    pub image: Option<String>,
    pub subtitle: Option<String>,
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

pub fn connect() -> Result<Connection> {
    let conn = Connection::open(crate::paths::data_dir().join("tx.db"))?;
    conn.busy_timeout(std::time::Duration::from_secs(30))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.execute_batch(SCHEMA)?;
    // Columns added after the first release of each table.
    for (table, column, kind) in [
        ("videos", "duration", "REAL"),
        ("videos", "link", "TEXT"),
        ("videos", "description", "TEXT"),
        ("videos", "transcript_url", "TEXT"),
        ("videos", "transcript_type", "TEXT"),
        ("channels", "feed_url", "TEXT"),
        ("channels", "image", "TEXT"),
        ("channels", "subtitle", "TEXT"),
    ] {
        let exists = conn
            .prepare(&format!(
                "SELECT 1 FROM pragma_table_info('{table}') WHERE name = ?1"
            ))?
            .exists([column])?;
        if !exists {
            conn.execute(
                &format!("ALTER TABLE {table} ADD COLUMN {column} {kind}"),
                [],
            )?;
        }
    }
    Ok(conn)
}

pub fn get_settings(conn: &Connection) -> Result<Settings> {
    let rows: BTreeMap<String, String> = conn
        .prepare("SELECT key, value FROM settings")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let d = Settings::default();
    Ok(Settings {
        profile: rows.get("profile").cloned().unwrap_or(d.profile),
        notify_min: rows
            .get("notify_min")
            .and_then(|v| v.parse().ok())
            .unwrap_or(d.notify_min),
        model: rows.get("model").cloned().unwrap_or(d.model),
    })
}

pub fn save_settings(conn: &Connection, s: &Settings) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO settings(key, value) VALUES(?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )?;
    for (k, v) in [
        ("profile", s.profile.clone()),
        ("notify_min", s.notify_min.to_string()),
        ("model", s.model.clone()),
    ] {
        stmt.execute(params![k, v])?;
    }
    Ok(())
}

/// Free-form settings not shown in the app's Settings form (digest folder, last digest time…).
pub fn get_value(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )
        .optional()?)
}

pub fn set_value(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO settings(key, value) VALUES(?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// What tx learned about the user from their coding-agent sessions (`tx learn`).
#[derive(Debug, Clone, PartialEq)]
pub struct Learned {
    pub text: String,
    /// RFC 3339.
    pub at: String,
    /// How many days of sessions it was learned from, reused when it's refreshed.
    pub days: u32,
}

pub fn learned(conn: &Connection) -> Result<Option<Learned>> {
    let Some(text) = get_value(conn, "learned_profile")? else {
        return Ok(None);
    };
    Ok(Some(Learned {
        text,
        at: get_value(conn, "learned_at")?.unwrap_or_default(),
        days: get_value(conn, "learned_days")?
            .and_then(|d| d.parse().ok())
            .unwrap_or(crate::learn::DEFAULT_DAYS),
    }))
}

pub fn save_learned(conn: &Connection, text: &str, days: u32) -> Result<()> {
    set_value(conn, "learned_profile", text)?;
    set_value(conn, "learned_at", &now())?;
    set_value(conn, "learned_days", &days.to_string())
}

pub fn forget_learned(conn: &Connection) -> Result<()> {
    conn.execute(
        "DELETE FROM settings WHERE key IN ('learned_profile', 'learned_at', 'learned_days')",
        [],
    )?;
    Ok(())
}

/// Whether a learned profile is older than `days` (or its date is unreadable), so `tx run` should
/// refresh it. Nothing learned means nothing to refresh: learning is opt-in through `tx learn`.
pub fn learned_is_stale(learned: &Learned, days: i64) -> bool {
    chrono::DateTime::parse_from_rfc3339(&learned.at)
        .ok()
        .is_none_or(|t| chrono::Utc::now().signed_duration_since(t).num_days() >= days)
}

/// Who every analysis is for: the user's own description, then what tx learned from their
/// sessions. Empty when there's neither.
pub fn profile(conn: &Connection) -> Result<String> {
    let own = get_settings(conn)?.profile;
    let own = own.trim();
    Ok(match learned(conn)? {
        Some(l) => {
            let date = l.at.get(..10).unwrap_or(&l.at);
            let learned = format!(
                "From your coding-agent sessions (updated {date}):\n{}",
                l.text.trim()
            );
            if own.is_empty() {
                learned
            } else {
                format!("{own}\n\n{learned}")
            }
        }
        None => own.to_string(),
    })
}

/// Store an analysis written by the user's own agent (MCP flow). False if the video is unknown.
pub fn save_analysis(
    conn: &Connection,
    id: &str,
    relevance: i64,
    verdict: &str,
    analysis: &str,
    duration: Option<f64>,
) -> Result<bool> {
    let n = conn.execute(
        "UPDATE videos SET status = 'done', relevance = ?1, verdict = ?2, analysis = ?3, \
         duration = COALESCE(?4, duration), error = NULL, analyzed_at = ?5 WHERE id = ?6",
        params![
            relevance.clamp(0, 5),
            verdict,
            analysis,
            duration,
            now(),
            id
        ],
    )?;
    Ok(n == 1)
}

/// The agent couldn't analyze a video (no transcript, etc.); retried up to MAX_ATTEMPTS times.
pub fn mark_failed(conn: &Connection, id: &str, reason: &str) -> Result<bool> {
    let n = conn.execute(
        "UPDATE videos SET status = 'error', error = ?1, attempts = attempts + 1 WHERE id = ?2",
        params![reason, id],
    )?;
    Ok(n == 1)
}

/// Analyzed videos, newest analysis first, optionally only those analyzed after `since`.
pub fn analyzed(
    conn: &Connection,
    since: Option<&str>,
    min_relevance: i64,
    limit: i64,
) -> Result<Vec<Video>> {
    let sql = format!(
        "SELECT {VIDEO_COLS} FROM videos v JOIN channels c ON c.id = v.channel_id \
         WHERE v.status = 'done' AND COALESCE(v.relevance, 0) >= ?1 AND (?2 IS NULL OR v.analyzed_at > ?2) \
         ORDER BY v.analyzed_at DESC LIMIT ?3"
    );
    let rows = conn
        .prepare(&sql)?
        .query_map(params![min_relevance, since, limit], video_row)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Remember what a channel's page or yt-dlp told about it; what wasn't found keeps its old value.
pub fn save_details(
    conn: &Connection,
    id: &str,
    image: Option<&str>,
    subtitle: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE channels SET image = COALESCE(?1, image), subtitle = COALESCE(?2, subtitle) WHERE id = ?3",
        params![image, subtitle, id],
    )?;
    Ok(())
}

pub fn upsert_channel(conn: &Connection, id: &str, title: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO channels(id, title) VALUES(?1, ?2) ON CONFLICT(id) DO UPDATE SET title = excluded.title",
        params![id, title],
    )?;
    Ok(())
}

/// Keeps the first `enabled_at`, so re-enabling does not re-skip videos already seen.
pub fn set_enabled(conn: &Connection, id: &str, on: bool) -> Result<bool> {
    let n = if on {
        conn.execute(
            "UPDATE channels SET enabled = 1, enabled_at = COALESCE(enabled_at, ?1) WHERE id = ?2",
            params![now(), id],
        )?
    } else {
        conn.execute("UPDATE channels SET enabled = 0 WHERE id = ?1", params![id])?
    };
    Ok(n == 1)
}

/// The agent judged a podcast episode off-profile from its title and notes, before spending
/// minutes transcribing it. Unlike a failure it is not retried.
pub fn mark_passed(conn: &Connection, id: &str, reason: &str) -> Result<bool> {
    let n = conn.execute(
        "UPDATE videos SET status = 'passed', error = ?1 WHERE id = ?2",
        params![reason, id],
    )?;
    Ok(n == 1)
}

const VIDEO_COLS: &str = "v.id, v.title, v.url, v.channel_id, c.title, v.published, v.status, v.relevance, \
    v.verdict, v.analysis, v.error, v.duration, v.analyzed_at, c.feed_url IS NOT NULL, v.link, v.description, \
    v.transcript_url, v.transcript_type, c.image";

fn video_row(r: &rusqlite::Row) -> rusqlite::Result<Video> {
    Ok(Video {
        id: r.get(0)?,
        title: r.get(1)?,
        url: r.get(2)?,
        channel_id: r.get(3)?,
        channel: r.get(4)?,
        published: r.get(5)?,
        status: r.get(6)?,
        relevance: r.get(7)?,
        verdict: r.get(8)?,
        analysis: r.get(9)?,
        error: r.get(10)?,
        duration: r.get(11)?,
        analyzed_at: r.get(12)?,
        podcast: r.get(13)?,
        link: r.get(14)?,
        description: r.get(15)?,
        transcript_url: r.get(16)?,
        transcript_type: r.get(17)?,
        image: r.get(18)?,
    })
}

pub fn video(conn: &Connection, id: &str) -> Result<Option<Video>> {
    let sql = format!(
        "SELECT {VIDEO_COLS} FROM videos v JOIN channels c ON c.id = v.channel_id WHERE v.id = ?1"
    );
    Ok(conn.query_row(&sql, params![id], video_row).optional()?)
}

/// For a first brief: queue the newest `per_channel` uploads of each followed channel that has
/// nothing analyzed or waiting yet. Normally only uploads after following are screened, so right
/// after setup there'd be nothing to show. Returns how many were queued.
pub fn queue_first_brief(conn: &Connection, per_channel: i64) -> Result<usize> {
    Ok(conn.execute(
        "UPDATE videos SET status = 'queued' WHERE id IN (
           SELECT id FROM (
             SELECT v.id, ROW_NUMBER() OVER (PARTITION BY v.channel_id ORDER BY v.published DESC) AS n
             FROM videos v JOIN channels c ON c.id = v.channel_id
             WHERE c.enabled = 1 AND v.status = 'skipped'
               AND NOT EXISTS (SELECT 1 FROM videos o WHERE o.channel_id = v.channel_id
                               AND o.status IN ('queued', 'working', 'done', 'error'))
           ) WHERE n <= ?1)",
        params![per_channel],
    )?)
}

/// Videos waiting for analysis, oldest first. `working` rows come from older tx versions'
/// background worker, interrupted mid-analysis.
pub fn pending(conn: &Connection) -> Result<Vec<Video>> {
    let sql = format!(
        "SELECT {VIDEO_COLS} FROM videos v JOIN channels c ON c.id = v.channel_id \
         WHERE v.status IN ('queued', 'working') OR (v.status = 'error' AND v.attempts < ?1) ORDER BY v.published"
    );
    let rows = conn
        .prepare(&sql)?
        .query_map(params![MAX_ATTEMPTS], video_row)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Latest uploads tx knows, any status, newest first; `channel` matches a channel id or part of
/// its name.
pub fn recent(conn: &Connection, channel: Option<&str>, limit: i64) -> Result<Vec<Video>> {
    let sql = format!(
        "SELECT {VIDEO_COLS} FROM videos v JOIN channels c ON c.id = v.channel_id \
         WHERE ?1 IS NULL OR c.id = ?1 OR c.title LIKE '%' || ?1 || '%' ORDER BY v.published DESC LIMIT ?2"
    );
    let rows = conn
        .prepare(&sql)?
        .query_map(params![channel, limit], video_row)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Videos of watched channels published in the last `days` days (not those from before following,
/// `skipped`), newest first; `done_only` keeps the analyzed ones.
pub fn week(conn: &Connection, days: i64, done_only: bool, limit: i64) -> Result<Vec<Video>> {
    let sql = format!(
        "SELECT {VIDEO_COLS} FROM videos v JOIN channels c ON c.id = v.channel_id \
         WHERE c.enabled = 1 AND v.status != 'skipped' AND (?2 = 0 OR v.status = 'done') \
         AND datetime(v.published) >= datetime('now', ?1) ORDER BY v.published DESC LIMIT ?3"
    );
    let rows = conn
        .prepare(&sql)?
        .query_map(
            params![format!("-{days} days"), done_only, limit],
            video_row,
        )?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn channels(conn: &Connection) -> Result<Vec<Channel>> {
    let rows = conn
        .prepare(
            "SELECT c.id, c.title, c.enabled, c.checked_at, \
             (SELECT COUNT(*) FROM videos v WHERE v.channel_id = c.id AND v.status = 'done'), \
             c.feed_url IS NOT NULL, c.image, c.subtitle \
             FROM channels c ORDER BY c.title COLLATE NOCASE",
        )?
        .query_map([], |r| {
            Ok(Channel {
                id: r.get(0)?,
                title: r.get(1)?,
                enabled: r.get::<_, i64>(2)? == 1,
                checked_at: r.get(3)?,
                analyzed: r.get(4)?,
                podcast: r.get(5)?,
                image: r.get(6)?,
                subtitle: r.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

#[cfg(test)]
pub fn memory() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(SCHEMA).unwrap();
    conn
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(conn: &Connection) {
        upsert_channel(conn, "UC1", "One").unwrap();
        for (id, status, rel) in [
            ("a", "done", Some(4)),
            ("b", "done", Some(1)),
            ("c", "queued", None),
            ("d", "skipped", None),
        ] {
            conn.execute(
                "INSERT INTO videos(id, channel_id, title, url, published, status, relevance) VALUES(?1, 'UC1', ?1, 'u', ?1, ?2, ?3)",
                params![id, status, rel],
            )
            .unwrap();
        }
    }

    fn ids(v: Vec<Video>) -> Vec<String> {
        let mut ids: Vec<String> = v.into_iter().map(|v| v.id).collect();
        ids.sort();
        ids
    }

    #[test]
    fn details_fill_in_and_keep_what_a_later_lookup_lacks() {
        let conn = memory();
        upsert_channel(&conn, "UC1", "One").unwrap();
        save_details(&conn, "UC1", Some("https://yt3.ggpht.com/a"), None).unwrap();
        save_details(&conn, "UC1", None, Some("@one · 5 subscribers")).unwrap();
        save_details(&conn, "UC1", None, None).unwrap();
        let c = &channels(&conn).unwrap()[0];
        assert_eq!(c.image.as_deref(), Some("https://yt3.ggpht.com/a"));
        assert_eq!(c.subtitle.as_deref(), Some("@one · 5 subscribers"));
    }

    #[test]
    fn failed_videos_retry_until_max_attempts_and_analyses_leave_the_queue() {
        let conn = memory();
        seed(&conn);
        assert_eq!(ids(pending(&conn).unwrap()), ["c"]);
        for _ in 0..MAX_ATTEMPTS - 1 {
            assert!(mark_failed(&conn, "c", "no transcript").unwrap());
            assert_eq!(ids(pending(&conn).unwrap()), ["c"], "still retried");
        }
        assert!(mark_failed(&conn, "c", "no transcript").unwrap());
        assert!(
            pending(&conn).unwrap().is_empty(),
            "gave up after MAX_ATTEMPTS"
        );

        assert!(save_analysis(&conn, "c", 9, "great", "## TL;DR\nx", Some(60.0)).unwrap());
        let done = analyzed(&conn, None, 5, 10).unwrap();
        assert_eq!(ids(done.clone()), ["c"], "score clamped to 5");
        assert!(done[0].error.is_none(), "error cleared");
        assert!(!save_analysis(&conn, "nope", 3, "", "", None).unwrap());
    }

    #[test]
    fn analyzed_filters_by_score_and_time() {
        let conn = memory();
        seed(&conn);
        conn.execute("UPDATE videos SET analyzed_at = '2026-01-0' || id", [])
            .unwrap();
        assert_eq!(ids(analyzed(&conn, None, 0, 10).unwrap()), ["a", "b"]);
        assert_eq!(ids(analyzed(&conn, None, 3, 10).unwrap()), ["a"]);
        assert!(
            analyzed(&conn, Some("2026-01-0z"), 0, 10)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn learned_profile_follows_the_users_own_words_and_can_be_forgotten() {
        let conn = memory();
        assert_eq!(profile(&conn).unwrap(), "");
        save_learned(&conn, "Working on now:\n- tx", 14).unwrap();
        let only = profile(&conn).unwrap();
        assert!(only.starts_with("From your coding-agent sessions (updated 20"));
        assert!(only.ends_with("Working on now:\n- tx"));
        let mut s = get_settings(&conn).unwrap();
        s.profile = "Founder, Prague. Skip crypto.".into();
        save_settings(&conn, &s).unwrap();
        let both = profile(&conn).unwrap();
        assert!(
            both.starts_with("Founder, Prague. Skip crypto.\n\nFrom your coding-agent sessions")
        );
        assert_eq!(learned(&conn).unwrap().unwrap().days, 14);
        forget_learned(&conn).unwrap();
        assert_eq!(profile(&conn).unwrap(), "Founder, Prague. Skip crypto.");
        assert!(learned(&conn).unwrap().is_none());
    }

    #[test]
    fn learned_profile_is_stale_after_the_refresh_window() {
        let at = |days_ago: i64| Learned {
            text: "x".into(),
            at: (chrono::Utc::now() - chrono::Duration::days(days_ago)).to_rfc3339(),
            days: 30,
        };
        assert!(!learned_is_stale(&at(6), 7));
        assert!(learned_is_stale(&at(7), 7));
        assert!(learned_is_stale(
            &Learned {
                at: "not a date".into(),
                ..at(0)
            },
            7
        ));
    }

    #[test]
    fn reenabling_keeps_original_enabled_at() {
        let conn = memory();
        upsert_channel(&conn, "UC1", "One").unwrap();
        conn.execute(
            "UPDATE channels SET enabled_at = '2020-01-01T00:00:00+00:00' WHERE id = 'UC1'",
            [],
        )
        .unwrap();
        set_enabled(&conn, "UC1", false).unwrap();
        set_enabled(&conn, "UC1", true).unwrap();
        let at: String = conn
            .query_row(
                "SELECT enabled_at FROM channels WHERE id = 'UC1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(at, "2020-01-01T00:00:00+00:00");
    }

    #[test]
    fn week_is_recent_uploads_of_watched_channels_before_following_excluded() {
        let conn = memory();
        upsert_channel(&conn, "UC1", "Watched").unwrap();
        upsert_channel(&conn, "UC2", "Not watched").unwrap();
        set_enabled(&conn, "UC1", true).unwrap();
        let ago = |days: i64| {
            (chrono::Utc::now() - chrono::Duration::days(days))
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
        };
        for (id, channel, status, published) in [
            ("new", "UC1", "queued", ago(1)),
            ("done", "UC1", "done", ago(3)),
            ("failed", "UC1", "error", ago(2)),
            ("before", "UC1", "skipped", ago(1)),
            ("old", "UC1", "done", ago(9)),
            ("other", "UC2", "done", ago(1)),
        ] {
            conn.execute(
                "INSERT INTO videos(id, channel_id, title, url, published, status) VALUES(?1, ?2, ?1, 'u', ?4, ?3)",
                params![id, channel, status, published],
            )
            .unwrap();
        }
        let all: Vec<String> = week(&conn, 7, false, 60)
            .unwrap()
            .into_iter()
            .map(|v| v.id)
            .collect();
        assert_eq!(all, ["new", "failed", "done"], "newest first");
        assert_eq!(ids(week(&conn, 7, true, 60).unwrap()), ["done"]);
        assert_eq!(week(&conn, 7, false, 2).unwrap().len(), 2);
    }

    #[test]
    fn first_brief_queues_only_the_newest_upload_of_untouched_followed_channels() {
        let conn = memory();
        for (id, enabled) in [("UC1", true), ("UC2", true), ("UC3", false)] {
            upsert_channel(&conn, id, id).unwrap();
            set_enabled(&conn, id, enabled).unwrap();
        }
        for (id, channel, status, published) in [
            ("a-old", "UC1", "skipped", "2026-10-20T00:00:00+00:00"),
            ("a-new", "UC1", "skipped", "2026-10-26T00:00:00+00:00"),
            ("b-skip", "UC2", "skipped", "2026-10-26T00:00:00+00:00"),
            ("b-done", "UC2", "done", "2026-10-25T00:00:00+00:00"),
            ("c-new", "UC3", "skipped", "2026-10-26T00:00:00+00:00"),
        ] {
            conn.execute(
                "INSERT INTO videos(id, channel_id, title, url, published, status) VALUES(?1, ?2, ?1, 'u', ?4, ?3)",
                params![id, channel, status, published],
            )
            .unwrap();
        }
        assert_eq!(queue_first_brief(&conn, 1).unwrap(), 1);
        assert_eq!(ids(pending(&conn).unwrap()), ["a-new"]);
        assert_eq!(queue_first_brief(&conn, 1).unwrap(), 0, "not twice");
    }
}
