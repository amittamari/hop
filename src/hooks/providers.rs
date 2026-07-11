use crate::core::AgentId;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

const HOP_HOOK_ID: &str = "hop-meta";
const CLAUDE_MARKETPLACE_NAME: &str = "hop-local";
const CLAUDE_PLUGIN_NAME: &str = "hop-session-metadata";
const CLAUDE_PLUGIN_SELECTOR: &str = "hop-session-metadata@hop-local";
const CODEX_MARKETPLACE_NAME: &str = "hop-local";
const CODEX_PLUGIN_NAME: &str = "hop-session-metadata";
const CODEX_PLUGIN_SELECTOR: &str = "hop-session-metadata@hop-local";

#[derive(Debug)]
pub struct ProviderStatus {
    pub agent: AgentId,
    pub detected: bool,
    pub installed: bool,
    pub config_path: PathBuf,
    pub best_effort: bool,
}

pub fn home_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn detect_providers() -> Vec<ProviderStatus> {
    let home = home_dir();
    vec![
        detect_claude(&home),
        detect_codex(&home),
        detect_cursor(&home),
    ]
}

fn detect_claude(home: &Path) -> ProviderStatus {
    let plugin_dir = claude_plugin_dir(home);
    let config_path = plugin_dir.join("hooks").join("hooks.json");
    let detected = home.join(".claude").exists();
    let installed = detected && is_claude_plugin_installed();
    ProviderStatus {
        agent: AgentId::Claude,
        detected,
        installed,
        config_path,
        best_effort: false,
    }
}

fn detect_codex(home: &Path) -> ProviderStatus {
    let plugin_dir = codex_plugin_dir(home);
    let config_path = plugin_dir.join("hooks.json");
    let detected = home.join(".codex").join("config.toml").exists();
    let installed = detected && is_codex_plugin_installed();
    ProviderStatus {
        agent: AgentId::Codex,
        detected,
        installed,
        config_path,
        best_effort: false,
    }
}

fn detect_cursor(home: &Path) -> ProviderStatus {
    let config_path = home.join(".cursor").join("hooks.json");
    let detected = home.join(".cursor").exists();
    let installed = detected && is_cursor_installed(&config_path);
    ProviderStatus {
        agent: AgentId::Cursor,
        detected,
        installed,
        config_path,
        best_effort: true,
    }
}

fn is_cursor_installed(path: &Path) -> bool {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return false,
    };
    text.contains("hop meta capture")
}

// --- Claude ---

pub fn claude_hooks_json() -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "hooks": {
            "SessionStart": [{"hooks": [{"type": "command", "command": "hop meta capture --agent claude --event start"}]}],
            "SessionEnd": [{"hooks": [{"type": "command", "command": "hop meta capture --agent claude --event stop"}]}]
        }
    }))
    .unwrap()
}

fn claude_marketplace_root(home: &Path) -> PathBuf {
    home.join(".hop").join("claude-plugin-marketplace")
}

fn claude_plugin_dir(home: &Path) -> PathBuf {
    claude_marketplace_root(home)
        .join("plugins")
        .join(CLAUDE_PLUGIN_NAME)
}

fn claude_plugin_manifest() -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "name": CLAUDE_PLUGIN_NAME,
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Capture Claude Code session metadata for hop.",
        "author": { "name": "hop" },
        "license": "MIT",
        "keywords": ["hop", "session-metadata"]
    }))
    .unwrap()
}

fn claude_marketplace_json() -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "name": CLAUDE_MARKETPLACE_NAME,
        "owner": { "name": "hop" },
        "metadata": { "description": "hop session metadata plugins." },
        "plugins": [{
            "name": CLAUDE_PLUGIN_NAME,
            "source": format!("./plugins/{CLAUDE_PLUGIN_NAME}"),
            "description": "Capture Claude Code session metadata for hop."
        }]
    }))
    .unwrap()
}

fn write_claude_plugin(home: &Path) -> Result<PathBuf> {
    let root = claude_marketplace_root(home);
    let plugin_dir = claude_plugin_dir(home);
    let manifest_dir = plugin_dir.join(".claude-plugin");
    let hooks_dir = plugin_dir.join("hooks");
    std::fs::create_dir_all(&manifest_dir)
        .with_context(|| format!("creating {}", manifest_dir.display()))?;
    std::fs::create_dir_all(&hooks_dir)?;
    std::fs::create_dir_all(root.join(".claude-plugin"))?;
    std::fs::write(manifest_dir.join("plugin.json"), claude_plugin_manifest())?;
    std::fs::write(hooks_dir.join("hooks.json"), claude_hooks_json())?;
    std::fs::write(
        root.join(".claude-plugin").join("marketplace.json"),
        claude_marketplace_json(),
    )?;
    Ok(root)
}

