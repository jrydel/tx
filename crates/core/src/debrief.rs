//! The debrief: what the last week's analyzed videos add up to for the user, written by their own
//! agent after the digest and shown on top of the brief.

use crate::captions::fmt_time;
use crate::db::{self, Video};
use crate::report;
use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The brief and the debrief cover this many days of uploads.
pub const DAYS: i64 = 7;
/// Most videos a week's brief or debrief covers.
pub const MAX_VIDEOS: i64 = 60;
/// Setting that holds the stored debrief as JSON.
const KEY: &str = "debrief";
const MAX_THREADS: usize = 3;
const MAX_HEADLINE: usize = 200;
const MAX_SUMMARY: usize = 800;
const MAX_THREAD_TITLE: usize = 80;
const MAX_THREAD_TEXT: usize = 400;
const MAX_INTRO: usize = 160;
const MAX_SAY: usize = 140;
const MAX_OUTRO: usize = 120;
const MAX_CLIPS: usize = 6;
/// Longest stretch of one video a clip plays, in seconds.
const MAX_CLIP_SECS: u64 = 600;
/// Shortest and longest key moment of a clip, in seconds.
const MIN_MOMENT_SECS: u64 = 4;
const MAX_MOMENT_SECS: u64 = 25;
/// A debrief is only worth writing over this many analyzed videos.
const MIN_VIDEOS: usize = 2;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Thread {
    pub title: String,
    pub text: String,
    /// Videos it runs through; each an analyzed video of the week.
    pub video_ids: Vec<String>,
}

/// One stretch of a YouTube video in the narrated show, with the line spoken before it. The
/// stretch plays muted and fast behind the line; the key moment inside it plays with sound.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Clip {
    pub video_id: String,
    /// "mm:ss" or "h:mm:ss" where the clip starts.
    pub from: String,
    pub to: String,
    /// `from` and `to` in seconds.
    pub start: u64,
    pub end: u64,
    /// The narrator's line, spoken before the clip.
    pub say: String,
    /// Where the key point is said, inside `start`..`end`, 4-25 seconds. None when the agent gave
    /// no usable moment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moment_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moment_to: Option<String>,
    /// `moment_from` and `moment_to` in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moment_start: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moment_end: Option<u64>,
}

/// The narrated show: an intro, one clip per line of narration, an outro.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Script {
    pub intro: String,
    pub clips: Vec<Clip>,
    /// Empty when there is none.
    pub outro: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Debrief {
    pub headline: String,
    pub summary: String,
    pub threads: Vec<Thread>,
    pub script: Option<Script>,
    /// The analyzed videos it was written over, sorted.
    pub video_ids: Vec<String>,
    /// RFC 3339.
    pub written_at: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ClipDraft {
    pub video_id: String,
    pub from: String,
    pub to: String,
    pub say: String,
    pub moment_from: String,
    pub moment_to: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ScriptDraft {
    pub intro: String,
    pub clips: Vec<ClipDraft>,
    pub outro: String,
}

/// The agent's debrief as it arrives, before validation.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Draft {
    pub headline: String,
    pub summary: String,
    pub threads: Vec<Thread>,
    pub script: Option<ScriptDraft>,
}

/// At most `max` characters, ending in "…" when cut.
fn clamp(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max - 1).collect();
    format!("{}…", cut.trim_end())
}

/// The key moment of a clip spanning `start`..`end` seconds as (start, end) seconds, or None when
/// there is no valid one: both timestamps parse, it starts inside the clip and lasts at least 4
/// seconds. A moment running past `end` is cut there, one longer than 25 seconds at 25.
fn clean_moment(from: &str, to: &str, start: u64, end: u64) -> Option<(u64, u64)> {
    let from = report::seconds(from).filter(|f| (start..end).contains(f))?;
    let to = report::seconds(to)?.min(end).min(from + MAX_MOMENT_SECS);
    (to >= from + MIN_MOMENT_SECS).then_some((from, to))
}

