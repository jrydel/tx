# tx

**Turn videos and podcasts into knowledge you keep.**

Paste a link into your terminal. tx gets the transcript, your chosen AI extracts what matters, and you can save the useful takeaways as a file on your own device.

- **Decide what to watch:** a colored verdict, relevance score, recommended watch ranges, and timestamped takeaways.
- **Ask your own question:** focus the analysis on your work or interests.
- **Keep what you learn:** save a Markdown note, review it, and add it to your own knowledge base.

Open source, MIT licensed. Runs locally on macOS, Linux, and Windows. No tx account or hosted tx service; AI analysis uses your installed agent and its account.

## Quick start

For analysis, first install and sign in to **[Codex CLI](https://learn.chatgpt.com/docs/codex/cli)** (`codex`) or **[Claude Code](https://code.claude.com/docs/en/quickstart)** (`claude`). You only need one. Fetching a transcript without AI does not require either.

### 1. Install

**macOS (Apple Silicon) or Linux (x86_64):**

```sh
curl -fsSL https://raw.githubusercontent.com/jrydel/tx/master/install.sh | sh
```

**Windows (x64), in PowerShell:**

```powershell
irm https://raw.githubusercontent.com/jrydel/tx/master/install.ps1 | iex
```

Open a new terminal if the installer added tx to your PATH, then check:

```sh
tx --version
```

Run in a terminal, the installer then starts the guided setup (`tx onboard`): your AI apps, what you want to learn (tx can learn it from your coding-agent sessions), what to follow, and a daily brief. **You can skip it with Ctrl+C for the single-link commands below**, or set `TX_NO_SETUP=1` before installing. No daily brief, plugin, or environment variable is required.

### 2. Try a video

Copy this example, then replace the quoted URL with your own:

```sh
tx "https://www.youtube.com/watch?v=USeyDcWyVCc" -q
```

`-q` asks: **“Is this video worth watching? Which parts should I watch, and why?”** It automatically selects an installed agent: Claude first, then Codex, unless you have configured an environment override.

Prefer Codex explicitly?

```sh
tx "https://www.youtube.com/watch?v=USeyDcWyVCc" -g codex -q
```

Or ask a question relevant to you:

```sh
tx "https://www.youtube.com/watch?v=USeyDcWyVCc" -g codex -q "As a software engineer, what matters here?"
```

The result includes a 0–5 relevance score, a WATCH / SKIM / SKIP verdict, timestamped takeaways, and a TL;DR. A timeline is shown when the source provides a duration. This is a text view, not an embedded video player.

**Click to watch:** in a terminal that supports OSC 8 hyperlinks, Ctrl+click a takeaway timestamp, either end of the recommended watch range, or a position on the timeline to open that moment in your browser. In iTerm2 on macOS, use Cmd+click. The links remain usable after tx exits.

Timestamp links use known URL conventions for YouTube, Vimeo, Twitch VODs, Dailymotion, and direct media files. Playback still depends on the source being accessible and its player honoring the offset. Other sites keep a clickable source URL without pretending to support seeking.

### 3. Save takeaways to your own notes

Use `-a` for a free-form answer and `>` to save it:

```sh
tx "https://www.youtube.com/watch?v=USeyDcWyVCc" -g codex -a "3 timestamped takeaways as Markdown. Separate claims from evidence. Include the source URL." > video-notes.md
```

Open the file:

```sh
cat video-notes.md
```

The note is saved in your terminal's current folder. `>` replaces an existing file with that name, so choose a new filename for each video you want to keep. The commands above also work in PowerShell.

Review the generated note before moving it into your knowledge base. **tx does not automatically add it to a memory system.** A single-link review also does not follow the channel or add an analysis to your daily brief.

## Terminal reference

In this table, replace `URL` with a quoted video or episode link.

| I want to… | Command |
| --- | --- |
| Read the transcript, without AI | `tx URL` |
| Decide whether to watch | `tx URL -q` |
| Ask a specific question | `tx URL -a "What should I learn from this?"` |
| Have an interactive conversation | `tx URL -c -g codex` |
| Use Claude Haiku | `tx URL -q -g claude:haiku` |
| Save the transcript | `tx URL > transcript.txt` |
| Fetch a fresh transcript | `tx URL -r` |
| Transcribe audio locally instead of using captions | `tx URL -w -m base` |
| See every option | `tx --help` |

**Keep the URL first.** `-q` takes an optional question; `-a` requires one. Choose one analysis mode per command: `-q`, `-a`, or `-c`. Running `tx` with no URL starts optional onboarding on first use; it is not a video review.

### Choose your AI and model

| Argument | Uses |
| --- | --- |
| `-g codex` | Codex with its configured default model |
| `-g claude` | Claude Code with its configured default model |
| `-g claude:haiku` | Claude Code with Haiku |
| `-g codex:MODEL` | The exact model name your Codex installation accepts |
| No `-g` | `TX_AGENT` if set (`TX_CHAT_AGENT` for `-c`), otherwise Claude first, then Codex |

`-g` / `--agent` overrides the environment settings. You do not need to set `TX_AGENT` or write `codex exec` yourself. Model names pass through unchanged; tx does not create aliases or guarantee that your provider offers a particular model.

**`-m` is different:** it selects the local Whisper transcription model, not the AI that analyzes the transcript. Choices: `tiny`, `base`, `small` (default), `medium`, `large-v3-turbo`.

### Colors, caching, and waiting

- Reviews are colorized in an interactive terminal; redirected output is plain text.
- Use `-C always` with `-q` to force colors for a recording, or `-C never` to disable them. `NO_COLOR=1` and `TERM=dumb` disable automatic colors.
- Clickable links are independent of color: `NO_COLOR` and `-C never` can still leave links active. Links are omitted from pipes/files and when `TERM=dumb`; forcing color does not force links into saved output.
- Transcripts are cached. Asking another question about the same URL reuses the transcript but runs a fresh AI analysis. `-r` bypasses the transcript cache.
- If captions are unavailable, tx downloads the audio and transcribes it locally with [Whisper](https://github.com/ggml-org/whisper.cpp). The first use downloads the model; long recordings take longer than captioned videos.
- `-q` cannot be combined with `-a`, `-c`, or `-T` (omit transcript timestamps).

## Sources: more than YouTube

tx uses [yt-dlp](https://github.com/yt-dlp/yt-dlp/blob/master/supportedsites.md) to handle links from many video and audio sites. It tries platform captions first, then local audio transcription. **An upstream extractor is not a guarantee that every link works.** Authentication, DRM, regional restrictions, and site changes can prevent extraction.

Actual checks with tx 0.18.1 on 2026-10-26:

| Source | What was checked |
| --- | --- |
| [YouTube example](https://www.youtube.com/watch?v=USeyDcWyVCc) | Real AI review and local-note workflows using a cached transcript |
| [InfoQ: Rust talk](https://www.infoq.com/presentations/rust/) | Downloaded and locally transcribed the full 48-minute talk |
| [Apple Podcasts: 60-Second Tech](https://podcasts.apple.com/us/podcast/smart-keyboard-gets-a-charge-out-of-you/id448935669?i=1000363227999) | Downloaded and locally transcribed a 90-second episode |
| Vimeo | Tested page required login; public-player attempt failed on a DRM-protected format. Not verified as working |

Other upstream sources include Twitch, X, TikTok, Dailymotion, SoundCloud, and Internet Archive. Those are backend coverage, not additional end-to-end checks. Single-link support does not imply automatic channel-following support on every platform.

## What stays on your device?

Transcript fetching, caching, and Whisper transcription run on your computer. Files you save with `>` stay where you put them, subject to any folder-sync services you use.

**AI analysis is not necessarily local.** Codex or Claude receives the transcript and may send it to its cloud service. Your provider's authentication, plan limits, and usage terms apply. Generated takeaways can be wrong; keep the source and timestamps so you can check them.

`tx learn` reads your coding-agent sessions on your computer and sends only redacted excerpts of your own prompts to the agent you choose; see [Learn who you are](#learn-who-you-are). Nothing is read until you run it.

## Optional: followed channels and chat apps

You do not need this section to review a link or save a note.

Run `tx onboard` for the guided setup: connect an AI app, describe what you want to learn, choose YouTube channels or podcasts, and decide whether to enable a daily brief. Scheduling is optional. Once configured, `tx` alone shows your status.

Restart your AI app after setup, then ask:

> Show me my tx brief.

To connect only an integration, choose the command for your app:

| App | Command |
| --- | --- |
| Codex | `tx setup codex` |
| Claude Code | `tx setup claude-code` |
| Claude Desktop | `tx setup claude-desktop` |

Claude Desktop is a chat integration; the terminal analysis commands above need a Codex or Claude Code executable.

<details>
<summary>Follow channels, import subscriptions, or use plugin marketplaces</summary>

Ask your connected AI:

> Use tx: follow Latent Space and Practical AI.

Podcasts can be found by name through Apple's directory or added by RSS feed URL. Import an OPML export from Pocket Casts, Overcast, AntennaPod, or another podcast app to bring your subscriptions over.

YouTube subscriptions can be imported from a signed-in browser or [Google Takeout](https://takeout.google.com): deselect everything, select **YouTube and YouTube Music**, then select only **subscriptions** and export. Give the resulting ZIP to your connected AI.

Only uploads published after you follow a channel are screened automatically. To analyze older material explicitly:

> Use tx: TL;DR of the latest 3 Fireship videos.

Published podcast transcripts are used when available. Otherwise episodes are transcribed locally; the agent can pass on an off-profile episode from its title and show notes before transcribing.

Instead of `tx setup`, you can install the plugin:

- **Codex:** `codex plugin marketplace add jrydel/tx`, then `codex plugin add tx@tx`.
- **Claude Code:** in a session, run `/plugin marketplace add jrydel/tx`, then `/plugin install tx@tx`.

Use either the plugin or `tx setup`, not both. The app must be able to find `tx` on its PATH.

Run `tx onboard` to change your setup. `tx reset` starts over; **`tx reset --all` also deletes saved analyses, digests, and cached transcripts.**

</details>

## Every morning, automatically

After configuring what to follow, run a screening pass with `tx run`, or enable a daily schedule:

```sh
tx schedule --at 08:00
```

| Task | Command |
| --- | --- |
| Another time or agent | `tx schedule --at 07:30 --agent codex` |
| Every few hours | `tx schedule --every 3h` |
| See the schedule | `tx schedule --status` |
| Stop it | `tx schedule --off` |
| Write a digest of new analyses | `tx digest` |

The OS scheduler runs `tx run`: a systemd user timer on Linux, a LaunchAgent on macOS, or Task Scheduler on Windows. Your agent checks followed channels and podcasts; tx writes a Markdown digest and sends a desktop notification if something meets your score threshold (3 by default). Your computer must be on; missed runs catch up at the next start on Linux and macOS.

To read digests on another device, point them at a folder you already sync. For example, on macOS:

```sh
tx digest --dir "$HOME/Library/Mobile Documents/com~apple~CloudDocs/tx"
```

`--dir` changes where future digests are saved. It does not configure a sync service.

## Learn who you are

Scores are only as good as tx's picture of you. Instead of describing yourself, let tx learn it from what you already tell your coding agents:

```sh
tx learn
```

tx reads your Claude Code, Codex and Oh My Pi sessions from the last 30 days on your computer, shows you which projects it found, and asks before sending anything. Your agent then writes a short profile: what you're working on now and with which stack, what you're strong at, what you'd likely want to keep up with, and what you already know. You see it before it's saved.

From then on, every followed video and episode is scored for that, and takeaways name the project a moment matters for ("for tx's transcript cache: …"). `tx run` refreshes the profile every 7 days, so the daily brief follows your work as it changes. Your own description from `tx onboard` stays first and wins where the two disagree.

| Task | Command |
| --- | --- |
| See exactly what would be sent, without sending it | `tx learn --dry-run` |
| Another window or agent | `tx learn --days 14 --agent codex` |
| See what tx learned | `tx learn --show` |
| Forget it and stop refreshing | `tx learn --off` |

What is read and sent:

- **Only what you typed.** Agent replies, tool output, file contents, injected context, subagent sessions and non-interactive runs (`claude -p`, `codex exec`, including tx's own) are skipped.
- **Redacted and cut short.** API keys, tokens, passwords, private keys, credentials in URLs and email addresses are replaced, and each prompt is cut to 400 characters. About 40 KB of excerpts, most active projects first.
- **To the agent you choose.** The excerpts go to Claude or Codex like a transcript does. Claude Code sessions sent to Codex (or the other way round) reach a provider that hasn't seen them before; pick the agent accordingly.
- **Work, not your private life.** The agent is told to describe your work and learning only, and to leave out health, family, money, relationships, politics, other people's names and any secrets.

**In your AI app.** Once tx is connected (`tx onboard` or `tx setup …`), Claude Desktop, Claude Code, the Codex app and the ChatGPT desktop app can do the same without a terminal: pick **Learn who I am** from tx's commands, click **Learn it from my coding sessions** in the setup panel, or ask:

> Use tx: learn who I am from my coding-agent sessions.

tx reads and redacts your sessions on your computer and hands the excerpts to the app's own model, which writes the profile, saves it and shows it to you. The excerpts go to that app's provider (Anthropic in Claude apps, OpenAI in Codex and ChatGPT). With a daily brief, `tx run` keeps it fresh; without one, the app offers to refresh it once it's a week old. The ChatGPT desktop app [shares Codex's MCP configuration](https://learn.chatgpt.com/docs/extend/mcp?surface=app), so `tx setup codex` connects both; ChatGPT on the web only uses remote tools, so tx isn't available there.

Cursor's chat history is stored in a different format and isn't read yet.

## Troubleshooting

| Problem | What to do |
| --- | --- |
| `tx: command not found` | Open a new terminal after installation; check that the install directory printed by the installer is on your PATH |
| No agent found, or authentication fails | Install and sign in to Codex CLI or Claude Code; use `-g codex` or `-g claude` to select the one you configured |
| Model rejected | Use a model your provider accepts, or omit the model suffix to use its default |
| Nothing prints while saving a note | `>` redirects the answer into the file; wait for the command to finish, then open the file |
| A link requires login or reports DRM | That link cannot be processed by the current invocation; do not assume all links on the platform work |
| Timestamps do not open when clicked | Use Ctrl+click (Cmd+click in iTerm2), in an OSC 8-capable terminal. Unsupported terminals still show readable text; try outside tmux/screen if they strip links |
| Want the latest version | Rerun the installer above, then check `tx --version` |

## MCP tools

`tx mcp` is a standard stdio MCP server, so any MCP client works. It also offers commands (MCP prompts) apps show in their menu: *Set up tx*, *Learn who I am*, *Today's brief*, *Follow channels or podcasts*, *Import my subscriptions*, *TL;DR a video or episode*. Tools:

| Tool | Does |
|---|---|
| `status` | where the user stands and the next step to guide them to |
| `onboarding` | the setup panel: about you, what to follow (checklist with the AI's picks), daily brief |
| `set_daily_brief` | turn the daily brief on or off |
| `get_profile` / `set_profile` | what the user wants to learn; `get_profile` includes what tx learned from their sessions |
| `learn_from_sessions` / `save_learned_profile` | redacted excerpts of what the user typed to their coding agents, for the app's model to write the learned profile; then save it (empty forgets it) |
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

A SQLite database and digests in `~/.local/share/tx` (Linux), `~/Library/Application Support/tx` (macOS) or `%LOCALAPPDATA%\tx` (Windows), plus the downloaded narrator voice in its `tts` folder. Transcripts and spoken lines are cached in the OS cache folder. A profile learned with `tx learn` is stored in the same database; `tx learn --off` or `tx reset` deletes it.

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