fn claude_json(args: &[&str]) -> Result<serde_json::Value> {
    let output = Command::new("claude")
        .args(args)
        .output()
        .with_context(|| format!("running claude {}", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("claude {} failed: {}", args.join(" "), stderr.trim());
    }
    serde_json::from_slice(&output.stdout)
        .with_context(|| format!("parsing claude {} output", args.join(" ")))
}

fn claude_run(args: &[&str]) -> Result<()> {
    let output = Command::new("claude")
        .args(args)
        .output()
        .with_context(|| format!("running claude {}", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("claude {} failed: {}", args.join(" "), stderr.trim());
    }
    Ok(())
}

fn is_claude_plugin_installed() -> bool {
    claude_json(&["plugin", "list", "--json"])
        .ok()
        .is_some_and(|value| claude_plugin_is_enabled(&value))
}

fn claude_plugin_is_enabled(value: &serde_json::Value) -> bool {
    value.as_array().is_some_and(|plugins| {
        plugins.iter().any(|plugin| {
            plugin.get("id").and_then(|id| id.as_str()) == Some(CLAUDE_PLUGIN_SELECTOR)
                && plugin.get("enabled").and_then(|e| e.as_bool()) == Some(true)
        })
    })
}

fn registered_claude_marketplace_root() -> Result<Option<PathBuf>> {
    let value = claude_json(&["plugin", "marketplace", "list", "--json"])?;
    Ok(claude_marketplace_root_from_json(&value))
}

fn claude_marketplace_root_from_json(value: &serde_json::Value) -> Option<PathBuf> {
    value.as_array().and_then(|marketplaces| {
        marketplaces.iter().find_map(|marketplace| {
            (marketplace.get("name").and_then(|n| n.as_str()) == Some(CLAUDE_MARKETPLACE_NAME))
                .then(|| {
                    marketplace
                        .get("path")
                        .and_then(|p| p.as_str())
                        .or_else(|| marketplace.get("installLocation").and_then(|p| p.as_str()))
                })
                .flatten()
                .map(PathBuf::from)
        })
    })
}

pub fn install_claude(home: &Path) -> Result<String> {
    let root = write_claude_plugin(home)?;
    match registered_claude_marketplace_root()? {
        Some(existing) if existing != root => anyhow::bail!(
            "Claude marketplace {CLAUDE_MARKETPLACE_NAME} already points to {}",
            existing.display()
        ),
        Some(_) => {}
        None => {
            let root_arg = root.to_string_lossy();
            claude_run(&["plugin", "marketplace", "add", root_arg.as_ref()])?;
        }
    }
    claude_run(&[
        "plugin",
        "install",
        CLAUDE_PLUGIN_SELECTOR,
        "--scope",
        "user",
    ])?;
    Ok(format!(
        "Claude Code: installed {CLAUDE_PLUGIN_SELECTOR} from {}",
        root.display()
    ))
}

pub fn uninstall_claude(home: &Path) -> Result<String> {
    let root = claude_marketplace_root(home);
    let installed = is_claude_plugin_installed();
    let registered = registered_claude_marketplace_root()?;
    if let Some(existing) = &registered {
        if existing != &root {
            anyhow::bail!(
                "Claude marketplace {CLAUDE_MARKETPLACE_NAME} points to {}, not hop's {}",
                existing.display(),
                root.display()
            );
        }
    }
    if installed {
        claude_run(&["plugin", "uninstall", CLAUDE_PLUGIN_SELECTOR])?;
    }
    if registered.is_some() {
        claude_run(&["plugin", "marketplace", "remove", CLAUDE_MARKETPLACE_NAME])?;
    }
    if root.exists() {
        std::fs::remove_dir_all(&root)?;
    }
    if installed {
        Ok(format!("Claude Code: removed {CLAUDE_PLUGIN_SELECTOR}"))
    } else {
        Ok("Claude Code: no hop plugin found, nothing to remove".into())
    }
}

// --- Codex ---

pub fn codex_hooks_json() -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "hooks": {
            "SessionStart": [{"id": HOP_HOOK_ID, "hooks": [{"type": "command", "command": "hop meta capture --agent codex --event start"}]}],
            "Stop": [{"id": HOP_HOOK_ID, "hooks": [{"type": "command", "command": "hop meta capture --agent codex --event stop"}]}]
        }
    }))
    .unwrap()
}

