//! Several videos being processed at once, shown live: one row each, redrawn in place in a
//! terminal. Elsewhere (scheduled runs, pipes) one plain line per finished video.

use dialoguer::console::{Term, style};
use std::io::IsTerminal;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub enum Stage {
    Queued,
    /// A step without a measurable end: "getting transcript", "checking show notes".
    Working(String),
    /// Local Whisper transcription, with its percentage once known.
    Transcribing(Option<u8>),
    /// Held back, e.g. behind another local transcription.
    Waiting(String),
    Analyzing(Instant),
    Done {
        relevance: i64,
        advice: String,
        range: Option<String>,
    },
    Passed(String),
    Failed(String),
}

impl Stage {
    fn finished(&self) -> bool {
        matches!(
            self,
            Stage::Done { .. } | Stage::Passed(_) | Stage::Failed(_)
        )
    }
}

struct Row {
    source: String,
    title: String,
    stage: Stage,
}

struct State {
    rows: Vec<Row>,
    /// Lines of the last frame, cleared before the next one.
    drawn: usize,
    tick: usize,
    stopped: bool,
}

/// Cheap to clone; every clone updates the same view.
#[derive(Clone)]
pub struct Live {
    state: Arc<Mutex<State>>,
    header: Arc<String>,
    started: Instant,
    /// Scores from this up count as worth the user's time.
    worth: i64,
    tty: bool,
    /// The thread redrawing the view; joined by `finish` so nothing draws after it.
    painter: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}

const SPINNER: [&str; 4] = ["◐", "◓", "◑", "◒"];
const BAR: usize = 12;
const SOURCE: usize = 16;
const STATUS: usize = 34;

impl Live {
    /// Start showing `rows` (source, title). In a terminal the view redraws itself until `finish`.
    pub fn start(header: String, rows: Vec<(String, String)>, worth: i64) -> Live {
        let tty = std::io::stderr().is_terminal() && std::env::var("TERM").as_deref() != Ok("dumb");
        let live = Live {
            state: Arc::new(Mutex::new(State {
                rows: rows
                    .into_iter()
                    .map(|(source, title)| Row {
                        source,
                        title,
                        stage: Stage::Queued,
                    })
                    .collect(),
                drawn: 0,
                tick: 0,
                stopped: false,
            })),
            header: Arc::new(header),
            started: Instant::now(),
            worth,
            tty,
            painter: Arc::new(Mutex::new(None)),
        };
        if tty {
            let view = live.clone();
            let painter = std::thread::spawn(move || {
                while !view.draw(false) {
                    std::thread::sleep(Duration::from_millis(120));
                }
            });
            *live.painter.lock().unwrap() = Some(painter);
        } else {
            eprintln!("tx: {}", live.header);
        }
        live
    }

    pub fn set(&self, i: usize, stage: Stage) {
        let mut s = self.state.lock().unwrap();
        let Some(row) = s.rows.get_mut(i) else { return };
        let done = stage.finished() && !row.stage.finished();
        row.stage = stage;
        if done && !self.tty {
            let n = s.rows.iter().filter(|r| r.stage.finished()).count();
            let row = &s.rows[i];
            eprintln!(
                "tx: [{n}/{}] {} · {} — {}",
                s.rows.len(),
                row.source,
                row.title,
                plain(&row.stage)
            );
        }
    }

    /// Replace a row's title once it's known (a pasted link shows its URL until then).
    pub fn title(&self, i: usize, source: &str, title: &str) {
        if let Some(row) = self.state.lock().unwrap().rows.get_mut(i) {
            row.source = source.to_string();
            row.title = title.to_string();
        }
    }

    /// Stop redrawing and leave the last frame on screen.
    pub fn finish(&self) {
        if self.tty {
            self.state.lock().unwrap().stopped = true;
            if let Some(painter) = self.painter.lock().unwrap().take() {
                let _ = painter.join();
            }
            self.draw(true);
        }
    }

    /// Redraw the frame. The painter thread stops (true) without drawing once the view is
    /// stopped; only `finish` draws the last frame, so later output is never cleared.
    fn draw(&self, last: bool) -> bool {
        let mut s = self.state.lock().unwrap();
        if s.stopped && !last {
            return true;
        }
        let term = Term::stderr();
        let (height, width) = term.size();
        let (height, width) = (height as usize, (width as usize).max(60));
        s.tick += 1;
        let tick = s.tick;
        let mut lines = vec![style(self.header.as_str()).bold().to_string()];
        // A long batch on a short terminal: the rows still moving, then the newest finished ones.
        let room = height.saturating_sub(4).max(3);
        let order: Vec<usize> = if s.rows.len() <= room {
            (0..s.rows.len()).collect()
        } else {
            let mut active: Vec<usize> = (0..s.rows.len())
                .filter(|&i| !s.rows[i].stage.finished())
                .collect();
            active.truncate(room);
            let mut shown = active;
            shown.extend(
                (0..s.rows.len())
                    .filter(|&i| s.rows[i].stage.finished())
                    .take(room - shown.len()),
            );
            shown.sort_unstable();
            shown
        };
        for i in order {
            lines.push(row(&s.rows[i], width, tick));
        }
        lines.push(style("─".repeat(width.min(96))).dim().to_string());
        lines.push(self.summary(&s.rows));
        let _ = term.clear_last_lines(s.drawn);
        for l in &lines {
            let _ = term.write_line(l);
        }
        s.drawn = lines.len();
        s.stopped
    }

