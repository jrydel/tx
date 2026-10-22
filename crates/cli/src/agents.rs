//! Running the screening with the user's own agent, and registering tx's MCP server with it.

use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tx_core::paths;

#[derive(Clone, Copy, Debug, PartialEq, ValueEnum)]
pub enum Agent {
    /// Claude Code (`claude -p`), your Claude subscription
    Claude,
    /// Codex CLI (`codex exec`), your ChatGPT subscription
    Codex,
}

/// Where the Codex and ChatGPT desktop apps keep their own `codex`, for users who never installed
/// the CLI.
const CODEX_IN_APPS: &[&str] = &[
    "/Applications/Codex.app/Contents/Resources/codex",
    "/Applications/ChatGPT.app/Contents/Resources/codex",
    "/usr/lib/chatgpt/resources/codex",
];

impl Agent {
    pub fn program(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
        }
    }

    pub fn find(self) -> Option<PathBuf> {
        paths::which(self.program()).or_else(|| {
            (self == Agent::Codex)
                .then(|| {
                    CODEX_IN_APPS
                        .iter()
                        .map(PathBuf::from)
                        .find(|p| p.is_file())
                })
                .flatten()
        })
    }

    pub fn detect() -> Result<Agent> {
        [Agent::Claude, Agent::Codex]
            .into_iter()
            .find(|a| a.find().is_some())
            .context("neither Claude Code (claude) nor Codex (codex) is installed")
    }
}

/// The screen-videos skill, minus its frontmatter.
const SKILL: &str = include_str!("../../../plugin/skills/screen-videos/SKILL.md");

pub fn screening_prompt() -> String {
    let body = SKILL.splitn(3, "---").nth(2).unwrap_or(SKILL).trim();
    format!(
        "You are running unattended on a schedule; do not ask questions, just complete the steps.\n\n{body}"
    )
}

fn exe() -> Result<PathBuf> {
    Ok(std::env::current_exe()?.canonicalize()?)
}

/// Screen new videos once with `agent`, streaming its output; returns its exit code.
pub fn run(agent: Agent) -> Result<i32> {
    let program = agent
        .find()
        .with_context(|| format!("{} is not installed", agent.program()))?;
    let tx = exe()?.display().to_string();
    let data = paths::data_dir().display().to_string();
    let prompt = screening_prompt();
    let mut cmd = Command::new(program);
    match agent {
        Agent::Claude => {
            let config = serde_json::json!({ "mcpServers": { "tx": { "command": tx, "args": ["mcp", "--data-dir", data] } } });
            cmd.args([
                "-p",
                &prompt,
                "--mcp-config",
                &config.to_string(),
                "--strict-mcp-config",
                "--allowedTools",
                "mcp__tx",
            ]);
        }
        Agent::Codex => {
            cmd.args(["exec", "--skip-git-repo-check", "--sandbox", "read-only"])
                .args([
                    "-c",
                    &format!("mcp_servers.tx.command={}", serde_json::to_string(&tx)?),
                ])
                .args([
                    "-c",
                    &format!(
                        "mcp_servers.tx.args={}",
                        serde_json::to_string(&["mcp", "--data-dir", &data])?
                    ),
                ])
                .args([
                    "-c",
                    r#"mcp_servers.tx.default_tools_approval_mode="approve""#,
                ])
                .arg(&prompt);
        }
    }
    let status = cmd
        .current_dir(paths::data_dir())
        .env("PATH", paths::search_path())
        .stdin(Stdio::null())
        .status()
        .with_context(|| format!("starting {}", agent.program()))?;
    Ok(status.code().unwrap_or(1))
}

