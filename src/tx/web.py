"""`tx serve`: local web UI (127.0.0.1 only) plus the background watcher thread."""

import html
import re
import sqlite3
import threading
from datetime import datetime
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, quote, urlparse

from . import cache, db, watch
from .parse import render
from .service import ICON_SVG, ui_url

CSS = """
:root{color-scheme:light dark;--fg:#1a1a1a;--muted:#6b6b6b;--line:#e4e4e4;--card:#fff;--bg:#f6f6f4;--accent:#d93636}
@media(prefers-color-scheme:dark){:root{--fg:#ececec;--muted:#9a9a9a;--line:#2c2c2c;--card:#181818;--bg:#101010}}
*{box-sizing:border-box}body{margin:0;font:15px/1.55 system-ui,sans-serif;background:var(--bg);color:var(--fg)}
header{display:flex;gap:18px;align-items:center;padding:12px 24px;border-bottom:1px solid var(--line);position:sticky;top:0;background:var(--bg)}
header b{font-size:17px}header a{color:var(--muted);text-decoration:none}header a.on{color:var(--fg);font-weight:600}
header .grow{flex:1}header small{color:var(--muted)}
main{max-width:860px;margin:0 auto;padding:20px 24px 60px}
a{color:inherit}h1{font-size:22px;margin:4px 0 6px}h2{font-size:15px;color:var(--muted);font-weight:600;margin:28px 0 8px}
h3{font-size:15px;margin:18px 0 6px}
.item{background:var(--card);border:1px solid var(--line);border-radius:10px;padding:14px 16px;margin:10px 0}
.item .t{font-weight:600;text-decoration:none;font-size:16px}.meta{color:var(--muted);font-size:13px;margin:2px 0 6px}
.rel{display:inline-block;min-width:34px;text-align:center;border-radius:6px;padding:0 6px;font-weight:700;font-size:12px;border:1px solid var(--line)}
.r4,.r5{background:var(--accent);color:#fff;border-color:var(--accent)}.r3{border-color:var(--accent);color:var(--accent)}
.verdict{margin:4px 0}.tldr{color:var(--muted);margin:4px 0 0}
.row{display:flex;align-items:center;gap:10px;padding:7px 0;border-bottom:1px solid var(--line)}.row .grow{flex:1}
button,input,textarea,select{font:inherit;color:inherit}
button{background:var(--card);border:1px solid var(--line);border-radius:7px;padding:4px 12px;cursor:pointer;white-space:nowrap}
button.primary,button.on{background:var(--fg);color:var(--bg);border-color:var(--fg)}
input[type=text],textarea,select{background:var(--card);border:1px solid var(--line);border-radius:7px;padding:7px 9px;width:100%}
textarea{min-height:180px}label{display:block;margin:14px 0 4px;font-weight:600}.hint{color:var(--muted);font-size:13px;margin:2px 0}
form.inline{display:inline}.flash{background:var(--card);border-left:3px solid var(--accent);padding:8px 12px;margin:0 0 14px}
.err{color:var(--accent);font-size:13px;white-space:pre-wrap}.bar{display:flex;gap:8px;align-items:center;margin:10px 0}
code{background:var(--card);border:1px solid var(--line);border-radius:5px;padding:1px 5px;font-size:13px}
.analysis li{margin:4px 0}.transcript p{margin:0 0 12px}
"""

_TS = re.compile(r"\[(?:(\d+):)?(\d{1,2}):(\d{2})\]")


def esc(s: object) -> str:
    return html.escape(str(s or ""), quote=True)


def _inline(s: str, video_url: str) -> str:
    s = esc(s)
    s = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", s)
    s = re.sub(r"`([^`]+)`", r"<code>\1</code>", s)

    def ts(m: re.Match) -> str:
        sec = int(m.group(1) or 0) * 3600 + int(m.group(2)) * 60 + int(m.group(3))
        return f'<a href="{esc(video_url)}&amp;t={sec}s" target="_blank">{m.group(0)}</a>'

    return _TS.sub(ts, s)


