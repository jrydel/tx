//! Terminal review of one analysed video: verdict, a proportional timeline of what is
//! worth watching, timestamped takeaways, TL;DR and source.
//!
//! Pure string rendering. Every transcript and model-written field is untrusted: it is
//! stripped of escape sequences and control characters before layout. Only our own SGR
//! styles and validated HTTP(S) OSC 8 links are emitted; both can be disabled independently.

use crate::terminal_links::Links;
use dialoguer::console::{Style, measure_text_width};
use std::iter::Peekable;
use std::str::Chars;
use tx_core::captions::{Transcript, fmt_time};
use tx_core::report::AgentAnalysis;

const MIN_WIDTH: usize = 40;
const MAX_WIDTH: usize = 120;
/// Left and right margin of every content line.
const INDENT: usize = 2;
const TRACK: char = '░';
const FILL: char = '█';

#[derive(Clone, Copy, PartialEq, Eq)]
enum Advice {
    Skip,
    Skim,
    Watch,
    Other,
}

/// Render `analysis` of `transcript` for a terminal `width` columns wide (clamped to 40..=120).
/// With both `color` and `hyperlinks` off the result contains no escape codes.
pub(crate) fn render(
    transcript: &Transcript,
    analysis: &AgentAnalysis,
    width: usize,
    color: bool,
    hyperlinks: bool,
) -> String {
    let width = width.clamp(MIN_WIDTH, MAX_WIDTH);
    let inner = width - 2 * INDENT;
    let mut v = View {
        color,
        links: hyperlinks.then(|| Links::new(&transcript.url)).flatten(),
        out: Vec::new(),
    };

    // Header rule.
    let plain_head = "── tx · video review ";
    let fill = width.saturating_sub(measure_text_width(plain_head));
    v.out.push(format!(
        "{}{}{}{}",
        v.dim("── "),
        v.st().bold().apply_to("tx"),
        v.dim(" · video review "),
        v.dim(&"─".repeat(fill)),
    ));
    v.blank();

    // Video identity.
    let title = one_line(&transcript.title);
    let title = if title.is_empty() {
        "(untitled)".to_string()
    } else {
        title
    };
    v.text(INDENT, &title, inner, &v.st().bold());
    let known_duration = known_duration(transcript.duration);
    let meta: Vec<String> = [
        transcript
            .uploader
            .as_deref()
            .map(one_line)
            .filter(|s| !s.is_empty()),
        known_duration.map(fmt_time),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !meta.is_empty() {
        v.text(INDENT, &meta.join(" · "), inner, &v.st().dim());
    }
    v.blank();

    // Verdict badge, score and verdict text.
    let (kind, advice_label) = advice_of(&analysis.watch.advice);
    let acc = accent(kind, &v);
    let badge = v
        .st()
        .black()
        .bold()
        .on_color256(accent_bg(kind))
        .apply_to(format!(" {} ", advice_label.to_uppercase()));
    let score = analysis.relevance.clamp(0, 5) as usize;
    let score_style = match score {
        4 | 5 => v.st().green(),
        2 | 3 => v.st().yellow(),
        _ => v.st().red(),
    };
    v.out.push(format!(
        "{}{badge}  {}{} {}",
        " ".repeat(INDENT),
        score_style.apply_to(FILLED_DOT.to_string().repeat(score)),
        v.dim(&EMPTY_DOT.to_string().repeat(5 - score)),
        score_style.clone().bold().apply_to(format!("{score}/5")),
    ));
    v.text(INDENT, &sanitize(&analysis.verdict), inner, &v.st());

    // Timeline and watch advice.
    v.section("Timeline");
    let skim_range = match (
        analysis.watch.from.as_deref().and_then(parse_stamp),
        analysis.watch.to.as_deref().and_then(parse_stamp),
    ) {
        (Some(f), Some(t)) if t > f => Some((f, t)),
        _ => None,
    };
    match known_duration {
        Some(dur) => {
            let cells = match kind {
                Advice::Watch => Some((0, inner)),
                Advice::Skim => skim_range.and_then(|(f, t)| range_cells(f, t, dur, inner)),
                Advice::Skip | Advice::Other => None,
            };
            v.out.push(format!(
                "{}{}",
                " ".repeat(INDENT),
                v.bar(inner, cells, &acc, dur)
            ));
            let (l, r) = (fmt_time(0.0), fmt_time(dur));
            let gap = inner.saturating_sub(measure_text_width(&l) + measure_text_width(&r));
            v.out.push(format!(
                "{}{}{}{}",
                " ".repeat(INDENT),
                v.st().cyan().apply_to(v.link(l, Some(0.0))),
                " ".repeat(gap),
                v.st().cyan().apply_to(v.link(r, Some(dur))),
            ));
        }
        None => v.text(
            INDENT,
            "duration unknown · no timeline",
            inner,
            &v.st().dim(),
        ),
    }
    v.blank();
    let detail = match (kind, skim_range) {
        (Advice::Skim, Some((f, t))) => {
            let (f, t) = match known_duration {
                Some(d) => (f.min(d), t.min(d)),
                None => (f, t),
            };
            Some(format!(
                "{}–{}",
                v.link(fmt_time(f), Some(f)),
                v.link(fmt_time(t), Some(t))
            ))
        }
        (Advice::Watch, _) => Some(v.link("full video".to_string(), Some(0.0))),
        _ => None,
    };
    let mut head = format!(
        "{}{}",
        " ".repeat(INDENT),
        acc.clone().bold().apply_to(advice_label.to_lowercase())
    );
    if let Some(d) = detail {
        head.push_str(&format!(" {} {}", v.dim("·"), v.st().cyan().apply_to(d)));
    }
    v.out.push(head);
    v.text(INDENT, &sanitize(&analysis.watch.reason), inner, &v.st());

    // Takeaways: `[mm:ss]` label column with a hanging indent.
    let takeaways: Vec<(String, String, Option<f64>)> = analysis
        .takeaways
        .iter()
        .filter_map(|t| {
            let text = sanitize(&t.text);
            if text.trim().is_empty() {
                return None;
            }
            let at = t.at.as_deref().and_then(parse_stamp);
            let label = at
                .map(|s| format!("[{}]", fmt_time(s)))
                .unwrap_or_else(|| "•".to_string());
            Some((label, text, at))
        })
        .collect();
    if !takeaways.is_empty() {
        v.section("Takeaways");
        let label_w = takeaways
            .iter()
            .map(|(l, _, _)| measure_text_width(l))
            .max()
            .unwrap_or(0);
        for (label, text, at) in &takeaways {
            let lines = wrap(text, inner - label_w - 1);
            for (i, line) in lines.iter().enumerate() {
                let lead = if i == 0 {
                    let pad = " ".repeat(label_w - measure_text_width(label));
                    if label == "•" {
                        format!("{}{pad}", v.dim(label))
                    } else {
                        format!(
                            "{}{pad}",
                            v.st().cyan().apply_to(v.link(label.clone(), *at))
                        )
                    }
                } else {
                    " ".repeat(label_w)
                };
                v.out.push(if line.is_empty() {
                    String::new()
                } else {
                    format!("{}{lead} {line}", " ".repeat(INDENT))
                });
            }
        }
    }

    let tldr = sanitize(&analysis.tldr);
    if !tldr.trim().is_empty() {
        v.section("TL;DR");
        v.text(INDENT, &tldr, inner, &v.st());
    }

    // Source.
    let url = one_line(&transcript.url);
    let source = one_line(&transcript.source);
    if !url.is_empty() || !source.is_empty() {
        v.section("Source");
        if !url.is_empty() {
            for line in wrap(&url, inner) {
                v.out.push(format!(
                    "{}{}",
                    " ".repeat(INDENT),
                    v.st().cyan().underlined().apply_to(v.link(line, None))
                ));
            }
        }
        if !source.is_empty() {
            v.text(
                INDENT,
                &format!("transcript: {source}"),
                inner,
                &v.st().dim(),
            );
        }
    }
    if v.links.as_ref().is_some_and(Links::can_seek) {
        v.blank();
        v.text(
            INDENT,
            "Ctrl/Cmd+click timestamps or timeline to open.",
            inner,
            &v.st().dim(),
        );
    }

    v.blank();
    v.out.push(v.dim(&"─".repeat(width)));
    let mut s = v.out.join("\n");
    s.push('\n');
    s
}

const FILLED_DOT: char = '●';
const EMPTY_DOT: char = '○';

fn advice_of(raw: &str) -> (Advice, String) {
    let label = one_line(raw);
    let kind = match label.to_lowercase().as_str() {
        "skip" => Advice::Skip,
        "skim" => Advice::Skim,
        "watch" | "watch fully" => Advice::Watch,
        _ => Advice::Other,
    };
    let label = match kind {
        Advice::Skip => "skip".to_string(),
        Advice::Skim => "skim".to_string(),
        Advice::Watch => "watch".to_string(),
        Advice::Other => label.chars().take(16).collect(),
    };
    (kind, label)
}

/// Foreground style for a verdict: watch green, skim amber, skip red.
fn accent(kind: Advice, v: &View) -> Style {
    match kind {
        Advice::Watch => v.st().green(),
        Advice::Skim => v.st().yellow(),
        Advice::Skip => v.st().red(),
        Advice::Other => v.st().dim(),
    }
}

/// Badge background as a 256-colour index (black text stays readable on all of them).
fn accent_bg(kind: Advice) -> u8 {
    match kind {
        Advice::Watch => 2,
        Advice::Skim => 3,
        Advice::Skip => 1,
        Advice::Other => 7,
    }
}

fn known_duration(d: Option<f64>) -> Option<f64> {
    d.filter(|d| d.is_finite() && *d > 0.0)
}

/// Half-open cell range `[lo, hi)` of a `w`-cell timeline covered by `from..to` seconds of
/// a `dur`-second video. Clamped to the video, never empty; `None` for an unusable range.
fn range_cells(from: f64, to: f64, dur: f64, w: usize) -> Option<(usize, usize)> {
    if !(dur.is_finite() && dur > 0.0)
        || w == 0
        || to.partial_cmp(&from) != Some(std::cmp::Ordering::Greater)
    {
        return None;
    }
    let (f, t) = (from.clamp(0.0, dur), to.clamp(0.0, dur));
    let lo = ((f / dur * w as f64).floor() as usize).min(w - 1);
    let hi = ((t / dur * w as f64).ceil() as usize).min(w).max(lo + 1);
    Some((lo, hi))
}

/// "mm:ss", "h:mm:ss" or either in brackets → seconds.
fn parse_stamp(t: &str) -> Option<f64> {
    let t = t
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();
    let parts: Vec<&str> = t.split(':').collect();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    let mut total = 0u64;
    for p in parts {
        if p.is_empty() || p.len() > 9 || !p.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        total = total.checked_mul(60)?.checked_add(p.parse().ok()?)?;
    }
    Some(total as f64)
}

struct View {
    color: bool,
    links: Option<Links>,
    out: Vec<String>,
}

impl View {
    fn link(&self, label: String, at: Option<f64>) -> String {
        match &self.links {
            Some(links) => links.wrap(label, at),
            None => label,
        }
    }

    /// Fresh style whose colouring is decided by `color` alone, not the environment.
    fn st(&self) -> Style {
        Style::new().force_styling(self.color)
    }

    fn dim(&self, s: &str) -> String {
        self.st().dim().apply_to(s).to_string()
    }

    fn blank(&mut self) {
        self.out.push(String::new());
    }

    fn section(&mut self, name: &str) {
        self.blank();
        self.out.push(format!(
            "{}{}",
            " ".repeat(INDENT),
            self.st().bold().apply_to(name)
        ));
    }

    /// Wrapped paragraph(s), each line styled on its own so wrapping never splits a code.
    fn text(&mut self, indent: usize, text: &str, width: usize, style: &Style) {
        for line in wrap(text, width) {
            self.out.push(if line.is_empty() {
                String::new()
            } else {
                format!("{}{}", " ".repeat(indent), style.apply_to(line))
            });
        }
    }

    fn bar(
        &self,
        w: usize,
        cells: Option<(usize, usize)>,
        accent: &Style,
        duration: f64,
    ) -> String {
        let (lo, hi) = cells.map_or((0, 0), |(lo, hi)| (lo.min(w), hi.min(w)));
        if self.links.as_ref().is_some_and(Links::can_seek) {
            let mut bar = String::new();
            for cell in 0..w {
                let highlighted = cell >= lo && cell < hi;
                let style = if highlighted {
                    accent.clone()
                } else {
                    self.st().dim()
                };
                let glyph = if highlighted { FILL } else { TRACK };
                bar.push_str(&self.link(
                    style.apply_to(glyph).to_string(),
                    Some(duration * cell as f64 / w as f64),
                ));
            }
            return bar;
        }
        let run = |n: usize, ch: char, st: Style| {
            if n == 0 {
                String::new()
            } else {
                st.apply_to(ch.to_string().repeat(n)).to_string()
            }
        };
        format!(
            "{}{}{}",
            run(lo, TRACK, self.st().dim()),
            run(hi - lo, FILL, accent.clone()),
            run(w - hi, TRACK, self.st().dim()),
        )
    }
}

/// Remove terminal control sequences and invisible/format characters from untrusted text.
/// Newlines survive (CR/CRLF become `\n`, tabs become spaces).
fn sanitize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\u{1b}' => skip_escape(&mut it),
            '\u{9b}' => skip_csi(&mut it),
            '\u{90}' | '\u{98}' | '\u{9d}' | '\u{9e}' | '\u{9f}' => skip_string(&mut it),
            '\r' => {
                if it.peek() != Some(&'\n') {
                    out.push('\n');
                }
            }
            '\n' => out.push('\n'),
            '\t' => out.push(' '),
            c if is_invisible(c) => {}
            c => out.push(c),
        }
    }
    out
}

