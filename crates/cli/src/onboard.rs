//! `tx onboard`: set tx up in the terminal, step by step: connect your AI apps, say what you want
//! to learn, pick what to follow (your AI can pre-pick from your subscriptions), turn on the
//! daily brief. Safe to run again: every step shows what's already set.

use crate::agents::{self, Agent, Target};
use crate::schedule;
use anyhow::{Context, Result, bail};
use dialoguer::{Confirm, Input, MultiSelect, Select, theme::ColorfulTheme};
use std::io::IsTerminal;
use std::process::{Command, Stdio};
use tx_core::db::{self, Db};
use tx_core::{feeds, podcasts};

const BROWSERS: [&str; 5] = ["chrome", "firefox", "safari", "edge", "brave"];

fn step(n: u8, title: &str) {
    println!("\n\x1b[1m{n}/4  {title}\x1b[0m");
}

pub async fn run() -> Result<()> {
    if !std::io::stdin().is_terminal() {
        bail!("tx onboard asks questions, so run it in a terminal");
    }
    let theme = ColorfulTheme::default();
    let db: Db = std::sync::Mutex::new(db::connect()?);
    println!(
        "Let's set up tx: 4 short steps. Run `tx onboard` again any time to change something."
    );

    connect_apps(&theme)?;
    about(&theme, &db)?;
    follow(&theme, &db).await?;
    daily(&theme)?;

    let following = db::channels(&db.lock().unwrap())?
        .iter()
        .filter(|c| c.enabled)
        .count();
    println!(
        "\n\x1b[1mYou're set.\x1b[0m Following {following}. In your AI app, start a new chat and ask:"
    );
    println!("  \"Use tx: show me my brief\"   or paste any video link with \"TL;DR this\".");
    if following > 0
        && let Ok(agent) = Agent::detect()
        && Confirm::with_theme(&theme)
            .with_prompt(format!(
                "Screen the newest uploads now with {}? (a few minutes)",
                agent.program()
            ))
            .default(false)
            .interact()?
    {
        agents::run(agent)?;
    }
    Ok(())
}

/// Which AI apps to connect tx to; the ones found on this computer are pre-ticked.
fn connect_apps(theme: &ColorfulTheme) -> Result<()> {
    step(1, "Connect your AI");
    let codex = Agent::Codex.find().is_some() || agents::codex_home().is_ok_and(|h| h.exists());
    let claude_code = Agent::Claude.find().is_some();
    let claude_desktop = agents::claude_desktop_config()
        .and_then(|p| p.parent().map(|d| d.exists()))
        .unwrap_or(false);
    let apps = [
        (
            Target::Codex,
            "Codex (the ChatGPT desktop app or CLI)",
            codex,
        ),
        (Target::ClaudeCode, "Claude Code", claude_code),
        (Target::ClaudeDesktop, "Claude Desktop", claude_desktop),
    ];
    if !apps.iter().any(|a| a.2) {
        println!(
            "No AI app found. Install Codex (ChatGPT), Claude Code or Claude Desktop, then run `tx onboard` again."
        );
    }
    let picked = MultiSelect::with_theme(theme)
        .with_prompt("Use tx in (space to tick, enter to confirm)")
        .items(apps.iter().map(|a| a.1).collect::<Vec<_>>())
        .defaults(&apps.iter().map(|a| a.2).collect::<Vec<_>>())
        .interact()?;
    for i in picked {
        if let Err(e) = agents::setup(apps[i].0) {
            println!("  {}: {e:#}", apps[i].1);
        }
    }
    Ok(())
}

fn about(theme: &ColorfulTheme, db: &Db) -> Result<()> {
    step(2, "About you");
    println!(
        "Every video and episode is scored against this: your role, what you want to learn, what to skip."
    );
    println!(
        "\x1b[2me.g. I'm a backend engineer building LLM features. I want concrete agent and eval techniques\n     and notable model launches; skip hype and drama.\x1b[0m"
    );
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
    step(3, "What to follow");
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
            .default(0)
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
                        println!("  Imported {found} subscriptions.");
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
                        println!("  Imported {found}.");
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
                        Ok(title) => println!("  Following {title}"),
                        Err(e) => println!("  {r}: {e:#}"),
                    }
                }
                Ok(())
            }
            3 => add_podcasts(theme, db).await,
            4 => pick(theme, db).await,
            _ => return Ok(()),
        };
        if let Err(e) = result {
            println!("  {e:#}");
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
            println!("  No podcast found for \"{name}\".");
            continue;
        }
        let labels: Vec<String> = found
            .iter()
            .take(5)
            .map(|s| {
                format!(
                    "{} ({})",
                    s["title"].as_str().unwrap_or("?"),
                    s["author"].as_str().unwrap_or("")
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
            println!("  Following {title}");
        }
    }
    Ok(())
}