pub fn claude_desktop_config() -> Option<PathBuf> {
    // macOS: ~/Library/Application Support; Windows: %APPDATA%; Linux: ~/.config
    let base = if cfg!(target_os = "macos") {
        dirs::home_dir()?.join("Library/Application Support")
    } else {
        dirs::config_dir()?
    };
    Some(base.join("Claude").join("claude_desktop_config.json"))
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Target {
    /// Claude Code (CLI and IDE extensions)
    ClaudeCode,
    /// Codex (desktop app, CLI and IDE extensions)
    Codex,
    /// Claude Desktop app
    ClaudeDesktop,
}

/// Copy the screen-videos skill where the agent looks for personal skills.
fn install_skill(agent_home: &std::path::Path) -> Result<PathBuf> {
    let dir = agent_home.join("skills/screen-videos");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("SKILL.md");
    std::fs::write(&path, SKILL)?;
    Ok(path)
}

/// Codex's settings folder, shared by the Codex app, CLI and IDE extensions.
pub fn codex_home() -> Result<PathBuf> {
    match std::env::var_os("CODEX_HOME") {
        Some(h) if !h.is_empty() => Ok(PathBuf::from(h)),
        _ => Ok(dirs::home_dir().context("no home dir")?.join(".codex")),
    }
}

/// Register `tx mcp` (by absolute path, so desktop apps with a minimal PATH find it) and install
/// the screening skill, so the agent can use tx in normal conversations.
pub fn setup(target: Target) -> Result<()> {
    let tx = exe()?.display().to_string();
    match target {
        Target::ClaudeCode => {
            let program = paths::which("claude").context(
                "Claude Code (claude) is not installed; get it at https://claude.com/claude-code",
            )?;
            let _ = Command::new(&program)
                .args(["mcp", "remove", "--scope", "user", "tx"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let ok = Command::new(&program)
                .args(["mcp", "add", "--scope", "user", "tx", "--", &tx, "mcp"])
                .status()?
                .success();
            if !ok {
                bail!("claude mcp add failed");
            }
            let home = dirs::home_dir().context("no home dir")?.join(".claude");
            println!(
                "Installed the screen-videos skill: {}",
                install_skill(&home)?.display()
            );
            println!("Done: tx is available in Claude Code.");
        }
        Target::Codex => {
            // Edit config.toml directly: the Codex app reads it too, and many app users don't
            // have the `codex` command that `codex mcp add` would need.
            let home = codex_home()?;
            let path = home.join("config.toml");
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let mut config: toml_edit::DocumentMut = text
                .parse()
                .with_context(|| format!("{} is not valid TOML", path.display()))?;
            let servers = config["mcp_servers"].or_insert(toml_edit::table());
            servers
                .as_table_mut()
                .with_context(|| format!("mcp_servers in {} is not a table", path.display()))?
                .set_implicit(true);
            let mut server = toml_edit::Table::new();
            server["command"] = toml_edit::value(tx.as_str());
            server["args"] = toml_edit::value(toml_edit::Array::from_iter(["mcp"]));
            config["mcp_servers"]["tx"] = toml_edit::Item::Table(server);
            std::fs::create_dir_all(&home)?;
            std::fs::write(&path, config.to_string())?;
            println!("Added tx to {}.", path.display());
            println!(
                "Installed the screen-videos skill: {}",
                install_skill(&home)?.display()
            );
            println!("Done: restart the Codex app (or start a new codex session) to load tx.");
        }
        Target::ClaudeDesktop => {
            let path =
                claude_desktop_config().context("can't locate the Claude Desktop config folder")?;
            let mut config: serde_json::Value = match std::fs::read_to_string(&path) {
                Ok(s) => serde_json::from_str(&s)
                    .with_context(|| format!("{} is not valid JSON", path.display()))?,
                Err(_) => serde_json::json!({}),
            };
            config["mcpServers"]["tx"] = serde_json::json!({ "command": tx, "args": ["mcp"] });
            std::fs::create_dir_all(path.parent().unwrap())?;
            std::fs::File::create(&path)?
                .write_all(serde_json::to_string_pretty(&config)?.as_bytes())?;
            println!(
                "Added tx to {}. Restart Claude Desktop to load it.",
                path.display()
            );
        }
    }
    Ok(())
}