/// Sanitized text with every whitespace run collapsed to one space.
fn one_line(s: &str) -> String {
    sanitize(s).split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_invisible(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{61c}'
                | '\u{200b}'
                | '\u{200e}'..='\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{206f}'
                | '\u{feff}'
                | '\u{e0000}'..='\u{e007f}'
        )
}

/// After ESC: CSI, string-type (OSC/DCS/SOS/PM/APC) or two-/three-byte sequences.
fn skip_escape(it: &mut Peekable<Chars>) {
    match it.peek().copied() {
        Some('[') => {
            it.next();
            skip_csi(it);
        }
        Some(']' | 'P' | 'X' | '^' | '_') => {
            it.next();
            skip_string(it);
        }
        Some(' '..='/') => {
            while matches!(it.peek(), Some(' '..='/')) {
                it.next();
            }
            if matches!(it.peek(), Some('0'..='~')) {
                it.next();
            }
        }
        Some(c) if !c.is_control() => {
            it.next();
        }
        _ => {}
    }
}

/// CSI body: parameter/intermediate bytes, then one final byte.
fn skip_csi(it: &mut Peekable<Chars>) {
    while let Some(&c) = it.peek() {
        match c {
            ' '..='?' => {
                it.next();
            }
            '@'..='~' => {
                it.next();
                return;
            }
            _ => return,
        }
    }
}

