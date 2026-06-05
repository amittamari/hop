pub mod claude;
pub mod codex;

use crate::core::{AgentId, ScanEntry, Session, SessionId};
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

pub trait Adapter: Send + Sync {
    fn id(&self) -> AgentId;
    /// True if this agent's data directory exists.
    fn is_available(&self) -> bool;
    /// Cheap stat-level scan: session id -> (path, mtime). No file bodies read.
    fn scan(&self) -> Result<HashMap<SessionId, ScanEntry>>;
    /// Full parse of one session file.
    fn parse(&self, path: &Path) -> Result<Session>;
    /// Re-parse a session file into structured, internals-filtered messages for
    /// the preview. Shares the same extractor as `parse`.
    fn transcript(&self, path: &Path) -> Result<Vec<crate::core::Message>>;
    /// argv for resuming this session (program + args).
    fn resume_command(&self, s: &Session, yolo: bool) -> Vec<String>;
    fn supports_yolo(&self) -> bool;
}

/// Default v1 adapters, honoring config data-dir overrides.
pub fn default_adapters(cfg: &crate::config::Config) -> Vec<Box<dyn Adapter>> {
    vec![
        Box::new(claude::ClaudeAdapter::new(cfg.data_dir(AgentId::Claude))),
        Box::new(codex::CodexAdapter::new(cfg.data_dir(AgentId::Codex))),
    ]
}

pub(crate) fn parse_ts_secs(s: &str) -> Option<i64> {
    let ts: jiff::Timestamp = s.parse().ok()?;
    Some(ts.as_second())
}

pub(crate) fn file_mtime_ms(entry: &std::fs::DirEntry) -> Result<i64> {
    let modified = entry.metadata()?.modified()?;
    let dur = modified
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    Ok(i64::try_from(dur.as_millis()).unwrap_or(i64::MAX))
}
