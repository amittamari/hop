use crate::adapters::{file_mtime_ms, git_remote_url, parse_ts_secs, Adapter, GitFieldCache};
use crate::core::{
    derive_session_title, is_command_tag_line, AgentId, ScanEntry, Session, SessionId,
    SessionSummary,
};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Read;
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
        let raw = read_rollout(path)?;
        let mut directory = String::new();
        let mut branch = None;
        let mut repo_url = None;
        let mut first_ts: Option<i64> = None;
        let mut event_messages: Vec<Message> = Vec::new();
        let mut response_messages: Vec<Message> = Vec::new();
        let mut history_mode = HistoryMode::Legacy;
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
                    history_mode = p.history_mode.unwrap_or_default();
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
                    event_messages.push(Message {
                        role: if is_user { Role::User } else { Role::Agent },
                        blocks,
                    });
                }
                "response_item" if p.sub.as_deref() == Some("message") => {
                    let role = match p.role.as_deref() {
                        Some("user") => Role::User,
                        Some("assistant") => Role::Agent,
                        _ => continue,
                    };
                    let text = p
                        .content
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|item| match item.kind.as_str() {
                            "input_text" | "output_text" => item.text,
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    let Some(text) = clean_event_message(&text) else {
                        continue;
                    };
                    response_messages.push(Message {
                        role,
                        blocks: split_blocks(&text),
                    });
                }
                _ => {}
            }
        }
        let messages = match history_mode {
            HistoryMode::Paginated if !response_messages.is_empty() => response_messages,
            HistoryMode::Paginated => event_messages,
            HistoryMode::Legacy if !event_messages.is_empty() => event_messages,
            HistoryMode::Legacy => response_messages,
        };
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

const DROP_XML_BLOCKS: [(&str, &str); 11] = [
    ("<user_instructions", "</user_instructions>"),
    ("<environment_context", "</environment_context>"),
    ("<apps_instructions", "</apps_instructions>"),
    ("<skills_instructions", "</skills_instructions>"),
    ("<plugins_instructions", "</plugins_instructions>"),
    ("<collaboration_mode", "</collaboration_mode>"),
    ("<multi_agent_mode", "</multi_agent_mode>"),
    ("<realtime_conversation", "</realtime_conversation>"),
    ("<context_window_guidance", "</context_window_guidance>"),
    ("<context_window", "</context_window>"),
    ("<system-reminder", "</system-reminder>"),
];

const USER_MESSAGE_BEGIN: &str = "## My request for Codex:";

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
    let trimmed = cleaned
        .find(USER_MESSAGE_BEGIN)
        .map(|idx| &cleaned[idx + USER_MESSAGE_BEGIN.len()..])
        .unwrap_or(&cleaned)
        .trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn strip_codex_wrappers(line: &str) -> String {
    line.replace("<context>", "").replace("</context>", "")
}

fn read_rollout(path: &Path) -> Result<String> {
    if is_compressed_rollout(path) {
        let file =
            std::fs::File::open(path).with_context(|| format!("reading {}", path.display()))?;
        let mut decoder = zstd::stream::read::Decoder::new(file)
            .with_context(|| format!("decompressing {}", path.display()))?;
        let mut raw = String::new();
        decoder
            .read_to_string(&mut raw)
            .with_context(|| format!("decompressing {}", path.display()))?;
        Ok(raw)
    } else {
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
    }
}

fn is_compressed_rollout(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".jsonl.zst"))
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
    #[serde(default)]
    history_mode: Option<HistoryMode>,
    // turn_context
    approval_policy: Option<String>,
    sandbox_policy: Option<SandboxPolicy>,
    // event_msg
    #[serde(rename = "type")]
    sub: Option<String>,
    message: Option<String>,
    role: Option<String>,
    content: Option<Vec<ContentItem>>,
}

#[derive(Deserialize)]
struct ContentItem {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum HistoryMode {
    Paginated,
    #[default]
    #[serde(other)]
    Legacy,
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
        let id = canonical_rollout_stem(path)
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
            meta: SessionSummary {
                id,
                agent: AgentId::Codex,
                title,
                directory: ex.directory,
                timestamp: ex.first_ts.unwrap_or(0),
                message_count: ex.messages.len() as u32,
                yolo: ex.yolo,
                branch: ex.branch,
                repo_url,
                source_path: Some(path.to_path_buf()),
                archived: is_archived_path(path),
                worktree: None,
                permission_mode: if ex.yolo {
                    Some("yolo".into())
                } else {
                    Some("default".into())
                },
            },
            content,
            mtime: 0,
        })
    }

    fn resume_command(&self, s: &Session, yolo: bool) -> Vec<String> {
        if yolo {
            vec![
                "codex".into(),
                "--dangerously-bypass-approvals-and-sandbox".into(),
                "resume".into(),
                s.meta.id.clone(),
            ]
        } else {
            vec!["codex".into(), "resume".into(), s.meta.id.clone()]
        }
    }

    fn transcript(&self, path: &Path) -> Result<Vec<crate::core::Message>> {
        Ok(self.extract(path)?.messages)
    }

    fn supports_yolo(&self) -> bool {
        true
    }

    fn unarchive_command(&self, s: &Session) -> Option<Vec<String>> {
        Some(vec!["codex".into(), "unarchive".into(), s.meta.id.clone()])
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

/// Recursively collect plain or zstd-compressed rollout files keyed by session id.
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
        let Some(stem) = canonical_rollout_stem(&path) else {
            continue;
        };
        let id = session_id_from_filename(stem);
        let mtime = file_mtime_ms(&entry)?;
        let should_insert = out.get(&id).is_none_or(|existing| {
            let same_siblings = existing.path.parent() == path.parent()
                && canonical_rollout_stem(&existing.path) == canonical_rollout_stem(&path);
            !same_siblings || is_compressed_rollout(&existing.path) || !is_compressed_rollout(&path)
        });
        if should_insert {
            out.insert(id, ScanEntry { path, mtime });
        }
    }
    Ok(())
}

fn canonical_rollout_stem(path: &Path) -> Option<&str> {
    let name = path.file_name()?.to_str()?;
    let plain = name.strip_suffix(".zst").unwrap_or(name);
    plain.strip_suffix(".jsonl")
}
