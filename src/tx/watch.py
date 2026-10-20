"""Background watcher: find new videos on watched channels, analyze them against the
user's profile, notify. Channel discovery uses yt-dlp; new-video detection uses the
public per-channel RSS feed (no login, no API quota)."""

import re
import sqlite3
import threading
import traceback
import urllib.request
import xml.etree.ElementTree as ET
from datetime import datetime

from yt_dlp import YoutubeDL

from . import agent, cache, db
from .notify import notify
from .parse import render
from .sources import _BASE_OPTS, log

FEED = "https://www.youtube.com/feeds/videos.xml?channel_id={}"
NS = {"a": "http://www.w3.org/2005/Atom", "yt": "http://www.youtube.com/xml/schemas/2015"}

# What the worker is doing right now; read by the web UI.
STATE = {"now": "idle"}

PROMPT = """You are screening a YouTube video for one specific person. Who they are and what knowledge they want:
<profile>
{profile}
</profile>

stdin is the transcript of "{title}" by {channel}. Paragraphs start with [mm:ss] timestamps.

Reply in markdown with exactly this structure and nothing before it:
RELEVANCE: <0-5>/5 — <one sentence: why, for this person>
## TL;DR
<2-3 sentences: the core content or claim>
## Takeaways for you
- <3-7 concrete, specific points this person should know or act on, each ending with its [mm:ss]>
## Worth watching?
<skip | skim [mm:ss]–[mm:ss] | watch fully> — <one sentence why>

Use only the transcript. Write in the language the profile is written in. If the video is irrelevant to the profile, give only the 1-2 most generally useful takeaways."""

NO_PROFILE = "(No profile set. Judge usefulness for a curious technical generalist.)"

_RELEVANCE = re.compile(r"^\W*relevance\W*(\d)\s*(?:/\s*5)?\s*[—–:-]*\s*(.*)$", re.I | re.M)


def parse_analysis(out: str) -> tuple[int | None, str, str]:
    """Split agent output into (relevance 0-5, one-line verdict, markdown body)."""
    m = _RELEVANCE.search(out)
    if not m:
        return None, "", out.strip()
    rel = min(int(m.group(1)), 5)
    verdict = m.group(2).strip().strip("*").strip()
    body = (out[: m.start()] + out[m.end() :]).strip()
    return rel, verdict, body


def _cookies(spec: str) -> tuple:
    browser, _, profile = spec.partition(":")
    return (browser.strip().lower(), profile.strip() or None, None, None)


def import_subscriptions(conn: sqlite3.Connection, browser: str) -> int:
    """Add every channel the browser's logged-in YouTube account subscribes to (disabled)."""
    opts = {**_BASE_OPTS, "extract_flat": "in_playlist", "cookiesfrombrowser": _cookies(browser)}
    with YoutubeDL(opts) as ydl:
        info = ydl.extract_info("https://www.youtube.com/feed/channels", download=False)
    n = 0
    for e in info.get("entries") or []:
        cid = e.get("channel_id") or e.get("id") or ""
        if cid.startswith("UC"):
            db.upsert_channel(conn, cid, e.get("title") or e.get("channel") or cid)
            n += 1
    return n


def add_channel(conn: sqlite3.Connection, ref: str) -> str:
    """Watch a channel given @handle, channel id, channel URL, or any of its video URLs."""
    ref = ref.strip()
    if ref.startswith("@"):
        ref = f"https://www.youtube.com/{ref}"
    elif re.fullmatch(r"UC[\w-]{22}", ref):
        ref = f"https://www.youtube.com/channel/{ref}"
    with YoutubeDL({**_BASE_OPTS, "extract_flat": True, "playlist_items": "1"}) as ydl:
        info = ydl.extract_info(ref, download=False)
    cid = info.get("channel_id") or ""
    if not cid.startswith("UC"):
        raise ValueError(f"not a YouTube channel: {ref}")
    title = info.get("channel") or info.get("uploader") or info.get("title") or cid
    db.upsert_channel(conn, cid, title, enable=True)
    return title