fn codex_marketplace_root(home: &Path) -> PathBuf {
    home.join(".hop").join("codex-plugin-marketplace")
}

fn codex_plugin_dir(home: &Path) -> PathBuf {
    codex_marketplace_root(home)
        .join("plugins")
        .join(CODEX_PLUGIN_NAME)
}

fn codex_plugin_manifest() -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "name": CODEX_PLUGIN_NAME,
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Capture Codex session metadata for hop.",
        "author": { "name": "hop" },
        "license": "MIT",
        "keywords": ["hop", "session-metadata"],
        "interface": {
            "displayName": "hop Session Metadata",
            "shortDescription": "Capture Codex session metadata for hop",
            "longDescription": "Captures session lifecycle metadata so hop can index the final working directory and Git state.",
            "developerName": "hop",
            "category": "Developer Tools",
            "capabilities": ["Write"],
            "defaultPrompt": "Show hop metadata hook status."
        }
    }))
    .unwrap()
}

fn codex_marketplace_json() -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "name": CODEX_MARKETPLACE_NAME,
        "interface": { "displayName": "hop" },
        "plugins": [{
            "name": CODEX_PLUGIN_NAME,
            "source": {
                "source": "local",
                "path": format!("./plugins/{CODEX_PLUGIN_NAME}")
            },
            "policy": {
                "installation": "AVAILABLE",
                "authentication": "ON_INSTALL"
            },
            "category": "Developer Tools"
        }]
    }))
    .unwrap()
}

fn write_codex_plugin(home: &Path) -> Result<PathBuf> {
    let root = codex_marketplace_root(home);
    let plugin_dir = codex_plugin_dir(home);
    let manifest_dir = plugin_dir.join(".codex-plugin");
    std::fs::create_dir_all(&manifest_dir)
        .with_context(|| format!("creating {}", manifest_dir.display()))?;
    std::fs::create_dir_all(root.join(".agents").join("plugins"))?;
    std::fs::write(manifest_dir.join("plugin.json"), codex_plugin_manifest())?;
    std::fs::write(plugin_dir.join("hooks.json"), codex_hooks_json())?;
    std::fs::write(
        root.join(".agents")
            .join("plugins")
            .join("marketplace.json"),
        codex_marketplace_json(),
    )?;
    Ok(root)
}