def md(text: str, video_url: str) -> str:
    """Minimal markdown (headings, bullets, bold, code, [mm:ss] links). Escapes all
    input first: analyses and transcripts are untrusted (video creators control them)."""
    out: list[str] = []
    para: list[str] = []
    in_list = False

    def close_para() -> None:
        if para:
            out.append(f"<p>{_inline(' '.join(para), video_url)}</p>")
            para.clear()

    for line in text.splitlines():
        st = line.strip()
        bullet = re.match(r"[-*•] +(.*)", st)
        if not bullet and in_list:
            out.append("</ul>")
            in_list = False
        if not st:
            close_para()
        elif st.startswith("#"):
            close_para()
            out.append(f"<h3>{_inline(st.lstrip('#').strip(), video_url)}</h3>")
        elif bullet:
            close_para()
            if not in_list:
                out.append("<ul>")
                in_list = True
            out.append(f"<li>{_inline(bullet.group(1), video_url)}</li>")
        else:
            para.append(st)
    close_para()
    if in_list:
        out.append("</ul>")
    return "\n".join(out)


def section(text: str, name: str) -> str:
    m = re.search(rf"^#+\s*{re.escape(name)}\s*$(.*?)(?=^#|\Z)", text or "", re.M | re.S | re.I)
    return m.group(1).strip() if m else ""


def when(iso: str | None) -> str:
    if not iso:
        return "never"
    return datetime.fromisoformat(iso).astimezone().strftime("%Y-%m-%d %H:%M")


def page(title: str, body: str, active: str = "", refresh: bool = False) -> bytes:
    nav = "".join(
        f'<a href="{href}" class="{"on" if key == active else ""}">{label}</a>'
        for key, href, label in (("feed", "/", "Videos"), ("channels", "/channels", "Channels"), ("settings", "/settings", "Settings"))
    )
    meta = '<meta http-equiv="refresh" content="20">' if refresh else ""
    return (
        f'<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width">{meta}'
        f'<title>{esc(title)} · tx</title><link rel="icon" href="/icon.svg"><style>{CSS}</style></head>'
        f'<body><header><b>tx</b>{nav}<span class="grow"></span><small>{esc(watch.STATE["now"])}</small>'
        f'<form class="inline" method="post" action="/check"><button>Check now</button></form></header>'
        f"<main>{body}</main></body></html>"
    ).encode()


def flash(q: dict) -> str:
    msg = q.get("msg", [""])[0]
    return f'<div class="flash">{esc(msg)}</div>' if msg else ""


# --- pages ----------------------------------------------------------------------------------


def feed_page(conn: sqlite3.Connection, q: dict) -> bytes:
    channel = q.get("channel", [""])[0]
    where, args = ("AND v.channel_id = ?", [channel]) if channel else ("", [])
    rows = conn.execute(
        f"SELECT v.*, c.title AS channel FROM videos v JOIN channels c ON c.id = v.channel_id "
        f"WHERE v.status != 'skipped' {where} ORDER BY v.published DESC LIMIT 300",
        args,
    ).fetchall()
    skipped = conn.execute(
        f"SELECT v.*, c.title AS channel FROM videos v JOIN channels c ON c.id = v.channel_id "
        f"WHERE v.status = 'skipped' {where} ORDER BY v.published DESC LIMIT 60",
        args,
    ).fetchall()
    watched = conn.execute("SELECT COUNT(*) FROM channels WHERE enabled = 1").fetchone()[0]
    parts = [flash(q)]
    if not watched:
        parts.append('<div class="flash">No channels watched yet. <a href="/channels">Pick channels</a> and <a href="/settings">describe what you want to learn</a>.</div>')
    pending = False
    for v in rows:
        status = v["status"]
        pending |= status in ("queued", "working")
        badge = f'<span class="rel r{v["relevance"]}">{v["relevance"]}/5</span> ' if v["relevance"] is not None else ""
        state = "" if status == "done" else f" · <b>{esc(status)}</b>"
        err = f'<div class="err">{esc(v["error"])}</div>' if status == "error" and v["error"] else ""
        tldr = section(v["analysis"], "TL;DR") if v["analysis"] else ""
        parts.append(
            f'<article class="item"><a class="t" href="/v/{esc(v["id"])}">{esc(v["title"])}</a>'
            f'<div class="meta">{badge}<a href="/?channel={quote(v["channel_id"])}">{esc(v["channel"])}</a> · {when(v["published"])}{state}</div>'
            f'{f"<p class=verdict>{esc(v["verdict"])}</p>" if v["verdict"] else ""}'
            f'{f"<p class=tldr>{esc(tldr)}</p>" if tldr else ""}{err}</article>'
        )
    if watched and not rows:
        parts.append('<p class="hint">No new videos since you started watching. Earlier uploads are listed below; analyze any of them on demand.</p>')
    if skipped:
        parts.append("<h2>Earlier uploads (published before you started watching)</h2>")
        for v in skipped:
            parts.append(
                f'<div class="row"><span class="grow"><a href="{esc(v["url"])}" target="_blank">{esc(v["title"])}</a>'
                f'<div class="meta">{esc(v["channel"])} · {when(v["published"])}</div></span>'
                f'<form class="inline" method="post" action="/v/{esc(v["id"])}/analyze"><button>Analyze</button></form></div>'
            )
    head = f'<h1>{"Channel videos" if channel else "Videos"}</h1>' + (' <a class="hint" href="/">show all channels</a>' if channel else "")
    return page("Videos", head + "".join(parts), "feed", refresh=pending)


