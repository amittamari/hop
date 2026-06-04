use crate::adapters::claude::{file_mtime_ms, parse_ts_secs};
use crate::adapters::Adapter;
use crate::core::{truncate_title, AgentId, ScanEntry, Session, SessionId};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const TITLE_MAX: usize = 80;

pub struct CodexAdapter {
    /// ~/.codex (we read sessions/ and archived_sessions/ under it).
    root: PathBuf,
}

impl CodexAdapter {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn session_roots(&self) -> Vec<PathBuf> {
        vec![self.root.join("sessions"), self.root.join("archived_sessions")]
    }
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
    id: Option<String>,
    cwd: Option<String>,
    // turn_context
    approval_policy: Option<String>,
    sandbox_policy: Option<SandboxPolicy>,
    // event_msg
    #[serde(rename = "type")]
    sub: Option<String>,
    message: Option<String>,
}

#[derive(Deserialize)]
struct SandboxPolicy {
    #[serde(rename = "type")]
    kind: String,
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
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;

        let mut id = String::new();
        let mut directory = String::new();
        let mut title: Option<String> = None;
        let mut first_ts: Option<i64> = None;
        let mut content = String::new();
        let mut message_count: u32 = 0;
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
                    if let Some(i) = p.id {
                        id = i;
                    }
                    if let Some(c) = p.cwd {
                        directory = c;
                    }
                }
                "turn_context" => {
                    let never = p.approval_policy.as_deref() == Some("never");
                    let danger =
                        p.sandbox_policy.as_ref().map(|s| s.kind.as_str()) == Some("danger-full-access");
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
                    if text.trim().is_empty() {
                        continue;
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
                _ => {}
            }
        }

        if id.is_empty() {
            // fall back to the filename-derived uuid
            id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(session_id_from_filename)
                .unwrap_or_else(|| "unknown".to_string());
        }

        Ok(Session {
            id,
            agent: AgentId::Codex,
            title: title.unwrap_or_else(|| "(untitled)".to_string()),
            directory,
            timestamp: first_ts.unwrap_or(0),
            content,
            message_count,
            mtime: 0,
            yolo,
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

    fn supports_yolo(&self) -> bool {
        true
    }
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
