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

/// Explicit agent for a single-video request, optionally with a model name.
#[derive(Clone, Debug)]
pub(crate) struct Selection {
    agent: Agent,
    model: Option<String>,
}

impl std::str::FromStr for Selection {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        let (name, model) = match value.split_once(':') {
            Some((name, model)) => {
                if model.is_empty()
                    || model.starts_with('-')
                    || model.chars().any(char::is_whitespace)
                {
                    return Err(
                        "model must be a nonempty name without whitespace or a leading '-'".into(),
                    );
                }
                (name, Some(model.to_string()))
            }
            None => (value, None),
        };
        let agent = match name {
            "codex" => Agent::Codex,
            "claude" => Agent::Claude,
            _ => return Err("expected codex[:MODEL] or claude[:MODEL]".into()),
        };
        Ok(Self { agent, model })
    }
}

impl Selection {
    /// Explicit flag wins over the environment; otherwise use the existing detection order.
    pub(crate) fn resolve(
        selected: Option<&Self>,
        interactive: bool,
    ) -> Result<(PathBuf, Vec<String>)> {
        if let Some(selected) = selected {
            return selected.command(interactive);
        }
        let key = if interactive {
            "TX_CHAT_AGENT"
        } else {
            "TX_AGENT"
        };
        if std::env::var_os(key).is_some() {
            return tx_core::agent::command(key, "")
                .with_context(|| format!("{key} does not name an installed agent"));
        }
        Self {
            agent: Agent::detect()?,
            model: None,
        }
        .command(interactive)
    }

    pub(crate) fn command(&self, interactive: bool) -> Result<(PathBuf, Vec<String>)> {
        let program = self
            .agent
            .find()
            .with_context(|| format!("{} is not installed", self.agent.program()))?;
        let mut args = if interactive {
            Vec::new()
        } else {
            match self.agent {
                Agent::Claude => vec!["-p".into()],
                Agent::Codex => vec!["exec".into(), "--skip-git-repo-check".into()],
            }
        };
        if let Some(model) = &self.model {
            args.extend(["--model".into(), model.clone()]);
        }
        Ok((program, args))
    }
}

impl Agent {
    /// Its non-interactive command with the default model, for one-shot answers.
    pub(crate) fn one_shot(self) -> Result<(PathBuf, Vec<String>)> {
        Selection {
            agent: self,
            model: None,
        }
        .command(false)
    }
}

#[cfg(test)]
mod selection_tests {
    use super::*;

    #[test]
    fn agent_names_and_model_boundary_are_unambiguous() {
        let codex: Selection = "codex".parse().unwrap();
        assert_eq!(codex.agent, Agent::Codex);
        assert!(codex.model.is_none());
        let claude: Selection = "claude:haiku".parse().unwrap();
        assert_eq!(claude.agent, Agent::Claude);
        assert_eq!(claude.model.as_deref(), Some("haiku"));
        let named: Selection = "codex:provider/model:variant".parse().unwrap();
        assert_eq!(named.model.as_deref(), Some("provider/model:variant"));
        for bad in [
            "",
            "unknown",
            ":haiku",
            "codex:",
            "claude:--help",
            "codex:two models",
            "codex:\n",
        ] {
            assert!(bad.parse::<Selection>().is_err(), "{bad:?}");
        }
    }
}

/// The screen-videos skill, minus its frontmatter.
const SKILL: &str = include_str!("../../../plugin/skills/screen-videos/SKILL.md");

fn exe() -> Result<PathBuf> {
    Ok(std::env::current_exe()?.canonicalize()?)
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
            // The tx plugin already brings the server and the skill; adding them again would
            // load tx twice.
            let plugin = config
                .get("plugins")
                .and_then(|p| p.get("tx@tx"))
                .and_then(|t| t.get("enabled"))
                .and_then(|e| e.as_bool());
            if plugin == Some(true) {
                println!("tx is already in Codex through its plugin; nothing to add.");
                return Ok(());
            }
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
