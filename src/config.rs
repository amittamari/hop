use crate::core::AgentId;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct PreviewConfig {
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default = "default_width_pct")]
    pub width_pct: u16,
}

fn default_true() -> bool { true }
fn default_width_pct() -> u16 { 50 }

impl Default for PreviewConfig {
    fn default() -> Self {
        PreviewConfig { visible: true, width_pct: 50 }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct ColumnsConfig {
    #[serde(default)]
    pub disabled: Vec<String>,
    /// Optional explicit order (column ids); empty = default order.
    #[serde(default)]
    pub order: Vec<String>,
}

fn default_keymap() -> String { "search".to_string() }

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub data_dirs: HashMap<String, PathBuf>,
    // theme/keybindings reserved for later tasks; parsed leniently.
    #[serde(default)]
    pub theme: HashMap<String, String>,
    #[serde(default)]
    pub keybindings: HashMap<String, String>,
    #[serde(default)]
    pub preview: PreviewConfig,
    #[serde(default = "default_keymap")]
    pub keymap: String,
    #[serde(default)]
    pub columns: ColumnsConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            data_dirs: HashMap::new(),
            theme: HashMap::new(),
            keybindings: HashMap::new(),
            preview: PreviewConfig::default(),
            keymap: default_keymap(),
            columns: ColumnsConfig::default(),
        }
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiState {
    pub preview_visible: bool,
    pub preview_width_pct: u16,
}

impl UiState {
    pub fn load(path: &std::path::Path) -> Option<UiState> {
        let text = std::fs::read_to_string(path).ok()?;
        toml::from_str(&text).ok()
    }

    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string(self).context("serializing ui_state")?;
        std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
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

    #[test]
    fn preview_and_keymap_defaults() {
        let cfg = Config::default();
        assert!(cfg.preview.visible);
        assert_eq!(cfg.preview.width_pct, 50);
        assert_eq!(cfg.keymap, "search");
    }

    #[test]
    fn preview_and_keymap_from_toml() {
        let toml = r#"
            keymap = "modal"
            [preview]
            visible = false
            width_pct = 40
        "#;
        let cfg = Config::from_toml_str(toml).unwrap();
        assert!(!cfg.preview.visible);
        assert_eq!(cfg.preview.width_pct, 40);
        assert_eq!(cfg.keymap, "modal");
    }

    #[test]
    fn disabled_columns_from_toml() {
        let toml = r#"
            [columns]
            disabled = ["pr", "msgs"]
        "#;
        let cfg = Config::from_toml_str(toml).unwrap();
        assert!(cfg.columns.disabled.contains(&"pr".to_string()));
    }

    #[test]
    fn ui_state_roundtrips() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("ui_state.toml");
        UiState { preview_visible: false, preview_width_pct: 35 }.save(&p).unwrap();
        let loaded = UiState::load(&p).unwrap();
        assert!(!loaded.preview_visible);
        assert_eq!(loaded.preview_width_pct, 35);
        assert!(UiState::load(&tmp.path().join("absent.toml")).is_none());
    }
}
