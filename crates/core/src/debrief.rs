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
const MAX_SEGMENT_INTRO: usize = 200;
const MAX_SAY: usize = 160;
const MAX_OUTRO: usize = 120;
const MAX_SEGMENTS: usize = 8;
/// Parts a segment needs to stay, and parts it keeps at most.
const MIN_PARTS: usize = 2;
const MAX_PARTS: usize = 4;
/// Shortest and longest moment of a part, in seconds.
const MIN_MOMENT_SECS: u64 = 6;
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

/// One part of a segment: the narrator's line that sets it up, then the stretch of the video where
/// it is said. The line runs as captions over the video fast-forwarded behind it; the stretch
/// then plays at normal speed with sound.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Part {
    pub say: String,
    /// "mm:ss" or "h:mm:ss" where the stretch starts and ends, 6-25 seconds apart.
    pub moment_from: String,
    pub moment_to: String,
    /// `moment_from` and `moment_to` in seconds.
    pub moment_start: u64,
    pub moment_end: u64,
}

/// One chapter of the narrated video: a YouTube video with the narrator's takeaway and 2-4 parts
/// sorted by time, not overlapping.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Segment {
    pub video_id: String,
    /// The narrator's takeaway for this video.
    pub intro: String,
    pub parts: Vec<Part>,
}