/// A validated clip, or None when it isn't one: the video must be an analyzed YouTube video of
/// the week, `from` and `to` timestamps with `to` after `from`, and a line to say. The clip is cut
/// at 10 minutes and at the video's length when known. An invalid key moment is dropped alone.
fn clean_clip(c: ClipDraft, week: &[Video]) -> Option<Clip> {
    let id = c.video_id.trim();
    let video = week.iter().find(|v| v.id == id && !v.podcast)?;
    let start = report::seconds(&c.from)?;
    let mut end = report::seconds(&c.to)?.min(start + MAX_CLIP_SECS);
    if let Some(length) = video.duration.filter(|d| *d > 0.0) {
        end = end.min(length as u64);
    }
    let say = clamp(&c.say, MAX_SAY);
    let moment = clean_moment(&c.moment_from, &c.moment_to, start, end);
    (end > start && !say.is_empty()).then(|| Clip {
        video_id: video.id.clone(),
        from: fmt_time(start as f64),
        to: fmt_time(end as f64),
        start,
        end,
        say,
        moment_from: moment.map(|(from, _)| fmt_time(from as f64)),
        moment_to: moment.map(|(_, to)| fmt_time(to as f64)),
        moment_start: moment.map(|(from, _)| from),
        moment_end: moment.map(|(_, to)| to),
    })
}

/// The agent's script without its invalid clips: at most 8 clips in the agent's order, None when
/// there is no intro or no clip survives.
fn clean_script(draft: ScriptDraft, week: &[Video]) -> Option<Script> {
    let intro = clamp(&draft.intro, MAX_INTRO);
    let clips: Vec<Clip> = draft
        .clips
        .into_iter()
        .filter_map(|c| clean_clip(c, week))
        .take(MAX_CLIPS)
        .collect();
    (!intro.is_empty() && !clips.is_empty()).then(|| Script {
        intro,
        clips,
        outro: clamp(&draft.outro, MAX_OUTRO),
    })
}

/// Validate `draft` against the week's analyzed videos `week`: trim, clamp lengths, drop unknown
/// ids and threads without title, text or ids, keep at most 3 threads, and clean the script.
pub fn clean(draft: Draft, week: &[Video], written_at: String) -> Result<Debrief> {
    let headline = clamp(&draft.headline, MAX_HEADLINE);
    let summary = clamp(&draft.summary, MAX_SUMMARY);
    if headline.is_empty() || summary.is_empty() {
        bail!("headline and summary are required");
    }
    let threads = draft
        .threads
        .into_iter()
        .filter_map(|t| {
            let mut video_ids: Vec<String> = Vec::new();
            for id in t.video_ids {
                let id = id.trim().to_string();
                if week.iter().any(|v| v.id == id) && !video_ids.contains(&id) {
                    video_ids.push(id);
                }
            }
            let (title, text) = (
                clamp(&t.title, MAX_THREAD_TITLE),
                clamp(&t.text, MAX_THREAD_TEXT),
            );
            (!title.is_empty() && !text.is_empty() && !video_ids.is_empty()).then_some(Thread {
                title,
                text,
                video_ids,
            })
        })
        .take(MAX_THREADS)
        .collect();
    let mut video_ids: Vec<String> = week.iter().map(|v| v.id.clone()).collect();
    video_ids.sort();
    Ok(Debrief {
        headline,
        summary,
        threads,
        script: draft.script.and_then(|s| clean_script(s, week)),
        video_ids,
        written_at,
    })
}

/// Validate and store the agent's debrief over the week's analyzed videos.
pub fn save(conn: &Connection, args: &Value) -> Result<Debrief> {
    let draft: Draft = serde_json::from_value(args.clone()).context("invalid debrief")?;
    let week = db::week(conn, DAYS, true, MAX_VIDEOS)?;
    if week.is_empty() {
        bail!("no analyzed videos from the last {DAYS} days to write a debrief over");
    }
    let debrief = clean(draft, &week, db::now())?;
    db::set_value(conn, KEY, &serde_json::to_string(&debrief)?)?;
    Ok(debrief)
}

/// The stored debrief, if there is a readable one.
pub fn stored(conn: &Connection) -> Result<Option<Debrief>> {
    Ok(db::get_value(conn, KEY)?.and_then(|j| serde_json::from_str(&j).ok()))
}

/// The stored debrief when it was written in the last week.
pub fn current(conn: &Connection) -> Result<Option<Debrief>> {
    let cutoff = chrono::Utc::now() - chrono::Duration::days(DAYS);
    Ok(stored(conn)?.filter(|d| {
        chrono::DateTime::parse_from_rfc3339(&d.written_at).is_ok_and(|at| at >= cutoff)
    }))
}

