"""Fetch a transcript: platform captions via yt-dlp, else local Whisper on the audio."""

import json
import subprocess
import sys
import tempfile
from pathlib import Path

from yt_dlp import YoutubeDL

from .parse import Seg, Transcript, fmt_time, parse_json3, parse_vtt

_BASE_OPTS = {
    "quiet": True,
    "no_warnings": True,
    "noprogress": True,
    "noplaylist": True,
    # YouTube needs a JS runtime for signature solving; use whichever is installed.
    "js_runtimes": {"deno": {}, "node": {}, "bun": {}},
}


def log(msg: str) -> None:
    print(f"tx: {msg}", file=sys.stderr, flush=True)


def _pick_track(tracks: dict, prefs: list[str]) -> str | None:
    keys = [k for k in tracks if k != "live_chat"]
    for p in prefs:
        for k in (f"{p}-orig", p):
            if k in keys:
                return k
    for p in prefs:
        for k in keys:
            if k.split("-")[0] == p:
                return k
    return next((k for k in keys if k.endswith("-orig")), None)


def _captions(ydl: YoutubeDL, info: dict, lang: str | None) -> tuple[list[Seg], str] | None:
    prefs = [lang] if lang else [p for p in (info.get("language"), "en") if p]
    manual = info.get("subtitles") or {}
    auto = info.get("automatic_captions") or {}
    for tracks, kind, fallback_any in ((manual, "manual", not lang), (auto, "auto", False)):
        key = _pick_track(tracks, prefs)
        if key is None and fallback_any:
            key = next((k for k in tracks if k != "live_chat"), None)
        if key is None:
            continue
        by_ext = {f.get("ext"): f for f in tracks[key] if f.get("url")}
        for ext in ("json3", "vtt", "srt"):
            if ext in by_ext:
                body = ydl.urlopen(by_ext[ext]["url"]).read().decode("utf-8", "replace")
                segs = parse_json3(json.loads(body)) if ext == "json3" else parse_vtt(body)
                if segs:
                    return segs, f"captions {key} ({kind})"
    return None


def _whisper(url: str, item: int | None, duration: float | None, lang: str | None, model_name: str) -> tuple[list[Seg], str]:
    import numpy as np
    from faster_whisper import WhisperModel

    with tempfile.TemporaryDirectory(prefix="tx-") as tmp:
        log("downloading audio…")
        opts = {**_BASE_OPTS, "format": "bestaudio/best", "outtmpl": f"{tmp}/audio.%(ext)s"}
        if item:
            opts["playlist_items"] = str(item)
        with YoutubeDL(opts) as ydl:
            ydl.download([url])
        audio = next(Path(tmp).glob("audio.*"))
        # Decode with ffmpeg: faster-whisper's PyAV path breaks on current av releases.
        pcm = subprocess.run(
            ["ffmpeg", "-nostdin", "-v", "error", "-i", str(audio), "-f", "f32le", "-ac", "1", "-ar", "16000", "-"],
            capture_output=True,
            check=True,
        ).stdout
        log(f"transcribing with whisper {model_name} (first run downloads the model)…")
        model = WhisperModel(model_name, device="auto", compute_type="auto")
        seg_iter, winfo = model.transcribe(np.frombuffer(pcm, np.float32), language=lang, vad_filter=True)
        segs = []
        total = f" / {fmt_time(duration)}" if duration else ""
        for s in seg_iter:
            segs.append(Seg(s.start, s.text.strip()))
            if sys.stderr.isatty():
                print(f"\rtx: {fmt_time(s.end)}{total}", end="", file=sys.stderr, flush=True)
        if sys.stderr.isatty():
            print(file=sys.stderr)
    return [s for s in segs if s.text], f"whisper {model_name} ({winfo.language})"


def fetch(url: str, lang: str | None, force_whisper: bool, model: str) -> Transcript:
    with YoutubeDL(_BASE_OPTS) as ydl:
        info = ydl.extract_info(url, download=False)
        item = None
        if info.get("_type") == "playlist":  # e.g. an X post with several media items
            entries = [e for e in info.get("entries") or [] if e]
            if not entries:
                raise SystemExit("tx: no video found at URL")
            if len(entries) > 1:
                log(f"{len(entries)} videos at URL; using the first")
            info = entries[0]
            item = info.get("playlist_index") or 1
        got = None if force_whisper else _captions(ydl, info, lang)
    if got is None:
        if not force_whisper:
            log("no captions found; falling back to whisper")
        got = _whisper(url, item, info.get("duration"), lang, model)
    segs, source = got
    return Transcript(
        title=info.get("title") or url,
        url=url,
        uploader=info.get("uploader") or info.get("channel"),
        duration=info.get("duration"),
        source=source,
        segs=segs,
    )
