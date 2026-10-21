//! Caption parsing and transcript rendering. Pure functions, no I/O.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Seg {
    pub start: f64, // seconds
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub title: String,
    pub url: String,
    pub uploader: Option<String>,
    pub duration: Option<f64>,
    pub source: String, // e.g. "captions en-orig (auto)" or "whisper small (en)"
    pub segs: Vec<Seg>,
}

static WS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").unwrap());
static CUE_TIME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:(\d+):)?(\d{1,2}):(\d{2})[.,](\d{3})\s*-->").unwrap());
static BLOCK_SPLIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n\s*\n").unwrap());

fn unescape(s: &str) -> String {
    let mut out = s
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ");
    // Numeric entities (&#8217; etc.)
    static NUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"&#(\d+);").unwrap());
    out = NUM
        .replace_all(&out, |c: &regex::Captures| {
            c[1].parse::<u32>()
                .ok()
                .and_then(char::from_u32)
                .map(String::from)
                .unwrap_or_default()
        })
        .into_owned();
    out
}

fn clean(s: &str) -> String {
    WS.replace_all(&unescape(s), " ").trim().to_string()
}

/// YouTube json3 captions. `aAppend` events only carry the line breaks of rolling
/// auto-captions; skipping them yields each word exactly once.
pub fn parse_json3(data: &serde_json::Value) -> Vec<Seg> {
    let Some(events) = data["events"].as_array() else {
        return vec![];
    };
    events
        .iter()
        .filter(|ev| ev.get("aAppend").is_none())
        .filter_map(|ev| {
            let segs = ev["segs"].as_array()?;
            let text = clean(
                &segs
                    .iter()
                    .filter_map(|s| s["utf8"].as_str())
                    .collect::<String>(),
            );
            (!text.is_empty()).then(|| Seg {
                start: ev["tStartMs"].as_f64().unwrap_or(0.0) / 1000.0,
                text,
            })
        })
        .collect()
}

/// WebVTT or SRT. Rolling captions repeat the previous line at the top of each cue;
/// lines identical to the last emitted one are dropped.
pub fn parse_vtt(text: &str) -> Vec<Seg> {
    let mut segs: Vec<Seg> = Vec::new();
    let normalized = text.replace("\r\n", "\n");
    for block in BLOCK_SPLIT.split(&normalized) {
        let lines: Vec<&str> = block.trim().split('\n').collect();
        let Some(i) = lines.iter().position(|l| CUE_TIME.is_match(l)) else {
            continue;
        };
        let c = CUE_TIME.captures(lines[i]).unwrap();
        let num = |k: usize| {
            c.get(k)
                .map_or(0.0, |m| m.as_str().parse::<f64>().unwrap_or(0.0))
        };
        let start = num(1) * 3600.0 + num(2) * 60.0 + num(3) + num(4) / 1000.0;
        for raw in &lines[i + 1..] {
            let t = clean(&TAG.replace_all(raw, ""));
            if !t.is_empty() && segs.last().is_none_or(|s| s.text != t) {
                segs.push(Seg { start, text: t });
            }
        }
    }
    segs
}

/// Merge segments into paragraphs: break at sentence end after `soft` seconds,
/// unconditionally after `hard` seconds (auto-captions often lack punctuation).
pub fn paragraphs(segs: &[Seg], soft: f64, hard: f64) -> Vec<Seg> {
    let mut out: Vec<Seg> = Vec::new();
    for s in segs {
        match out.last_mut() {
            Some(cur) => {
                let span = s.start - cur.start;
                if span >= hard || (span >= soft && cur.text.ends_with(['.', '?', '!'])) {
                    out.push(s.clone());
                } else {
                    cur.text.push(' ');
                    cur.text.push_str(&s.text);
                }
            }
            None => out.push(s.clone()),
        }
    }
    out
}

pub fn fmt_time(sec: f64) -> String {
    let s = sec.max(0.0) as u64;
    let (h, m, ss) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{ss:02}")
    } else {
        format!("{m:02}:{ss:02}")
    }
}

/// Markdown-ish text for terminals and agents: header, then `[mm:ss]` paragraphs.
pub fn render(t: &Transcript, timestamps: bool) -> String {
    let meta: Vec<String> = [
        t.uploader.clone(),
        t.duration.map(fmt_time),
        Some(t.url.clone()),
    ]
    .into_iter()
    .flatten()
    .collect();
    let mut out = format!(
        "# {}\n{}\nsource: {}\n\n",
        t.title,
        meta.join(" · "),
        t.source
    );
    for p in paragraphs(&t.segs, 30.0, 60.0) {
        if timestamps {
            out.push_str(&format!("[{}] ", fmt_time(p.start)));
        }
        out.push_str(&p.text);
        out.push_str("\n\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn seg(start: f64, text: &str) -> Seg {
        Seg {
            start,
            text: text.into(),
        }
    }

    #[test]
    fn json3_skips_append_events_so_rolling_words_appear_once() {
        let data = json!({"events": [
            {"tStartMs": 0, "segs": [{"utf8": "hello"}, {"utf8": " world"}]},
            {"tStartMs": 1500, "aAppend": 1, "segs": [{"utf8": "\n"}]},
            {"tStartMs": 1500, "segs": [{"utf8": "it&#39;s"}, {"utf8": " me"}]},
            {"tStartMs": 3000}
        ]});
        assert_eq!(
            parse_json3(&data),
            vec![seg(0.0, "hello world"), seg(1.5, "it's me")]
        );
    }

    #[test]
    fn vtt_rolling_captions_drop_repeated_line_and_inline_tags() {
        let vtt = "WEBVTT\nKind: captions\n\n00:00:01.000 --> 00:00:03.000 align:start\nfirst line<00:00:01.500><c> here</c>\n\n00:00:03.000 --> 00:00:03.010\nfirst line here\n\n00:00:03.010 --> 00:00:05.000\nfirst line here\nsecond line\n";
        assert_eq!(
            parse_vtt(vtt),
            vec![seg(1.0, "first line here"), seg(3.01, "second line")]
        );
    }

    #[test]
    fn srt_with_hours_and_comma_millis() {
        let srt = "1\r\n01:02:03,500 --> 01:02:05,000\r\nHi &amp; bye\r\n\r\n2\r\n01:02:05,000 --> 01:02:06,000\r\nNext\r\n";
        assert_eq!(
            parse_vtt(srt),
            vec![seg(3723.5, "Hi & bye"), seg(3725.0, "Next")]
        );
    }

    #[test]
    fn paragraphs_break_on_sentence_after_soft_and_always_after_hard() {
        let segs = [
            seg(0.0, "a."),
            seg(10.0, "b."),
            seg(31.0, "c"),
            seg(50.0, "d"),
            seg(95.0, "e"),
        ];
        assert_eq!(
            paragraphs(&segs, 30.0, 60.0),
            vec![seg(0.0, "a. b."), seg(31.0, "c d"), seg(95.0, "e")]
        );
    }

    #[test]
    fn fmt_time_switches_to_hours() {
        assert_eq!(
            (fmt_time(59.9), fmt_time(3725.0)),
            ("00:59".into(), "1:02:05".into())
        );
    }
}
