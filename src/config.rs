use crate::core::AgentId;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub data_dirs: HashMap<String, PathBuf>,
    // theme/keybindings reserved for later tasks; parsed leniently.
    #[serde(default)]
    pub theme: HashMap<String, String>,
    #[serde(default)]
    pub keybindings: HashMap<String, String>,
}

impl Config {
    /// Load from the platform config dir; missing file => defaults.
    pub fn load() -> Result<Config> {
        let Some(dirs) = directories::ProjectDirs::from("dev", "hop", "hop") else {
            return Ok(Config::default());
        };
        let path = dirs.config_dir().join("config.toml");
        if !path.exists() {
            return Ok(Config::default());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        Config::from_toml_str(&text)
    }

    pub fn from_toml_str(s: &str) -> Result<Config> {
        toml::from_str(s).context("parsing config.toml")
    }

    /// Resolved data directory for an agent (config override or default).
    pub fn data_dir(&self, agent: AgentId) -> PathBuf {
        if let Some(p) = self.data_dirs.get(agent.slug()) {
            return p.clone();
        }
        let home = directories::BaseDirs::new()
            .map(|b| b.home_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        match agent {
            AgentId::Claude => home.join(".claude").join("projects"),
            AgentId::Codex => home.join(".codex"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::AgentId;

    #[test]
    fn defaults_when_no_file() {
        let cfg = Config::default();
        // Claude default ends in .claude/projects, Codex default ends in .codex
        assert!(cfg.data_dir(AgentId::Claude).ends_with("projects"));
        assert!(cfg.data_dir(AgentId::Codex).to_string_lossy().contains(".codex"));
    }

    #[test]
    fn data_dir_override_from_toml() {
        let toml = r#"
            [data_dirs]
            claude = "/custom/claude"
        "#;
        let cfg = Config::from_toml_str(toml).unwrap();
        assert_eq!(cfg.data_dir(AgentId::Claude), std::path::PathBuf::from("/custom/claude"));
        // unset agent falls back to default
        assert!(cfg.data_dir(AgentId::Codex).to_string_lossy().contains(".codex"));
    }
}
