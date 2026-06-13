use crate::adapters::{file_mtime_ms, git_remote_url, parse_ts_secs, Adapter, GitFieldCache};
use crate::core::{
    derive_session_title, is_command_tag_line, AgentId, ScanEntry, Session, SessionId,
};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct CodexAdapter {
    /// ~/.codex (we read sessions/ and archived_sessions/ under it).
    root: PathBuf,
    /// Fallback when a session_meta carries no git remote (e.g. older rollouts).
    repo_cache: GitFieldCache,
}

impl CodexAdapter {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            repo_cache: GitFieldCache::new(git_remote_url),
        }
    }

    fn session_roots(&self) -> Vec<PathBuf> {
        vec![
            self.root.join("sessions"),
            self.root.join("archived_sessions"),
        ]
    }

    fn extract(&self, path: &Path) -> Result<Extracted> {
        use crate::core::{split_blocks, Message, Role};
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let mut directory = String::new();
        let mut branch = None;
        let mut repo_url = None;
        let mut first_ts: Option<i64> = None;
        let mut messages: Vec<Message> = Vec::new();
        let mut yolo = false;

        for line in raw.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let mut buf = line.as_bytes().to_vec();
            let parsed: Line = match simd_json::serde::from_slice(&mut buf) {
                Ok(l) => l,
                Err(_) => continue,
            };
            if first_ts.is_none() {
                if let Some(ts) = parsed.timestamp.as_deref() {
                    first_ts = parse_ts_secs(ts);
                }
            }
            let Some(p) = parsed.payload else { continue };
            match parsed.kind.as_str() {
                "session_meta" => {
                    if let Some(c) = p.cwd {
                        directory = c;
                    }
                    if let Some(g) = p.git {
                        branch = g.branch.filter(|b| !b.trim().is_empty());
                        repo_url = g.repository_url.filter(|u| !u.trim().is_empty());
                    }
                }
                "turn_context" => {
                    let never = p.approval_policy.as_deref() == Some("never");
                    let danger = p.sandbox_policy.as_ref().map(|s| s.kind.as_str())
                        == Some("danger-full-access");
                    if never && danger {
                        yolo = true;
                    }
                }
                "event_msg" => {
                    let sub = p.sub.as_deref().unwrap_or("");
                    let is_user = sub == "user_message";
                    let is_agent = sub == "agent_message";
                    if !is_user && !is_agent {
                        continue;
                    }
                    let Some(text) = p.message else { continue };
                    let Some(text) = clean_event_message(&text) else {
                        continue;
                    };
                    let blocks = split_blocks(&text);
                    messages.push(Message {
                        role: if is_user { Role::User } else { Role::Agent },
                        blocks,
                    });
                }
                _ => {}
            }
        }
        Ok(Extracted {
            messages,
            directory,
            branch,
            repo_url,
            first_ts,
            yolo,
        })
    }
}

const DROP_XML_BLOCKS: [(&str, &str); 2] = [
    ("<environment_context", "</environment_context>"),
    ("<system-reminder", "</system-reminder>"),
];

