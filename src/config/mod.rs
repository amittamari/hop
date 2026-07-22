use crate::core::AgentId;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

fn default_true() -> bool {
    true
}
fn default_width_pct() -> u16 {
    30
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

#[derive(Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct ColumnsConfig {
    #[serde(default)]
    pub disabled: Vec<String>,
    /// Optional explicit order (column ids); empty = default order.
    #[serde(default)]
    pub order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowStyle {
    Card,
    Compact,
}

impl RowStyle {
    pub fn from_config(s: &str) -> RowStyle {
        match s.trim().to_ascii_lowercase().as_str() {
            "compact" => RowStyle::Compact,
            _ => RowStyle::Card,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DisplayConfig {
    #[serde(default = "default_row_style")]
    pub row_style: String,
    /// Nerd-font icon facelift. Default `true` (opt-out): the TUI renders
    /// Private Use Area icons in its chrome, which requires a patched Nerd Font.
    /// Set to `false` to fall back to the pre-icon text layout (no tofu).
    #[serde(default = "default_true")]
    pub icons: bool,
    /// Whether the preview pane starts visible. Seeds the in-memory preview
    /// state at launch; runtime toggles are not persisted.
    #[serde(default)]
    pub visible: bool,
    /// Preview pane width as a percentage of the terminal width.
    #[serde(default = "default_width_pct")]
    pub width_pct: u16,
    /// Preview metadata header. Only applies in the compact row style; the card
    /// layout never renders the preview header regardless of this value.
    #[serde(default = "default_true")]
    pub metadata_header: bool,
}

fn default_row_style() -> String {
    "card".to_string()
}

impl Default for DisplayConfig {
    fn default() -> Self {
        DisplayConfig {
            row_style: default_row_style(),
            icons: true,
            visible: false,
            width_pct: default_width_pct(),
            metadata_header: true,
        }
    }
}

impl DisplayConfig {
    pub fn resolved_row_style(&self) -> RowStyle {
        RowStyle::from_config(&self.row_style)
    }
}

#[derive(Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct LauncherConfig {
    #[serde(default)]
    pub command: Option<String>,
}

impl LauncherConfig {
    pub fn rewrite_argv(
        &self,
        agent: AgentId,
        argv: &[String],
    ) -> Option<anyhow::Result<Vec<String>>> {
        let tmpl = self.command.as_deref()?;
        Some(rewrite_argv_inner(tmpl, agent, argv))
    }
}

fn rewrite_argv_inner(tmpl: &str, agent: AgentId, argv: &[String]) -> anyhow::Result<Vec<String>> {
    let expanded = tmpl.replace("{agent}", agent.slug());
    if let Some(pos) = expanded.find('{')
        && let Some(end) = expanded[pos..].find('}')
    {
        let unknown = &expanded[pos..pos + end + 1];
        anyhow::bail!("unknown launcher template variable: {unknown}");
    }
    let mut prefix = shlex::split(&expanded)
        .ok_or_else(|| anyhow::anyhow!("unterminated quote in launcher command"))?;
    if prefix.is_empty() {
        anyhow::bail!("launcher command expands to empty string");
    }
    prefix.extend_from_slice(&argv[1..]);
    Ok(prefix)
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Config {
    #[serde(default, skip_serializing_if = "is_default")]
    pub data_dirs: HashMap<String, PathBuf>,
    // theme reserved for later tasks; parsed leniently.
    #[serde(default, skip_serializing_if = "is_default")]
    pub theme: HashMap<String, String>,
    /// Ctrl-chord overrides, keyed by command name (e.g. `toggle_preview`).
    /// Resolved by `tui::keymap::Keymap::from_config`.
    #[serde(default, skip_serializing_if = "is_default")]
    pub keybindings: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub columns: ColumnsConfig,
    #[serde(default, skip_serializing_if = "is_default")]
    pub launcher: LauncherConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    /// Initial search mode: `"simple"` (guided toolbar, the default) or `"raw"`
    /// (type the query DSL directly). Unknown/empty values resolve to simple.
    /// Interpreted by `tui::SearchMode::from_config`.
    #[serde(default = "default_search_mode")]
    pub search_mode: String,
}

fn default_search_mode() -> String {
    "simple".to_owned()
}

pub mod commands;

pub fn config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("dev", "hop", "hop")
        .map(|dirs| dirs.config_dir().join("config.toml"))
}

pub fn config_template() -> &'static str {
    r#"## hop configuration
## Uncomment and edit the settings you want to change.
## Full documentation: https://github.com/amittamari/hop

## Override default data directories per agent.
# [data_dirs]
# claude = "~/.claude/projects"
# codex = "~/.codex"
# cursor = "~/.cursor/projects"

## Display settings.
# [display]
## Row style: "card" (multi-line, default) or "compact" (single-line table).
# row_style = "card"
## Nerd Font icons. Set to false if your font lacks Private Use Area glyphs.
# icons = true
## Whether the preview pane starts visible.
# visible = false
## Preview pane width as a percentage of the terminal width.
# width_pct = 30
## Show metadata header above the preview (compact row style only).
# metadata_header = true

## Initial search mode: "simple" (guided toolbar, default) or "raw" (DSL).
# search_mode = "simple"

## Column visibility and order.
# [columns]
## Hide columns by id (e.g. "pr", "msgs", "agent", "branch").
# disabled = []
## Explicit column order (column ids). Empty = default order.
# order = []

## Keybinding overrides, keyed by command name.
# [keybindings]
# toggle_preview = "ctrl+p"
# quit = "ctrl+c"

## Launcher command template. {agent} is replaced with the agent slug.
# [launcher]
# command = "kv --ai {agent}"

## Theme overrides (reserved for future use).
# [theme]
"#
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
            AgentId::Cursor => home.join(".cursor").join("projects"),
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

    #[test]
    fn preview_defaults() {
        let cfg = Config::default();
        assert!(!cfg.display.visible);
        assert_eq!(cfg.display.width_pct, 30);
        assert!(cfg.display.metadata_header);
    }

    #[test]
    fn preview_from_toml() {
        let toml = r#"
            [display]
            visible = false
            width_pct = 40
            metadata_header = false
        "#;
        let cfg = Config::from_toml_str(toml).unwrap();
        assert!(!cfg.display.visible);
        assert_eq!(cfg.display.width_pct, 40);
        assert!(!cfg.display.metadata_header);
    }

    #[test]
    fn icons_default_on_and_opt_out() {
        // Opt-out: default enables the nerd-font icon layer.
        assert!(Config::default().display.icons);
        let toml = r#"
            [display]
            icons = false
        "#;
        let cfg = Config::from_toml_str(toml).unwrap();
        assert!(!cfg.display.icons);
        // Unset icons with an otherwise-present [display] stays on.
        let cfg = Config::from_toml_str("[display]\nrow_style = \"compact\"\n").unwrap();
        assert!(cfg.display.icons);
    }

    #[test]
    fn preview_width_same_with_or_without_config_file() {
        let no_file = Config::default().display.width_pct;
        let partial = Config::from_toml_str("[display]\nrow_style = \"compact\"\n")
            .unwrap()
            .display
            .width_pct;
        assert_eq!(no_file, partial);
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
    fn keybindings_from_toml() {
        let toml = r#"
            [keybindings]
            toggle_preview = "ctrl+t"
            quit = "ctrl+q"
        "#;
        let cfg = Config::from_toml_str(toml).unwrap();
        assert_eq!(cfg.keybindings.get("toggle_preview").map(String::as_str), Some("ctrl+t"));
        assert_eq!(cfg.keybindings.get("quit").map(String::as_str), Some("ctrl+q"));
    }

    #[test]
    fn launcher_rewrites_argv() {
        let cfg = LauncherConfig { command: Some("kv --ai {agent}".into()) };
        let argv: Vec<String> = vec!["claude".into(), "--resume".into(), "abc-123".into()];
        let result = cfg.rewrite_argv(AgentId::Claude, &argv).unwrap().unwrap();
        assert_eq!(result, vec!["kv", "--ai", "claude", "--resume", "abc-123"]);
    }

    #[test]
    fn launcher_preserves_yolo_flags() {
        let cfg = LauncherConfig { command: Some("kv --ai {agent}".into()) };
        let argv: Vec<String> = vec![
            "claude".into(),
            "--dangerously-skip-permissions".into(),
            "--resume".into(),
            "id".into(),
        ];
        let result = cfg.rewrite_argv(AgentId::Claude, &argv).unwrap().unwrap();
        assert_eq!(
            result,
            vec!["kv", "--ai", "claude", "--dangerously-skip-permissions", "--resume", "id"]
        );
    }

    #[test]
    fn launcher_none_when_unconfigured() {
        let cfg = LauncherConfig::default();
        let argv: Vec<String> = vec!["claude".into()];
        assert!(cfg.rewrite_argv(AgentId::Claude, &argv).is_none());
    }

    #[test]
    fn launcher_unknown_variable_is_error() {
        let cfg = LauncherConfig { command: Some("kv {unknown}".into()) };
        let argv: Vec<String> = vec!["claude".into()];
        assert!(cfg.rewrite_argv(AgentId::Claude, &argv).unwrap().is_err());
    }

    #[test]
    fn template_parses_when_uncommented() {
        let template = super::config_template();
        let uncommented: String = template
            .lines()
            .map(|line| {
                if let Some(rest) = line.strip_prefix("# ") {
                    rest
                } else if line == "#" {
                    ""
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        Config::from_toml_str(&uncommented).expect("template should parse as valid config");
    }

    #[test]
    fn launcher_from_toml() {
        let toml = r#"
            [launcher]
            command = "kv --ai {agent}"
        "#;
        let cfg = Config::from_toml_str(toml).unwrap();
        assert_eq!(cfg.launcher.command.as_deref(), Some("kv --ai {agent}"));
    }
}
