# tx

Don't watch the video. Read it, then decide.

tx watches the YouTube channels you pick. Every new upload is transcribed and screened against a description of what *you* want to learn: a 0–5 score, a TL;DR, takeaways at their timestamps, and whether it's worth watching at all. Each morning you get a digest of what's worth your time.

**Your own AI does the thinking, on your existing subscription.** tx runs on your computer and fetches channels and transcripts (on your own internet connection, which YouTube doesn't block). Claude Code, Claude Desktop or Codex (ChatGPT) reads them and writes the analysis. No accounts, no servers, no API keys, no cost beyond the subscription you already have.

Open source under the MIT license. macOS, Linux and Windows.

## Install

```sh
# macOS (Apple Silicon) and Linux (x86_64)
curl -fsSL https://raw.githubusercontent.com/jrydel/tx/master/install.sh | sh
```

```powershell
# Windows (PowerShell)
irm https://raw.githubusercontent.com/jrydel/tx/master/install.ps1 | iex
```

It installs one `tx` binary. On first use it downloads the official [yt-dlp](https://github.com/yt-dlp/yt-dlp) and [QuickJS](https://github.com/quickjs-ng/quickjs) builds into its data folder and keeps yt-dlp current.

## Connect your AI

| You have | Run | Then ask |
|---|---|---|
| **Claude Code** | `tx setup claude`, or install the plugin: `/plugin marketplace add jrydel/tx` then `/plugin install tx@tx` | "watch @fireship and @t3dotgg in tx", "screen my new videos" |
| **Claude Desktop** | `tx setup claude-desktop`, then restart Claude | same |
| **ChatGPT** (via [Codex CLI](https://github.com/openai/codex)) | `tx setup codex` | same, in `codex` |

The first time, tell it who you are and what you want to learn ("I'm a backend engineer building LLM features; I want concrete agent and eval techniques; skip hype"). tx stores that profile and scores every video against it.

The Claude Code plugin also adds the **screen-videos** skill, so Claude follows the same procedure every time.

## Every morning, automatically

```sh
tx schedule                    # daily at 08:00 with whichever agent is installed
tx schedule --at 07:30 --agent codex
tx schedule --every 3h
tx schedule --status
tx schedule --off
```

This uses the OS scheduler (a systemd user timer on Linux, a LaunchAgent on macOS, Task Scheduler on Windows) to run `tx run`: your agent checks the watched channels, analyzes new uploads, and tx writes a markdown digest. If something scores at or above your bar (3 by default), you get a desktop notification. Your computer has to be on; a missed run catches up at the next start on Linux and macOS.

**Read it on your phone:** point the digests at a synced folder and open them in any notes app.

```sh
tx digest --dir ~/Library/Mobile\ Documents/com~apple~CloudDocs/tx    # iCloud Drive
tx digest --dir ~/OneDrive/tx                                          # OneDrive, Dropbox, Drive…
```

Follow-up questions are just a normal conversation with your AI: "in the Gemini video, what exactly did he say about pricing?"

## Terminal

```sh
tx "https://youtu.be/VIDEO_ID"                   # transcript in your pager, [mm:ss] paragraphs
tx URL -a "what does he say about pricing?"      # one-shot answer via claude -p
tx URL -c                                        # interactive claude session on the transcript
tx URL | llm "summarize"                         # pipe anywhere
tx run                                           # one screening pass now
tx digest                                        # digest of everything analyzed since the last one
```

Works with any [yt-dlp-supported](https://github.com/yt-dlp/yt-dlp/blob/master/supportedsites.md) site (YouTube, X, Vimeo, TikTok, podcasts…). Videos without captions are transcribed locally with [whisper.cpp](https://github.com/ggml-org/whisper.cpp) (`-m tiny|base|small|medium|large-v3-turbo`).

## MCP tools

`tx mcp` is a standard stdio MCP server, so any MCP client works. Tools:

| Tool | Does |
|---|---|
| `get_profile` / `set_profile` | what the user wants to learn |
| `watch_channel` / `unwatch_channel` / `list_channels` | channels by `@handle`, URL or id |
| `import_subscriptions` | read YouTube subscriptions from a signed-in browser (`chrome`, `firefox`, `chrome:Profile 2`…) |
| `check_new_videos` | poll watched channels (RSS); list uploads waiting for analysis |
| `get_transcript` | timestamped transcript of a tx video or any URL, in parts for long videos |
| `save_analysis` / `skip_video` | store the agent's score, verdict, TL;DR, takeaways and watch advice |
| `feed` | recently analyzed videos |
| `write_digest` | markdown digest + notification |

Only uploads published after you start watching a channel are screened automatically, so adding a channel doesn't trigger a backlog.

## Desktop app (optional)

A desktop app with a feed, filters, film-strip timelines and per-video chat is available from [Releases](https://github.com/jrydel/tx/releases/latest). It shares the same data, so channels and analyses show up in both. It updates itself from GitHub Releases.

## Data

A SQLite database and digests in `~/.local/share/tx` (Linux), `~/Library/Application Support/tx` (macOS) or `%LOCALAPPDATA%\tx` (Windows). Transcripts are cached in the OS cache folder.

## Build from source

Needs Rust, CMake and Clang (for whisper.cpp); [bun](https://bun.sh) and [Tauri's dependencies](https://v2.tauri.app/start/prerequisites/) only for the desktop app.

```sh
git clone https://github.com/jrydel/tx && cd tx/src-tauri
cargo build --release -p tx-cli        # target/release/tx
cargo test --workspace
```

Layout: `src-tauri/core` is the engine, `src-tauri/cli` the `tx` command and MCP server, `plugin/` the Claude Code plugin and skill, `src/` + `src-tauri/src` the optional desktop app. Releases are built by `.github/workflows/release.yml` when a `v*` tag is pushed.

## License

MIT
