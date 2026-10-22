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
4. Call `write_digest`. If you're talking with the user, call `show_videos` with the ids you analyzed, best first. Then reply in at most two sentences: which video to watch first and why, and how many others were analyzed. Don't repeat what the cards show.

## A channel's latest videos, or one video

When the user asks for a TL;DR of specific or recent videos ("the latest Fireship videos"):

1. Find them with `recent_videos` (pass part of the channel name). For a channel tx doesn't know, `watch_channel` it first. A video URL from a channel tx doesn't know can't be stored: use `get_transcript` with the URL and write the TL;DR, takeaways (with timestamps) and watch advice in your reply.
2. Videos already analyzed (status `done`) need no new analysis. For each other one, `get_transcript` and `save_analysis` exactly as in step 3 above, so it's stored and never redone.
3. Call `show_videos` with their ids in the order the user asked for, then reply in at most two sentences.

Rules: use only the transcript, never guess content. Take timestamps from the transcript's `[mm:ss]` paragraph markers. Write in the language the profile is written in. Be specific: numbers, names, and concrete techniques beat general statements. Whenever you present analyzed videos, use `show_videos` rather than writing them out yourself.
