import argparse
import hashlib
import json
import os
import shlex
import subprocess
import sys
from pathlib import Path

from .parse import Transcript, render

CACHE = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "tx"


def _load(args) -> tuple[Transcript, Path]:
    mode = f"whisper:{args.model}" if args.whisper else "auto"
    key = hashlib.sha1(f"{args.url}|{args.lang}|{mode}".encode()).hexdigest()[:16]
    path = CACHE / f"{key}.json"
    if path.exists() and not args.refresh:
        return Transcript.from_json(json.loads(path.read_text())), path
    from .sources import fetch  # heavy import; skipped on cache hits

    t = fetch(args.url, args.lang, args.whisper, args.model)
    CACHE.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(t.to_json(), ensure_ascii=False))
    return t, path


def _page(text: str) -> None:
    if not sys.stdout.isatty():
        sys.stdout.write(text)
        return
    pager = shlex.split(os.environ.get("PAGER") or "less -FRX")
    try:
        subprocess.run(pager, input=text, text=True)
    except FileNotFoundError:
        sys.stdout.write(text)


def main() -> None:
    ap = argparse.ArgumentParser(
        prog="tx",
        description="Print a video's transcript (any yt-dlp-supported URL), or ask an agent about it.",
    )
    ap.add_argument("url")
    ap.add_argument("-a", "--ask", metavar="QUESTION", help="one-shot question to $TX_AGENT (default: claude -p)")
    ap.add_argument("-c", "--chat", action="store_true", help="open an interactive $TX_CHAT_AGENT (default: claude) session on the transcript")
    ap.add_argument("-l", "--lang", help="caption/audio language code, e.g. en, cs (default: video language, then en)")
    ap.add_argument("-w", "--whisper", action="store_true", help="skip platform captions; transcribe audio locally")
    ap.add_argument("-m", "--model", default=os.environ.get("TX_WHISPER_MODEL", "small"), help="whisper model (default: small)")
    ap.add_argument("-T", "--no-timestamps", action="store_true", help="omit [mm:ss] paragraph markers")
    ap.add_argument("-r", "--refresh", action="store_true", help="ignore cached transcript")
    args = ap.parse_args()
    if args.ask and args.chat:
        ap.error("--ask and --chat are mutually exclusive")

    try:
        t, cache_path = _load(args)
    except KeyboardInterrupt:
        raise SystemExit(130)
    except Exception as e:  # yt-dlp DownloadError etc.: one line, not a traceback
        raise SystemExit(f"tx: {e}")
    text = render(t, timestamps=not args.no_timestamps)

    if args.ask:
        agent = shlex.split(os.environ.get("TX_AGENT", "claude -p"))
        prompt = (
            f'stdin is the transcript of the video "{t.title}" ({t.url}). '
            "Answer from the transcript; cite [mm:ss] timestamps where useful.\n\n"
            f"Question: {args.ask}"
        )
        raise SystemExit(subprocess.run([*agent, prompt], input=text, text=True).returncode)

    if args.chat:
        md = cache_path.with_suffix(".md")
        md.write_text(text)
        agent = shlex.split(os.environ.get("TX_CHAT_AGENT", "claude"))
        prompt = (
            f'Read the transcript of the video "{t.title}" at {md}. '
            "Reply with a 5-bullet summary, then answer my questions from it, citing [mm:ss] timestamps."
        )
        os.execvp(agent[0], [*agent, prompt])

    _page(text)