fn codex_json(args: &[&str]) -> Result<serde_json::Value> {
    let output = Command::new("codex")
        .args(args)
        .output()
        .with_context(|| format!("running codex {}", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("codex {} failed: {}", args.join(" "), stderr.trim());
    }
    serde_json::from_slice(&output.stdout)
        .with_context(|| format!("parsing codex {} output", args.join(" ")))
}

fn is_codex_plugin_installed() -> bool {
    codex_json(&["plugin", "list", "--json"])
        .ok()
        .is_some_and(|value| codex_plugin_is_enabled(&value))
}

fn codex_plugin_is_enabled(value: &serde_json::Value) -> bool {
    value
        .get("installed")
        .and_then(|i| i.as_array())
        .is_some_and(|installed| {
            installed.iter().any(|plugin| {
                plugin.get("pluginId").and_then(|id| id.as_str()) == Some(CODEX_PLUGIN_SELECTOR)
                    && plugin.get("enabled").and_then(|e| e.as_bool()) == Some(true)
            })
        })
}

fn registered_codex_marketplace_root() -> Result<Option<PathBuf>> {
    let value = codex_json(&["plugin", "marketplace", "list", "--json"])?;
    Ok(codex_marketplace_root_from_json(&value))
}

fn codex_marketplace_root_from_json(value: &serde_json::Value) -> Option<PathBuf> {
    value
        .get("marketplaces")
        .and_then(|m| m.as_array())
        .and_then(|marketplaces| {
            marketplaces.iter().find_map(|marketplace| {
                (marketplace.get("name").and_then(|n| n.as_str()) == Some(CODEX_MARKETPLACE_NAME))
                    .then(|| {
                        marketplace
                            .get("marketplaceSource")
                            .and_then(|source| source.get("source"))
                            .and_then(|source| source.as_str())
                            .or_else(|| marketplace.get("root").and_then(|root| root.as_str()))
                    })
                    .flatten()
                    .map(PathBuf::from)
            })
        })
}

pub fn install_codex(home: &Path) -> Result<String> {
    let root = write_codex_plugin(home)?;
    match registered_codex_marketplace_root()? {
        Some(existing) if existing != root => anyhow::bail!(
            "Codex marketplace {CODEX_MARKETPLACE_NAME} already points to {}",
            existing.display()
        ),
        Some(_) => {}
        None => {
            let root_arg = root.to_string_lossy();
            codex_json(&["plugin", "marketplace", "add", root_arg.as_ref(), "--json"])?;
        }
    }
    codex_json(&["plugin", "add", CODEX_PLUGIN_SELECTOR, "--json"])?;
    Ok(format!(
        "Codex: installed {CODEX_PLUGIN_SELECTOR} from {}",
        root.display()
    ))
}

pub fn uninstall_codex(home: &Path) -> Result<String> {
    let root = codex_marketplace_root(home);
    let installed = is_codex_plugin_installed();
    let registered = registered_codex_marketplace_root()?;
    if let Some(existing) = &registered {
        if existing != &root {
            anyhow::bail!(
                "Codex marketplace {CODEX_MARKETPLACE_NAME} points to {}, not hop's {}",
                existing.display(),
                root.display()
            );
        }
    }
    if installed {
        codex_json(&["plugin", "remove", CODEX_PLUGIN_SELECTOR, "--json"])?;
    }
    if registered.is_some() {
        codex_json(&[
            "plugin",
            "marketplace",
            "remove",
            CODEX_MARKETPLACE_NAME,
            "--json",
        ])?;
    }
    if root.exists() {
        std::fs::remove_dir_all(&root)?;
    }
    if installed {
        Ok(format!("Codex: removed {CODEX_PLUGIN_SELECTOR}"))
    } else {
        Ok("Codex: no hop plugin found, nothing to remove".into())
    }
}

// --- Cursor ---

pub fn install_cursor(home: &Path) -> Result<String> {
    let path = home.join(".cursor").join("hooks.json");
    let existing = if path.exists() {
        std::fs::read_to_string(&path).context("reading hooks.json")?
    } else {
        r#"{"hooks":{},"version":1}"#.to_string()
    };
    let mut v: serde_json::Value = serde_json::from_str(&existing)?;
    let hooks = v
        .as_object_mut()
        .context("not an object")?
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));
    let hooks_obj = hooks.as_object_mut().context("hooks not object")?;
    let stop_arr = hooks_obj
        .entry("stop")
        .or_insert_with(|| serde_json::json!([]));
    let arr = stop_arr.as_array_mut().context("stop not array")?;
    arr.retain(|e| {
        e.get("command")
            .and_then(|c| c.as_str())
            .is_none_or(|c| !c.contains("hop meta capture"))
    });
    arr.push(serde_json::json!({"command": "hop meta capture --agent cursor --event stop"}));
    let json = serde_json::to_string_pretty(&v)?;
    std::fs::write(&path, &json)?;
    Ok(format!(
        "Cursor: added stop hook to {} [best-effort]",
        path.display()
    ))
}

pub fn uninstall_cursor(home: &Path) -> Result<String> {
    let path = home.join(".cursor").join("hooks.json");
    if !path.exists() {
        return Ok("Cursor: no hooks.json found, nothing to remove".into());
    }
    let existing = std::fs::read_to_string(&path)?;
    let mut v: serde_json::Value = serde_json::from_str(&existing)?;
    if let Some(hooks) = v.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        if let Some(stop) = hooks.get_mut("stop").and_then(|s| s.as_array_mut()) {
            stop.retain(|e| {
                e.get("command")
                    .and_then(|c| c.as_str())
                    .is_none_or(|c| !c.contains("hop meta capture"))
            });
        }
    }
    let json = serde_json::to_string_pretty(&v)?;
    std::fs::write(&path, &json)?;
    Ok(format!("Cursor: removed hop hooks from {}", path.display()))
}

pub fn install_provider(agent: AgentId, home: &Path) -> Result<String> {
    match agent {
        AgentId::Claude => install_claude(home),
        AgentId::Codex => install_codex(home),
        AgentId::Cursor => install_cursor(home),
    }
}