def video_page(conn: sqlite3.Connection, vid: str) -> bytes | None:
    v = conn.execute("SELECT v.*, c.title AS channel FROM videos v JOIN channels c ON c.id = v.channel_id WHERE v.id = ?", (vid,)).fetchone()
    if not v:
        return None
    badge = f'<span class="rel r{v["relevance"]}">{v["relevance"]}/5</span> ' if v["relevance"] is not None else ""
    body = [
        f'<h1>{esc(v["title"])}</h1><div class="meta">{badge}{esc(v["channel"])} · {when(v["published"])} · {esc(v["status"])}</div>',
        f'<div class="bar"><a href="{esc(v["url"])}" target="_blank"><button class="primary">Open on YouTube</button></a>'
        f'<a href="/v/{esc(vid)}/transcript"><button>Transcript</button></a>'
        f'<form class="inline" method="post" action="/v/{esc(vid)}/analyze"><button>{"Re-analyze" if v["status"] == "done" else "Analyze"}</button></form></div>',
    ]
    if v["verdict"]:
        body.append(f'<p class="verdict"><b>{esc(v["verdict"])}</b></p>')
    if v["analysis"]:
        body.append(f'<div class="analysis">{md(v["analysis"], v["url"])}</div>')
    if v["status"] == "error" and v["error"]:
        body.append(f'<div class="err">{esc(v["error"])}</div>')
    body.append(f'<p class="hint">Ask follow-ups in your terminal: <code>tx "{esc(v["url"])}" -c</code></p>')
    return page(v["title"], "".join(body), refresh=v["status"] in ("queued", "working"))


def transcript_page(conn: sqlite3.Connection, vid: str) -> bytes | None:
    v = conn.execute("SELECT url, title FROM videos WHERE id = ?", (vid,)).fetchone()
    if not v:
        return None
    t, _ = cache.load(v["url"], model=db.get_settings(conn)["model"])
    return page(v["title"], f'<p><a href="/v/{esc(vid)}">← analysis</a></p><div class="transcript">{md(render(t), v["url"])}</div>')


