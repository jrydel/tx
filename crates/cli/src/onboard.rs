//! `tx onboard`: set tx up in the terminal, step by step: connect your AI apps, say what you want
//! to learn, pick what to follow (your AI can pre-pick from your subscriptions), turn on the
//! daily brief. Safe to run again: every step shows what's already set.

use crate::agents::{self, Agent, Target};
use crate::schedule;
use anyhow::{Context, Result, bail};
use dialoguer::console::{Style, measure_text_width, style};
use dialoguer::{Confirm, Input, MultiSelect, Select, theme::ColorfulTheme};
use std::io::IsTerminal;
use tx_core::db::{self, Db};
use tx_core::{debrief, feeds, podcasts};

const BROWSERS: [&str; 5] = ["chrome", "firefox", "safari", "edge", "brave"];
const STEPS: [&str; 4] = ["Your AI", "About you", "What to follow", "Daily brief"];
/// Width of one stop on the progress rail: the longest step name plus a gap.
const STOP: usize = 16;

/// The same visual language as the brief in the AI apps: green is done, bold is now, dim is later.
pub(crate) fn theme() -> ColorfulTheme {
    let s = |t: &str| style(t.to_string()).for_stderr();
    ColorfulTheme {
        prompt_prefix: s("›").dim(),
        success_prefix: s("✓").green(),
        error_prefix: s("✗").red(),
        active_item_prefix: s("›").bold(),
        inactive_item_prefix: s(" "),
        checked_item_prefix: s("●").green(),
        unchecked_item_prefix: s("○").dim(),
        picked_item_prefix: s("›").bold(),
        unpicked_item_prefix: s(" "),
        hint_style: Style::new().for_stderr().dim(),
        values_style: Style::new().for_stderr().green(),
        active_item_style: Style::new().for_stderr().bold(),
        ..ColorfulTheme::default()
    }
}

/// The setup's progress as a timeline, like a video's in the brief: stops fill in as you go.
///
/// ```text
///  ●━━━━━━━━━━━━━━━◉───────────────○───────────────○
///  Your AI         About you       What to follow  Daily brief
/// ```
fn rail(current: usize) {
    let mut line = String::from(" ");
    let mut labels = String::from(" ");
    for (i, name) in STEPS.iter().enumerate() {
        let done = i < current;
        let dot = match i.cmp(&current) {
            std::cmp::Ordering::Less => style("●").green(),
            std::cmp::Ordering::Equal => style("◉").bold(),
            std::cmp::Ordering::Greater => style("○").dim(),
        };
        line.push_str(&dot.to_string());
        if i + 1 < STEPS.len() {
            let seg = if done {
                style("━".repeat(STOP - 1)).green()
            } else {
                style("─".repeat(STOP - 1)).dim()
            };
            line.push_str(&seg.to_string());
        }
        let label = format!("{name:<STOP$}");
        let label = if i == current {
            style(label).bold()
        } else {
            style(label).dim()
        };
        labels.push_str(&label.to_string());
    }
    println!("\n{line}\n{}\n", labels.trim_end());
}

/// A rounded box around a few lines, for the welcome and the end.
fn card(title: &str, lines: &[String]) {
    let inner = lines
        .iter()
        .map(|l| measure_text_width(l))
        .chain([measure_text_width(title) + 2])
        .max()
        .unwrap_or(0);
    // "╭─ title " then the rule, as wide as the "│ line │" rows below.
    let rule = "─".repeat(inner - measure_text_width(title) - 1);
    println!("╭─ {} {rule}╮", style(title).bold());
    for l in lines {
        println!("│ {l}{} │", " ".repeat(inner - measure_text_width(l)));
    }
    println!("╰{}╯", "─".repeat(inner + 2));
}

fn note(text: &str) {
    println!("  {}", style(text).dim());
}