/// Tick what to follow from everything tx knows; the AI's picks are pre-ticked when it can help.
async fn pick(theme: &ColorfulTheme, db: &Db) -> Result<()> {
    let (channels, profile) = {
        let conn = db.lock().unwrap();
        (db::channels(&conn)?, db::get_settings(&conn)?.profile)
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
        println!("  Asking {}…", agent.program());
        match suggest(agent, &profile, &channels) {
            Ok(p) => picks = p,
            Err(e) => println!("  Couldn't get suggestions ({e:#}); tick them yourself."),
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
                Some(w) => format!("{}  \x1b[2m{kind} · {w}\x1b[0m", c.title),
                None => format!("{}  \x1b[2m{kind}\x1b[0m", c.title),
            }
        })
        .collect();
    let defaults: Vec<bool> = order
        .iter()
        .map(|c| c.enabled || why(&c.id).is_some())
        .collect();
    let chosen = MultiSelect::with_theme(theme)
        .with_prompt("Follow (space to tick, enter to confirm)")
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
    println!("  Following {followed}.");
    Ok(())
}

/// Ask the user's own agent which channels fit their profile: (id, short reason) pairs.
fn suggest(agent: Agent, profile: &str, channels: &[db::Channel]) -> Result<Vec<(String, String)>> {
    let list: String = channels
        .iter()
        .map(|c| {
            format!(
                "{} | {} | {}\n",
                c.id,
                c.title,
                if c.podcast { "podcast" } else { "YouTube" }
            )
        })
        .collect();
    let prompt = format!(
        "A person describes what they want to learn. Below are the YouTube channels and podcasts they subscribe to. \
         Pick the ones whose new uploads fit that, usually 5 to 20. Reply with only lines like `<id> | <why, under 10 words>`, \
         nothing else.\n\nPerson: {profile}\n\nChannels (id | title | kind):\n{list}"
    );
    let program = agent.find().context("agent not installed")?;
    let text = match agent {
        Agent::Claude => {
            let out = Command::new(program)
                .args(["-p", &prompt])
                .stdin(Stdio::null())
                .output()?;
            String::from_utf8_lossy(&out.stdout).into_owned()
        }
        Agent::Codex => {
            let answer = tempfile::NamedTempFile::new()?;
            Command::new(program)
                .args([
                    "exec",
                    "--skip-git-repo-check",
                    "--sandbox",
                    "read-only",
                    "-o",
                ])
                .arg(answer.path())
                .arg(&prompt)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?;
            std::fs::read_to_string(answer.path())?
        }
    };
    let picks: Vec<(String, String)> = text
        .lines()
        .filter_map(|l| {
            let (id, why) = l.trim().trim_matches('`').split_once('|')?;
            let id = id.trim();
            channels
                .iter()
                .any(|c| c.id == id)
                .then(|| (id.to_string(), why.trim().to_string()))
        })
        .collect();
    if picks.is_empty() {
        bail!("no usable answer");
    }
    Ok(picks)
}

fn daily(theme: &ColorfulTheme) -> Result<()> {
    step(4, "Daily brief");
    let Ok(agent) = Agent::detect() else {
        println!(
            "The daily brief needs Claude Code or Codex installed; you can still ask for your brief in chat."
        );
        return Ok(());
    };
    if schedule::scheduled() {
        println!("Already on. Change it with `tx schedule --at HH:MM`, or `tx schedule --off`.");
        return Ok(());
    }
    println!(
        "Every morning tx checks what you follow, {} screens what's new, and a notification says what's worth your time.",
        agent.program()
    );
    if !Confirm::with_theme(theme)
        .with_prompt("Turn it on?")
        .default(true)
        .interact()?
    {
        return Ok(());
    }
    let at: String = Input::with_theme(theme)
        .with_prompt("At (HH:MM)")
        .default("08:00".into())
        .interact_text()?;
    let when = schedule::install(agent, schedule::When::parse(Some(&at), None)?)?;
    println!("  Daily brief on, {when}.");
    Ok(())
}
