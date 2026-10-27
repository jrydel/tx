//! `tx learn`: learn who you are from your coding-agent sessions, so every video and episode is
//! scored for what you actually work on. Once saved, `tx run` refreshes it every week.

use crate::agents::Agent;
use anyhow::{Result, bail};
use dialoguer::console::style;
use dialoguer::{Confirm, theme::ColorfulTheme};
use std::io::IsTerminal;
use tx_core::sessions::{self, Activity, Source};
use tx_core::{db, learn};

pub struct Options {
    pub agent: Option<Agent>,
    pub days: u32,
    pub dry_run: bool,
    pub yes: bool,
}

async fn gather(days: u32) -> Result<Activity> {
    let now = chrono::Utc::now();
    let prompts = tokio::task::spawn_blocking(move || sessions::collect(days, now)).await?;
    Ok(Activity::summarize(prompts, days, now))
}

/// What was found, for the user to check before anything is sent.
fn describe(a: &Activity) {
    let counts: Vec<String> = a
        .by_source
        .iter()
        .map(|(s, n)| format!("{} {n}", s.name()))
        .collect();
    eprintln!(
        "Read {} of your prompts from the last {} days ({}). Most active:",
        a.prompts,
        a.days,
        counts.join(" · ")
    );
    for p in a.projects.iter().take(5) {
        eprintln!(
            "  {:<36} {}",
            sessions::display(&p.path),
            style(format!("{} prompts · {} days", p.prompts, p.days)).dim()
        );
    }
}

/// Read the sessions, have the agent write the profile, and save it once the user agrees.
/// Returns whether it was saved.
pub async fn run(theme: &ColorfulTheme, o: Options) -> Result<bool> {
    let activity = gather(o.days).await?;
    if activity.excerpts() == 0 {
        let looked: Vec<String> = Source::ALL
            .iter()
            .filter_map(|s| Some(sessions::display(&s.dir()?)))
            .collect();
        bail!(
            "found no prompts of yours from the last {} days (looked in {})",
            o.days,
            looked.join(", ")
        );
    }
    describe(&activity);
    let material = activity.render();
    if o.dry_run {
        print!("{material}");
        return Ok(false);
    }
    let agent = match o.agent {
        Some(a) => a,
        None => Agent::detect()?,
    };
    if !o.yes && !std::io::stdin().is_terminal() {
        bail!("run it in a terminal to confirm, or pass --yes");
    }
    let ask = |prompt: String| -> Result<bool> {
        Ok(o.yes
            || Confirm::with_theme(theme)
                .with_prompt(prompt)
                .default(true)
                .interact()?)
    };
    if !ask(format!(
        "Send {} redacted excerpts of your prompts ({} KB) to {} to write your profile? Agent replies, tool output and files stay here",
        activity.excerpts(),
        material.len().div_ceil(1024),
        agent.program()
    ))? {
        return Ok(false);
    }
    eprintln!("Asking {}…", agent.program());
    let about = db::get_settings(&db::connect()?)?.profile;
    let text = learn::write(&activity, &about, agent.one_shot()?).await?;
    println!("\n{text}\n");
    if !ask(
        "Score your videos and episodes for this, and refresh it weekly from your sessions?".into(),
    )? {
        eprintln!("Not saved.");
        return Ok(false);
    }
    db::save_learned(&db::connect()?, &text, o.days)?;
    eprintln!(
        "{} Saved. `tx run` refreshes it every {} days. See it with `tx learn --show`; stop with `tx learn --off`.",
        style("✓").green(),
        learn::REFRESH_DAYS
    );
    Ok(true)
}

pub fn show() -> Result<()> {
    match db::learned(&db::connect()?)? {
        Some(l) => println!(
            "{}\n\n{}",
            style(format!(
                "Learned from {} days of your coding-agent sessions, updated {}. Refreshed every {} days by `tx run`.",
                l.days,
                l.at.get(..10).unwrap_or(&l.at),
                learn::REFRESH_DAYS
            ))
            .dim(),
            l.text
        ),
        None => println!("tx hasn't learned anything about you yet. Run `tx learn`."),
    }
    Ok(())
}

pub fn off() -> Result<()> {
    db::forget_learned(&db::connect()?)?;
    println!("Forgot what tx learned from your sessions; it won't be refreshed.");
    Ok(())
}

/// For `tx run`: refresh a saved profile once it's a week old. Returns how many of the user's
/// prompts it was learned from, or None when nothing was due (or there's nothing new to read).
pub async fn refresh(agent: Agent) -> Result<Option<usize>> {
    let (old, about) = {
        let conn = db::connect()?;
        let Some(old) = db::learned(&conn)? else {
            return Ok(None);
        };
        (old, db::get_settings(&conn)?.profile)
    };
    if !db::learned_is_stale(&old, learn::REFRESH_DAYS) {
        return Ok(None);
    }
    let activity = gather(old.days).await?;
    if activity.excerpts() == 0 {
        return Ok(None);
    }
    let text = learn::write(&activity, &about, agent.one_shot()?).await?;
    db::save_learned(&db::connect()?, &text, old.days)?;
    Ok(Some(activity.prompts))
}