/// "Codex", "Codex or Claude Code", "Codex, Claude Code or Claude Desktop"; "your AI app" if none.
fn either(apps: &[&str]) -> String {
    match apps {
        [] => "your AI app".into(),
        [one] => one.to_string(),
        [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
    }
}

/// AI apps on this computer that have tx: registered directly or through the tx plugin.
fn connected_apps() -> Vec<&'static str> {
    let mut apps = vec![];
    let codex = agents::codex_home()
        .ok()
        .and_then(|h| std::fs::read_to_string(h.join("config.toml")).ok())
        .and_then(|t| t.parse::<toml_edit::DocumentMut>().ok())
        .is_some_and(|c| {
            c.get("mcp_servers").and_then(|s| s.get("tx")).is_some()
                || c.get("plugins")
                    .and_then(|p| p.get("tx@tx"))
                    .and_then(|t| t.get("enabled"))
                    .and_then(|e| e.as_bool())
                    == Some(true)
        });
    let json = |path: Option<std::path::PathBuf>| {
        path.and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .unwrap_or_default()
    };
    let claude_code =
        !json(dirs::home_dir().map(|h| h.join(".claude.json")))["mcpServers"]["tx"].is_null();
    let claude_desktop = !json(agents::claude_desktop_config())["mcpServers"]["tx"].is_null();
    for (on, name) in [
        (codex, "Codex"),
        (claude_code, "Claude Code"),
        (claude_desktop, "Claude Desktop"),
    ] {
        if on {
            apps.push(name);
        }
    }
    apps
}

/// Width the home screen wraps the debrief's summary to.
const DEBRIEF_WIDTH: usize = 68;

/// `text` broken into lines of at most `width` characters, at spaces.
fn wrap(text: &str, width: usize) -> impl Iterator<Item = String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(l) if l.chars().count() + 1 + word.chars().count() <= width => {
                l.push(' ');
                l.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines.into_iter()
}

/// What a bare `tx` shows once setup is done: you use tx inside your AI app, and where you stand.
pub fn home() -> Result<()> {
    let conn = db::connect()?;
    let profile = db::profile(&conn)?;
    let debrief = debrief::current(&conn)?;
    let channels = db::channels(&conn)?;
    let waiting = db::pending(&conn)?.len();
    let learned = db::learned(&conn)?;
    drop(conn);
    let watched: Vec<&db::Channel> = channels.iter().filter(|c| c.enabled).collect();
    let shows = watched.iter().filter(|c| c.podcast).count();
    let apps = connected_apps();
    let app = either(&apps);
    let tick = |t: String| format!("{} {t}", style("✓").green());
    let open = |t: String| format!("{} {t}", style("○").dim());
    // One line in the card: the profile can span paragraphs (your words, then what tx learned).
    let profile = profile.split_whitespace().collect::<Vec<_>>().join(" ");
    let short: String = profile.chars().take(60).collect();
    let mut lines = vec![
        format!(
            "tx is set up. Use it in {}: start a new chat and type",
            style(&app).bold()
        ),
        format!("  {}", style("Show me my tx brief").bold()),
        String::new(),
    ];
    if let Some(d) = debrief {
        lines.push(style(&d.headline).bold().to_string());
        lines.extend(wrap(&d.summary, DEBRIEF_WIDTH).map(|l| style(l).dim().to_string()));
        lines.push(String::new());
    }
    lines.extend([
        tick(format!(
            "Scored for: {short}{}",
            if profile.chars().count() > 60 {
                "…"
            } else {
                ""
            }
        )),
        tick(match shows {
            0 => format!("Following {}", count(watched.len(), "channel")),
            s => format!(
                "Following {} and {}",
                count(watched.len() - s, "channel"),
                count(s, "podcast")
            ),
        }),
        if schedule::scheduled() {
            tick("A brief every morning".into())
        } else {
            open(format!(
                "No daily brief yet {}",
                style("(tx schedule)").dim()
            ))
        },
    ]);
    match &learned {
        Some(l) => lines.insert(
            lines.len() - 1,
            tick(format!(
                "Learned from your coding sessions {}",
                style(format!(
                    "(updated {}; tx learn --show)",
                    l.at.get(..10).unwrap_or(&l.at)
                ))
                .dim()
            )),
        ),
        None if tx_core::sessions::available() => lines.insert(
            lines.len() - 1,
            open(format!(
                "Not learned from your coding sessions yet {}",
                style("(tx learn)").dim()
            )),
        ),
        None => {}
    }
    if waiting > 0 {
        lines.push(open(format!(
            "{waiting} new videos waiting {}",
            style("(ask in your app, or tx run)").dim()
        )));
    }
    if apps.is_empty() {
        lines.push(open(format!(
            "Not connected to an AI app yet {}",
            style("(tx onboard)").dim()
        )));
    }
    println!();
    card("tx", &lines);
    println!();
    for (cmd, what) in [
        (
            "tx <link> -q",
            "should you watch it: verdict, best part, takeaways",
        ),
        ("tx <link>", "transcript of any video or episode"),
        ("tx learn", "learn who you are from your coding sessions"),
        ("tx onboard", "change your setup"),
        ("tx run", "screen new uploads now, in this terminal"),
        ("tx reset", "start over as a new user"),
        ("tx --help", "everything else"),
    ] {
        println!("  {:<14} {}", style(cmd).bold(), style(what).dim());
    }
    println!();
    Ok(())
}

