//! `tx schedule`: run `tx run` on a timer with the OS scheduler (systemd user timer on Linux,
//! a LaunchAgent on macOS, Task Scheduler on Windows). Missed runs catch up where the OS supports it.

use crate::agents::Agent;
use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use std::process::Command;
use tx_core::paths;

pub enum When {
    /// Daily at hour:minute local time.
    Daily(u32, u32),
    /// Every N hours.
    Every(u32),
}

impl When {
    pub fn parse(at: Option<&str>, every: Option<&str>) -> Result<When> {
        if let Some(e) = every {
            let hours: u32 = e
                .trim()
                .trim_end_matches('h')
                .parse()
                .context("--every takes hours, e.g. 2h")?;
            if !(1..=24).contains(&hours) {
                bail!("--every must be 1h to 24h");
            }
            return Ok(When::Every(hours));
        }
        let at = at.unwrap_or("08:00");
        let (h, m) = at.split_once(':').context("--at takes HH:MM")?;
        let (h, m): (u32, u32) = (h.parse()?, m.parse()?);
        if h > 23 || m > 59 {
            bail!("--at takes HH:MM");
        }
        Ok(When::Daily(h, m))
    }

    fn describe(&self) -> String {
        match self {
            When::Daily(h, m) => format!("daily at {h:02}:{m:02}"),
            When::Every(n) => format!("every {n} hour(s)"),
        }
    }
}

fn exe() -> Result<String> {
    Ok(std::env::current_exe()?
        .canonicalize()?
        .display()
        .to_string())
}

fn agent_arg(agent: Agent) -> &'static str {
    match agent {
        Agent::Claude => "claude",
        Agent::Codex => "codex",
    }
}

fn run_ok(cmd: &mut Command) -> Result<()> {
    let out = cmd
        .output()
        .with_context(|| format!("running {:?}", cmd.get_program()))?;
    if !out.status.success() {
        bail!(
            "{:?} failed: {}",
            cmd.get_program(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn unit_dir() -> Result<PathBuf> {
    Ok(dirs::config_dir()
        .context("no config dir")?
        .join("systemd/user"))
}

#[cfg(target_os = "linux")]
pub fn install(agent: Agent, when: When) -> Result<String> {
    let dir = unit_dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = paths::search_path().to_string_lossy().replace('%', "%%");
    std::fs::write(
        dir.join("tx-run.service"),
        format!(
            "[Unit]\nDescription=tx: screen new videos with {a}\n\n[Service]\nType=oneshot\nExecStart=\"{exe}\" run --agent {a}\nEnvironment=\"PATH={path}\"\n",
            a = agent_arg(agent),
            exe = exe()?
        ),
    )?;
    let calendar = match when {
        When::Daily(h, m) => format!("*-*-* {h:02}:{m:02}:00"),
        When::Every(n) => format!("*-*-* 00/{n}:00:00"),
    };
    std::fs::write(
        dir.join("tx-run.timer"),
        format!(
            "[Unit]\nDescription=tx: screen new videos\n\n[Timer]\nOnCalendar={calendar}\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n"
        ),
    )?;
    run_ok(Command::new("systemctl").args(["--user", "daemon-reload"]))?;
    run_ok(Command::new("systemctl").args(["--user", "enable", "--now", "tx-run.timer"]))?;
    Ok(when.describe())
}

#[cfg(target_os = "linux")]
pub fn uninstall() -> Result<()> {
    let _ = Command::new("systemctl")
        .args(["--user", "disable", "--now", "tx-run.timer"])
        .output();
    let dir = unit_dir()?;
    for f in ["tx-run.timer", "tx-run.service"] {
        let _ = std::fs::remove_file(dir.join(f));
    }
    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output();
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn status() -> Result<String> {
    let out = Command::new("systemctl")
        .args(["--user", "list-timers", "tx-run.timer", "--no-pager"])
        .output()?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(target_os = "macos")]
const LABEL: &str = "com.jrydel.tx.run";

#[cfg(target_os = "macos")]
fn plist() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .context("no home dir")?
        .join(format!("Library/LaunchAgents/{LABEL}.plist")))
}

#[cfg(target_os = "macos")]
fn domain() -> String {
    // The GUI launchd domain of the current user; the home directory's owner is that user.
    use std::os::unix::fs::MetadataExt;
    let uid = dirs::home_dir()
        .and_then(|h| std::fs::metadata(h).ok())
        .map(|m| m.uid())
        .unwrap_or(501);
    format!("gui/{uid}")
}

#[cfg(target_os = "macos")]
pub fn install(agent: Agent, when: When) -> Result<String> {
    let log = paths::data_dir().join("run.log").display().to_string();
    let timing = match when {
        When::Daily(h, m) => format!(
            "<key>StartCalendarInterval</key><dict><key>Hour</key><integer>{h}</integer><key>Minute</key><integer>{m}</integer></dict>"
        ),
        When::Every(n) => format!("<key>StartInterval</key><integer>{}</integer>", n * 3600),
    };
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;");
    let body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>{LABEL}</string>
<key>ProgramArguments</key><array><string>{exe}</string><string>run</string><string>--agent</string><string>{a}</string></array>
<key>EnvironmentVariables</key><dict><key>PATH</key><string>{path}</string></dict>
{timing}
<key>StandardOutPath</key><string>{log}</string><key>StandardErrorPath</key><string>{log}</string>
</dict></plist>
"#,
        exe = esc(&exe()?),
        a = agent_arg(agent),
        path = esc(&paths::search_path().to_string_lossy()),
        log = esc(&log),
    );
    let p = plist()?;
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(&p, body)?;
    let _ = Command::new("launchctl")
        .args(["bootout", &domain()])
        .arg(&p)
        .output();
    run_ok(
        Command::new("launchctl")
            .args(["bootstrap", &domain()])
            .arg(&p),
    )?;
    Ok(when.describe())
}

#[cfg(target_os = "macos")]
pub fn uninstall() -> Result<()> {
    let p = plist()?;
    let _ = Command::new("launchctl")
        .args(["bootout", &domain()])
        .arg(&p)
        .output();
    let _ = std::fs::remove_file(p);
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn status() -> Result<String> {
    Ok(if plist()?.exists() {
        format!("LaunchAgent {LABEL} installed")
    } else {
        String::new()
    })
}

#[cfg(windows)]
const TASK: &str = "tx run";

#[cfg(windows)]
pub fn install(agent: Agent, when: When) -> Result<String> {
    let action = format!("\"{}\" run --agent {}", exe()?, agent_arg(agent));
    let mut cmd = Command::new("schtasks");
    cmd.args(["/Create", "/F", "/TN", TASK, "/TR", &action]);
    match when {
        When::Daily(h, m) => cmd.args(["/SC", "DAILY", "/ST", &format!("{h:02}:{m:02}")]),
        When::Every(n) => cmd.args(["/SC", "HOURLY", "/MO", &n.to_string()]),
    };
    run_ok(&mut cmd)?;
    Ok(when.describe())
}

#[cfg(windows)]
pub fn uninstall() -> Result<()> {
    let _ = Command::new("schtasks")
        .args(["/Delete", "/F", "/TN", TASK])
        .output();
    Ok(())
}

#[cfg(windows)]
pub fn status() -> Result<String> {
    let out = Command::new("schtasks")
        .args(["/Query", "/TN", TASK, "/FO", "LIST"])
        .output()?;
    Ok(if out.status.success() {
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    } else {
        String::new()
    })
}