def channels_page(conn: sqlite3.Connection, q: dict) -> bytes:
    s = db.get_settings(conn)
    rows = conn.execute(
        "SELECT c.*, (SELECT COUNT(*) FROM videos v WHERE v.channel_id = c.id AND v.status = 'done') AS analyzed "
        "FROM channels c ORDER BY c.enabled DESC, c.title COLLATE NOCASE"
    ).fetchall()
    body = [
        flash(q),
        "<h1>Channels</h1>",
        '<p class="hint">Turn on the channels to watch. New uploads on those channels get analyzed and trigger a notification.</p>',
        '<form method="post" action="/channels/import" class="bar"><input type="text" name="browser" '
        f'value="{esc(s["browser"])}" style="max-width:260px" title="Browser logged in to YouTube">'
        '<button class="primary">Import my YouTube subscriptions</button></form>',
        '<p class="hint">Reads the YouTube login of that browser (<code>chrome</code>, <code>firefox</code>, <code>edge</code>, '
        '<code>safari</code>, <code>brave</code>; a profile with <code>chrome:Profile 2</code>). Nothing is sent anywhere except YouTube.</p>',
        '<form method="post" action="/channels/add" class="bar"><input type="text" name="ref" placeholder="@handle, channel URL, or a video URL">'
        "<button>Add channel</button></form>",
    ]
    if rows:
        body.append('<input type="text" id="filter" placeholder="Filter channels…" style="margin:14px 0 6px">')
        for c in rows:
            on = bool(c["enabled"])
            info = f'{c["analyzed"]} analyzed · checked {when(c["checked_at"])}' if on else ""
            body.append(
                f'<div class="row ch" id="c-{esc(c["id"])}"><span class="grow"><a href="/?channel={quote(c["id"])}">{esc(c["title"])}</a>'
                f'<div class="meta">{info}</div></span>'
                f'<form class="inline" method="post" action="/channels/{esc(c["id"])}/toggle">'
                f'<button class="{"on" if on else ""}">{"Watching" if on else "Watch"}</button></form></div>'
            )
        body.append(
            "<script>document.getElementById('filter').oninput=e=>{const q=e.target.value.toLowerCase();"
            "document.querySelectorAll('.ch').forEach(r=>r.style.display=r.textContent.toLowerCase().includes(q)?'':'none')}</script>"
        )
    return page("Channels", "".join(body), "channels")


def settings_page(conn: sqlite3.Connection, q: dict) -> bytes:
    s = db.get_settings(conn)
    models = "".join(f'<option {"selected" if m == s["model"] else ""}>{m}</option>' for m in ("tiny", "base", "small", "medium", "large-v3"))
    placeholder = (
        "Example: I'm a backend engineer building LLM features. I want concrete techniques for agents, RAG and evals, "
        "and notable model or tooling launches. Skip hype, drama, and beginner tutorials."
    )
    body = f"""{flash(q)}<h1>Settings</h1><form method="post" action="/settings">
<label>Who you are and what knowledge you want</label>
<p class="hint">Every video is scored 0–5 against this and summarized for you. Write it in the language you want analyses in.</p>
<textarea name="profile" placeholder="{esc(placeholder)}">{esc(s["profile"])}</textarea>
<label>Notify when relevance is at least</label>
<select name="notify_min">{"".join(f'<option {"selected" if str(n) == s["notify_min"] else ""}>{n}</option>' for n in range(6))}</select>
<label>Check for new videos every (minutes)</label><input type="text" name="interval_min" value="{esc(s["interval_min"])}">
<label>Browser for subscription import</label><input type="text" name="browser" value="{esc(s["browser"])}">
<label>Local transcription model (videos without captions)</label><select name="model">{models}</select>
<p class="hint">Larger is more accurate and slower. Most YouTube videos have captions and skip this.</p>
<p><button class="primary">Save</button></p></form>"""
    return page("Settings", body, "settings")


# --- server ---------------------------------------------------------------------------------