/// "1 channel", "3 channels".
fn count(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// Start over: forget the profile and what's followed, and turn off the daily brief, so the next
/// `tx` runs setup again. With `all`, also delete every analysis, digest and cached transcript.
pub fn reset(all: bool, yes: bool) -> Result<()> {
    let what = if all {
        "your profile, what you follow, the daily brief, and every analysis, digest and cached transcript"
    } else {
        "your profile, what you follow and the daily brief (analyses and transcripts are kept)"
    };
    println!("This resets {what}.");
    if !yes {
        if !std::io::stdin().is_terminal() {
            bail!("run it in a terminal to confirm, or pass --yes");
        }
        if !Confirm::with_theme(&theme())
            .with_prompt("Reset tx?")
            .default(false)
            .interact()?
        {
            return Ok(());
        }
    }
    if schedule::scheduled() {
        schedule::uninstall()?;
    }
    let data = tx_core::paths::data_dir();
    if all {
        // Tools tx downloaded (yt-dlp, whisper models) stay; they're not your data.
        for f in ["tx.db", "tx.db-wal", "tx.db-shm"] {
            let _ = std::fs::remove_file(data.join(f));
        }
        let _ = std::fs::remove_dir_all(data.join("digests"));
        let _ = std::fs::remove_dir_all(tx_core::paths::cache_dir());
    } else {
        let conn = db::connect()?;
        let mut s = db::get_settings(&conn)?;
        s.profile.clear();
        db::save_settings(&conn, &s)?;
        db::forget_learned(&conn)?;
        conn.execute("UPDATE channels SET enabled = 0, enabled_at = NULL", [])?;
    }
    println!(
        "{} Reset. Run {} to set it up again.",
        style("✓").green(),
        style("tx").bold()
    );
    Ok(())
}

pub async fn run() -> Result<()> {
    if !std::io::stdin().is_terminal() {
        bail!("tx onboard asks questions, so run it in a terminal");
    }
    let theme = theme();
    let db: Db = std::sync::Mutex::new(db::connect()?);
    println!();
    card(
        "tx",
        &[
            "Don't watch the video. Read it, then decide.".into(),
            style(
                "Four questions, about two minutes. Ctrl+C stops; run `tx` any time to continue.",
            )
            .dim()
            .to_string(),
        ],
    );

    rail(0);
    let apps = connect_apps(&theme)?;
    rail(1);
    about(&theme, &db).await?;
    rail(2);
    follow(&theme, &db).await?;
    rail(3);
    let daily = daily(&theme)?;
    rail(4);

    let (following, profile) = {
        let conn = db.lock().unwrap();
        let n = db::channels(&conn)?.iter().filter(|c| c.enabled).count();
        (n, db::profile(&conn)?)
    };
    let tick = |t: String| format!("{} {t}", style("✓").green());
    let mut lines = vec![];
    if !profile.trim().is_empty() {
        lines.push(tick("Scored for what you want to learn".into()));
    }
    if following > 0 {
        lines.push(tick(format!("Following {following}")));
    }
    if let Some(at) = &daily {
        lines.push(tick(format!("A brief every day at {at}")));
    }
    let app = either(&apps);
    lines.push(String::new());
    lines.push(format!(
        "Quit {app} completely, open it again, start a new chat and type:"
    ));
    lines.push(format!("  {}", style("Show me my tx brief").bold()));
    card("You're set", &lines);
    if following > 0
        && let Ok(agent) = Agent::detect()
        && Confirm::with_theme(&theme)
            .with_prompt(format!(
                "Screen the latest upload of each channel you follow now, with {}? (a few minutes)",
                agent.program()
            ))
            .default(false)
            .interact()?
    {
        if let Err(e) = feeds::poll(&db).await {
            println!("  {} checking for uploads failed: {e:#}", style("✗").red());
        }
        db::queue_first_brief(&db.lock().unwrap(), 1)?;
        crate::pipeline::screen(agent, 3).await?;
    }
    Ok(())
}

/// Which AI apps to connect tx to; the ones found on this computer are pre-ticked. Returns the
/// names of the apps connected.
fn connect_apps(theme: &ColorfulTheme) -> Result<Vec<&'static str>> {
    note("tx works inside your AI app. Tick the ones you use (space), then press enter.");
    let codex = Agent::Codex.find().is_some() || agents::codex_home().is_ok_and(|h| h.exists());
    let claude_code = Agent::Claude.find().is_some();
    let claude_desktop = agents::claude_desktop_config()
        .and_then(|p| p.parent().map(|d| d.exists()))
        .unwrap_or(false);
    let apps = [
        (
            Target::Codex,
            "Codex",
            "Codex (the ChatGPT desktop app or CLI)",
            codex,
        ),
        (
            Target::ClaudeCode,
            "Claude Code",
            "Claude Code",
            claude_code,
        ),
        (
            Target::ClaudeDesktop,
            "Claude Desktop",
            "Claude Desktop",
            claude_desktop,
        ),
    ];
    if !apps.iter().any(|a| a.3) {
        note(
            "No AI app found. Install Codex (ChatGPT), Claude Code or Claude Desktop, then run `tx onboard` again.",
        );
    }
    let picked = MultiSelect::with_theme(theme)
        .with_prompt("Use tx in")
        .items(apps.iter().map(|a| a.2))
        .defaults(&apps.iter().map(|a| a.3).collect::<Vec<_>>())
        .interact()?;
    let mut connected = vec![];
    for i in picked {
        match agents::setup(apps[i].0) {
            Ok(()) => connected.push(apps[i].1),
            Err(e) => println!("  {} {}: {e:#}", style("✗").red(), apps[i].2),
        }
    }
    Ok(connected)
}

