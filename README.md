# tx

Don't watch the video. Read it, then ask an agent about it.

```sh
tx https://youtu.be/VIDEO_ID                      # transcript in your pager, [mm:ss] paragraphs
tx URL -a "what does he say about pricing?"        # one-shot answer via claude -p
tx URL -c                                          # interactive claude session on the transcript
tx URL | llm "summarize"                           # pipe anywhere
```

Works with any [yt-dlp-supported](https://github.com/yt-dlp/yt-dlp/blob/master/supportedsites.md) URL (YouTube, X, Vimeo, TikTok, podcasts…).

## How it gets the text

1. Platform captions via yt-dlp — manual subtitles first, then auto-captions, in the video's language (or `--lang`). No download, takes seconds.
2. No captions (e.g. X/Twitter clips) → downloads audio and transcribes locally with [faster-whisper](https://github.com/SYSTRAN/faster-whisper) (`small` model by default, `-m medium` for accuracy). The model downloads on first use.

Transcripts are cached in `~/.cache/tx/` keyed by URL, language, and mode, so repeated `--ask` calls are instant. `-r` refetches.

## Install

```sh
uv tool install -e ~/Projects/tx
```

YouTube requires a JavaScript runtime for yt-dlp; deno, node, or bun on `PATH` is used automatically. The Whisper fallback needs `ffmpeg` on `PATH`.

## Options

| Flag | |
|---|---|
| `-a, --ask Q` | one-shot question; agent = `$TX_AGENT` (default `claude -p`), transcript on stdin |
| `-c, --chat` | exec `$TX_CHAT_AGENT` (default `claude`) pointed at the transcript file |
| `-l, --lang CODE` | caption/audio language (`en`, `cs`, …) |
| `-w, --whisper` | skip captions, always transcribe locally |
| `-m, --model NAME` | whisper model (`tiny`…`large-v3`, default `$TX_WHISPER_MODEL` or `small`) |
| `-T, --no-timestamps` | plain paragraphs |
| `-r, --refresh` | ignore cache |

## Develop

```sh
uv run pytest
```