/// String-type sequence body up to BEL / ST. A newline or a fresh ESC ends it unconsumed.
fn skip_string(it: &mut Peekable<Chars>) {
    while let Some(&c) = it.peek() {
        match c {
            '\u{7}' | '\u{9c}' => {
                it.next();
                return;
            }
            '\u{1b}' => {
                let mut ahead = it.clone();
                ahead.next();
                if ahead.peek() == Some(&'\\') {
                    ahead.next();
                    *it = ahead;
                }
                return;
            }
            '\n' | '\r' => return,
            _ => {
                it.next();
            }
        }
    }
}

fn char_width(c: char) -> usize {
    measure_text_width(c.encode_utf8(&mut [0; 4]))
}

/// Greedy word wrap by display width. Newlines in `text` start new lines (blank runs collapse
/// to one blank line); words wider than `width` are split by character. No leading/trailing
/// blank lines.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines: Vec<String> = Vec::new();
    for para in text.split('\n') {
        let mut cur = String::new();
        let mut cw = 0;
        for word in para.split_whitespace() {
            let ww = measure_text_width(word);
            if cw > 0 && cw + 1 + ww <= width {
                cur.push(' ');
                cur.push_str(word);
                cw += 1 + ww;
                continue;
            }
            if cw > 0 {
                lines.push(std::mem::take(&mut cur));
                cw = 0;
            }
            if ww <= width {
                cur.push_str(word);
                cw = ww;
                continue;
            }
            for c in word.chars() {
                let w = char_width(c);
                if cw > 0 && cw + w > width {
                    lines.push(std::mem::take(&mut cur));
                    cw = 0;
                }
                cur.push(c);
                cw += w;
            }
        }
        lines.push(cur);
    }
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    for l in lines {
        if l.is_empty() && out.last().is_none_or(|p| p.is_empty()) {
            continue;
        }
        out.push(l);
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tx_core::report::{Takeaway, Watch};

    fn transcript(duration: Option<f64>) -> Transcript {
        Transcript {
            title: "A talk".into(),
            url: "https://example.com/watch?v=abc".into(),
            uploader: Some("Someone".into()),
            duration,
            source: "captions en".into(),
            segs: vec![],
        }
    }

    fn analysis(advice: &str, from: Option<&str>, to: Option<&str>) -> AgentAnalysis {
        AgentAnalysis {
            relevance: 3,
            verdict: "Useful in parts.".into(),
            tldr: "Short summary.".into(),
            takeaways: vec![
                Takeaway {
                    at: Some("01:05".into()),
                    text: "First point".into(),
                },
                Takeaway {
                    at: None,
                    text: "Unplaced point".into(),
                },
            ],
            watch: Watch {
                advice: advice.into(),
                from: from.map(Into::into),
                to: to.map(Into::into),
                reason: "Because.".into(),
            },
        }
    }

    /// Panics unless every escape in `s` is a plain SGR (`ESC [ digits/; m`).
    fn assert_only_sgr(s: &str) {
        let mut it = s.chars().peekable();
        while let Some(c) = it.next() {
            if c == '\u{1b}' {
                assert_eq!(it.next(), Some('['), "non-CSI escape in output");
                loop {
                    match it.next() {
                        Some('0'..='9' | ';') => {}
                        Some('m') => break,
                        other => panic!("non-SGR escape terminator {other:?}"),
                    }
                }
            } else {
                assert!(c == '\n' || !c.is_control(), "control char {c:?} in output");
            }
        }
    }

    fn bar_line(out: &str) -> &str {
        out.lines()
            .find(|l| {
                let t = l.trim();
                !t.is_empty() && t.chars().all(|c| c == TRACK || c == FILL)
            })
            .expect("timeline bar line")
    }

    fn count(s: &str, ch: char) -> usize {
        s.chars().filter(|&c| c == ch).count()
    }

    #[test]
    fn terminal_controls_from_every_field_are_neutralised() {
        let evil = "a\u{1b}]8;;http://evil.test\u{7}link\u{1b}]8;;\u{7}\u{1b}[2J\u{1b}[31mred\u{1b}[0m\
            \u{9b}2Jb\u{9d}0;pwn\u{9c}c\u{1b}Pdcs\u{1b}\\d\u{7}\u{8}\r\u{202e}e\u{e0041}f";
        let mut t = transcript(Some(600.0));
        t.title = evil.into();
        t.url = format!("https://x.test/{evil}");
        t.uploader = Some(evil.into());
        t.source = evil.into();
        let mut a = analysis(evil, Some(evil), Some("09:00"));
        a.verdict = evil.into();
        a.tldr = evil.into();
        a.watch.reason = evil.into();
        a.takeaways[0].text = evil.into();
        a.takeaways[0].at = Some(evil.into());
        for color in [false, true] {
            let out = render(&t, &a, 60, color, true);
            assert_only_sgr(&out);
            if !color {
                assert!(!out.contains('\u{1b}'));
            }
            for bad in [
                "evil",
                "2J",
                "pwn",
                "dcs",
                "\u{202e}",
                "\u{e0041}",
                "\u{9b}",
            ] {
                assert!(!out.contains(bad), "{bad:?} leaked");
            }
            assert!(out.contains("link"), "visible text around sequences kept");
        }
    }

    #[test]
    fn color_flag_is_deterministic_and_plain_output_has_no_escapes() {
        let (t, a) = (
            transcript(Some(100.0)),
            analysis("skim", Some("00:10"), Some("00:20")),
        );
        assert!(!render(&t, &a, 80, false, false).contains('\u{1b}'));
        let colored = render(&t, &a, 80, true, false);
        assert!(colored.contains('\u{1b}'));
        assert_only_sgr(&colored);
    }

    #[test]
    fn every_line_fits_width_with_unicode_and_long_urls() {
        let mut t = transcript(Some(3725.0));
        t.title = "日本語のとても長いタイトル ".repeat(8);
        t.url = format!("https://example.com/{}", "ä".repeat(300));
        let mut a = analysis("skim", Some("10:00"), Some("20:00"));
        a.verdict = "漢字".repeat(60);
        a.watch.reason = format!("😀 {}", "x".repeat(200));
        a.takeaways[0].text = "multi word ".repeat(30);
        for (asked, effective) in [(5, 40), (40, 40), (47, 47), (80, 80), (500, 120)] {
            let out = render(&t, &a, asked, false, false);
            for line in out.lines() {
                let w = measure_text_width(line);
                assert!(w <= effective, "{w} > {effective}: {line:?}");
            }
            assert_eq!(measure_text_width(bar_line(&out)), effective - INDENT);
        }
    }

    #[test]
    fn wrap_keeps_newlines_and_splits_oversized_words() {
        assert_eq!(wrap("one\n\n\n\ntwo three", 20), ["one", "", "two three"]);
        assert_eq!(wrap("\n\n  x  \n\n", 10), ["x"]);
        assert_eq!(wrap("abcdefghij", 4), ["abcd", "efgh", "ij"]);
        // A double-width char never straddles the edge.
        assert_eq!(wrap("日日日", 5), ["日日", "日"]);
        assert!(wrap("", 10).is_empty());
    }

    #[test]
    fn unknown_duration_has_no_timeline() {
        let a = analysis("skim", Some("01:00"), Some("02:00"));
        for d in [
            None,
            Some(0.0),
            Some(-3.0),
            Some(f64::NAN),
            Some(f64::INFINITY),
        ] {
            let out = render(&transcript(d), &a, 70, false, false);
            assert!(!out.contains(TRACK) && !out.contains(FILL), "{d:?}");
            assert!(out.contains("01:00–02:00"), "range still stated for {d:?}");
        }
    }

    #[test]
    fn timeline_highlight_follows_advice_and_range() {
        let t = transcript(Some(100.0));
        let w = 80 - 2 * INDENT;

        let skim = render(
            &t,
            &analysis("skim", Some("00:25"), Some("01:15")),
            80,
            false,
            false,
        );
        let bar = bar_line(&skim).trim();
        assert_eq!(measure_text_width(bar), w);
        assert_eq!(count(bar, FILL), w / 2);
        assert_eq!(bar.chars().position(|c| c == FILL), Some(w / 4));

        let watch = render(&t, &analysis("watch", None, None), 80, false, false);
        assert_eq!(count(bar_line(&watch), FILL), w);

        let skip = render(
            &t,
            &analysis("skip", Some("00:10"), Some("00:20")),
            80,
            false,
            false,
        );
        assert_eq!(count(bar_line(&skip), FILL), 0);

        // Skim without a usable range highlights nothing.
        for (f, to) in [
            (None, None),
            (Some("00:30"), Some("00:30")),
            (Some("02:00"), Some("01:00")),
        ] {
            let out = render(&t, &analysis("skim", f, to), 80, false, false);
            assert_eq!(count(bar_line(&out), FILL), 0);
        }
    }

    #[test]
    fn range_cells_boundaries() {
        assert_eq!(range_cells(0.0, 100.0, 100.0, 50), Some((0, 50)));
        // Sub-cell ranges still show one cell, at the edges too.
        assert_eq!(range_cells(0.0, 0.1, 100.0, 50), Some((0, 1)));
        assert_eq!(range_cells(99.9, 100.0, 100.0, 50), Some((49, 50)));
        // Past the end is clamped into the last cell.
        assert_eq!(range_cells(150.0, 200.0, 100.0, 50), Some((49, 50)));
        assert_eq!(range_cells(50.0, 500.0, 100.0, 50), Some((25, 50)));
        assert_eq!(range_cells(10.0, 10.0, 100.0, 50), None);
        assert_eq!(range_cells(20.0, 10.0, 100.0, 50), None);
        assert_eq!(range_cells(1.0, 2.0, 0.0, 50), None);
        assert_eq!(range_cells(1.0, 2.0, f64::NAN, 50), None);
        assert_eq!(range_cells(1.0, 2.0, 100.0, 0), None);
    }

    #[test]
    fn stamps_parse_strictly() {
        assert_eq!(parse_stamp("[1:02:03]"), Some(3723.0));
        assert_eq!(parse_stamp(" 12:34 "), Some(754.0));
        for bad in [
            "",
            "12",
            "1:2:3:4",
            "a:bc",
            "1:",
            ":30",
            "-1:30",
            "99999999999:00",
        ] {
            assert_eq!(parse_stamp(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn takeaway_without_valid_stamp_is_still_listed() {
        let mut a = analysis("watch", None, None);
        a.takeaways[0].at = Some("not a time".into());
        a.takeaways[1].text = "   ".into();
        let out = render(&transcript(Some(100.0)), &a, 70, false, false);
        assert!(out.contains("First point"));
        assert!(!out.contains("not a time"));
    }

    #[test]
    fn clickable_sections_open_the_corresponding_video_time() {
        let mut t = transcript(Some(100.0));
        t.url = "https://www.youtube.com/watch?v=abc&t=1s".into();
        let a = analysis("skim", Some("00:25"), Some("01:15"));
        let out = render(&t, &a, 80, true, true);
        let links: Vec<_> = out
            .split("\x1b]8;;")
            .skip(1)
            .filter_map(|part| part.split_once("\x1b\\"))
            .filter(|(url, _)| !url.is_empty())
            .map(|(url, label)| (url, sanitize(label)))
            .collect();
        for (label, seconds) in [("[01:05]", 65), ("00:25", 25), ("01:15", 75)] {
            assert!(
                links.iter().any(|(url, text)| {
                    text == label
                        && *url == format!("https://www.youtube.com/watch?v=abc&t={seconds}s")
                }),
                "{label} did not target {seconds}s"
            );
        }
        let cells: Vec<_> = links
            .iter()
            .filter(|(_, label)| label == "░" || label == "█")
            .collect();
        assert_eq!(cells.len(), 76);
        assert_eq!(cells[0].0, "https://www.youtube.com/watch?v=abc&t=0s");
        assert_eq!(cells[38].0, "https://www.youtube.com/watch?v=abc&t=50s");
        assert_eq!(cells[75].0, "https://www.youtube.com/watch?v=abc&t=98s");
        for line in out.lines() {
            assert!(measure_text_width(&sanitize(line)) <= 80);
        }
    }

    #[test]
    fn hyperlink_policy_is_independent_of_color_and_rejects_model_escape_injection() {
        let mut t = transcript(None);
        t.url = "https://youtu.be/abc".into();
        let mut a = analysis("skim", Some("01:00"), Some("02:00"));
        a.takeaways[0].text = "\x1b]8;;https://evil.test\x07injected\x1b]8;;\x07".into();
        let linked = render(&t, &a, 80, false, true);
        assert!(linked.contains("\x1b]8;;https://www.youtube.com/watch?v=abc&t=65s"));
        assert!(!linked.contains("\x1b["));
        assert!(!linked.contains("evil.test"));
        assert!(!linked.contains(TRACK));
        assert!(linked.contains("injected"));
        assert!(!render(&t, &a, 80, true, false).contains("\x1b]8;;"));
        assert!(!render(&t, &a, 80, false, false).contains('\x1b'));
    }
}