async fn about(theme: &ColorfulTheme, db: &Db) -> Result<()> {
    note("Who you are and what you want to learn. Every video and episode is scored against this.");
    let learned = db::learned(&db.lock().unwrap())?.is_some();
    if !learned
        && tx_core::sessions::available()
        && Agent::detect().is_ok()
        && Confirm::with_theme(theme)
            .with_prompt("Learn it from your recent Claude Code, Codex and Oh My Pi sessions? (your own prompts, redacted)")
            .default(true)
            .interact()?
    {
        let options = crate::learn::Options {
            agent: None,
            days: tx_core::learn::DEFAULT_DAYS,
            dry_run: false,
            yes: false,
        };
        if let Err(e) = crate::learn::run(theme, options).await {
            println!("  {} {e:#}", style("✗").red());
        }
        note("Anything to add in your own words, e.g. what you want to learn? Leave it empty to skip.");
    } else {
        note("e.g. I'm a backend engineer building LLM features. I want concrete agent and eval");
        note("     techniques and notable model launches; skip hype and drama.");
    }
    let conn = db.lock().unwrap();
    let mut settings = db::get_settings(&conn)?;
    let profile: String = Input::with_theme(theme)
        .with_prompt("You")
        .with_initial_text(settings.profile.clone())
        .allow_empty(true)
        .interact_text()?;
    if !profile.trim().is_empty() {
        settings.profile = profile.trim().to_string();
        db::save_settings(&conn, &settings)?;
    }
    Ok(())
}