pub fn uninstall_provider(agent: AgentId, home: &Path) -> Result<String> {
    match agent {
        AgentId::Claude => uninstall_claude(home),
        AgentId::Codex => uninstall_codex(home),
        AgentId::Cursor => uninstall_cursor(home),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_plugin_hooks_json() {
        let json = claude_hooks_json();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(!v["hooks"]["SessionStart"].is_null());
        assert!(!v["hooks"]["SessionEnd"].is_null());
        assert!(v["hooks"]["SessionStart"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("hop meta capture --agent claude"));
    }

    #[test]
    fn claude_plugin_files_have_manifest_and_marketplace_entry() {
        let home = tempfile::tempdir().unwrap();
        let root = write_claude_plugin(home.path()).unwrap();
        let plugin_dir = root.join("plugins").join(CLAUDE_PLUGIN_NAME);

        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(plugin_dir.join(".claude-plugin/plugin.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["name"], CLAUDE_PLUGIN_NAME);
        assert_eq!(manifest["version"], env!("CARGO_PKG_VERSION"));

        let marketplace: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(".claude-plugin/marketplace.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(marketplace["name"], CLAUDE_MARKETPLACE_NAME);
        assert_eq!(marketplace["plugins"][0]["name"], CLAUDE_PLUGIN_NAME);
        assert_eq!(
            marketplace["plugins"][0]["source"],
            format!("./plugins/{CLAUDE_PLUGIN_NAME}")
        );
        assert!(plugin_dir.join("hooks/hooks.json").is_file());
    }

    #[test]
    fn claude_status_requires_enabled_installed_plugin() {
        let enabled = serde_json::json!([{
            "id": CLAUDE_PLUGIN_SELECTOR,
            "enabled": true
        }]);
        assert!(claude_plugin_is_enabled(&enabled));

        let disabled = serde_json::json!([{
            "id": CLAUDE_PLUGIN_SELECTOR,
            "enabled": false
        }]);
        assert!(!claude_plugin_is_enabled(&disabled));
    }

    #[test]
    fn claude_marketplace_lookup_uses_registered_name() {
        let value = serde_json::json!([{
            "name": CLAUDE_MARKETPLACE_NAME,
            "source": "directory",
            "path": "/tmp/hop-marketplace",
            "installLocation": "/tmp/cache/hop-marketplace"
        }]);
        assert_eq!(
            claude_marketplace_root_from_json(&value),
            Some(PathBuf::from("/tmp/hop-marketplace"))
        );
    }

    #[test]
    fn codex_plugin_hooks_json() {
        let json = codex_hooks_json();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(!v["hooks"]["SessionStart"].is_null());
        assert!(!v["hooks"]["Stop"].is_null());
    }

    #[test]
    fn codex_plugin_files_have_manifest_and_marketplace_entry() {
        let home = tempfile::tempdir().unwrap();
        let root = write_codex_plugin(home.path()).unwrap();
        let plugin_dir = root.join("plugins").join(CODEX_PLUGIN_NAME);

        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(plugin_dir.join(".codex-plugin/plugin.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["name"], CODEX_PLUGIN_NAME);
        assert_eq!(manifest["version"], env!("CARGO_PKG_VERSION"));
        assert!(manifest["interface"]["defaultPrompt"].is_string());

        let marketplace: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(".agents/plugins/marketplace.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(marketplace["name"], CODEX_MARKETPLACE_NAME);
        assert_eq!(marketplace["plugins"][0]["name"], CODEX_PLUGIN_NAME);
        assert_eq!(
            marketplace["plugins"][0]["source"]["path"],
            format!("./plugins/{CODEX_PLUGIN_NAME}")
        );
        assert!(plugin_dir.join("hooks.json").is_file());
    }

    #[test]
    fn codex_status_requires_enabled_installed_plugin() {
        let enabled = serde_json::json!({
            "installed": [{
                "pluginId": CODEX_PLUGIN_SELECTOR,
                "enabled": true
            }]
        });
        assert!(codex_plugin_is_enabled(&enabled));

        let disabled = serde_json::json!({
            "installed": [{
                "pluginId": CODEX_PLUGIN_SELECTOR,
                "enabled": false
            }]
        });
        assert!(!codex_plugin_is_enabled(&disabled));
    }

    #[test]
    fn codex_marketplace_lookup_uses_registered_name() {
        let value = serde_json::json!({
            "marketplaces": [{
                "name": CODEX_MARKETPLACE_NAME,
                "root": "/tmp/cache/hop-marketplace",
                "marketplaceSource": {
                    "sourceType": "local",
                    "source": "/tmp/hop-marketplace"
                }
            }]
        });
        assert_eq!(
            codex_marketplace_root_from_json(&value),
            Some(PathBuf::from("/tmp/hop-marketplace"))
        );
    }
}
