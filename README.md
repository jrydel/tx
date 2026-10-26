# tx

Don't watch the video. Read it, then decide.

tx watches the YouTube channels and podcasts you pick. Every new video and episode is transcribed and screened against a description of what *you* want to learn: a 0–5 score, a TL;DR, takeaways at their timestamps, and whether it's worth your time at all. Each morning you get a brief of what's worth watching or hearing.

**Your own AI does the thinking, on your existing subscription.** tx runs on your computer and fetches channels and transcripts (on your own internet connection, which YouTube doesn't block). Claude Code, Claude Desktop or Codex (ChatGPT) reads them and writes the analysis. No accounts, no servers, no API keys, no cost beyond the subscription you already have.

Open source under the MIT license. macOS, Linux and Windows.

## TL;DR: get it running

You need the **Codex** (ChatGPT) or **Claude** app, signed in.

**1.** Open a terminal (on a Mac: <kbd>⌘</kbd> <kbd>Space</kbd>, type *Terminal*, <kbd>Enter</kbd>) and paste:

```sh
curl -fsSL https://raw.githubusercontent.com/jrydel/tx/master/install.sh | sh
```

On Windows, paste this into PowerShell instead:

```powershell
irm https://raw.githubusercontent.com/jrydel/tx/master/install.ps1 | iex
```

**2.** Type `tx` and press <kbd>Enter</kbd>. Answer its four questions (about 2 minutes): which AI app, what you want to learn, what to follow, and whether you want a morning brief.

**3.** Quit your AI app completely, open it again, start a new chat and type:

> Show me my tx brief

That's it. From then on, ask for your brief whenever you like, or paste any video link and say "TL;DR this".

## Get started, in detail

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

This installs one `tx` program. Then type `tx` to set it up with you, in four short steps:

1. **Connect your AI**: the apps it finds (Codex / ChatGPT, Claude Code, Claude Desktop) are pre-ticked.
2. **About you**: who you are and what you want to learn. Every score is against this.
3. **What to follow**: import your YouTube subscriptions (your AI pre-picks the ones that fit you), a Google Takeout export, a podcast app's OPML file, or add channels and podcasts by name.
4. **Daily brief**: every morning new uploads are screened and a notification says what's worth your time.

Run `tx onboard` any time to change something, or `tx reset` to start over as a new user (`tx reset --all` also deletes your analyses and transcripts). Once you're set up, `tx` alone shows where you stand. Restart your AI app afterwards so it loads tx.

### 2. Use it in your AI app

Start a new chat. If you skipped setup, say **"Set up tx"** (also in the app's command menu): the Codex app and Claude Desktop show the same steps as a setup panel, with your channels as a checklist.

If tx can't read your browser for the import (it needs to be signed in to YouTube), export your subscriptions instead: at [Google Takeout](https://takeout.google.com) click **Deselect all**, tick **YouTube and YouTube Music**, click **All YouTube data included** and keep only **subscriptions**, then export, download, and import the .zip.

Then, whenever you like:

> Use tx to check my channels for new videos and analyze them for me.

Only videos uploaded after you start watching a channel are analyzed automatically, so nothing happens until a channel posts something new. To try it right away:

> Use tx: TL;DR of the latest 3 Fireship videos.

**Podcasts** work the same way:

> Use tx: follow Latent Space and Practical AI.

It finds shows by name in Apple's podcast directory (no account needed) or takes a feed URL. To bring all your shows at once, export an OPML file from your podcast app (Pocket Casts, Overcast, AntennaPod and most others can) and say "import my podcasts from ~/Downloads/podcasts.opml". Many shows publish transcripts, which tx uses as is. Episodes without one are transcribed on your computer, which takes a few minutes for a long episode, so the agent first passes on episodes whose title and show notes clearly don't fit you.

### 3. Optional: change the daily schedule

```sh
tx schedule --at 07:30
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
| `tx URL -q` | colored worth-watching review with the default question and detected agent |
| `tx URL -q "What matters to a software engineer?" -g codex` | personalized terminal review using Codex |
| `tx URL -c -g claude:haiku` | interactive Claude session using Haiku |
| `tx URL \| llm "summarize"` | pipe the transcript anywhere |
| `tx run` | one screening pass now |
| `tx digest` | digest of everything analyzed since the last one |

Transcripts are cached, so asking more questions about the same video only waits for the agent's answer. `-r` fetches the transcript again.

### A video review in your terminal

```sh
tx "https://www.youtube.com/watch?v=USeyDcWyVCc" -g codex \
  -q "I'm a software engineer. Should I care about this video? Which parts should I watch, and why?"
```

No app setup or environment variable is required for this workflow—just an installed, signed-in Codex CLI or Claude Code. `-q` / `--review` shows a 0–5 relevance score, colored verdict, skip/skim/watch advice, proportional timeline, timestamped takeaways, and a TL;DR. Use `tx URL -q` for the default question: “Is this video worth watching? Which parts should I watch, and why?” Put the URL first when omitting the optional question.

Agent selection for `-q`, `-a`, and `-c`:

| Argument | Behavior |
|---|---|
| `-g codex` / `--agent codex` | Codex CLI, with its configured default model |
| `-g codex:MODEL` | Pass `MODEL` to Codex's `--model` option |
| `-g claude:haiku` | Claude Code with Haiku |
| no `-g` | Use `TX_AGENT` (`TX_CHAT_AGENT` for `-c`) if set; otherwise detect Claude first, then Codex |

The explicit `-g` flag overrides environment settings. Model names pass through unchanged; tx does not create aliases or guarantee availability (for example, `codex:luna` works only if your Codex accepts `luna`). `-m` / `--model` still selects the local Whisper transcription model, not the agent model.

The review reuses a cached transcript when available and asks for a fresh analysis of your question. It does not follow the channel or save a new analysis to your brief. `-q` cannot be combined with `-a`, `-c`, or `-T`.

Colors default to `auto` in an interactive terminal; redirected output is plain text. `NO_COLOR=1` or `TERM=dumb` disables automatic colors. Use `-C always` / `--color always` for recordings, or `-C never` to disable colors explicitly. The layout adapts between 40 and 120 columns. This is a text view—not the embedded video player. Existing MCP/chat-app integrations remain available separately.

`-a` remains a streamed free-form answer; `-c` remains an interactive agent session. The analysis runs through your own agent subscription; that agent may send the transcript to its cloud service.

When working from source, build with `cargo build -p tx-cli` and substitute `./target/debug/tx` for `tx` above.

Works with any [yt-dlp-supported](https://github.com/yt-dlp/yt-dlp/blob/master/supportedsites.md) site (YouTube, X, Vimeo, TikTok, podcasts…). Videos without captions are transcribed locally with [whisper.cpp](https://github.com/ggml-org/whisper.cpp) (`-m tiny|base|small|medium|large-v3-turbo`).

## MCP tools

`tx mcp` is a standard stdio MCP server, so any MCP client works. It also offers commands (MCP prompts) apps show in their menu: *Set up tx*, *Today's brief*, *Follow channels or podcasts*, *Import my subscriptions*, *TL;DR a video or episode*. Tools:

| Tool | Does |
|---|---|
| `status` | where the user stands and the next step to guide them to |
| `onboarding` | the setup panel: about you, what to follow (checklist with the AI's picks), daily brief |
| `set_daily_brief` | turn the daily brief on or off |
| `get_profile` / `set_profile` | what the user wants to learn |
| `watch_channel` / `unwatch_channel` / `list_channels` | channels by `@handle`, URL or id, and podcasts by feed URL (several at once) |
| `find_podcasts` | search Apple's podcast directory for a show's feed |
| `import_subscriptions` | import your YouTube subscriptions from a signed-in browser (`chrome`, `firefox`, `chrome:Profile 2`…) or a Google Takeout export, or your podcasts from an OPML file |
| `check_new_videos` | poll watched channels and podcasts (RSS); list uploads waiting for analysis, with show notes for episodes |
| `get_transcript` | timestamped transcript of a tx video or episode or any URL, in parts for long ones; long local transcriptions keep running between calls |
| `save_analysis` / `skip_video` / `pass_video` | store the agent's score, verdict, TL;DR, takeaways and watch advice; retry later; or pass on an off-profile episode without transcribing it |
| `recent_videos` | a channel's latest uploads, including ones from before you watched it |
| `feed` | recently analyzed videos |
| `show_videos` | your brief: the last 7 days of uploads from what you follow, with a debrief across them written by your agent, then videos and episodes grouped Watch / Skim / Skip, each with a timeline of where the value is, TL;DR and takeaways that jump to the moment ([MCP App](https://modelcontextprotocol.io/extensions/apps/overview)); markdown in apps without MCP Apps. With ids it shows just those |
| `write_digest` | markdown digest + notification; asks your agent for a debrief when the week's analyzed videos changed |
| `save_debrief` | store your agent's debrief across the last 7 days' analyzed videos (headline, summary, threads, and a script for the narrated video: one segment per channel) |

Only uploads published after you start watching a channel are screened automatically, so adding a channel doesn't trigger a backlog.

The narrated debrief video plays inside the app: the view gets each YouTube video's file straight from Google's servers (`video_stream`, through yt-dlp) and the narrator's voice, which speaks on your computer (`narrate`): English with [Kokoro](https://github.com/hexgrad/kokoro) through [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx), Czech with [Piper](https://github.com/rhasspy/piper). Both tools are for the view, not the agent. The first English narration downloads sherpa-onnx (about 20-28 MB, by platform) and the Kokoro voice (about 132 MB, 155 MB unpacked) into the data folder; the first Czech one downloads Piper (about 26 MB) and its voice (about 63 MB).

In apps that support MCP Apps (the Codex app, Claude Desktop, claude.ai), results appear as a brief: a list grouped by Watch, Skim and Skip, with the selected video beside it. Its timeline shades the part worth watching and marks each takeaway; click one to open the video at that moment.

## Data

A SQLite database and digests in `~/.local/share/tx` (Linux), `~/Library/Application Support/tx` (macOS) or `%LOCALAPPDATA%\tx` (Windows), plus the downloaded narrator voice in its `tts` folder. Transcripts and spoken lines are cached in the OS cache folder.

## Build from source

Needs Rust, CMake and Clang (for whisper.cpp). The binary ends up at `target/release/tx`.

```sh
git clone https://github.com/jrydel/tx && cd tx && cargo build --release -p tx-cli
```

Run the tests with `cargo test --workspace`.

The brief and the setup panel are a React + shadcn/ui app in `ui/`, built into one file that the binary embeds (`crates/cli/src/videos.html`, committed, so `cargo build` doesn't need it). After changing the view: `cd ui && bun install && bun run build`.

Layout: `crates/core` is the engine, `crates/cli` the `tx` command and MCP server, `plugin/` the Claude Code plugin and skill. Releases are built by `.github/workflows/release.yml` when a `v*` tag is pushed.

## License

MIT