def make_handler(wake: threading.Event, stop):
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *args) -> None:  # quiet access log
            pass

        def _send(self, body: bytes, status: int = 200, ctype: str = "text/html; charset=utf-8") -> None:
            self.send_response(status)
            self.send_header("Content-Type", ctype)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def _redirect(self, to: str) -> None:
            self.send_response(303)
            self.send_header("Location", to)
            self.send_header("Content-Length", "0")
            self.end_headers()

        def do_GET(self) -> None:
            u = urlparse(self.path)
            q = parse_qs(u.query)
            if u.path == "/health":
                return self._send(b"tx", ctype="text/plain")
            if u.path == "/icon.svg":
                return self._send(ICON_SVG.encode(), ctype="image/svg+xml")
            conn = db.connect()
            try:
                if u.path == "/":
                    return self._send(feed_page(conn, q))
                if u.path == "/channels":
                    return self._send(channels_page(conn, q))
                if u.path == "/settings":
                    return self._send(settings_page(conn, q))
                if m := re.fullmatch(r"/v/([\w-]+)", u.path):
                    if body := video_page(conn, m.group(1)):
                        return self._send(body)
                if m := re.fullmatch(r"/v/([\w-]+)/transcript", u.path):
                    try:
                        body = transcript_page(conn, m.group(1))
                    except (Exception, SystemExit) as e:
                        return self._send(page("Transcript", f'<div class="err">{esc(e)}</div>'), 502)
                    if body:
                        return self._send(body)
                self._send(page("Not found", "<h1>Not found</h1>"), 404)
            finally:
                conn.close()

        def do_POST(self) -> None:
            # Reject cross-site form posts: browsers send Origin on them.
            origin = self.headers.get("Origin")
            if origin and urlparse(origin).netloc != self.headers.get("Host"):
                return self._send(b"forbidden", 403, "text/plain")
            n = int(self.headers.get("Content-Length") or 0)
            form = {k: v[0] for k, v in parse_qs(self.rfile.read(n).decode("utf-8"), keep_blank_values=True).items()}
            path = urlparse(self.path).path
            if path == "/quit":
                self._send(b"bye", ctype="text/plain")
                return stop()
            conn = db.connect()
            try:
                self._redirect(self._post(conn, path, form))
            finally:
                conn.close()

        def _post(self, conn: sqlite3.Connection, path: str, form: dict) -> str:
            back = self.headers.get("Referer") or "/"
            if path == "/check":
                wake.set()
                return back
            if m := re.fullmatch(r"/v/([\w-]+)/analyze", path):
                db.requeue(conn, m.group(1))
                wake.set()
                return f"/v/{m.group(1)}"
            if m := re.fullmatch(r"/channels/([\w-]+)/toggle", path):
                row = conn.execute("SELECT enabled FROM channels WHERE id = ?", (m.group(1),)).fetchone()
                if row:
                    db.set_enabled(conn, m.group(1), not row["enabled"])
                    if not row["enabled"]:
                        wake.set()  # fetch its feed now
                return f"/channels#c-{m.group(1)}"
            if path == "/channels/import":
                browser = form.get("browser", "").strip() or "chrome"
                db.set_settings(conn, {"browser": browser})
                try:
                    n = watch.import_subscriptions(conn, browser)
                    msg = f"Imported {n} subscriptions. Turn on the ones to watch."
                except Exception as e:
                    msg = f"Import failed ({e}). Is that browser logged in to YouTube? Close it if its cookie database is locked."
                return "/channels?msg=" + quote(msg)
            if path == "/channels/add":
                try:
                    msg = f"Watching {watch.add_channel(conn, form.get('ref', ''))}."
                    wake.set()
                except Exception as e:
                    msg = f"Could not add channel: {e}"
                return "/channels?msg=" + quote(msg)
            if path == "/settings":
                values = {k: form[k].strip() for k in db.DEFAULTS if k in form}
                if not values.get("interval_min", "30").isdigit():
                    return "/settings?msg=" + quote("Interval must be a whole number of minutes.")
                db.set_settings(conn, values)
                return "/settings?msg=" + quote("Saved.")
            return "/"

    return Handler


def serve(port: int) -> None:
    wake = threading.Event()
    srv: ThreadingHTTPServer | None = None

    def stop() -> None:
        threading.Thread(target=srv.shutdown, daemon=True).start()

    srv = ThreadingHTTPServer(("127.0.0.1", port), make_handler(wake, stop))
    threading.Thread(target=watch.run_forever, args=(wake, ui_url(port)), daemon=True).start()
    print(f"tx: serving {ui_url(port)}", flush=True)
    srv.serve_forever()
