use crate::core::AgentId;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

const HOP_HOOK_ID: &str = "hop-meta";

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
    let config_path = home.join(".claude").join("settings.json");
    let detected = home.join(".claude").exists();
    let installed = detected && is_claude_installed(&config_path);
    ProviderStatus {
        agent: AgentId::Claude,
        detected,
        installed,
        config_path,
        best_effort: false,
    }
}

fn detect_codex(home: &Path) -> ProviderStatus {
    let plugin_dir = home
        .join(".codex")
        .join(".tmp")
        .join("plugins")
        .join("plugins")
        .join("hop");
    let config_path = plugin_dir.join("hooks.json");
    let detected = home.join(".codex").exists();
    let installed = config_path.exists();
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

fn is_claude_installed(path: &Path) -> bool {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return false,
    };
    text.contains(HOP_HOOK_ID)
}

fn is_cursor_installed(path: &Path) -> bool {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return false,
    };
    text.contains("hop meta capture")
}

// --- Claude ---

pub fn claude_hook_entry(event: &str) -> String {
    let cli_event = match event {
        "SessionStart" => "start",
        "SessionEnd" => "stop",
        _ => event,
    };
    format!(
        r#"{{"id":"{HOP_HOOK_ID}","hooks":[{{"type":"command","command":"hop meta capture --agent claude --event {cli_event}"}}]}}"#
    )
}

pub fn merge_claude_hooks(existing_json: &str) -> Result<String> {
    let mut v: serde_json::Value =
        serde_json::from_str(existing_json).context("parsing settings.json")?;
    let hooks = v
        .as_object_mut()
        .context("settings.json is not an object")?
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));
    let hooks_obj = hooks
        .as_object_mut()
        .context("hooks is not an object")?;

    let start_entry: serde_json::Value =
        serde_json::from_str(&claude_hook_entry("SessionStart"))?;
    let end_entry: serde_json::Value = serde_json::from_str(&claude_hook_entry("SessionEnd"))?;

    for (event_name, entry) in [("SessionStart", start_entry), ("SessionEnd", end_entry)] {
        let arr = hooks_obj
            .entry(event_name)
            .or_insert_with(|| serde_json::json!([]));
        let arr = arr.as_array_mut().context("hook event is not an array")?;
        arr.retain(|e| e.get("id").and_then(|i| i.as_str()) != Some(HOP_HOOK_ID));
        arr.push(entry);
    }
    serde_json::to_string_pretty(&v).context("serializing settings.json")
}

pub fn unmerge_claude_hooks(existing_json: &str) -> Result<String> {
    let mut v: serde_json::Value = serde_json::from_str(existing_json).context("parsing")?;
    if let Some(hooks) = v.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        for (_event, arr) in hooks.iter_mut() {
            if let Some(a) = arr.as_array_mut() {
                a.retain(|e| e.get("id").and_then(|i| i.as_str()) != Some(HOP_HOOK_ID));
            }
        }
        // Remove empty arrays
        let empty_keys: Vec<String> = hooks
            .iter()
            .filter(|(_, v)| v.as_array().map_or(false, |a| a.is_empty()))
            .map(|(k, _)| k.clone())
            .collect();
        for k in empty_keys {
            hooks.remove(&k);
        }
    }
    serde_json::to_string_pretty(&v).context("serializing")
}

pub fn install_claude(home: &Path) -> Result<String> {
    let path = home.join(".claude").join("settings.json");
    let existing = if path.exists() {
        std::fs::read_to_string(&path).context("reading settings.json")?
    } else {
        "{}".to_string()
    };
    let merged = merge_claude_hooks(&existing)?;
    std::fs::write(&path, &merged).context("writing settings.json")?;
    Ok(format!(
        "Claude Code: added SessionStart and SessionEnd hooks to {}",
        path.display()
    ))
}

pub fn uninstall_claude(home: &Path) -> Result<String> {
    let path = home.join(".claude").join("settings.json");
    if !path.exists() {
        return Ok("Claude Code: no settings.json found, nothing to remove".into());
    }
    let existing = std::fs::read_to_string(&path)?;
    let cleaned = unmerge_claude_hooks(&existing)?;
    std::fs::write(&path, &cleaned)?;
    Ok(format!(
        "Claude Code: removed hop hooks from {}",
        path.display()
    ))
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

pub fn install_codex(home: &Path) -> Result<String> {
    let plugin_dir = home
        .join(".codex")
        .join(".tmp")
        .join("plugins")
        .join("plugins")
        .join("hop");
    std::fs::create_dir_all(&plugin_dir)?;
    let hooks_path = plugin_dir.join("hooks.json");
    std::fs::write(&hooks_path, codex_hooks_json())?;
    Ok(format!(
        "Codex: installed hop plugin at {}",
        plugin_dir.display()
    ))
}

pub fn uninstall_codex(home: &Path) -> Result<String> {
    let plugin_dir = home
        .join(".codex")
        .join(".tmp")
        .join("plugins")
        .join("plugins")
        .join("hop");
    if plugin_dir.exists() {
        std::fs::remove_dir_all(&plugin_dir)?;
        Ok(format!(
            "Codex: removed hop plugin from {}",
            plugin_dir.display()
        ))
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
            .map_or(true, |c| !c.contains("hop meta capture"))
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
                    .map_or(true, |c| !c.contains("hop meta capture"))
            });
        }
    }
    let json = serde_json::to_string_pretty(&v)?;
    std::fs::write(&path, &json)?;
    Ok(format!(
        "Cursor: removed hop hooks from {}",
        path.display()
    ))
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
    fn claude_hook_json_generation() {
        let json = claude_hook_entry("start");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], "hop-meta");
        assert!(parsed["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("hop meta capture"));
    }

    #[test]
    fn claude_settings_merge_preserves_existing() {
        let existing = r#"{
            "hooks": {
                "PreToolUse": [{"hooks": [{"type": "command", "command": "echo hi"}]}]
            }
        }"#;
        let merged = merge_claude_hooks(existing).unwrap();
        let v: serde_json::Value = serde_json::from_str(&merged).unwrap();
        // Existing hooks preserved
        assert!(!v["hooks"]["PreToolUse"].is_null());
        // Hop hooks added
        assert!(!v["hooks"]["SessionStart"].is_null());
        assert!(!v["hooks"]["SessionEnd"].is_null());
    }

    #[test]
    fn claude_settings_unmerge_removes_only_hop() {
        let with_hop = r#"{
            "hooks": {
                "PreToolUse": [{"hooks": [{"type": "command", "command": "echo hi"}]}],
                "SessionStart": [{"id": "hop-meta", "hooks": [{"type": "command", "command": "hop meta capture --agent claude --event start"}]}],
                "SessionEnd": [{"id": "hop-meta", "hooks": [{"type": "command", "command": "hop meta capture --agent claude --event stop"}]}]
            }
        }"#;
        let cleaned = unmerge_claude_hooks(with_hop).unwrap();
        let v: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
        assert!(!v["hooks"]["PreToolUse"].is_null());
        assert!(
            v["hooks"]["SessionStart"].is_null()
                || v["hooks"]["SessionStart"]
                    .as_array()
                    .unwrap()
                    .is_empty()
        );
    }

    #[test]
    fn codex_plugin_hooks_json() {
        let json = codex_hooks_json();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(!v["hooks"]["SessionStart"].is_null());
        assert!(!v["hooks"]["Stop"].is_null());
    }
}