async fn follow(theme: &ColorfulTheme, db: &Db) -> Result<()> {
    note(
        "Only what you follow gets screened. Add from as many places as you like, then choose Done.",
    );
    loop {
        let following = db::channels(&db.lock().unwrap())?
            .iter()
            .filter(|c| c.enabled)
            .count();
        let done = if following > 0 {
            format!("Done (following {following})")
        } else {
            "Skip for now".to_string()
        };
        let choices = [
            "Import my YouTube subscriptions from a browser",
            "Import a file (Google Takeout .zip, or a podcast app's .opml)",
            "Add YouTube channels (@handles or links)",
            "Add podcasts by name",
            "Change what I follow",
            done.as_str(),
        ];
        let choice = Select::with_theme(theme)
            .with_prompt("Add")
            .items(choices)
            .default(if following > 0 { 5 } else { 0 })
            .interact()?;
        let result = match choice {
            0 => {
                let b = Select::with_theme(theme)
                    .with_prompt("Browser signed in to YouTube")
                    .items(BROWSERS)
                    .default(0)
                    .interact()?;
                match feeds::import_subscriptions(db, BROWSERS[b]).await {
                    Ok((found, _)) => {
                        println!("  {} Imported {found} subscriptions.", style("✓").green());
                        pick(theme, db).await
                    }
                    Err(e) => Err(e),
                }
            }
            1 => {
                let path: String = Input::with_theme(theme)
                    .with_prompt("File")
                    .interact_text()?;
                let path = path.trim().trim_matches(['"', '\'']);
                let path = match path.strip_prefix("~/") {
                    Some(rest) => dirs::home_dir().context("no home dir")?.join(rest),
                    None => path.into(),
                };
                let opml = path.extension().is_some_and(|e| {
                    e.eq_ignore_ascii_case("opml") || e.eq_ignore_ascii_case("xml")
                });
                let imported = if opml {
                    podcasts::import_opml(db, &path)
                } else {
                    feeds::import_takeout(db, &path)
                };
                match imported {
                    Ok((found, _)) => {
                        println!("  {} Imported {found}.", style("✓").green());
                        pick(theme, db).await
                    }
                    Err(e) => Err(e),
                }
            }
            2 => {
                let refs: String = Input::with_theme(theme)
                    .with_prompt("Channels, comma-separated")
                    .interact_text()?;
                for r in refs.split(',').map(str::trim).filter(|r| !r.is_empty()) {
                    match feeds::add_channel(db, r).await {
                        Ok(title) => println!("  {} Following {title}", style("✓").green()),
                        Err(e) => println!("  {} {r}: {e:#}", style("✗").red()),
                    }
                }
                Ok(())
            }
            3 => add_podcasts(theme, db).await,
            4 => pick(theme, db).await,
            _ => return Ok(()),
        };
        if let Err(e) = result {
            println!("  {} {e:#}", style("✗").red());
        }
    }
}

/// Search each name in Apple's directory and follow the chosen show.
async fn add_podcasts(theme: &ColorfulTheme, db: &Db) -> Result<()> {
    let names: String = Input::with_theme(theme)
        .with_prompt("Podcasts, comma-separated")
        .interact_text()?;
    for name in names.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        let found = podcasts::search(name).await?;
        if found.is_empty() {
            println!("  {} No podcast found for \"{name}\".", style("✗").red());
            continue;
        }
        let labels: Vec<String> = found
            .iter()
            .take(5)
            .map(|s| {
                format!(
                    "{}  {}",
                    s["title"].as_str().unwrap_or("?"),
                    style(s["author"].as_str().unwrap_or("")).dim()
                )
            })
            .chain(["None of these".to_string()])
            .collect();
        let i = Select::with_theme(theme)
            .with_prompt(format!("\"{name}\""))
            .items(&labels)
            .default(0)
            .interact()?;
        if let Some(show) = found.get(i).filter(|_| i < labels.len() - 1) {
            let title = podcasts::add(db, show["feed_url"].as_str().unwrap_or_default()).await?;
            println!("  {} Following {title}", style("✓").green());
        }
    }
    Ok(())
}