def _feed(cid: str) -> list[tuple[str, str, str, str]]:
    req = urllib.request.Request(FEED.format(cid), headers={"User-Agent": "Mozilla/5.0 tx"})
    with urllib.request.urlopen(req, timeout=20) as r:
        root = ET.fromstring(r.read())
    out = []
    for e in root.findall("a:entry", NS):
        link = e.find("a:link", NS)
        out.append((e.findtext("yt:videoId", "", NS), e.findtext("a:title", "", NS), link.get("href", "") if link is not None else "", e.findtext("a:published", "", NS)))
    return out


def poll(conn: sqlite3.Connection) -> int:
    """Insert unseen videos of enabled channels; returns how many were queued."""
    queued = 0
    for ch in conn.execute("SELECT id, title, enabled_at FROM channels WHERE enabled = 1").fetchall():
        try:
            entries = _feed(ch["id"])
        except Exception as e:
            log(f"feed failed for {ch['title']}: {e}")
            continue
        since = datetime.fromisoformat(ch["enabled_at"])
        for vid, title, url, published in entries:
            if not vid or "/shorts/" in url:
                continue
            status = "queued" if datetime.fromisoformat(published) >= since else "skipped"
            cur = conn.execute(
                "INSERT OR IGNORE INTO videos(id, channel_id, title, url, published, status) VALUES(?, ?, ?, ?, ?, ?)",
                (vid, ch["id"], title, f"https://www.youtube.com/watch?v={vid}", published, status),
            )
            queued += cur.rowcount if status == "queued" else 0
        conn.execute("UPDATE channels SET checked_at = ? WHERE id = ?", (db.now(), ch["id"]))
    return queued


def analyze(video: sqlite3.Row, settings: dict[str, str]) -> tuple[int | None, str, str]:
    t, _ = cache.load(video["url"], model=settings["model"])
    prompt = PROMPT.format(profile=settings["profile"].strip() or NO_PROFILE, title=video["title"], channel=video["channel"])
    return parse_analysis(agent.ask(prompt, render(t)))


def process_queue(conn: sqlite3.Connection, ui_url: str) -> int:
    settings = db.get_settings(conn)
    rows = conn.execute(
        "SELECT v.*, c.title AS channel FROM videos v JOIN channels c ON c.id = v.channel_id "
        "WHERE v.status = 'queued' OR (v.status = 'error' AND v.attempts < ?) ORDER BY v.published",
        (db.MAX_ATTEMPTS,),
    ).fetchall()
    done = 0
    for v in rows:
        if not db.claim(conn, v["id"]):
            continue
        STATE["now"] = f"analyzing: {v['title']}"
        try:
            rel, verdict, body = analyze(v, settings)
        except (Exception, SystemExit) as e:
            log(f"analysis failed for {v['url']}: {e}")
            conn.execute("UPDATE videos SET status = 'error', error = ? WHERE id = ?", (str(e)[-2000:], v["id"]))
            continue
        finally:
            STATE["now"] = "idle"
        conn.execute(
            "UPDATE videos SET status = 'done', relevance = ?, verdict = ?, analysis = ?, error = NULL, analyzed_at = ? WHERE id = ?",
            (rel, verdict, body, db.now(), v["id"]),
        )
        done += 1
        if rel is None or rel >= int(settings["notify_min"]):
            score = f"{rel}/5 · " if rel is not None else ""
            notify(v["title"], f"{score}{v['channel']}\n{verdict}", f"{ui_url}/v/{v['id']}")
    return done


def run_forever(wake: threading.Event, ui_url: str) -> None:
    conn = db.connect()
    conn.execute("UPDATE videos SET status = 'queued' WHERE status = 'working'")  # interrupted by a restart
    while True:
        try:
            STATE["now"] = "checking channels"
            poll(conn)
            STATE["now"] = "idle"
            process_queue(conn, ui_url)
        except Exception:
            log(traceback.format_exc())
            STATE["now"] = "idle"
        wake.wait(max(1, int(db.get_settings(conn)["interval_min"])) * 60)
        wake.clear()
