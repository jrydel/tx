//! SQLite store for watched channels, discovered videos, and settings.
//! Schema is shared with tx 0.1 (Python), so existing databases keep working.

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS channels (
    id         TEXT PRIMARY KEY,           -- YouTube channel id (UC…)
    title      TEXT NOT NULL,
    enabled    INTEGER NOT NULL DEFAULT 0,
    enabled_at TEXT,                       -- videos published before this are not auto-analyzed
    checked_at TEXT
);
CREATE TABLE IF NOT EXISTS videos (
    id          TEXT PRIMARY KEY,          -- YouTube video id
    channel_id  TEXT NOT NULL REFERENCES channels(id),
    title       TEXT NOT NULL,
    url         TEXT NOT NULL,
    published   TEXT NOT NULL,
    status      TEXT NOT NULL,             -- skipped | queued | working | done | error
    attempts    INTEGER NOT NULL DEFAULT 0,
    relevance   INTEGER,                   -- 0-5 against the user's profile
    verdict     TEXT,                      -- one-line reason for the relevance score
    analysis    TEXT,                      -- markdown
    error       TEXT,
    analyzed_at TEXT,
    duration    REAL                       -- seconds
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
    /// yt-dlp --cookies-from-browser spec, e.g. "firefox" or "chrome:Profile 2".
    pub browser: String,
    pub interval_min: u32,
    /// Notify (and list under "Worth your time") when relevance >= this.
    pub notify_min: u8,
    /// Whisper model for videos without captions.
    pub model: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            profile: String::new(),
            browser: "chrome".into(),
            interval_min: 30,
            notify_min: 3,
            model: "small".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
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
}

#[derive(Debug, Clone, Serialize)]
pub struct Channel {
    pub id: String,
    pub title: String,
    pub enabled: bool,
    pub checked_at: Option<String>,
    pub analyzed: i64,
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

pub fn connect() -> Result<Connection> {
    let conn = Connection::open(crate::paths::data_dir().join("tx.db"))?;
    conn.busy_timeout(std::time::Duration::from_secs(30))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.execute_batch(SCHEMA)?;
    let has_duration = conn
        .prepare("SELECT 1 FROM pragma_table_info('videos') WHERE name = 'duration'")?
        .exists([])?;
    if !has_duration {
        conn.execute("ALTER TABLE videos ADD COLUMN duration REAL", [])?; // databases from tx 0.1
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
        browser: rows.get("browser").cloned().unwrap_or(d.browser),
        interval_min: rows
            .get("interval_min")
            .and_then(|v| v.parse().ok())
            .unwrap_or(d.interval_min),
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
        ("browser", s.browser.clone()),
        ("interval_min", s.interval_min.to_string()),
        ("notify_min", s.notify_min.to_string()),
        ("model", s.model.clone()),
    ] {
        stmt.execute(params![k, v])?;
    }
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

/// Atomically move a video to `working`; false if another worker got it first.
pub fn claim(conn: &Connection, id: &str) -> Result<bool> {
    let n = conn.execute(
        "UPDATE videos SET status = 'working', attempts = attempts + 1 \
         WHERE id = ?1 AND (status = 'queued' OR (status = 'error' AND attempts < ?2))",
        params![id, MAX_ATTEMPTS],
    )?;
    Ok(n == 1)
}

pub fn requeue(conn: &Connection, id: &str) -> Result<bool> {
    let n = conn.execute(
        "UPDATE videos SET status = 'queued', attempts = 0, error = NULL WHERE id = ?1",
        params![id],
    )?;
    Ok(n == 1)
}

const VIDEO_COLS: &str = "v.id, v.title, v.url, v.channel_id, c.title, v.published, v.status, v.relevance, \
    v.verdict, v.analysis, v.error, v.duration, v.analyzed_at";

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
    })
}

/// `worth`: at/above the notification threshold plus anything still in progress;
/// `all`: every analyzed or pending video; `earlier`: uploads from before watching started.
pub fn videos(conn: &Connection, view: &str, channel: Option<&str>) -> Result<Vec<Video>> {
    let filter = match view {
        "worth" => {
            "(v.status IN ('queued', 'working', 'error') OR (v.status = 'done' AND (v.relevance IS NULL OR v.relevance >= ?1)))"
        }
        "all" => "(v.status != 'skipped' AND ?1 IS NOT NULL)",
        "earlier" => "(v.status = 'skipped' AND ?1 IS NOT NULL)",
        other => anyhow::bail!("unknown view: {other}"),
    };
    let sql = format!(
        "SELECT {VIDEO_COLS} FROM videos v JOIN channels c ON c.id = v.channel_id \
         WHERE {filter} AND (?2 IS NULL OR v.channel_id = ?2) ORDER BY v.published DESC LIMIT 300"
    );
    let min = get_settings(conn)?.notify_min;
    let rows = conn
        .prepare(&sql)?
        .query_map(params![min, channel], video_row)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn video(conn: &Connection, id: &str) -> Result<Option<Video>> {
    let sql = format!(
        "SELECT {VIDEO_COLS} FROM videos v JOIN channels c ON c.id = v.channel_id WHERE v.id = ?1"
    );
    Ok(conn.query_row(&sql, params![id], video_row).optional()?)
}

/// Videos the worker should process now, oldest first.
pub fn pending(conn: &Connection) -> Result<Vec<Video>> {
    let sql = format!(
        "SELECT {VIDEO_COLS} FROM videos v JOIN channels c ON c.id = v.channel_id \
         WHERE v.status = 'queued' OR (v.status = 'error' AND v.attempts < ?1) ORDER BY v.published"
    );
    let rows = conn
        .prepare(&sql)?
        .query_map(params![MAX_ATTEMPTS], video_row)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn channels(conn: &Connection) -> Result<Vec<Channel>> {
    let rows = conn
        .prepare(
            "SELECT c.id, c.title, c.enabled, c.checked_at, \
             (SELECT COUNT(*) FROM videos v WHERE v.channel_id = c.id AND v.status = 'done') \
             FROM channels c ORDER BY c.title COLLATE NOCASE",
        )?
        .query_map([], |r| {
            Ok(Channel {
                id: r.get(0)?,
                title: r.get(1)?,
                enabled: r.get::<_, i64>(2)? == 1,
                checked_at: r.get(3)?,
                analyzed: r.get(4)?,
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
    fn views_split_by_threshold_and_status() {
        let conn = memory();
        seed(&conn);
        assert_eq!(ids(videos(&conn, "worth", None).unwrap()), ["a", "c"]);
        assert_eq!(ids(videos(&conn, "all", None).unwrap()), ["a", "b", "c"]);
        assert_eq!(ids(videos(&conn, "earlier", None).unwrap()), ["d"]);
        assert!(videos(&conn, "bogus", None).is_err());
    }

    #[test]
    fn claim_is_exclusive_and_stops_after_max_attempts() {
        let conn = memory();
        seed(&conn);
        assert!(claim(&conn, "c").unwrap());
        assert!(!claim(&conn, "c").unwrap(), "already working");
        conn.execute(
            "UPDATE videos SET status = 'error', attempts = ?1 WHERE id = 'c'",
            params![MAX_ATTEMPTS],
        )
        .unwrap();
        assert!(!claim(&conn, "c").unwrap(), "out of attempts");
        assert!(requeue(&conn, "c").unwrap());
        assert!(claim(&conn, "c").unwrap());
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
}