/// Tick what to follow from everything tx knows; the AI's picks are pre-ticked when it can help.
async fn pick(theme: &ColorfulTheme, db: &Db) -> Result<()> {
    let (channels, profile) = {
        let conn = db.lock().unwrap();
        (db::channels(&conn)?, db::profile(&conn)?)
    };
    if channels.is_empty() {
        return Ok(());
    }
    let mut picks: Vec<(String, String)> = Vec::new();
    if !profile.trim().is_empty()
        && let Ok(agent) = Agent::detect()
        && Confirm::with_theme(theme)
            .with_prompt(format!(
                "Let {} pick the ones that fit you? (about half a minute)",
                agent.program()
            ))
            .default(true)
            .interact()?
    {
        note(&format!("Asking {}…", agent.program()));
        match crate::guide::suggest(agent, &profile, &channels) {
            Ok(p) => {
                println!(
                    "  {} {} picked {} for you, marked ●. Change anything.",
                    style("✓").green(),
                    agent.program(),
                    p.len()
                );
                picks = p;
            }
            Err(e) => note(&format!(
                "Couldn't get suggestions ({e:#}); tick them yourself."
            )),
        }
    }
    let why = |id: &str| picks.iter().find(|(p, _)| p == id).map(|(_, w)| w.as_str());
    // Picks first, then what's already followed, then the rest.
    let mut order: Vec<&db::Channel> = channels.iter().collect();
    order.sort_by_key(|c| (why(&c.id).is_none(), !c.enabled, c.title.to_lowercase()));
    let labels: Vec<String> = order
        .iter()
        .map(|c| {
            let kind = if c.podcast { "podcast" } else { "YouTube" };
            match why(&c.id) {
                Some(w) => format!("{}  {}", c.title, style(format!("{kind} · {w}")).dim()),
                None => format!("{}  {}", c.title, style(kind).dim()),
            }
        })
        .collect();
    let defaults: Vec<bool> = order
        .iter()
        .map(|c| c.enabled || why(&c.id).is_some())
        .collect();
    note("Space ticks, ↑↓ scroll, a ticks or clears all, enter saves.");
    let chosen = MultiSelect::with_theme(theme)
        .with_prompt("Follow")
        .items(&labels)
        .defaults(&defaults)
        .max_length(15)
        .report(false)
        .interact()?;
    let mut followed = 0;
    for (i, c) in order.iter().enumerate() {
        let on = chosen.contains(&i);
        if on != c.enabled {
            if on {
                feeds::add_channel(db, &c.id).await?;
            } else {
                db::set_enabled(&db.lock().unwrap(), &c.id, false)?;
            }
        }
        followed += on as usize;
    }
    println!("  {} Following {followed}.", style("✓").green());
    Ok(())
}

/// Offer the daily brief; returns the time it runs at, if on.
fn daily(theme: &ColorfulTheme) -> Result<Option<String>> {
    let Ok(agent) = Agent::detect() else {
        note(
            "The daily brief needs Claude Code or Codex installed; you can still ask for your brief in chat.",
        );
        return Ok(None);
    };
    if schedule::scheduled() {
        note(
            "Already on. Change it with `tx schedule --at HH:MM`, or turn it off with `tx schedule --off`.",
        );
        return Ok(Some("the time you set".into()));
    }
    note(&format!(
        "Each morning tx checks what you follow, {} screens what's new, and a notification says what's worth your time.",
        agent.program()
    ));
    note("It runs on this computer, even when your AI app is closed.");
    if !Confirm::with_theme(theme)
        .with_prompt("Turn on the daily brief?")
        .default(true)
        .interact()?
    {
        return Ok(None);
    }
    let at: String = Input::with_theme(theme)
        .with_prompt("At (HH:MM)")
        .default("08:00".into())
        .interact_text()?;
    schedule::install(agent, schedule::When::parse(Some(&at), None)?)?;
    Ok(Some(at))
}
