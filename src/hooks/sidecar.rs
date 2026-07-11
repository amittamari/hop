use crate::core::AgentId;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    Start,
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidecarEvent {
    pub event: HookEvent,
    pub timestamp: i64,
    pub cwd: Option<String>,
    pub branch: Option<String>,
    pub repo_url: Option<String>,
    pub worktree: Option<String>,
    pub permission_mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sidecar {
    pub version: u32,
    pub session_id: String,
    #[serde(serialize_with = "ser_agent", deserialize_with = "de_agent")]
    pub agent: AgentId,
    pub events: Vec<SidecarEvent>,
}

fn ser_agent<S: serde::Serializer>(agent: &AgentId, s: S) -> std::result::Result<S::Ok, S::Error> {
    s.serialize_str(agent.slug())
}

fn de_agent<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<AgentId, D::Error> {
    let s = String::deserialize(d)?;
    AgentId::from_slug(&s).ok_or_else(|| serde::de::Error::custom(format!("unknown agent: {s}")))
}

impl Sidecar {
    pub fn new(agent: AgentId, session_id: String) -> Self {
        Self {
            version: 1,
            session_id,
            agent,
            events: Vec::new(),
        }
    }

    pub fn append_event(&mut self, event: SidecarEvent) {
        self.events.push(event);
    }

    pub fn read(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(self).context("serializing sidecar")?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, &json).with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, path).with_context(|| format!("renaming to {}", path.display()))?;
        Ok(())
    }

    /// The most recently appended snapshot. Its optional Git fields are
    /// authoritative, including `None` when the final state has no branch,
    /// origin remote, or linked worktree.
    pub fn last_event(&self) -> Option<&SidecarEvent> {
        self.events.last()
    }
}

pub fn sidecar_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|b| b.home_dir().join(".hop").join("meta"))
        .unwrap_or_else(|| PathBuf::from(".hop/meta"))
}

pub fn sidecar_path_in(base: &Path, agent: AgentId, session_id: &str) -> PathBuf {
    base.join(agent.slug()).join(format!("{session_id}.json"))
}

pub fn sidecar_path(agent: AgentId, session_id: &str) -> PathBuf {
    sidecar_path_in(&sidecar_dir(), agent, session_id)
}

/// A cheap file stamp for incremental indexing. Length is included so an
/// append is still detected on filesystems whose modification-time resolution
/// is too coarse to distinguish adjacent hook writes.
pub fn sidecar_stamp_in(base: &Path, agent: AgentId, session_id: &str) -> Option<String> {
    let metadata = std::fs::metadata(sidecar_path_in(base, agent, session_id)).ok()?;
    let modified = metadata.modified().ok()?;
    let nanos = modified
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    Some(format!("{nanos}:{}", metadata.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::AgentId;

    #[test]
    fn sidecar_roundtrip_write_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        let mut sidecar = Sidecar {
            version: 1,
            session_id: "abc-123".into(),
            agent: AgentId::Claude,
            events: vec![],
        };
        sidecar.append_event(SidecarEvent {
            event: HookEvent::Start,
            timestamp: 1719500000,
            cwd: Some("/home/user/project".into()),
            branch: Some("main".into()),
            repo_url: Some("git@github.com:user/repo.git".into()),
            worktree: None,
            permission_mode: None,
        });
        sidecar.write(&path).unwrap();
        let loaded = Sidecar::read(&path).unwrap();
        assert_eq!(loaded.session_id, "abc-123");
        assert_eq!(loaded.events.len(), 1);
        assert_eq!(loaded.events[0].branch.as_deref(), Some("main"));
    }

    #[test]
    fn sidecar_append_adds_stop_event() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        let mut sidecar = Sidecar {
            version: 1,
            session_id: "abc-123".into(),
            agent: AgentId::Claude,
            events: vec![SidecarEvent {
                event: HookEvent::Start,
                timestamp: 1719500000,
                cwd: Some("/project".into()),
                branch: Some("main".into()),
                repo_url: None,
                worktree: None,
                permission_mode: None,
            }],
        };
        sidecar.append_event(SidecarEvent {
            event: HookEvent::Stop,
            timestamp: 1719500300,
            cwd: Some("/project".into()),
            branch: Some("feature".into()),
            repo_url: None,
            worktree: None,
            permission_mode: None,
        });
        sidecar.write(&path).unwrap();
        let loaded = Sidecar::read(&path).unwrap();
        assert_eq!(loaded.events.len(), 2);
        assert_eq!(loaded.events[1].branch.as_deref(), Some("feature"));
    }

    #[test]
    fn sidecar_read_missing_file_returns_none() {
        assert!(Sidecar::read(std::path::Path::new("/nonexistent/path.json")).is_none());
    }

    #[test]
    fn sidecar_path_builds_correct_location() {
        let path = sidecar_path(AgentId::Claude, "abc-123");
        assert!(path.to_string_lossy().contains("claude"));
        assert!(path.to_string_lossy().contains("abc-123.json"));
    }
}
