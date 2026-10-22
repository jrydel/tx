# tx

Don't watch the video. Read it, then decide.

tx watches the YouTube channels you pick. Every new upload is transcribed and screened against a description of what *you* want to learn: a 0–5 score, a TL;DR, takeaways at their timestamps, and whether it's worth watching at all. Each morning you get a digest of what's worth your time.

**Your own AI does the thinking, on your existing subscription.** tx runs on your computer and fetches channels and transcripts (on your own internet connection, which YouTube doesn't block). Claude Code, Claude Desktop or Codex (ChatGPT) reads them and writes the analysis. No accounts, no servers, no API keys, no cost beyond the subscription you already have.

Open source under the MIT license. macOS, Linux and Windows.

## Get started

You need **Claude Code**, **Claude Desktop** or **Codex** (the ChatGPT desktop app or CLI), signed in with your subscription.

### 1. Install tx

On macOS (Apple Silicon) or Linux (x86_64), run in a terminal:

```sh
curl -fsSL https://raw.githubusercontent.com/jrydel/tx/master/install.sh | sh
```

On Windows, run in PowerShell:

```powershell
irm https://raw.githubusercontent.com/jrydel/tx/master/install.ps1 | iex
```

This installs one `tx` program. Open a new terminal afterwards so it's on your PATH.

### 2. Connect it to your AI

Run the line for the AI you use:

| You use | Run |
|---|---|
| Codex (ChatGPT app or CLI) | `tx setup codex` |
| Claude Code | `tx setup claude-code` |
| Claude Desktop | `tx setup claude-desktop` |

Then restart that app if it's open. This adds tx's tools and the **screen-videos** skill.

### 3. Tell it what you want and which channels to watch

Start a new chat and say, in your own words:

> Use tx. I'm a backend engineer building LLM features; I want concrete agent and eval techniques and notable model launches; skip hype. Watch @fireship and @t3dotgg.

Or let it pick from the channels you already follow:

> Use tx: import my YouTube subscriptions from Chrome and suggest which to watch.

It suggests the channels that fit what you want to learn and watches them once you confirm. If it can't read your browser (it needs to be signed in to YouTube), export your subscriptions instead: at [Google Takeout](https://takeout.google.com) click **Deselect all**, tick **YouTube and YouTube Music**, click **All YouTube data included** and keep only **subscriptions**, then export, download, and tell it where the .zip is.

Then, whenever you like:

> Use tx to check my channels for new videos and analyze them for me.

Only videos uploaded after you start watching a channel are analyzed automatically, so nothing happens until a channel posts something new. To try it right away:

> Use tx: TL;DR of the latest 3 Fireship videos.

### 4. Optional: every morning, automatically

```sh
tx schedule
```

See [Every morning](#every-morning-automatically) for options and reading digests on your phone.

<details>
<summary>Prefer plugin marketplaces?</summary>

Instead of `tx setup`, you can install tx as a plugin (it needs `tx` on the PATH your AI app sees):

- **Codex:** run `codex plugin marketplace add jrydel/tx`, then `codex plugin add tx@tx`
- **Claude Code:** in a session, run `/plugin marketplace add jrydel/tx`, then `/plugin install tx@tx`

Use either the plugin or `tx setup`, not both.
</details>

## Every morning, automatically

Screen new videos every morning at 08:00, with whichever agent is installed:

```sh
tx schedule
```

| Instead | Run |
|---|---|
| another time or agent | `tx schedule --at 07:30 --agent codex` |
| every few hours | `tx schedule --every 3h` |
| see the schedule | `tx schedule --status` |
| stop it | `tx schedule --off` |

This uses the OS scheduler (a systemd user timer on Linux, a LaunchAgent on macOS, Task Scheduler on Windows) to run `tx run`: your agent checks the watched channels, analyzes new uploads, and tx writes a markdown digest. If something scores at or above your bar (3 by default), you get a desktop notification. Your computer has to be on; a missed run catches up at the next start on Linux and macOS.

**Read it on your phone:** point the digests at a synced folder and open them in any notes app.

| Synced folder | Run |
|---|---|
| iCloud Drive (Mac) | `tx digest --dir ~/Library/Mobile\ Documents/com~apple~CloudDocs/tx` |
| OneDrive, Dropbox, Google Drive… | `tx digest --dir ~/OneDrive/tx` (use your sync folder's path) |

Follow-up questions are just a normal conversation with your AI: "in the Gemini video, what exactly did he say about pricing?"

## Terminal

| Command | Does |
|---|---|
| `tx "https://youtu.be/VIDEO_ID"` | transcript in your pager, `[mm:ss]` paragraphs |
| `tx URL -a "what does he say about pricing?"` | one-shot answer from your agent, streamed as it's written |
| `tx URL -c` | interactive Claude session on the transcript |
| `tx URL \| llm "summarize"` | pipe the transcript anywhere |
| `tx run` | one screening pass now |
| `tx digest` | digest of everything analyzed since the last one |

Transcripts are cached, so asking more questions about the same video only waits for the agent's answer. `-r` fetches the transcript again.

Works with any [yt-dlp-supported](https://github.com/yt-dlp/yt-dlp/blob/master/supportedsites.md) site (YouTube, X, Vimeo, TikTok, podcasts…). Videos without captions are transcribed locally with [whisper.cpp](https://github.com/ggml-org/whisper.cpp) (`-m tiny|base|small|medium|large-v3-turbo`).

## MCP tools

`tx mcp` is a standard stdio MCP server, so any MCP client works. Tools:

| Tool | Does |
|---|---|
| `get_profile` / `set_profile` | what the user wants to learn |
| `watch_channel` / `unwatch_channel` / `list_channels` | channels by `@handle`, URL or id (several at once) |
| `import_subscriptions` | import your YouTube subscriptions from a signed-in browser (`chrome`, `firefox`, `chrome:Profile 2`…) or a Google Takeout export |
| `check_new_videos` | poll watched channels (RSS); list uploads waiting for analysis |
| `get_transcript` | timestamped transcript of a tx video or any URL, in parts for long videos |
| `save_analysis` / `skip_video` | store the agent's score, verdict, TL;DR, takeaways and watch advice |
| `recent_videos` | a channel's latest uploads, including ones from before you watched it |
| `feed` | recently analyzed videos |
| `show_videos` | video cards: thumbnail, score, TL;DR, takeaways that jump to the moment ([MCP App](https://modelcontextprotocol.io/extensions/apps/overview)); markdown in apps without MCP Apps |
| `write_digest` | markdown digest + notification |

Only uploads published after you start watching a channel are screened automatically, so adding a channel doesn't trigger a backlog.

In apps that support MCP Apps (the Codex app, Claude Desktop, claude.ai), results appear as cards with thumbnails. Click a timestamp to open the video at that moment.

## Data

A SQLite database and digests in `~/.local/share/tx` (Linux), `~/Library/Application Support/tx` (macOS) or `%LOCALAPPDATA%\tx` (Windows). Transcripts are cached in the OS cache folder.

## Build from source

Needs Rust, CMake and Clang (for whisper.cpp). The binary ends up at `target/release/tx`.

```sh
git clone https://github.com/jrydel/tx && cd tx && cargo build --release -p tx-cli
```

Run the tests with `cargo test --workspace`.

Layout: `crates/core` is the engine, `crates/cli` the `tx` command and MCP server, `plugin/` the Claude Code plugin and skill. Releases are built by `.github/workflows/release.yml` when a `v*` tag is pushed.

## License

MIT
