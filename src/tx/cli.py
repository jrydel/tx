import argparse
import os
import shlex
import subprocess
import sys

from . import agent, cache
from .parse import render
from .paths import data_dir
from .service import PORT

COMMANDS = ("app", "serve", "check", "service")


def _page(text: str) -> None:
    if not sys.stdout.isatty():
        sys.stdout.write(text)
        return
    pager = shlex.split(os.environ.get("PAGER") or "less -FRX")
    try:
        subprocess.run(pager, input=text, text=True, encoding="utf-8")
    except FileNotFoundError:
        sys.stdout.write(text)


def _log_to_file_if_windowless() -> None:
    # pythonw (Windows launcher/autostart) has no stdio; keep logs instead of crashing on print.
    if sys.stderr is None or sys.stdout is None:
        f = open(data_dir() / "tx.log", "a", buffering=1, encoding="utf-8")
        sys.stdout = sys.stderr = f


def transcript_main(argv: list[str]) -> None:
    ap = argparse.ArgumentParser(
        prog="tx",
        description="Print a video's transcript (any yt-dlp-supported URL), or ask an agent about it.",
        epilog="Watcher: tx app | tx serve | tx check | tx service install|uninstall",
    )
    ap.add_argument("url")
    ap.add_argument("-a", "--ask", metavar="QUESTION", help="one-shot question to $TX_AGENT (default: claude -p)")
    ap.add_argument("-c", "--chat", action="store_true", help="open an interactive $TX_CHAT_AGENT (default: claude) session on the transcript")
    ap.add_argument("-l", "--lang", help="caption/audio language code, e.g. en, cs (default: video language, then en)")
    ap.add_argument("-w", "--whisper", action="store_true", help="skip platform captions; transcribe audio locally")
    ap.add_argument("-m", "--model", default=os.environ.get("TX_WHISPER_MODEL", "small"), help="whisper model (default: small)")
    ap.add_argument("-T", "--no-timestamps", action="store_true", help="omit [mm:ss] paragraph markers")
    ap.add_argument("-r", "--refresh", action="store_true", help="ignore cached transcript")
    args = ap.parse_args(argv)
    if args.ask and args.chat:
        ap.error("--ask and --chat are mutually exclusive")

    try:
        t, cache_path = cache.load(args.url, args.lang, args.whisper, args.model, args.refresh)
    except KeyboardInterrupt:
        raise SystemExit(130)
    except Exception as e:  # yt-dlp DownloadError etc.: one line, not a traceback
        raise SystemExit(f"tx: {e}")
    text = render(t, timestamps=not args.no_timestamps)

    if args.ask:
        prompt = (
            f'stdin is the transcript of the video "{t.title}" ({t.url}). '
            "Answer from the transcript; cite [mm:ss] timestamps where useful.\n\n"
            f"Question: {args.ask}"
        )
        cmd = agent.command("TX_AGENT", "claude -p")
        raise SystemExit(subprocess.run([*cmd, prompt], input=text, text=True, encoding="utf-8").returncode)

    if args.chat:
        md = cache_path.with_suffix(".md")
        md.write_text(text, encoding="utf-8")
        prompt = (
            f'Read the transcript of the video "{t.title}" at {md}. '
            "Reply with a 5-bullet summary, then answer my questions from it, citing [mm:ss] timestamps."
        )
        raise SystemExit(subprocess.run([*agent.command("TX_CHAT_AGENT", "claude"), prompt]).returncode)

    _page(text)


def command_main(argv: list[str]) -> None:
    ap = argparse.ArgumentParser(prog="tx", description="Watch YouTube channels and get TL;DRs of new videos.")
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name, help_ in (("app", "open the tx window (starts the background server if needed)"), ("serve", "run web UI + watcher in the foreground")):
        p = sub.add_parser(name, help=help_)
        p.add_argument("--port", type=int, default=PORT)
    sub.add_parser("check", help="check watched channels and analyze new videos once, then exit")
    p = sub.add_parser("service", help="start tx at login and add a 'tx' app to the OS app menu")
    p.add_argument("action", choices=("install", "uninstall"))
    p.add_argument("--port", type=int, default=PORT)
    args = ap.parse_args(argv)

    if args.cmd in ("app", "serve"):
        _log_to_file_if_windowless()
    if args.cmd == "app":
        from .service import open_app

        open_app(args.port)
    elif args.cmd == "serve":
        from .web import serve

        serve(args.port)
    elif args.cmd == "check":
        from . import db, notify, watch
        from .service import ui_url

        conn = db.connect()
        print(f"tx: {watch.poll(conn)} new video(s) queued", file=sys.stderr)
        print(f"tx: {watch.process_queue(conn, ui_url())} analyzed", file=sys.stderr)
        notify.flush()
    elif args.cmd == "service":
        from . import service

        paths = service.install(args.port) if args.action == "install" else service.uninstall(args.port)
        verb = "installed" if args.action == "install" else "removed"
        print("\n".join(f"tx: {verb} {p}" for p in paths))
        if args.action == "install":
            print(f"tx: running at {service.ui_url(args.port)}; open the 'tx' app from your app menu")


def main() -> None:
    argv = sys.argv[1:]
    if argv and argv[0] in COMMANDS:
        command_main(argv)
    else:
        transcript_main(argv)
