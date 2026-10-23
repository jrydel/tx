---
name: screen-videos
description: Check the YouTube channels and podcasts watched in tx for new uploads and analyze each against the user's profile (0-5 score, TL;DR, timestamped takeaways, whether to watch), then write a digest. Use when the user asks what's new or worth watching or hearing from their channels and shows, wants a video or episode TL;DR, or when running on a schedule.
---

# Screen new videos with tx

tx (MCP server `tx`) fetches YouTube videos and podcast episodes on this computer; you do the analysis. "Video" below means either. Follow these steps.

When you're talking with the user (not on a schedule), start with `status` and guide them to its next step. While setup isn't finished (no profile, nothing followed), show the `onboarding` panel instead of asking in chat; after they import channels, read `list_channels` and call `onboarding` with `suggest` (the ids that fit their profile, a few words why each).

1. Call `get_profile`. It says who the user is and what they want to learn. If it is empty and you can ask the user, ask for 2-3 sentences and save them with `set_profile`; otherwise judge usefulness for a curious technical generalist.
2. Call `check_new_videos`. It checks watched channels and returns uploads waiting for analysis. If no channels are watched, offer to import the user's YouTube subscriptions (see Choosing channels) or to watch channels they name (`watch_channel` takes `@handle`s, channel URLs, or video URLs), and stop.
3. For each waiting video, oldest first, at most 15 per run:
   - Podcast episodes (`kind` podcast) without `transcript_published` have to be transcribed on the user's computer, which takes minutes for a long episode. Judge them by title and `show_notes` first: if one clearly doesn't fit the profile, call `pass_video` with a one-line reason instead of transcribing it. When in doubt, transcribe.
   - Call `get_transcript` with the video id. Long transcripts come in parts; fetch every part. If it replies that it's still transcribing, continue with other videos and call it again later; keep going until every video is done.
   - If there is no usable transcript, call `skip_video` with the reason and move on.
   - Otherwise call `save_analysis` with:
     - `relevance` 0-5 for this person: 0 irrelevant, 1 barely, 2 some general interest, 3 useful, 4 clearly valuable, 5 must-watch.
     - `verdict`: one sentence on why, for this person.
     - `tldr`: 2-3 sentences with the core content or claim. This is the only place that retells the video.
     - `takeaways`: 3-5 points for this person, each with `at`, the `mm:ss` of the transcript paragraph it comes from. A takeaway is what the profile's owner should know, try, or change because of that moment: name the concrete thing (number, tool, technique, date) and say what it means for them. Don't narrate the video ("The video describes…", "He reports…"); the TL;DR does that. Bad: "Fireship reports introductory pricing of $2 per million input tokens through November 30." Good (for someone tracking AI industry shifts): "Argon's $2 per million input tokens is a launch price that ends November 30, so read it as a land grab, not the new price floor." For an irrelevant video give only the 1-2 points that still matter to them.
     - `watch`: `advice` is `skip`, `skim` (with `from`/`to` timestamps of the part worth seeing) or `watch`, plus a one-sentence `reason`.
4. Call `write_digest`. If its reply ends with analyses and asks for a debrief, call `save_debrief` once over them: `headline` (one sentence), `summary` (2-4 sentences), up to 3 `threads` (`title`, `text`, `video_ids`) and a `script`. Say what the week's videos add up to for this person (their profile), what to watch first, and which themes run through two or more videos; every thread names the ids it runs through. The `script` is a narrated video over the week for this person: `intro` (one sentence framing the week, 160 characters at most), up to 8 `segments`, best first, and a short `outro` (one line). A segment is one YouTube video (no podcast episodes), one per channel: pick the best video of each. It has `video_id`, `intro` (the narrator's takeaway for that video, third person, 200 characters at most: what the creator says and why it matters to this person, e.g. "Theo on this week's security leaks: he argues the real story is stolen API keys, and why that matters for you.") and 3-4 `parts`. Each part is `say` (a narrator line, third person, 160 characters at most, that sets the part up and ends by pointing at it: "He then shows how … — watch the next bit.") and `moment_from`/`moment_to` (`mm:ss`), the 6-25 seconds where it is said, taken from the analyses' takeaway timestamps; parts of a video must not overlap. The viewer reads each `say` as captions while the video runs fast in the background, then plays the part at normal speed with sound. Write as a narrator, no hype: use only what the analyses say, invent no facts, write in the language the profile is written in. If it says no debrief is needed, skip that. If you're talking with the user, then call `show_videos` **without ids**: it shows their brief for the last 7 days with the debrief on top, sorted Watch, Skim, Skip, best score first. Reply in at most two sentences: which video to watch first and why, and how many others were analyzed. Don't repeat what the cards or the debrief show.

## A channel's latest videos, or one video

When the user asks for a TL;DR of specific or recent videos ("the latest Fireship videos"):

1. Call `get_profile` (unless you already have it in this conversation): scores, verdicts and takeaways are always for this person. Then find the videos with `recent_videos` (pass part of the channel name). For a channel tx doesn't know, `watch_channel` it first (the user asked about it, so that counts as choosing it). A video URL from a channel tx doesn't know can't be stored: use `get_transcript` with the URL and write the TL;DR, takeaways for the user (with timestamps) and watch advice in your reply.
2. Videos already analyzed (status `done`) need no new analysis unless the user asks to redo them. For each other one, `get_transcript` and `save_analysis` exactly as in step 3 above, so it's stored and never redone.
3. Call `show_videos` with their ids, then reply in at most two sentences.

## Choosing channels and podcasts

To add podcasts, call `find_podcasts` with the show's name and `watch_channel` with the chosen `feed_url`s. If the user has many podcast subscriptions, ask them to export an OPML file from their podcast app and import it with `import_subscriptions` `file`, then continue with step 2 below.

When the user wants their YouTube subscriptions in tx ("import my subscriptions", "use the channels I follow"):

1. Call `import_subscriptions` with their browser (ask which one if you don't know). If it fails, give the user the Google Takeout steps from the error, wait for them to download the export, and import it with `file` (the .zip path is enough).
2. Call `get_profile`, then `list_channels` with `watching_only` false. Suggest the channels that fit the profile, usually 5-20, one short reason each, and say how many you left out and why. Ask the user to confirm or change the list.
3. Call `watch_channel` once with all chosen channel ids. Watch everything only if the user asks for it: every watched channel's new uploads are analyzed on their subscription.

Rules: use only the transcript, never guess content. Take timestamps from the transcript's `[mm:ss]` paragraph markers. Write in the language the profile is written in. Be specific: numbers, names, and concrete techniques beat general statements. Whenever you present analyzed videos, use `show_videos` rather than writing them out yourself.
