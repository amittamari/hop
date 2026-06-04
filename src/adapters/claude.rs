use crate::adapters::Adapter;
use crate::core::{truncate_title, AgentId, ScanEntry, Session, SessionId};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const TITLE_MAX: usize = 80;

pub struct ClaudeAdapter {
    /// Root holding `<encoded-cwd>/<session-uuid>.jsonl` (default ~/.claude/projects).
    root: PathBuf,
}

impl ClaudeAdapter {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

#[derive(Deserialize)]
struct Line {
    #[serde(rename = "type")]
    kind: Option<String>,
    cwd: Option<String>,
    timestamp: Option<String>,
    #[serde(rename = "isMeta")]
    is_meta: Option<bool>,
    #[serde(rename = "toolUseResult")]
    tool_use_result: Option<serde_json::Value>,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    content: Option<Content>,
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

const COMMAND_PREFIXES: [&str; 5] = [
    "<command-name>",
    "<command-message>",
    "<command-args>",
    "<local-command-stdout>",
    "<local-command-caveat>",
];

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
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .context("session file has no stem")?
            .to_string();

        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;

        let mut directory = String::new();
        let mut title: Option<String> = None;
        let mut first_ts: Option<i64> = None;
        let mut content = String::new();
        let mut message_count: u32 = 0;

        for line in raw.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let mut buf = line.as_bytes().to_vec();
            let parsed: Line = match simd_json::serde::from_slice(&mut buf) {
                Ok(l) => l,
                Err(_) => continue, // skip malformed line, non-fatal
            };

            if directory.is_empty() {
                if let Some(cwd) = &parsed.cwd {
                    directory = cwd.clone();
                }
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
            if title.is_none() && is_user {
                title = Some(truncate_title(&text, TITLE_MAX));
            }
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str(text.trim());
            message_count += 1;
        }

        Ok(Session {
            id,
            agent: AgentId::Claude,
            title: title.unwrap_or_else(|| "(untitled)".to_string()),
            directory,
            timestamp: first_ts.unwrap_or(0),
            content,
            message_count,
            mtime: 0, // filled by engine from ScanEntry
            yolo: false,
            branch: None,
            repo_url: None,
        })
    }

    fn resume_command(&self, s: &Session, yolo: bool) -> Vec<String> {
        if yolo {
            vec![
                "claude".into(),
                "--dangerously-skip-permissions".into(),
                "--resume".into(),
                s.id.clone(),
            ]
        } else {
            vec!["claude".into(), "--resume".into(), s.id.clone()]
        }
    }

    fn transcript(&self, _path: &Path) -> Result<Vec<crate::core::Message>> {
        Ok(Vec::new())
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
            if is_user && COMMAND_PREFIXES.iter().any(|p| s.trim_start().starts_with(p)) {
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
            if joined.is_empty() {
                None
            } else {
                Some(joined.join(" "))
            }
        }
    }
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
