# tx

Don't watch the video. Read it, then decide.

tx watches the YouTube channels you pick. Each new upload is transcribed and scored 0–5 against a description of what *you* want to learn, with a TL;DR, takeaways at their timestamps, and whether it's worth watching at all. Uploads that clear your bar send a desktop notification. The rest wait in a list you can skim.

It also works one-off from a terminal: `tx URL` prints any video's transcript.

## Install

Download the installer for your system from [Releases](https://github.com/jrydel/tx/releases/latest):

| System | File |
|---|---|
| macOS (Apple Silicon) | `tx_*_aarch64.dmg` |
| Windows | `tx_*_x64-setup.exe` or `.msi` |
| Linux | `.deb` (Debian/Ubuntu), `.rpm` (Fedora), `.AppImage` (anything) |

tx uses [Claude Code](https://claude.com/claude-code) to analyze videos, on your own subscription: install it and sign in once in a terminal (`claude`).

The installers aren't code-signed. On macOS, open tx once, then allow it under System Settings → Privacy & Security. On Windows, SmartScreen asks you to confirm.

## Use

1. **Channels** → *Import subscriptions* reads the YouTube login of a browser on this computer (`chrome`, `firefox`, `edge`, `safari`, `brave`; another profile as `chrome:Profile 2`). Or add any channel by `@handle` or URL. Switch on **Watch** for the ones you care about.
2. **Settings** → describe who you are and what you want to learn. Write it in the language you want analyses in.
3. That's it. tx checks watched channels every 30 minutes (from YouTube's public RSS feeds, no login needed) and lives in the tray; closing the window keeps it running. It starts at login unless you turn that off in Settings.

Each analyzed video gets a film strip: the whole video as one strip, with a yellow mark wherever a takeaway is said and a band over the part worth skimming. Click a mark to jump there.

Uploads from before you started watching a channel are listed under *Earlier uploads*, with an **Analyze** button; nothing is analyzed in bulk without you asking.

## Terminal

The installers include the `tx` command (also attached to each release as `tx-cli-<platform>`).

```sh
tx "https://youtu.be/VIDEO_ID"                   # transcript in your pager, [mm:ss] paragraphs
tx URL -a "what does he say about pricing?"      # one-shot answer via claude -p
tx URL -c                                        # interactive claude session on the transcript
tx URL | llm "summarize"                         # pipe anywhere
tx check                                         # one watch-and-analyze pass, like the app does
```

Works with any [yt-dlp-supported](https://github.com/yt-dlp/yt-dlp/blob/master/supportedsites.md) site (YouTube, X, Vimeo, TikTok, podcasts…).

| Flag | |
|---|---|
| `-a, --ask Q` | one-shot question; agent = `$TX_AGENT` (default `claude -p`), transcript on stdin |
| `-c, --chat` | run `$TX_CHAT_AGENT` (default `claude`) pointed at the transcript file |
| `-l, --lang CODE` | caption/audio language (`en`, `cs`, …) |
| `-w, --whisper` | skip captions, always transcribe locally |
| `-m, --model NAME` | `tiny`, `base`, `small` (default), `medium`, `large-v3-turbo`; or `$TX_WHISPER_MODEL` |
| `-T, --no-timestamps` | plain paragraphs |
| `-r, --refresh` | ignore the cached transcript |

## How it works

- **Transcripts:** platform captions first (manual, then auto-generated, in the video's language), fetched by a bundled [yt-dlp](https://github.com/yt-dlp/yt-dlp) with [QuickJS](https://github.com/quickjs-ng/quickjs) as its JavaScript runtime. Videos without captions (e.g. X clips) are transcribed on your computer with [whisper.cpp](https://github.com/ggml-org/whisper.cpp), compiled into tx (Metal GPU on Apple Silicon). Models download on first use.
- **Staying current:** YouTube breaks old yt-dlp releases regularly, so tx downloads the latest official yt-dlp once a day into its data folder.
- **Analysis:** `claude -p` with your profile and the transcript. Nothing is sent anywhere else.
- **Data:** a SQLite database in `~/.local/share/tx` (Linux), `~/Library/Application Support/tx` (macOS), or `%LOCALAPPDATA%\tx` (Windows).

## Build from source

Needs [bun](https://bun.sh), Rust, CMake and Clang (for whisper.cpp), plus [Tauri's system dependencies](https://v2.tauri.app/start/prerequisites/).

```sh
git clone https://github.com/jrydel/tx && cd tx
bun install
bun run sidecars          # downloads yt-dlp + QuickJS, builds the tx CLI into src-tauri/binaries/
bun run tauri dev         # the desktop app with hot reload
bun run tauri build       # installers for this platform (add -- --features metal on Apple Silicon)
cd src-tauri && cargo test --workspace
```

Layout: `src/` is the React + shadcn UI, `src-tauri/core` the engine (no Tauri), `src-tauri/cli` the `tx` command, `src-tauri/src` the desktop shell (window, tray, commands). Releases are built by `.github/workflows/release.yml` when a `v*` tag is pushed.
