"""SQLite store for watched channels, discovered videos, and settings."""

import sqlite3
from datetime import datetime, timezone

from .paths import data_dir

SCHEMA = """
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
    analyzed_at TEXT
);
CREATE INDEX IF NOT EXISTS videos_published ON videos(published DESC);
CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
"""

DEFAULTS = {
    "profile": "",
    "browser": "chrome",  # yt-dlp --cookies-from-browser spec, e.g. "firefox" or "chrome:Profile 2"
    "interval_min": "30",
    "notify_min": "3",  # notify when relevance >= this
    "model": "small",
}

MAX_ATTEMPTS = 3


def now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def connect() -> sqlite3.Connection:
    conn = sqlite3.connect(data_dir() / "tx.db", timeout=30, isolation_level=None)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA journal_mode=WAL")
    conn.executescript(SCHEMA)
    return conn


def get_settings(conn: sqlite3.Connection) -> dict[str, str]:
    return {**DEFAULTS, **{r["key"]: r["value"] for r in conn.execute("SELECT key, value FROM settings")}}


def set_settings(conn: sqlite3.Connection, values: dict[str, str]) -> None:
    conn.executemany(
        "INSERT INTO settings(key, value) VALUES(?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [(k, v) for k, v in values.items() if k in DEFAULTS],
    )


def upsert_channel(conn: sqlite3.Connection, cid: str, title: str, enable: bool = False) -> None:
    conn.execute(
        "INSERT INTO channels(id, title) VALUES(?, ?) ON CONFLICT(id) DO UPDATE SET title = excluded.title",
        (cid, title),
    )
    if enable:
        set_enabled(conn, cid, True)


def set_enabled(conn: sqlite3.Connection, cid: str, on: bool) -> None:
    if on:  # keep the original enabled_at so re-enabling does not re-skip videos already seen
        conn.execute("UPDATE channels SET enabled = 1, enabled_at = COALESCE(enabled_at, ?) WHERE id = ?", (now(), cid))
    else:
        conn.execute("UPDATE channels SET enabled = 0 WHERE id = ?", (cid,))


def claim(conn: sqlite3.Connection, vid: str) -> bool:
    """Atomically move a video to `working`; False if another worker got it first."""
    cur = conn.execute(
        "UPDATE videos SET status = 'working', attempts = attempts + 1 "
        "WHERE id = ? AND (status = 'queued' OR (status = 'error' AND attempts < ?))",
        (vid, MAX_ATTEMPTS),
    )
    return cur.rowcount == 1


def requeue(conn: sqlite3.Connection, vid: str) -> None:
    conn.execute("UPDATE videos SET status = 'queued', attempts = 0, error = NULL WHERE id = ?", (vid,))
