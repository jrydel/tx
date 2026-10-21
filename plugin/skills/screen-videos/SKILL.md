---
name: screen-videos
description: Check the YouTube channels watched in tx for new uploads and analyze each against the user's profile (0-5 score, TL;DR, timestamped takeaways, whether to watch), then write a digest. Use when the user asks what's new or worth watching from their channels, wants a video TL;DR, or when running on a schedule.
---

# Screen new videos with tx

tx (MCP server `tx`) fetches YouTube data on this computer; you do the analysis. Follow these steps.

1. Call `get_profile`. It says who the user is and what they want to learn. If it is empty and you can ask the user, ask for 2-3 sentences and save them with `set_profile`; otherwise judge usefulness for a curious technical generalist.
2. Call `check_new_videos`. It checks watched channels and returns uploads waiting for analysis. If no channels are watched, tell the user to add some (`watch_channel` takes `@handle`, a channel URL, or one of its video URLs) and stop.
3. For each waiting video, oldest first, at most 15 per run:
   - Call `get_transcript` with the video id. Long transcripts come in parts; fetch every part.
   - If there is no usable transcript, call `skip_video` with the reason and move on.
   - Otherwise call `save_analysis` with:
     - `relevance` 0-5 for this person: 0 irrelevant, 1 barely, 2 some general interest, 3 useful, 4 clearly valuable, 5 must-watch.
     - `verdict`: one sentence on why, for this person.
     - `tldr`: 2-3 sentences with the core content or claim.
     - `takeaways`: 3-7 concrete, specific points this person should know or act on, each with `at`, the `mm:ss` of the transcript paragraph where it is said. For an irrelevant video give only the 1-2 most generally useful points.
     - `watch`: `advice` is `skip`, `skim` (with `from`/`to` timestamps of the part worth seeing) or `watch`, plus a one-sentence `reason`.
4. Call `write_digest`. Then reply with a short summary: the videos worth the user's time (score, title, one-line verdict, link), and how many others were analyzed.

Rules: use only the transcript, never guess content. Take timestamps from the transcript's `[mm:ss]` paragraph markers. Write in the language the profile is written in. Be specific: numbers, names, and concrete techniques beat general statements.