fn clean_event_message(text: &str) -> Option<String> {
    let mut lines = Vec::new();
    let mut skip_external_agent_block = false;
    let mut skip_xml_until: Option<&'static str> = None;

    for line in text.lines() {
        let trimmed = line.trim_start();

        if let Some(end) = skip_xml_until {
            if trimmed.contains(end) {
                skip_xml_until = None;
            }
            continue;
        }

        if skip_external_agent_block {
            if trimmed.starts_with("[/external_agent_") {
                skip_external_agent_block = false;
            }
            continue;
        }

        if trimmed.starts_with("[external_agent_") {
            if !trimmed.contains("[/external_agent_") {
                skip_external_agent_block = true;
            }
            continue;
        }

        if is_command_tag_line(trimmed) {
            continue;
        }

        if let Some((_, end)) = DROP_XML_BLOCKS
            .iter()
            .find(|(start, _)| trimmed.starts_with(start))
        {
            if !trimmed.contains(end) {
                skip_xml_until = Some(*end);
            }
            continue;
        }

        lines.push(strip_codex_wrappers(line));
    }

    let cleaned = lines.join("\n");
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn strip_codex_wrappers(line: &str) -> String {
    line.replace("<context>", "")
        .replace("</context>", "")
        .replace("<user_instructions>", "")
        .replace("</user_instructions>", "")
}

#[derive(Deserialize)]
struct Line {
    #[serde(rename = "type")]
    kind: String,
    timestamp: Option<String>,
    payload: Option<Payload>,
}

#[derive(Deserialize)]
struct Payload {
    // session_meta
    cwd: Option<String>,
    git: Option<Git>,
    // turn_context
    approval_policy: Option<String>,
    sandbox_policy: Option<SandboxPolicy>,
    // event_msg
    #[serde(rename = "type")]
    sub: Option<String>,
    message: Option<String>,
}

#[derive(Deserialize)]
struct Git {
    branch: Option<String>,
    repository_url: Option<String>,
}

#[derive(Deserialize)]
struct SandboxPolicy {
    #[serde(rename = "type")]
    kind: String,
}

struct Extracted {
    messages: Vec<crate::core::Message>,
    directory: String,
    branch: Option<String>,
    repo_url: Option<String>,
    first_ts: Option<i64>,
    yolo: bool,
}

impl Adapter for CodexAdapter {
    fn id(&self) -> AgentId {
        AgentId::Codex
    }

    fn is_available(&self) -> bool {
        self.session_roots().iter().any(|p| p.is_dir())
    }

    fn scan(&self) -> Result<HashMap<SessionId, ScanEntry>> {
        let mut out = HashMap::new();
        for root in self.session_roots() {
            collect_jsonl(&root, &mut out)?;
        }
        Ok(out)
    }

    fn parse(&self, path: &Path) -> Result<Session> {
        use crate::core::flatten_messages;
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(session_id_from_filename)
            .unwrap_or_else(|| "unknown".to_string());
        let ex = self.extract(path)?;
        let title = derive_session_title(None, &ex.messages);
        let content = flatten_messages(&ex.messages);
        // Prefer the remote recorded in session_meta; fall back to resolving it
        // from the cwd so older rollouts without git metadata still get a repo.
        let repo_url = ex
            .repo_url
            .or_else(|| self.repo_cache.resolve(&ex.directory));
        Ok(Session {
            id,
            agent: AgentId::Codex,
            title,
            directory: ex.directory,
            timestamp: ex.first_ts.unwrap_or(0),
            content,
            message_count: ex.messages.len() as u32,
            mtime: 0,
            yolo: ex.yolo,
            branch: ex.branch,
            repo_url,
            source_path: Some(path.to_path_buf()),
            archived: is_archived_path(path),
        })
    }

    fn resume_command(&self, s: &Session, yolo: bool) -> Vec<String> {
        if yolo {
            vec![
                "codex".into(),
                "--dangerously-bypass-approvals-and-sandbox".into(),
                "resume".into(),
                s.id.clone(),
            ]
        } else {
            vec!["codex".into(), "resume".into(), s.id.clone()]
        }
    }

    fn transcript(&self, path: &Path) -> Result<Vec<crate::core::Message>> {
        Ok(self.extract(path)?.messages)
    }

    fn supports_yolo(&self) -> bool {
        true
    }

    fn unarchive_command(&self, s: &Session) -> Option<Vec<String>> {
        Some(vec!["codex".into(), "unarchive".into(), s.id.clone()])
    }
}

/// A session is archived when its file lives under `archived_sessions/`.
/// Codex archives by moving the rollout file there; the JSONL itself carries no
/// archive flag, so the directory is the only signal.
fn is_archived_path(path: &Path) -> bool {
    path.components()
        .any(|c| c.as_os_str() == "archived_sessions")
}

/// Extract the session id from a `rollout-<timestamp>-<uuid>` filename stem.
/// The timestamp portion is fixed-width (`YYYY-MM-DDTHH-MM-SS`, 19 chars), so we
/// strip the `rollout-` prefix and the 20-char `<timestamp>-` that follows it.
/// This yields the full UUID, which matches `session_meta.payload.id`.
fn session_id_from_filename(stem: &str) -> String {
    stem.strip_prefix("rollout-")
        .filter(|rest| rest.len() > 20)
        .map(|rest| rest[20..].to_string())
        .unwrap_or_else(|| stem.to_string())
}

/// Recursively collect `rollout-*.jsonl` files keyed by trailing-uuid id.
fn collect_jsonl(dir: &Path, out: &mut HashMap<SessionId, ScanEntry>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl(&path, out)?;
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let id = session_id_from_filename(stem);
        let mtime = file_mtime_ms(&entry)?;
        out.insert(id, ScanEntry { path, mtime });
    }
    Ok(())
}