    fn summary(&self, rows: &[Row]) -> String {
        let done = rows.iter().filter(|r| r.stage.finished()).count();
        let worth = rows
            .iter()
            .filter(|r| matches!(r.stage, Stage::Done { relevance, .. } if relevance >= self.worth))
            .count();
        let failed = rows
            .iter()
            .filter(|r| matches!(r.stage, Stage::Failed(_)))
            .count();
        let mut s = format!(
            " {done} of {} done · {worth} worth your time · {}",
            rows.len(),
            clock(self.started.elapsed())
        );
        if failed > 0 {
            s.push_str(&format!(" · {}", style(format!("{failed} failed")).red()));
        }
        s
    }
}

fn clock(d: Duration) -> String {
    let s = d.as_secs();
    format!("{}:{:02}", s / 60, s % 60)
}

fn fit(text: &str, width: usize) -> String {
    let n = text.chars().count();
    if n <= width {
        format!("{text}{}", " ".repeat(width - n))
    } else {
        let mut t: String = text.chars().take(width.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

/// A bar filled to `pct`.
fn bar(pct: u8) -> String {
    let filled = (usize::from(pct.min(100)) * BAR).div_ceil(100);
    format!(
        "{}{}",
        style("━".repeat(filled)).cyan(),
        style("─".repeat(BAR - filled)).dim()
    )
}

/// A bar with a segment sweeping across: busy, no known end.
fn sweep(tick: usize) -> String {
    let span = 4;
    let pos = tick % (BAR + span);
    (0..BAR)
        .map(|i| {
            if i + span > pos && i <= pos {
                style("━").cyan().to_string()
            } else {
                style("─").dim().to_string()
            }
        })
        .collect()
}

fn row(r: &Row, width: usize, tick: usize) -> String {
    let spin = style(SPINNER[tick % SPINNER.len()]).cyan();
    let (icon, status) = match &r.stage {
        Stage::Queued => (
            style("○").dim().to_string(),
            style("queued").dim().to_string(),
        ),
        Stage::Working(what) => (
            spin.to_string(),
            format!("{} {}", fit(what, STATUS - BAR - 1), sweep(tick)),
        ),
        Stage::Transcribing(Some(p)) => (
            spin.to_string(),
            format!("{} {} {p:>3}%", fit("transcribing", 16), bar(*p)),
        ),
        Stage::Transcribing(None) => (
            spin.to_string(),
            format!("{} {}", fit("transcribing", 21), sweep(tick)),
        ),
        Stage::Waiting(what) => (
            style("◌").dim().to_string(),
            style(fit(what, STATUS)).dim().to_string(),
        ),
        Stage::Analyzing(since) => (
            spin.to_string(),
            format!(
                "{} {} {}",
                fit("analyzing", 16),
                sweep(tick),
                clock(since.elapsed())
            ),
        ),
        Stage::Done {
            relevance,
            advice,
            range,
        } => {
            let verdict = advice.to_uppercase();
            let verdict = match advice.as_str() {
                "watch" => style(verdict).green().bold(),
                "skim" => style(verdict).yellow().bold(),
                _ => style(verdict).dim(),
            };
            let icon = if advice == "skip" {
                style("✓").dim()
            } else {
                style("✓").green()
            };
            let range = range
                .as_deref()
                .map_or(String::new(), |r| style(format!(" {r}")).dim().to_string());
            (icon.to_string(), format!("{relevance}/5 {verdict}{range}"))
        }
        Stage::Passed(why) => (
            style("–").dim().to_string(),
            style(fit(&format!("passed: {why}"), STATUS))
                .dim()
                .to_string(),
        ),
        Stage::Failed(why) => (
            style("✗").red().to_string(),
            style(fit(why, STATUS)).red().to_string(),
        ),
    };
    // One column spare: a line that fills the terminal exactly can wrap, and the redraw miscount.
    let title = width.saturating_sub(2 + SOURCE + 1 + STATUS + 3).max(12);
    format!(
        " {icon} {} {} {status}",
        style(fit(&r.source, SOURCE)).bold(),
        fit(&r.title, title)
    )
}

/// A finished row for a log line.
fn plain(stage: &Stage) -> String {
    match stage {
        Stage::Done {
            relevance,
            advice,
            range,
        } => match range {
            Some(r) => format!("{relevance}/5 {advice} {r}"),
            None => format!("{relevance}/5 {advice}"),
        },
        Stage::Passed(why) => format!("passed: {why}"),
        Stage::Failed(why) => format!("failed: {why}"),
        _ => String::new(),
    }
}
