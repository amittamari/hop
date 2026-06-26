pub mod claude;
pub mod codex;
pub mod cursor;

use crate::core::{AgentId, ScanEntry, Session, SessionId};
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

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
    /// argv for unarchiving an archived session before it can be resumed, or
    /// `None` when the agent has no archive notion. Only consulted for sessions
    /// flagged `archived`.
    fn unarchive_command(&self, _s: &Session) -> Option<Vec<String>> {
        None
    }
}

/// Default v1 adapters, honoring config data-dir overrides.
pub fn default_adapters(cfg: &crate::config::Config) -> Vec<Box<dyn Adapter>> {
    vec![
        Box::new(claude::ClaudeAdapter::new(cfg.data_dir(AgentId::Claude))),
        Box::new(codex::CodexAdapter::new(cfg.data_dir(AgentId::Codex))),
        Box::new(cursor::CursorAdapter::new(cfg.data_dir(AgentId::Cursor))),
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

/// Run `git -C <dir> <args...>` and return trimmed stdout. `None` if `dir` is
/// empty, the command fails (not a repo, no such ref), or git is unavailable.
fn git_field(dir: &str, args: &[&str]) -> Option<String> {
    if dir.is_empty() {
        return None;
    }
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

/// The `origin` remote URL. Same across all worktrees of a repo, which is what
/// makes it a stable repo key.
///
/// When `dir` no longer exists on disk (e.g. a deleted worktree), walk up
/// ancestor directories until we find one that exists and can resolve the
/// remote — the parent repo root will have the same origin.
pub fn git_remote_url(dir: &str) -> Option<String> {
    let result = git_field(dir, &["remote", "get-url", "origin"]);
    if result.is_some() {
        return result;
    }
    if Path::new(dir).exists() {
        return None;
    }
    for ancestor in Path::new(dir).ancestors().skip(1) {
        if ancestor.as_os_str().is_empty() {
            break;
        }
        if ancestor.exists() {
            if let Some(url) =
                git_field(&ancestor.to_string_lossy(), &["remote", "get-url", "origin"])
            {
                return Some(url);
            }
            break;
        }
    }
    None
}

/// Directory-keyed cache over a git resolver. Many sessions share a working
/// directory, so a `--rebuild` would otherwise spawn one `git` per session;
/// this collapses that to one per unique directory.
pub(crate) struct GitFieldCache {
    cache: Mutex<HashMap<String, Option<String>>>,
    resolver: fn(&str) -> Option<String>,
}

impl GitFieldCache {
    pub(crate) fn new(resolver: fn(&str) -> Option<String>) -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            resolver,
        }
    }

    pub(crate) fn resolve(&self, dir: &str) -> Option<String> {
        if dir.is_empty() {
            return None;
        }
        let mut cache = self.cache.lock().unwrap();
        if let Some(hit) = cache.get(dir) {
            return hit.clone();
        }
        let result = (self.resolver)(dir);
        cache.insert(dir.to_string(), result.clone());
        result
    }
}
