use crate::adapters::{Adapter, GitFieldCache, file_mtime_ms, git_remote_url};
use crate::core::{
    AgentId, Message, Role, ScanEntry, Session, SessionId, SessionSummary, derive_session_title,
    split_blocks,
};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct CursorAdapter {
    root: PathBuf,
    // cache: project_dir -> Option<workspacePath>
    wp_cache: RefCell<HashMap<PathBuf, Option<String>>>,
    /// Cursor records no git remote; resolve it from the workspace path at parse time.
    repo_cache: GitFieldCache,
}

impl CursorAdapter {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            wp_cache: RefCell::new(HashMap::new()),
            repo_cache: GitFieldCache::new(git_remote_url),
        }
    }

    /// Read workspacePath from <project_dir>/worker.log, cached.
    fn workspace_path(&self, project_dir: &Path) -> Option<String> {
        let mut cache = self.wp_cache.borrow_mut();
        if let Some(cached) = cache.get(project_dir) {
            return cached.clone();
        }
        let log = project_dir.join("worker.log");
        let result = (|| -> Option<String> {
            let text = std::fs::read_to_string(&log).ok()?;
            for line in text.lines() {
                if let Some(idx) = line.find("workspacePath=") {
                    let path = line[idx + "workspacePath=".len()..].trim_end().to_string();
                    if !path.is_empty() {
                        return Some(path);
                    }
                }
            }
            None
        })();
        cache.insert(project_dir.to_path_buf(), result.clone());
        result
    }
}

// ── JSONL deserialization ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct Line {
    role: Option<String>,
    message: Option<LineMessage>,
    /// Control lines carry a top-level `type` (e.g. `turn_ended`) instead of a role.
    #[serde(rename = "type")]
    kind: Option<String>,
    /// On a `turn_ended` line: `success`, `aborted`, or `error`.
    status: Option<String>,
}

#[derive(Deserialize)]
struct LineMessage {
    content: Option<Vec<Block>>,
}

#[derive(Deserialize)]
struct Block {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

// ── store.db metadata ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct StoreMeta {
    name: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: Option<u64>,
    #[serde(rename = "isRunEverything")]
    is_run_everything: Option<bool>,
}

struct Meta {
    title: Option<String>,
    timestamp_secs: Option<i64>,
    yolo: bool,
}

fn read_store_meta(chats_root: &Path, workspace: &str, uuid: &str) -> Option<Meta> {
    // hash = md5(realpath(workspace))
    let real = std::fs::canonicalize(workspace).unwrap_or_else(|_| PathBuf::from(workspace));
    let hash = format!("{:x}", md5::compute(real.to_string_lossy().as_bytes()));
    let db_path = chats_root.join(&hash).join(uuid).join("store.db");

    let conn =
        rusqlite::Connection::open_with_flags(&db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .ok()?;

    let hex_value: String =
        conn.query_row("SELECT value FROM meta WHERE key='0'", [], |row| row.get(0)).ok()?;

    let bytes = hex::decode(hex_value.trim()).ok()?;
    let store: StoreMeta = serde_json::from_slice(&bytes).ok()?;

    Some(Meta {
        title: store.name.filter(|s| !s.trim().is_empty()),
        timestamp_secs: store.created_at.map(|ms| (ms / 1000) as i64),
        yolo: store.is_run_everything.unwrap_or(false),
    })
}

// ── Extraction ────────────────────────────────────────────────────────────────

struct Extracted {
    messages: Vec<Message>,
}

/// If the text contains <user_query>…</user_query>, return the inner content.
/// Otherwise return the text as-is.
fn clean_user_text(text: &str) -> &str {
    if let Some(start) = text.find("<user_query>") {
        let inner_start = start + "<user_query>".len();
        if let Some(rel_end) = text[inner_start..].find("</user_query>") {
            return text[inner_start..inner_start + rel_end].trim();
        }
    }
    text.trim()
}

/// Cursor transcripts redact extended thinking as `[REDACTED]` in text blocks.
fn strip_redacted(text: &str) -> std::borrow::Cow<'_, str> {
    use std::borrow::Cow;
    let trimmed = text.trim();
    if trimmed == "[REDACTED]" {
        return Cow::Borrowed("");
    }
    if let Some(prefix) = trimmed.strip_suffix("[REDACTED]") {
        return Cow::Borrowed(prefix.trim_end());
    }
    Cow::Borrowed(trimmed)
}

