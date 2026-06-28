use crate::core::AgentId;
use crate::hooks::git_meta::GitMeta;
use crate::hooks::sidecar::{HookEvent, Sidecar, SidecarEvent};
use anyhow::{Context, Result};
use std::path::Path;

pub struct HookContext {
    pub session_id: String,
    pub cwd: String,
}

pub fn parse_hook_stdin(input: &str, _agent: AgentId) -> Result<HookContext> {
    let v: serde_json::Value = serde_json::from_str(input).context("parsing hook stdin")?;
    let session_id = v["session_id"].as_str().unwrap_or("").to_string();
    let cwd = v["cwd"].as_str().unwrap_or("").to_string();
    Ok(HookContext { session_id, cwd })
}

pub fn capture_to_dir(
    agent: AgentId,
    event: HookEvent,
    stdin: &str,
    sidecar_base: &Path,
) -> Result<()> {
    let ctx = parse_hook_stdin(stdin, agent)?;
    if ctx.session_id.is_empty() {
        anyhow::bail!("no session_id in hook input");
    }

    let git = GitMeta::collect(&ctx.cwd);
    let ts = jiff::Timestamp::now().as_second();
    let se = SidecarEvent {
        event,
        timestamp: ts,
        cwd: if ctx.cwd.is_empty() {
            None
        } else {
            Some(ctx.cwd)
        },
        branch: git.branch,
        repo_url: git.repo_url,
        worktree: git.worktree,
        permission_mode: None,
    };

    let path = sidecar_base
        .join(agent.slug())
        .join(format!("{}.json", ctx.session_id));

    let mut sidecar = Sidecar::read(&path).unwrap_or_else(|| Sidecar::new(agent, ctx.session_id));
    sidecar.append_event(se);
    sidecar.write(&path)?;
    Ok(())
}

pub fn capture(agent: AgentId, event: HookEvent, stdin: &str) -> Result<()> {
    let base = crate::hooks::sidecar::sidecar_dir();
    capture_to_dir(agent, event, stdin, &base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::AgentId;
    use crate::hooks::sidecar::{HookEvent, Sidecar};

    #[test]
    fn parse_claude_stdin() {
        let input = r#"{"session_id":"abc-123","cwd":"/home/user/project","hook_event_name":"SessionStart"}"#;
        let ctx = parse_hook_stdin(input, AgentId::Claude).unwrap();
        assert_eq!(ctx.session_id, "abc-123");
        assert_eq!(ctx.cwd, "/home/user/project");
    }

    #[test]
    fn parse_codex_stdin() {
        let input = r#"{"session_id":"def-456","cwd":"/work","hook_event_name":"SessionStart"}"#;
        let ctx = parse_hook_stdin(input, AgentId::Codex).unwrap();
        assert_eq!(ctx.session_id, "def-456");
        assert_eq!(ctx.cwd, "/work");
    }

    #[test]
    fn capture_writes_sidecar_to_dir() {
        let dir = tempfile::tempdir().unwrap();
        let sidecar_base = dir.path().to_path_buf();
        let input = r#"{"session_id":"s1","cwd":".","hook_event_name":"SessionStart"}"#;
        capture_to_dir(AgentId::Claude, HookEvent::Start, input, &sidecar_base).unwrap();
        let path = sidecar_base.join("claude").join("s1.json");
        let loaded = Sidecar::read(&path).unwrap();
        assert_eq!(loaded.session_id, "s1");
        assert_eq!(loaded.events.len(), 1);
    }

    #[test]
    fn capture_stop_appends_to_existing() {
        let dir = tempfile::tempdir().unwrap();
        let sidecar_base = dir.path().to_path_buf();
        let start_input = r#"{"session_id":"s1","cwd":".","hook_event_name":"SessionStart"}"#;
        capture_to_dir(
            AgentId::Claude,
            HookEvent::Start,
            start_input,
            &sidecar_base,
        )
        .unwrap();
        let stop_input = r#"{"session_id":"s1","cwd":".","hook_event_name":"Stop"}"#;
        capture_to_dir(AgentId::Claude, HookEvent::Stop, stop_input, &sidecar_base).unwrap();
        let path = sidecar_base.join("claude").join("s1.json");
        let loaded = Sidecar::read(&path).unwrap();
        assert_eq!(loaded.events.len(), 2);
    }
}