/// The narrated video: an intro, one segment per channel, an outro.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Script {
    pub intro: String,
    pub segments: Vec<Segment>,
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
pub struct PartDraft {
    pub say: String,
    pub moment_from: String,
    pub moment_to: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SegmentDraft {
    pub video_id: String,
    pub intro: String,
    pub parts: Vec<PartDraft>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ScriptDraft {
    pub intro: String,
    pub segments: Vec<SegmentDraft>,
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

/// The moment of a part as (start, end) seconds in a video of `length` seconds (when known), or
/// None when there is no valid one: both timestamps parse and it starts inside the video. A
/// moment running past the video's end is cut there, one longer than 25 seconds at 25, and one
/// that ends up shorter than 6 seconds is dropped.
fn clean_moment(from: &str, to: &str, length: Option<u64>) -> Option<(u64, u64)> {
    let from = report::seconds(from).filter(|f| length.is_none_or(|l| *f < l))?;
    let mut to = report::seconds(to)?.min(from + MAX_MOMENT_SECS);
    if let Some(length) = length {
        to = to.min(length);
    }
    (to >= from + MIN_MOMENT_SECS).then_some((from, to))
}

/// A validated part, or None when it has no line to say or no valid moment.
fn clean_part(p: PartDraft, length: Option<u64>) -> Option<Part> {
    let say = clamp(&p.say, MAX_SAY);
    let (start, end) = clean_moment(&p.moment_from, &p.moment_to, length)?;
    (!say.is_empty()).then(|| Part {
        say,
        moment_from: fmt_time(start as f64),
        moment_to: fmt_time(end as f64),
        moment_start: start,
        moment_end: end,
    })
}

/// A validated segment, or None when it isn't one: the video must be an analyzed YouTube video of
/// the week, there must be a takeaway to say, and at least 2 parts must survive. The parts are
/// sorted by time, those overlapping an earlier one dropped, and at most 4 kept.
fn clean_segment(s: SegmentDraft, week: &[Video]) -> Option<Segment> {
    let id = s.video_id.trim();
    let video = week.iter().find(|v| v.id == id && !v.podcast)?;
    let intro = clamp(&s.intro, MAX_SEGMENT_INTRO);
    let length = video.duration.filter(|d| *d > 0.0).map(|d| d as u64);
    let mut parts: Vec<Part> = s
        .parts
        .into_iter()
        .filter_map(|p| clean_part(p, length))
        .collect();
    parts.sort_by_key(|p| (p.moment_start, p.moment_end));
    let mut kept: Vec<Part> = Vec::new();
    for p in parts {
        if kept
            .last()
            .is_none_or(|prev| p.moment_start >= prev.moment_end)
        {
            kept.push(p);
        }
    }
    kept.truncate(MAX_PARTS);
    (!intro.is_empty() && kept.len() >= MIN_PARTS).then(|| Segment {
        video_id: video.id.clone(),
        intro,
        parts: kept,
    })
}

/// The agent's script without its invalid segments: at most 8 segments in the agent's order, one
/// per channel (a later segment of a channel already used is dropped), None when there is no
/// intro or no segment survives.
fn clean_script(draft: ScriptDraft, week: &[Video]) -> Option<Script> {
    let intro = clamp(&draft.intro, MAX_INTRO);
    let mut channels: Vec<&str> = Vec::new();
    let mut segments: Vec<Segment> = Vec::new();
    for draft in draft.segments {
        if segments.len() == MAX_SEGMENTS {
            break;
        }
        let Some(segment) = clean_segment(draft, week) else {
            continue;
        };
        let channel = week
            .iter()
            .find(|v| v.id == segment.video_id)
            .map_or("", |v| v.channel_id.as_str());
        if !channels.contains(&channel) {
            channels.push(channel);
            segments.push(segment);
        }
    }
    (!intro.is_empty() && !segments.is_empty()).then(|| Script {
        intro,
        segments,
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
            s.push_str("Podcast episode: no segment.\n");
        } else if let Some(d) = v.duration.filter(|d| *d > 0.0) {
            s.push_str(&format!("Length: {}\n", fmt_time(d)));
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
         The `script` is a narrated video over the week for this person: `intro` (one sentence framing the week, \
         {MAX_INTRO} characters at most), up to {MAX_SEGMENTS} `segments`, best first, and `outro` (one short line, {MAX_OUTRO} at \
         most). A segment is one YouTube video (podcast episodes can't be segments) and there is one segment per channel: pick the \
         best video of each. It has `video_id`, `intro` (the narrator's takeaway for this video, third person, {MAX_SEGMENT_INTRO} \
         characters at most, e.g. \"Theo on this week's security leaks: he argues the real story is stolen API keys, and why that \
         matters for you.\") and 3-4 `parts`, each with `say` (a narrator line that sets up the part and ends by pointing at it, \
         third person, {MAX_SAY} characters at most, e.g. \"He then shows how … — watch the next bit.\") and \
         `moment_from`/`moment_to` (mm:ss or h:mm:ss in that video): the {MIN_MOMENT_SECS}-{MAX_MOMENT_SECS} seconds where the \
         point is said, taken from the analyses' takeaway timestamps (a longer stretch is cut at {MAX_MOMENT_SECS} seconds, a \
         shorter one dropped). Parts of a video must not overlap. The viewer reads each `say` as captions while the video runs \
         fast in the background, then watches the part itself at normal speed with sound.\n\n\
         Write as a narrator: third person about what the creator says and why it matters to this person (the profile), no hype. \
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

    fn video(id: &str, channel_id: &str, podcast: bool, duration: Option<f64>) -> Video {
        Video {
            id: id.into(),
            channel_id: channel_id.into(),
            podcast,
            duration,
            ..Video::default()
        }
    }

    fn week() -> Vec<Video> {
        ["b", "a", "c"]
            .map(|id| video(id, "ch", false, None))
            .to_vec()
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

    fn part(say: &str, from: &str, to: &str) -> PartDraft {
        PartDraft {
            say: say.into(),
            moment_from: from.into(),
            moment_to: to.into(),
        }
    }

    /// A segment of `id` with the agent's `parts`.
    fn segment(id: &str, parts: Vec<PartDraft>) -> SegmentDraft {
        SegmentDraft {
            video_id: id.into(),
            intro: "Takeaway.".into(),
            parts,
        }
    }

    /// A segment of `id` with two valid parts, at 1:00 and 2:00.
    fn good(id: &str) -> SegmentDraft {
        segment(
            id,
            vec![part("One.", "1:00", "1:10"), part("Two.", "2:00", "2:10")],
        )
    }

    fn script(segments: Vec<SegmentDraft>) -> ScriptDraft {
        ScriptDraft {
            intro: "The week.".into(),
            segments,
            outro: "That's it.".into(),
        }
    }

    /// The script `clean` makes of `script` over a week of YouTube videos "a" (channel 1, length
    /// 20 min), "a2" (channel 1), "b" (channel 2, length unknown), "c" (channel 3) and the podcast
    /// episode "p" (channel 4).
    fn cleaned(script: ScriptDraft) -> Option<Script> {
        let week = [
            video("a", "1", false, Some(1200.0)),
            video("a2", "1", false, None),
            video("b", "2", false, None),
            video("c", "3", false, None),
            video("p", "4", true, Some(3000.0)),
        ];
        cleaned_over(script, &week)
    }

    fn cleaned_over(script: ScriptDraft, week: &[Video]) -> Option<Script> {
        let draft = Draft {
            headline: "H".into(),
            summary: "S".into(),
            script: Some(script),
            ..Draft::default()
        };
        clean(draft, week, String::new()).unwrap().script
    }

    fn ids(s: &Script) -> Vec<&str> {
        s.segments.iter().map(|s| s.video_id.as_str()).collect()
    }

    #[test]
    fn script_segments_are_normalized_and_keep_the_agents_order() {
        let s = cleaned(script(vec![
            SegmentDraft {
                intro: "  Why it matters.  ".into(),
                ..segment(
                    " b ",
                    vec![
                        part("  Later bit.  ", "[1:02:03]", "1:02:20"),
                        part("Earlier bit.", "5:00", "5:12"),
                    ],
                )
            },
            good("a"),
        ]))
        .unwrap();
        assert_eq!(
            (s.intro.as_str(), s.outro.as_str()),
            ("The week.", "That's it.")
        );
        assert_eq!(
            s.segments[0],
            Segment {
                video_id: "b".into(),
                intro: "Why it matters.".into(),
                parts: vec![
                    Part {
                        say: "Earlier bit.".into(),
                        moment_from: "05:00".into(),
                        moment_to: "05:12".into(),
                        moment_start: 300,
                        moment_end: 312,
                    },
                    Part {
                        say: "Later bit.".into(),
                        moment_from: "1:02:03".into(),
                        moment_to: "1:02:20".into(),
                        moment_start: 3723,
                        moment_end: 3740,
                    },
                ],
            },
            "parts sorted by time, texts trimmed, timestamps re-formatted"
        );
        assert_eq!(ids(&s), ["b", "a"], "segments keep the agent's order");
    }

    #[test]
    fn script_keeps_one_segment_per_channel() {
        let s = cleaned(script(vec![
            good("a"),
            good("b"),
            good("a2"),
            good("a"),
            good("c"),
        ]))
        .unwrap();
        assert_eq!(
            ids(&s),
            ["a", "b", "c"],
            "later segments of channel 1 dropped"
        );

        let s = cleaned(script(vec![
            segment("a", vec![part("Only.", "1:00", "1:10")]),
            good("a2"),
        ]))
        .unwrap();
        assert_eq!(
            ids(&s),
            ["a2"],
            "an invalid segment doesn't use up its channel"
        );
    }

    #[test]
    fn script_drops_podcasts_unknown_and_unanalyzed_videos() {
        let s = cleaned(script(vec![good("p"), good("zzz"), good(" "), good("b")])).unwrap();
        assert_eq!(ids(&s), ["b"], "podcast, unknown and empty ids are dropped");
        let unanalyzed = cleaned_over(script(vec![good("q")]), &[video("a", "1", false, None)]);
        assert_eq!(unanalyzed, None, "q isn't among the week's analyzed videos");
    }

    /// The moment a part of `a` (20 minutes) at the given stretch gets, next to a valid part at
    /// 2:00; None when it is dropped.
    fn moment_of(from: &str, to: &str) -> Option<(u64, u64)> {
        let s = cleaned(script(vec![segment(
            "a",
            vec![part("Say.", from, to), part("Filler.", "2:00", "2:10")],
        )]));
        let found = s?
            .segments
            .remove(0)
            .parts
            .into_iter()
            .find(|p| p.say == "Say.")?;
        assert_eq!(
            (found.moment_from.clone(), found.moment_to.clone()),
            (
                fmt_time(found.moment_start as f64),
                fmt_time(found.moment_end as f64)
            ),
            "formatted strings match the seconds"
        );
        Some((found.moment_start, found.moment_end))
    }

    #[test]
    fn script_keeps_moments_of_6_to_25_seconds() {
        assert_eq!(moment_of("[5:30]", "5:42"), Some((330, 342)));
        assert_eq!(
            moment_of("5:00", "5:06"),
            Some((300, 306)),
            "6 s is the least"
        );
        assert_eq!(moment_of("5:10", "5:35"), Some((310, 335)), "exactly 25 s");
        assert_eq!(
            moment_of("5:10", "6:10"),
            Some((310, 335)),
            "60 s is cut at 25"
        );
        assert_eq!(
            moment_of("19:50", "20:30"),
            Some((1190, 1200)),
            "cut at the video's length"
        );
    }

    #[test]
    fn script_drops_parts_with_unusable_moments() {
        assert_eq!(moment_of("5:00", "5:05"), None, "5 s is too short");
        assert_eq!(
            moment_of("19:57", "20:30"),
            None,
            "cut at the length it is 3 s"
        );
        assert_eq!(
            moment_of("20:00", "20:20"),
            None,
            "starts at the video's end"
        );
        assert_eq!(moment_of("25:00", "25:20"), None, "after the video");
        assert_eq!(moment_of("6:00", "5:50"), None, "backwards");
        assert_eq!(moment_of("5:10", "soon"), None, "no usable end");
        assert_eq!(moment_of("", ""), None, "no moment given");
        assert_eq!(moment_of("5:10", ""), None, "half a moment");
        assert_eq!(
            cleaned(script(vec![segment(
                "a",
                vec![part(" ", "5:00", "5:10"), part("Fine.", "6:00", "6:10")]
            )])),
            None,
            "a part without a line is dropped, leaving one"
        );
    }

    #[test]
    fn script_drops_overlapping_parts_keeping_the_earlier() {
        let s = cleaned(script(vec![segment(
            "a",
            vec![
                part("Late.", "8:00", "8:10"),
                part("Second.", "5:10", "5:30"),
                part("First.", "5:00", "5:20"),
                part("Touching.", "5:30", "5:40"),
            ],
        )]))
        .unwrap();
        let parts: Vec<(&str, u64)> = s.segments[0]
            .parts
            .iter()
            .map(|p| (p.say.as_str(), p.moment_start))
            .collect();
        assert_eq!(
            parts,
            [("First.", 300), ("Touching.", 330), ("Late.", 480)],
            "Second overlaps First; one starting where another ends stays"
        );
    }

    #[test]
    fn script_drops_segments_with_fewer_than_two_parts() {
        let s = cleaned(script(vec![
            segment("a", vec![]),
            segment("a", vec![part("Alone.", "1:00", "1:10")]),
            segment(
                "a",
                vec![
                    part("One.", "1:00", "1:10"),
                    part("Overlaps.", "1:05", "1:20"),
                ],
            ),
            segment(
                "a",
                vec![
                    part("One.", "1:00", "1:10"),
                    part("Too short.", "2:00", "2:03"),
                ],
            ),
            good("b"),
        ]))
        .unwrap();
        assert_eq!(ids(&s), ["b"]);
    }

    #[test]
    fn script_keeps_at_most_four_parts_of_a_segment() {
        let parts = (1..=6)
            .rev()
            .map(|i| {
                part(
                    &format!("Part {i}."),
                    &format!("{i}:00"),
                    &format!("{i}:10"),
                )
            })
            .collect();
        let s = cleaned(script(vec![segment("a", parts)])).unwrap();
        let says: Vec<&str> = s.segments[0].parts.iter().map(|p| p.say.as_str()).collect();
        assert_eq!(
            says,
            ["Part 1.", "Part 2.", "Part 3.", "Part 4."],
            "the first four by time"
        );
        assert_eq!(MAX_PARTS, 4, "the request promises this");
    }

    #[test]
    fn script_keeps_at_most_eight_segments() {
        let week: Vec<Video> = (0..10)
            .map(|i| video(&format!("v{i}"), &format!("ch{i}"), false, None))
            .collect();
        let segments = (0..10).map(|i| good(&format!("v{i}"))).collect();
        let s = cleaned_over(script(segments), &week).unwrap();
        assert_eq!(
            ids(&s),
            ["v0", "v1", "v2", "v3", "v4", "v5", "v6", "v7"],
            "the first eight"
        );
        assert_eq!(MAX_SEGMENTS, 8, "the request promises this");
    }

    #[test]
    fn script_clamps_texts() {
        let s = cleaned(ScriptDraft {
            intro: "i".repeat(500),
            segments: vec![SegmentDraft {
                intro: "n".repeat(500),
                ..segment(
                    "a",
                    vec![
                        part(&"s".repeat(400), "1:00", "1:10"),
                        part("Two.", "2:00", "2:10"),
                    ],
                )
            }],
            outro: "o".repeat(400),
        })
        .unwrap();
        assert_eq!(s.intro.chars().count(), 160);
        assert_eq!(s.segments[0].intro.chars().count(), 200);
        assert_eq!(s.segments[0].parts[0].say.chars().count(), 160);
        assert_eq!(s.outro.chars().count(), 120);
        assert!(s.intro.ends_with('…'));
        assert_eq!(
            (MAX_INTRO, MAX_SEGMENT_INTRO, MAX_SAY, MAX_OUTRO),
            (160, 200, 160, 120),
            "the request promises these"
        );
    }

    #[test]
    fn script_needs_an_intro_and_a_surviving_segment() {
        let no_intro = ScriptDraft {
            intro: "  ".into(),
            ..script(vec![good("a")])
        };
        assert_eq!(cleaned(no_intro), None, "no intro, no script");
        assert_eq!(cleaned(script(vec![])), None);
        assert_eq!(
            cleaned(script(vec![good("p"), segment("a", vec![])])),
            None,
            "no segment survives"
        );
        let no_segment_intro = cleaned(script(vec![SegmentDraft {
            intro: " ".into(),
            ..good("a")
        }]));
        assert_eq!(no_segment_intro, None, "a segment needs its takeaway");
        let no_outro = cleaned(ScriptDraft {
            outro: String::new(),
            ..script(vec![good("a")])
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
            ask.contains("`segments`")
                && ask.contains("`parts`")
                && ask.contains("6-25 seconds")
                && ask.contains("3-4 `parts`")
                && ask.contains("160 characters")
                && ask.contains("200 characters")
                && !ask.contains("clip")
        );
        let mut args = draft(&["a"]);
        args["script"] = serde_json::json!({
            "intro": "Two videos.",
            "segments": [
                { "video_id": "a", "intro": "One on evals.", "parts": [
                    { "say": "He starts here.", "moment_from": "8:30", "moment_to": "8:42" },
                    { "say": "Then this.", "moment_from": "1:00", "moment_to": "1:20" },
                    { "say": "Past the end.", "moment_from": "11:00", "moment_to": "11:20" } ] },
                { "video_id": "b", "intro": "Same channel.", "parts": [
                    { "say": "One.", "moment_from": "1:00", "moment_to": "1:10" },
                    { "say": "Two.", "moment_from": "2:00", "moment_to": "2:10" } ] },
                { "video_id": "q", "intro": "Not analyzed.", "parts": [
                    { "say": "One.", "moment_from": "1:00", "moment_to": "1:10" },
                    { "say": "Two.", "moment_from": "2:00", "moment_to": "2:10" } ] },
            ],
        });
        let d = save(&conn, &args).unwrap();
        let s = d.script.as_ref().unwrap();
        assert_eq!(ids(s), ["a"], "b shares a's channel, q isn't analyzed");
        assert_eq!(
            s.segments[0].parts.len(),
            2,
            "the part past the video's end is dropped"
        );
        assert_eq!(stored(&conn).unwrap(), Some(d));
        let json = serde_json::to_value(stored(&conn).unwrap().unwrap()).unwrap();
        let parts = &json["script"]["segments"][0]["parts"];
        assert_eq!(
            (
                parts[0]["moment_start"].clone(),
                parts[0]["moment_end"].clone()
            ),
            (60.into(), 80.into()),
            "sorted by time"
        );
        assert_eq!(
            (
                parts[1]["moment_from"].clone(),
                parts[1]["moment_to"].clone()
            ),
            ("08:30".into(), "08:42".into())
        );
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