impl CursorAdapter {
    fn extract(&self, path: &Path) -> Result<Extracted> {
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let mut messages: Vec<Message> = Vec::new();
        let mut errored = false;

        // One decode buffer reused across lines; simd-json needs `&mut [u8]`, so we
        // refill this rather than allocating a fresh Vec per line.
        let mut buf: Vec<u8> = Vec::new();
        for line in raw.lines() {
            if line.trim().is_empty() {
                continue;
            }
            buf.clear();
            buf.extend_from_slice(line.as_bytes());
            let parsed: Line = match simd_json::serde::from_slice(&mut buf) {
                Ok(l) => l,
                Err(_) => continue,
            };
            if parsed.kind.as_deref() == Some("turn_ended")
                && parsed.status.as_deref() == Some("error")
            {
                errored = true;
                continue;
            }
            let role = match parsed.role.as_deref() {
                Some("user") => Role::User,
                Some("assistant") => Role::Agent,
                _ => continue,
            };
            let Some(msg) = parsed.message else { continue };
            let Some(blocks) = msg.content else { continue };

            // Keep only text blocks; drop tool_use and everything else.
            let text_parts: Vec<&str> = blocks
                .iter()
                .filter(|b| b.kind == "text")
                .filter_map(|b| b.text.as_deref())
                .collect();

            if text_parts.is_empty() {
                continue;
            }

            let joined = text_parts.join(" ");
            let cleaned: std::borrow::Cow<'_, str> = if role == Role::User {
                std::borrow::Cow::Borrowed(clean_user_text(&joined))
            } else {
                strip_redacted(&joined)
            };

            if cleaned.is_empty() {
                continue;
            }

            let split = split_blocks(&cleaned);
            messages.push(Message { role, blocks: split });
        }

        // A turn that errored before the agent replied (e.g. a blocked subagent
        // spawn) has no usable conversation. Drop it so it isn't indexed.
        let has_agent_reply = messages.iter().any(|m| m.role == Role::Agent);
        if errored && !has_agent_reply {
            return Ok(Extracted { messages: Vec::new() });
        }

        Ok(Extracted { messages })
    }
}

// ── Adapter impl ──────────────────────────────────────────────────────────────

impl Adapter for CursorAdapter {
    fn id(&self) -> AgentId {
        AgentId::Cursor
    }

    fn is_available(&self) -> bool {
        self.root.is_dir()
    }

    fn scan(&self) -> Result<HashMap<SessionId, ScanEntry>> {
        let mut out = HashMap::new();
        if !self.root.is_dir() {
            return Ok(out);
        }
        for project in std::fs::read_dir(&self.root)?.flatten() {
            let project_dir = project.path();
            if !project_dir.is_dir() {
                continue;
            }
            let transcripts_dir = project_dir.join("agent-transcripts");
            if !transcripts_dir.is_dir() {
                continue;
            }
            for conv in std::fs::read_dir(&transcripts_dir)?.flatten() {
                let conv_dir = conv.path();
                if !conv_dir.is_dir() {
                    continue;
                }
                let Some(uuid) = conv_dir.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                // Canonical transcript file: <uuid>/<uuid>.jsonl
                for file_entry in std::fs::read_dir(&conv_dir)?.flatten() {
                    let fpath = file_entry.path();
                    if fpath.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                        continue;
                    }
                    let Some(stem) = fpath.file_stem().and_then(|s| s.to_str()) else {
                        continue;
                    };
                    if stem != uuid {
                        continue; // skip hook sidecars
                    }
                    let mtime = file_mtime_ms(&file_entry)?;
                    out.insert(uuid.to_string(), ScanEntry { path: fpath, mtime });
                    break; // only one canonical file per conv dir
                }
            }
        }
        Ok(out)
    }

    fn parse(&self, path: &Path) -> Result<Session> {
        use crate::core::flatten_messages;

        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .context("session file has no stem")?
            .to_string();

        // project_dir is  root/<slug>
        // path is          root/<slug>/agent-transcripts/<uuid>/<uuid>.jsonl
        let project_dir = path
            .parent() // <uuid>/
            .and_then(|p| p.parent()) // agent-transcripts/
            .and_then(|p| p.parent()) // <slug>/
            .unwrap_or(&self.root);

        let directory = self.workspace_path(project_dir).unwrap_or_default();

        let chats_root = self
            .root
            .parent()
            .map(|p| p.join("chats"))
            .unwrap_or_else(|| PathBuf::from(".cursor/chats"));

        let store = if !directory.is_empty() {
            read_store_meta(&chats_root, &directory, &id)
        } else {
            None
        };

        let ex = self.extract(path)?;
        let title = store
            .as_ref()
            .and_then(|m| m.title.clone())
            .unwrap_or_else(|| derive_session_title(None, &ex.messages));

        let timestamp = store.as_ref().and_then(|m| m.timestamp_secs).unwrap_or_else(|| {
            std::fs::metadata(path)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        });

        let yolo = store.as_ref().map(|m| m.yolo).unwrap_or(false);
        let content = flatten_messages(&ex.messages);
        let repo_url = self.repo_cache.resolve(&directory);

        Ok(Session {
            meta: SessionSummary {
                id,
                agent: AgentId::Cursor,
                title,
                directory,
                timestamp,
                message_count: ex.messages.len() as u32,
                yolo,
                branch: None,
                repo_url,
                source_path: Some(path.to_path_buf()),
                archived: false,
                worktree: None,
                permission_mode: None,
                model: None,
                commit: None,
                source: None,
            },
            content,
            mtime: 0,
        })
    }

    fn transcript(&self, path: &Path) -> Result<Vec<Message>> {
        Ok(self.extract(path)?.messages)
    }

    fn resume_command(&self, s: &Session, yolo: bool) -> Vec<String> {
        if yolo {
            vec!["cursor-agent".into(), "--force".into(), "--resume".into(), s.meta.id.clone()]
        } else {
            vec!["cursor-agent".into(), "--resume".into(), s.meta.id.clone()]
        }
    }

    fn supports_yolo(&self) -> bool {
        true
    }
}
