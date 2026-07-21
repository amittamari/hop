use crate::adapters::{Adapter, GitFieldCache, file_mtime_ms, git_remote_url, parse_ts_secs};
use crate::core::{
    AgentId, ScanEntry, Session, SessionId, SessionSummary, derive_session_title,
    is_command_tag_line,
};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct ClaudeAdapter {
    /// Root holding `<encoded-cwd>/<session-uuid>.jsonl` (default ~/.claude/projects).
    root: PathBuf,
    /// Claude transcripts record `cwd` but no git remote; resolve it at parse time.
    repo_cache: GitFieldCache,
}

impl ClaudeAdapter {
    pub fn new(root: PathBuf) -> Self {
        Self { root, repo_cache: GitFieldCache::new(git_remote_url) }
    }
}

#[derive(Deserialize)]
struct Line {
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(rename = "aiTitle")]
    ai_title: Option<String>,
    summary: Option<String>,
    cwd: Option<String>,
    timestamp: Option<String>,
    #[serde(rename = "isMeta")]
    is_meta: Option<bool>,
    #[serde(rename = "toolUseResult")]
    tool_use_result: Option<serde_json::Value>,
    #[serde(rename = "gitBranch")]
    git_branch: Option<String>,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    content: Option<Content>,
    model: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Content {
    Text(String),
    Blocks(Vec<Block>),
}

#[derive(Deserialize)]
struct Block {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

struct Extracted {
    messages: Vec<crate::core::Message>,
    directory: String,
    branch: Option<String>,
    title: Option<String>,
    first_ts: Option<i64>,
    model: Option<String>,
}

impl ClaudeAdapter {
    fn extract(&self, path: &Path) -> Result<Extracted> {
        use crate::core::{Message, Role, split_blocks};
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let mut directory = String::new();
        let mut branch: Option<String> = None;
        let mut title: Option<String> = None;
        let mut has_ai_title = false;
        let mut first_ts: Option<i64> = None;
        let mut messages: Vec<Message> = Vec::new();
        let mut model: Option<String> = None;

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
            if let Some(t) = nonempty_text(parsed.ai_title.as_deref()) {
                title = Some(t.to_string());
                has_ai_title = true;
            } else if !has_ai_title && let Some(t) = nonempty_text(parsed.summary.as_deref()) {
                title = Some(t.to_string());
            }
            if directory.is_empty()
                && let Some(cwd) = &parsed.cwd
            {
                directory = cwd.clone();
            }
            if branch.is_none()
                && let Some(b) = parsed.git_branch.as_deref()
                && !b.trim().is_empty()
            {
                branch = Some(b.to_string());
            }
            let kind = parsed.kind.as_deref().unwrap_or("");
            let is_user = kind == "user";
            let is_assistant = kind == "assistant";
            if !is_user && !is_assistant {
                continue;
            }
            if parsed.is_meta == Some(true) || parsed.tool_use_result.is_some() {
                continue;
            }
            // Keep the last real model a genuine assistant turn reports. Skip
            // synthetic sentinels like "<synthetic>" that Claude writes for
            // injected turns. Runs after the meta/tool-result skip so an injected
            // assistant line can't overwrite the model the user conversed with.
            if is_assistant
                && let Some(m) = parsed.message.as_ref().and_then(|m| m.model.as_deref())
            {
                let m = m.trim();
                if !m.is_empty() && !m.starts_with('<') {
                    model = Some(m.to_string());
                }
            }
            let text = parsed
                .message
                .as_ref()
                .and_then(|m| m.content.as_ref())
                .and_then(|c| extract_text(c, is_user));
            let Some(text) = text else { continue };
            if text.trim().is_empty() {
                continue;
            }
            if first_ts.is_none() {
                first_ts = parsed.timestamp.as_deref().and_then(parse_ts_secs);
            }
            let blocks = split_blocks(&text);
            messages.push(Message { role: if is_user { Role::User } else { Role::Agent }, blocks });
        }
        Ok(Extracted { messages, directory, branch, title, first_ts, model })
    }
}

impl Adapter for ClaudeAdapter {
    fn id(&self) -> AgentId {
        AgentId::Claude
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
            let pdir = project.path();
            if !pdir.is_dir() {
                continue;
            }
            for entry in std::fs::read_dir(&pdir)?.flatten() {
                let path = entry.path();
                // top-level *.jsonl only (skip <uuid>/subagents/*)
                if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                let mtime = file_mtime_ms(&entry)?;
                out.insert(id.to_string(), ScanEntry { path, mtime });
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
        let ex = self.extract(path)?;
        let title = derive_session_title(ex.title.as_deref(), &ex.messages);
        let content = flatten_messages(&ex.messages);
        let repo_url = self.repo_cache.resolve(&ex.directory);
        Ok(Session {
            meta: SessionSummary {
                id,
                agent: AgentId::Claude,
                title,
                directory: ex.directory,
                timestamp: ex.first_ts.unwrap_or(0),
                message_count: ex.messages.len() as u32,
                yolo: false,
                branch: ex.branch,
                repo_url,
                source_path: Some(path.to_path_buf()),
                archived: false,
                worktree: None,
                permission_mode: None,
                model: ex.model,
                commit: None,
                source: None,
                snippet: None,
            },
            content,
            mtime: 0,
        })
    }

    fn resume_command(&self, s: &Session, yolo: bool) -> Vec<String> {
        if yolo {
            vec![
                "claude".into(),
                "--dangerously-skip-permissions".into(),
                "--resume".into(),
                s.meta.id.clone(),
            ]
        } else {
            vec!["claude".into(), "--resume".into(), s.meta.id.clone()]
        }
    }

    fn transcript(&self, path: &Path) -> Result<Vec<crate::core::Message>> {
        Ok(self.extract(path)?.messages)
    }

    fn supports_yolo(&self) -> bool {
        true
    }
}

/// For user lines, a string is real text unless it starts with a command tag.
/// For block arrays (either role), keep only `text` blocks joined by space.
fn extract_text(content: &Content, is_user: bool) -> Option<String> {
    match content {
        Content::Text(s) => {
            if is_user && is_command_tag_line(s) {
                None
            } else {
                Some(s.clone())
            }
        }
        Content::Blocks(blocks) => {
            let joined: Vec<&str> = blocks
                .iter()
                .filter(|b| b.kind == "text")
                .filter_map(|b| b.text.as_deref())
                .collect();
            if joined.is_empty() { None } else { Some(joined.join(" ")) }
        }
    }
}

fn nonempty_text(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|s| !s.is_empty())
}
