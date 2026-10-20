# tx

Don't watch the video. Read it, then ask an agent about it.

Two ways to use it:

- **App**: pick YouTube channels you follow; every new upload gets a TL;DR scored against a description of what *you* want to learn, plus a desktop notification. Linux, macOS, Windows.
- **Terminal**: `tx URL` prints any video's transcript; `-a` asks your agent a question about it.

## Install

Requires [uv](https://docs.astral.sh/uv/) and [Claude Code](https://docs.claude.com/en/docs/claude-code) (`claude`, logged in) for analysis.

```sh
uv tool install git+https://github.com/jrydel/tx
tx service install     # start at login + add a "tx" app to your app menu
```

YouTube extraction needs a JavaScript runtime on `PATH` (deno, node, or bun).

## App

Open **tx** from your app menu (or run `tx app`). It opens in its own window (Chrome/Edge/Chromium app mode, else your default browser) at `http://127.0.0.1:8765`.

1. **Channels** → *Import my YouTube subscriptions*. Reads the YouTube login of a browser on this machine (`chrome`, `firefox`, `edge`, `safari`, `brave`; pick a profile with `chrome:Profile 2`). Or add channels by `@handle` / URL. Then switch on **Watch** for the ones you care about.
2. **Settings** → describe who you are and what knowledge you want. Every video is scored 0–5 against it.
3. Done. Every 30 minutes (configurable) tx checks watched channels via YouTube's public RSS feeds. New uploads are transcribed, analyzed, and listed under **Videos**: relevance score, one-line verdict, TL;DR, takeaways with clickable `[mm:ss]` timestamps, and whether it's worth watching. Videos at or above your threshold trigger a desktop notification.

Uploads published before you started watching a channel are listed as *Earlier uploads* with an **Analyze** button: nothing is analyzed in bulk without asking.

| | Background service | Notifications |
|---|---|---|
| Linux | systemd user unit `tx.service` + `tx.desktop` | `notify-send`; click opens the analysis |
| macOS | LaunchAgent `dev.tx.watch` + `~/Applications/tx.app` | Notification Center via `osascript` |
| Windows | hidden Startup script + Start Menu shortcut | native toast; click opens the analysis |

`tx service uninstall` removes all of it. `tx check` runs one check/analyze pass in the foreground (handy with cron). `tx serve` runs the app in the foreground.

Data lives in `~/.local/share/tx` (Linux), `~/Library/Application Support/tx` (macOS), or `%LOCALAPPDATA%\tx` (Windows).

**Windows note:** Chrome's app-bound cookie encryption blocks subscription import from Chrome; use Firefox, or add channels by `@handle`.

## Terminal

```sh
tx "https://youtu.be/VIDEO_ID"                     # transcript in your pager, [mm:ss] paragraphs
tx URL -a "what does he say about pricing?"        # one-shot answer via claude -p
tx URL -c                                          # interactive claude session on the transcript
tx URL | llm "summarize"                           # pipe anywhere
```

Works with any [yt-dlp-supported](https://github.com/yt-dlp/yt-dlp/blob/master/supportedsites.md) URL (YouTube, X, Vimeo, TikTok, podcasts…).

| Flag | |
|---|---|
| `-a, --ask Q` | one-shot question; agent = `$TX_AGENT` (default `claude -p`), transcript on stdin |
| `-c, --chat` | run `$TX_CHAT_AGENT` (default `claude`) pointed at the transcript file |
| `-l, --lang CODE` | caption/audio language (`en`, `cs`, …) |
| `-w, --whisper` | skip captions, always transcribe locally |
| `-m, --model NAME` | whisper model (`tiny`…`large-v3`, default `$TX_WHISPER_MODEL` or `small`) |
| `-T, --no-timestamps` | plain paragraphs |
| `-r, --refresh` | ignore cache |

## How it gets the text

1. Platform captions via yt-dlp: manual subtitles first, then auto-captions, in the video's language (or `--lang`). No download, seconds.
2. No captions (e.g. X clips) → downloads audio and transcribes locally with [faster-whisper](https://github.com/SYSTRAN/faster-whisper). The model downloads on first use; ffmpeg comes bundled if you don't have one.

Transcripts are cached per URL, so follow-up questions are instant. The analysis agent is whatever `$TX_AGENT` names (default `claude -p`); it receives the transcript on stdin.

## Develop

```sh
git clone https://github.com/jrydel/tx && cd tx
uv sync && uv run pytest
uv tool install -e .   # live-edit install
```