/// The week's analyzed videos when a new debrief is due: at least two, and not the set the stored
/// debrief was written over.
pub fn due(conn: &Connection) -> Result<Option<Vec<Video>>> {
    let videos = db::week(conn, DAYS, true, MAX_VIDEOS)?;
    if videos.len() < MIN_VIDEOS {
        return Ok(None);
    }
    let mut ids: Vec<&str> = videos.iter().map(|v| v.id.as_str()).collect();
    ids.sort();
    let same = stored(conn)?.is_some_and(|d| d.video_ids == ids);
    Ok((!same).then_some(videos))
}

/// What `write_digest` tells the agent after the digest: the week's analyses and the instruction
/// to call `save_debrief`, or that none is needed.
pub fn request(conn: &Connection) -> Result<String> {
    let Some(videos) = due(conn)? else {
        return Ok("No debrief needed: the week's analyzed videos haven't changed since the last one (or there are fewer than two).".into());
    };
    let profile = db::get_settings(conn)?.profile;
    let mut s = format!(
        "Now write the debrief: the {} videos from the last {DAYS} days that you or earlier runs analyzed, below.\n\nProfile: {}\n",
        videos.len(),
        if profile.is_empty() {
            "(none)"
        } else {
            &profile
        }
    );
    for v in &videos {
        let p = report::parts(v.analysis.as_deref().unwrap_or_default());
        s.push_str(&format!(
            "\n### {} (id {})\n{} · {}/5 · {}\n",
            v.title,
            v.id,
            v.channel,
            v.relevance.unwrap_or(0),
            p.advice,
        ));
        if v.podcast {
            s.push_str("Podcast episode: no clip.\n");
        } else {
            if let Some(d) = v.duration.filter(|d| *d > 0.0) {
                s.push_str(&format!("Length: {}\n", fmt_time(d)));
            }
            if let (Some(from), Some(to)) = (&p.skim_from, &p.skim_to) {
                s.push_str(&format!("Skim range: {from}–{to}\n"));
            }
        }
        s.push_str(&format!(
            "Verdict: {}\nTL;DR: {}\n",
            v.verdict.as_deref().unwrap_or_default(),
            p.tldr
        ));
        for t in &p.takeaways {
            match &t.at {
                Some(at) => s.push_str(&format!("- {} [{at}]\n", t.text)),
                None => s.push_str(&format!("- {}\n", t.text)),
            }
        }
    }
    s.push_str(&format!(
        "\nCall save_debrief once with: `headline` (one sentence, {MAX_HEADLINE} characters at most), `summary` (2-4 sentences, \
         {MAX_SUMMARY} at most), up to {MAX_THREADS} `threads` (`title`, `text`, `video_ids`) and a `script`. Say what these videos add up to for \
         the profile's owner, what to watch first, and which themes run through two or more of them (each thread names the ids \
         it runs through).\n\n\
         The `script` is a short reel for this person, about a minute: `intro` (one short sentence that frames the week, \
         {MAX_INTRO} characters at most), 3-6 `clips`, best first, and `outro` (one short line, {MAX_OUTRO} at most). Each clip is \
         `video_id` (podcast episodes can't be clips), `say`, `moment_from`/`moment_to` and `from`/`to`. `say` is one punchy \
         sentence (120 characters at most) on why the clip matters to them; no hype. \
         `moment_from`/`moment_to` (mm:ss or h:mm:ss in that YouTube video) are the 8-15 seconds where the key point is said, taken \
         from the analyses' takeaway timestamps; the clip plays them at normal speed with sound. `from`/`to` is the stretch around \
         the moment, 1-6 minutes (10 at most), from the skim ranges; it plays fast and muted behind `say`.\n\n\
         Use only what the analyses above say; invent no facts. Write in the language the profile is written in."
    ));
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    fn thread(title: &str, text: &str, ids: &[&str]) -> Thread {
        Thread {
            title: title.into(),
            text: text.into(),
            video_ids: ids.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn video(id: &str, podcast: bool, duration: Option<f64>) -> Video {
        Video {
            id: id.into(),
            podcast,
            duration,
            ..Video::default()
        }
    }

    fn week() -> Vec<Video> {
        ["b", "a", "c"].map(|id| video(id, false, None)).to_vec()
    }

    #[test]
    fn clean_clamps_lengths_and_trims() {
        let d = clean(
            Draft {
                headline: format!("  {}  ", "h".repeat(300)),
                summary: "s".repeat(900),
                threads: vec![thread(&"t".repeat(100), &"x".repeat(500), &["a"])],
                ..Draft::default()
            },
            &week(),
            "now".into(),
        )
        .unwrap();
        assert_eq!(d.headline.chars().count(), MAX_HEADLINE);
        assert!(d.headline.ends_with('…'));
        assert_eq!(d.summary.chars().count(), MAX_SUMMARY);
        assert_eq!(d.threads[0].title.chars().count(), MAX_THREAD_TITLE);
        assert_eq!(d.threads[0].text.chars().count(), MAX_THREAD_TEXT);
        assert_eq!(d.video_ids, ["a", "b", "c"], "sorted, all of the week");
        assert_eq!(d.written_at, "now");
        let short = clean(
            Draft {
                headline: " One. ".into(),
                summary: " Two. ".into(),
                ..Draft::default()
            },
            &week(),
            String::new(),
        )
        .unwrap();
        assert_eq!(
            (short.headline.as_str(), short.summary.as_str()),
            ("One.", "Two.")
        );
    }

    #[test]
    fn clean_keeps_three_valid_threads_and_drops_the_rest() {
        let d = clean(
            Draft {
                headline: "H".into(),
                summary: "S".into(),
                threads: vec![
                    thread("no ids", "text", &[]),
                    thread("unknown only", "text", &["zzz"]),
                    thread("no text", " ", &["a"]),
                    thread(" ", "no title", &["a"]),
                    thread("one", "text", &["a", "zzz", "a", " b "]),
                    thread("two", "text", &["c"]),
                    thread("three", "text", &["b"]),
                    thread("four", "text", &["b"]),
                ],
                ..Draft::default()
            },
            &week(),
            String::new(),
        )
        .unwrap();
        let titles: Vec<&str> = d.threads.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["one", "two", "three"]);
        assert_eq!(
            d.threads[0].video_ids,
            ["a", "b"],
            "unknown and repeated ids dropped"
        );
    }

    #[test]
    fn clean_requires_headline_and_summary() {
        for (h, s) in [("", "S"), ("H", "  "), ("", "")] {
            let draft = Draft {
                headline: h.into(),
                summary: s.into(),
                ..Draft::default()
            };
            assert!(clean(draft, &week(), String::new()).is_err());
        }
    }

    fn clip(id: &str, from: &str, to: &str, say: &str) -> ClipDraft {
        ClipDraft {
            video_id: id.into(),
            from: from.into(),
            to: to.into(),
            say: say.into(),
            ..ClipDraft::default()
        }
    }

    fn script(clips: Vec<ClipDraft>) -> ScriptDraft {
        ScriptDraft {
            intro: "The week.".into(),
            clips,
            outro: "That's it.".into(),
        }
    }

    /// The script `clean` makes of `script` over a week of YouTube videos "a" (length 20 min) and
    /// "b" (length unknown) and the podcast episode "p".
    fn cleaned(script: ScriptDraft) -> Option<Script> {
        let week = [
            video("a", false, Some(1200.0)),
            video("b", false, None),
            video("p", true, Some(3000.0)),
        ];
        let draft = Draft {
            headline: "H".into(),
            summary: "S".into(),
            script: Some(script),
            ..Draft::default()
        };
        clean(draft, &week, String::new()).unwrap().script
    }

    #[test]
    fn script_clips_are_normalized_and_keep_the_agents_order() {
        let s = cleaned(script(vec![
            clip(" b ", "[1:02:03]", "1:04:00", "  Later one first.  "),
            clip("a", "2:05", "4:00", "Then this."),
        ]))
        .unwrap();
        assert_eq!(
            (s.intro.as_str(), s.outro.as_str()),
            ("The week.", "That's it.")
        );
        assert_eq!(
            s.clips,
            [
                Clip {
                    video_id: "b".into(),
                    from: "1:02:03".into(),
                    to: "1:04:00".into(),
                    start: 3723,
                    end: 3840,
                    say: "Later one first.".into(),
                    ..Clip::default()
                },
                Clip {
                    video_id: "a".into(),
                    from: "02:05".into(),
                    to: "04:00".into(),
                    start: 125,
                    end: 240,
                    say: "Then this.".into(),
                    ..Clip::default()
                },
            ]
        );
    }

    #[test]
    fn script_drops_invalid_clips() {
        let s = cleaned(script(vec![
            clip("p", "1:00", "3:00", "a podcast"),
            clip("zzz", "1:00", "3:00", "unknown or not analyzed"),
            clip("a", "3:00", "3:00", "empty"),
            clip("a", "4:00", "3:00", "backwards"),
            clip("a", "", "3:00", "no from"),
            clip("a", "1:00", "soon", "no to"),
            clip("a", "25:00", "26:00", "past the end"),
            clip("a", "1:00", "3:00", " "),
            clip("a", "1:00", "3:00", "fine"),
        ]))
        .unwrap();
        assert_eq!(s.clips.len(), 1);
        assert_eq!((s.clips[0].start, s.clips[0].end), (60, 180));
    }

    #[test]
    fn script_cuts_a_clip_at_ten_minutes_and_at_the_videos_length() {
        let s = cleaned(script(vec![
            clip("b", "5:00", "30:00", "long, length unknown"),
            clip("a", "15:00", "40:00", "past the end of a 20 minute video"),
            clip("a", "1:00", "30:00", "long"),
        ]))
        .unwrap();
        let ends: Vec<(u64, u64)> = s.clips.iter().map(|c| (c.start, c.end)).collect();
        assert_eq!(ends, [(300, 900), (900, 1200), (60, 660)]);
        assert_eq!(
            (s.clips[0].to.as_str(), s.clips[1].to.as_str()),
            ("15:00", "20:00")
        );
    }

    /// The moment `a` clip 5:00-7:00 of a 20 minute video gets from the agent's moment.
    fn moment_of(from: &str, to: &str) -> Option<(u64, u64)> {
        let s = cleaned(script(vec![ClipDraft {
            moment_from: from.into(),
            moment_to: to.into(),
            ..clip("a", "5:00", "7:00", "say")
        }]))
        .unwrap();
        assert_eq!(s.clips.len(), 1, "the clip stays whatever the moment");
        let c = &s.clips[0];
        let present = [
            c.moment_from.is_some(),
            c.moment_to.is_some(),
            c.moment_start.is_some(),
            c.moment_end.is_some(),
        ];
        assert!(
            present.iter().all(|p| *p == present[0]),
            "all four fields or none"
        );
        c.moment_start.zip(c.moment_end)
    }

    #[test]
    fn script_keeps_a_moment_inside_the_clip_and_formats_it() {
        let s = cleaned(script(vec![ClipDraft {
            moment_from: "[5:30]".into(),
            moment_to: "5:42".into(),
            ..clip("a", "5:00", "7:00", "say")
        }]))
        .unwrap();
        let c = &s.clips[0];
        assert_eq!((c.moment_start, c.moment_end), (Some(330), Some(342)));
        assert_eq!(
            (c.moment_from.as_deref(), c.moment_to.as_deref()),
            (Some("05:30"), Some("05:42"))
        );
        assert_eq!(
            moment_of("5:00", "5:04"),
            Some((300, 304)),
            "4 s at the start"
        );
        assert_eq!(
            moment_of("6:40", "7:00"),
            Some((400, 420)),
            "ends at the clip's end"
        );
    }

    #[test]
    fn script_cuts_a_moment_at_the_clips_end_and_at_25_seconds() {
        assert_eq!(
            moment_of("6:50", "7:30"),
            Some((410, 420)),
            "runs past the end"
        );
        assert_eq!(
            moment_of("5:10", "6:10"),
            Some((310, 335)),
            "60 s is cut at 25"
        );
        assert_eq!(moment_of("5:10", "5:35"), Some((310, 335)), "exactly 25 s");
    }

    #[test]
    fn script_drops_a_moment_that_is_too_short_or_outside_but_keeps_the_clip() {
        assert_eq!(moment_of("5:10", "5:13"), None, "3 s is too short");
        assert_eq!(moment_of("6:57", "7:30"), None, "cut at the end it is 3 s");
        assert_eq!(moment_of("4:50", "5:10"), None, "starts before the clip");
        assert_eq!(moment_of("7:00", "7:20"), None, "starts at the clip's end");
        assert_eq!(moment_of("8:00", "8:12"), None, "after the clip");
        assert_eq!(moment_of("6:00", "5:50"), None, "backwards");
        assert_eq!(moment_of("5:10", "soon"), None, "no usable end");
        assert_eq!(moment_of("", ""), None, "no moment given");
        assert_eq!(moment_of("5:10", ""), None, "half a moment");
    }

    #[test]
    fn script_keeps_six_clips_and_clamps_texts() {
        let clips = (0..10)
            .map(|i| {
                clip(
                    "a",
                    &format!("{i}:00"),
                    &format!("{i}:30"),
                    &"s".repeat(400),
                )
            })
            .collect();
        let s = cleaned(ScriptDraft {
            intro: "i".repeat(500),
            clips,
            outro: "o".repeat(400),
        })
        .unwrap();
        assert_eq!(s.clips.len(), 6, "the request asks for 3-6");
        assert_eq!(s.clips[5].start, 300, "the first six, in order");
        assert_eq!(s.clips[0].say.chars().count(), 140);
        assert_eq!(s.intro.chars().count(), 160);
        assert_eq!(s.outro.chars().count(), 120);
        assert_eq!(
            (MAX_SAY, MAX_INTRO, MAX_OUTRO),
            (140, 160, 120),
            "the request promises these"
        );
    }

    #[test]
    fn script_needs_an_intro_and_a_surviving_clip() {
        let good = || clip("a", "1:00", "2:00", "say");
        let no_intro = ScriptDraft {
            intro: "  ".into(),
            ..script(vec![good()])
        };
        assert_eq!(cleaned(no_intro), None);
        assert_eq!(cleaned(script(vec![])), None);
        assert_eq!(
            cleaned(script(vec![
                clip("p", "1:00", "2:00", "podcast"),
                clip("a", "2:00", "1:00", "x")
            ])),
            None
        );
        let no_outro = cleaned(ScriptDraft {
            outro: String::new(),
            ..script(vec![good()])
        })
        .unwrap();
        assert_eq!(no_outro.outro, "");
    }

    #[test]
    fn save_stores_the_script_and_the_request_asks_for_it() {
        let conn = db::memory();
        seed(
            &conn,
            &[("a", "done", 1), ("b", "done", 2), ("q", "queued", 1)],
        );
        conn.execute("UPDATE videos SET duration = 600 WHERE id = 'a'", [])
            .unwrap();
        let ask = request(&conn).unwrap();
        assert!(ask.contains("`script`") && ask.contains("Length: 10:00"));
        assert!(
            ask.contains("`moment_from`/`moment_to`")
                && ask.contains("8-15 seconds")
                && ask.contains("3-6 `clips`")
                && ask.contains("120 characters")
        );
        let mut args = draft(&["a"]);
        args["script"] = serde_json::json!({
            "intro": "Two videos.",
            "clips": [
                { "video_id": "a", "from": "8:00", "to": "12:00", "say": "Start here.",
                  "moment_from": "8:30", "moment_to": "8:42" },
                { "video_id": "b", "from": "1:00", "to": "3:00", "say": "Bad moment.",
                  "moment_from": "9:00", "moment_to": "9:12" },
                { "video_id": "q", "from": "1:00", "to": "2:00", "say": "Not analyzed." },
            ],
        });
        let d = save(&conn, &args).unwrap();
        let s = d.script.as_ref().unwrap();
        assert_eq!(s.clips.len(), 2);
        assert_eq!((s.clips[0].to.as_str(), s.clips[0].end), ("10:00", 600));
        assert_eq!(
            s.clips[1].moment_start, None,
            "outside the clip: moment dropped"
        );
        assert_eq!(stored(&conn).unwrap(), Some(d));
        let json = serde_json::to_value(stored(&conn).unwrap().unwrap()).unwrap();
        let clips = &json["script"]["clips"];
        assert_eq!(clips[0]["start"], 480);
        assert_eq!(
            (
                clips[0]["moment_start"].clone(),
                clips[0]["moment_end"].clone()
            ),
            (510.into(), 522.into())
        );
        assert_eq!(clips[0]["moment_from"], "08:30");
        assert!(clips[1].get("moment_start").is_none() && clips[1].get("moment_to").is_none());
        assert_eq!(json["script"]["outro"], "");

        let none = save(&conn, &draft(&["a"])).unwrap();
        assert_eq!(none.script, None);
    }

    fn seed(conn: &Connection, videos: &[(&str, &str, i64)]) {
        db::upsert_channel(conn, "UC1", "One").unwrap();
        db::set_enabled(conn, "UC1", true).unwrap();
        for (id, status, days_ago) in videos {
            let published = (chrono::Utc::now() - chrono::Duration::days(*days_ago))
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, false);
            conn.execute(
                "INSERT INTO videos(id, channel_id, title, url, published, status, relevance, verdict, analysis) \
                 VALUES(?1, 'UC1', ?1, 'u', ?3, ?2, 4, 'fits', '## TL;DR\nx\n## Worth watching?\nwatch — y')",
                params![id, status, published],
            )
            .unwrap();
        }
    }

    fn draft(ids: &[&str]) -> Value {
        serde_json::json!({
            "headline": "A week of evals", "summary": "It adds up.",
            "threads": [{ "title": "Evals", "text": "Both test models.", "video_ids": ids }],
        })
    }

    #[test]
    fn save_stores_the_debrief_over_the_weeks_analyzed_videos() {
        let conn = db::memory();
        seed(
            &conn,
            &[
                ("a", "done", 1),
                ("b", "done", 2),
                ("q", "queued", 1),
                ("old", "done", 9),
            ],
        );
        let d = save(&conn, &draft(&["a", "q", "old"])).unwrap();
        assert_eq!(d.video_ids, ["a", "b"]);
        assert_eq!(
            d.threads[0].video_ids,
            ["a"],
            "queued and old videos aren't in the set"
        );
        assert_eq!(stored(&conn).unwrap(), Some(d.clone()));
        assert_eq!(current(&conn).unwrap(), Some(d));
        assert!(save(&conn, &serde_json::json!({ "headline": "H" })).is_err());
    }

    #[test]
    fn save_needs_analyzed_videos_this_week() {
        let conn = db::memory();
        seed(&conn, &[("old", "done", 9), ("q", "queued", 1)]);
        assert!(save(&conn, &draft(&["old"])).is_err());
        assert_eq!(stored(&conn).unwrap(), None);
    }

    #[test]
    fn current_ignores_a_debrief_older_than_a_week() {
        let conn = db::memory();
        seed(&conn, &[("a", "done", 1), ("b", "done", 1)]);
        save(&conn, &draft(&["a"])).unwrap();
        let mut d = stored(&conn).unwrap().unwrap();
        d.written_at = (chrono::Utc::now() - chrono::Duration::days(8)).to_rfc3339();
        db::set_value(&conn, KEY, &serde_json::to_string(&d).unwrap()).unwrap();
        assert!(stored(&conn).unwrap().is_some());
        assert_eq!(current(&conn).unwrap(), None);
    }

    #[test]
    fn debrief_is_due_when_two_are_analyzed_and_the_set_changed() {
        let conn = db::memory();
        seed(&conn, &[("a", "done", 1), ("q", "queued", 1)]);
        assert!(due(&conn).unwrap().is_none(), "one analyzed video");
        assert!(request(&conn).unwrap().starts_with("No debrief needed"));

        conn.execute("UPDATE videos SET status = 'done' WHERE id = 'q'", [])
            .unwrap();
        let ids: Vec<String> = due(&conn)
            .unwrap()
            .unwrap()
            .into_iter()
            .map(|v| v.id)
            .collect();
        assert_eq!(ids.len(), 2, "two analyzed, none stored");
        let ask = request(&conn).unwrap();
        assert!(ask.contains("(id a)") && ask.contains("(id q)") && ask.contains("save_debrief"));
        assert!(ask.contains("TL;DR: x") && ask.contains("Verdict: fits"));

        save(&conn, &draft(&["a", "q"])).unwrap();
        assert!(
            due(&conn).unwrap().is_none(),
            "same set as the stored debrief"
        );
        assert!(request(&conn).unwrap().starts_with("No debrief needed"));

        seed_more(&conn);
        assert_eq!(due(&conn).unwrap().unwrap().len(), 3, "set changed");
    }

    fn seed_more(conn: &Connection) {
        conn.execute(
            "INSERT INTO videos(id, channel_id, title, url, published, status) VALUES('n', 'UC1', 'n', 'u', ?1, 'done')",
            params![db::now()],
        )
        .unwrap();
    }
}
