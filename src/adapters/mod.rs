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
    /// argv for resuming this session (program + args).
    fn resume_command(&self, s: &Session, yolo: bool) -> Vec<String>;
    fn supports_yolo(&self) -> bool;
}
