use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Prose(String),
    Code { lang: Option<String>, text: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub blocks: Vec<Block>,
}

/// Split a text body into prose and fenced-code blocks. ```` ```lang ```` opens a
/// code block; a line that is exactly ``` (after trim) closes it. Empty prose runs
/// are dropped. Trailing/leading blank lines within prose are trimmed.
pub fn split_blocks(text: &str) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let mut prose: Vec<&str> = Vec::new();
    let mut code: Vec<&str> = Vec::new();
    let mut lang: Option<String> = None;
    let mut in_code = false;

    let flush_prose = |prose: &mut Vec<&str>, out: &mut Vec<Block>| {
        let joined = prose.join("\n");
        let trimmed = joined.trim();
        if !trimmed.is_empty() {
            out.push(Block::Prose(trimmed.to_string()));
        }
        prose.clear();
    };

    for line in text.lines() {
        let t = line.trim_end();
        if in_code {
            // A closing fence is a line of only backticks (>=3) after trimming;
            // any other line — including indented backtick examples — is captured verbatim.
            let trimmed = t.trim();
            if trimmed.len() >= 3 && trimmed.chars().all(|c| c == '`') {
                out.push(Block::Code {
                    lang: lang.take(),
                    text: code.join("\n"),
                });
                code.clear();
                in_code = false;
            } else {
                code.push(line); // preserve indentation for code
            }
            continue;
        }
        if let Some(rest) = t.trim_start().strip_prefix("```") {
            flush_prose(&mut prose, &mut out);
            let l = rest.trim();
            lang = if l.is_empty() {
                None
            } else {
                Some(l.to_string())
            };
            in_code = true;
            continue;
        }
        prose.push(line);
    }
    if in_code {
        // unterminated fence: keep what we have as code
        out.push(Block::Code {
            lang: lang.take(),
            text: code.join("\n"),
        });
    } else {
        flush_prose(&mut prose, &mut out);
    }
    out
}

/// Flatten messages into a single newline-joined string for the search index.
pub fn flatten_messages(msgs: &[Message]) -> String {
    let mut out = String::new();
    for m in msgs {
        for b in &m.blocks {
            let t = match b {
                Block::Prose(s) => s.trim(),
                Block::Code { text, .. } => text.trim(),
            };
            if t.is_empty() {
                continue;
            }
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(t);
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentId {
    Claude,
    Codex,
}

impl AgentId {
    pub const ALL: [AgentId; 2] = [AgentId::Claude, AgentId::Codex];

    pub fn slug(self) -> &'static str {
        match self {
            AgentId::Claude => "claude",
            AgentId::Codex => "codex",
        }
    }

    pub fn badge(self) -> &'static str {
        match self {
            AgentId::Claude => "CLAUDE",
            AgentId::Codex => "CODEX",
        }
    }

    pub fn from_slug(s: &str) -> Option<AgentId> {
        match s {
            "claude" => Some(AgentId::Claude),
            "codex" => Some(AgentId::Codex),
            _ => None,
        }
    }
}

pub type SessionId = String;
pub type DocumentKey = String;

/// Stable index identity for a session row. Raw session ids are only unique
/// within an agent, so indexed state uses `agent:id` while resume commands keep
/// the raw id.
pub fn document_key(agent: AgentId, id: &str) -> DocumentKey {
    format!("{}:{id}", agent.slug())
}

/// Cheap stat-level scan result for one session file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanEntry {
    pub path: PathBuf,
    /// File modification time, unix milliseconds.
    pub mtime: i64,
}

/// A fully parsed session, ready to index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: SessionId,
    pub agent: AgentId,
    pub title: String,
    pub directory: String,
    /// Session timestamp (first message), unix seconds.
    pub timestamp: i64,
    /// Indexed conversation content (user + assistant text only).
    pub content: String,
    pub message_count: u32,
    /// Source file mtime, unix milliseconds (drives incremental sync).
    pub mtime: i64,
    pub yolo: bool,
    /// Git branch at session time, captured from conversation data when present.
    pub branch: Option<String>,
    /// Git remote URL when the agent records it (Codex). None otherwise.
    pub repo_url: Option<String>,
    /// Source JSONL path used for preview transcript loading.
    pub source_path: Option<PathBuf>,
}

impl Session {
    pub fn document_key(&self) -> DocumentKey {
        document_key(self.agent, &self.id)
    }
}

/// Collapse whitespace runs to single spaces, trimming ends.
pub fn normalize_title(raw: &str) -> String {
    let mut out = String::new();
    let mut prev_space = false;
    for c in raw.trim().chars() {
        if c.is_whitespace() {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out
}

/// Collapse whitespace runs to single spaces and truncate to `max` chars
/// (counting the ellipsis), trimming ends.
pub fn truncate_title(raw: &str, max: usize) -> String {
    let collapsed = normalize_title(raw);
    let count = collapsed.chars().count();
    if count <= max {
        return collapsed;
    }
    let keep = max.saturating_sub(1);
    let mut s: String = collapsed.chars().take(keep).collect();
    s.push('…');
    s
}

/// Derive the display/search title from a source-specific title candidate or
/// the first user prose message. Width truncation is a rendering concern.
pub fn derive_session_title(explicit: Option<&str>, messages: &[Message]) -> String {
    let raw = explicit.or_else(|| {
        messages
            .iter()
            .find(|m| m.role == Role::User)
            .and_then(|m| {
                m.blocks.iter().find_map(|b| match b {
                    Block::Prose(s) => Some(s.as_str()),
                    _ => None,
                })
            })
    });

    raw.map(normalize_title)
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "(untitled)".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_slug_roundtrip() {
        assert_eq!(AgentId::Claude.slug(), "claude");
        assert_eq!(AgentId::Codex.slug(), "codex");
        assert_eq!(AgentId::from_slug("codex"), Some(AgentId::Codex));
        assert_eq!(AgentId::from_slug("nope"), None);
    }

    #[test]
    fn agent_badge() {
        assert_eq!(AgentId::Claude.badge(), "CLAUDE");
        assert_eq!(AgentId::Codex.badge(), "CODEX");
    }

    #[test]
    fn truncate_title_shortens_with_ellipsis() {
        assert_eq!(truncate_title("hello", 10), "hello");
        assert_eq!(truncate_title("hello world", 5), "hell…");
        // collapses internal whitespace/newlines
        assert_eq!(truncate_title("a\n  b\tc", 100), "a b c");
    }

    #[test]
    fn normalize_title_collapses_without_truncating() {
        let title = "one\n  two\tthree four five six";
        assert_eq!(normalize_title(title), "one two three four five six");
        assert!(normalize_title(title).chars().count() > 10);
    }

    #[test]
    fn derive_session_title_prefers_explicit_then_first_user_prose() {
        let messages = vec![Message {
            role: Role::User,
            blocks: vec![
                Block::Code {
                    lang: None,
                    text: "ignored".into(),
                },
                Block::Prose("first\nuser\tprompt".into()),
            ],
        }];

        assert_eq!(
            derive_session_title(Some("explicit  title"), &messages),
            "explicit title"
        );
        assert_eq!(derive_session_title(None, &messages), "first user prompt");
        assert_eq!(derive_session_title(None, &[]), "(untitled)");
    }

    #[test]
    fn split_blocks_separates_fenced_code() {
        let input = "before\n```rust\nfn x() {}\n```\nafter";
        let blocks = split_blocks(input);
        assert_eq!(
            blocks,
            vec![
                Block::Prose("before".into()),
                Block::Code {
                    lang: Some("rust".into()),
                    text: "fn x() {}".into()
                },
                Block::Prose("after".into()),
            ]
        );
    }

    #[test]
    fn split_blocks_plain_prose_is_single_block() {
        assert_eq!(
            split_blocks("just text"),
            vec![Block::Prose("just text".into())]
        );
    }

    #[test]
    fn split_blocks_unlabeled_fence_has_no_lang() {
        let blocks = split_blocks("```\nraw\n```");
        assert_eq!(
            blocks,
            vec![Block::Code {
                lang: None,
                text: "raw".into()
            }]
        );
    }

    #[test]
    fn split_blocks_empty_input_is_empty() {
        assert_eq!(split_blocks(""), vec![]);
    }

    #[test]
    fn split_blocks_unterminated_fence_kept_as_code() {
        assert_eq!(
            split_blocks("```rust\nlet x=1;"),
            vec![Block::Code {
                lang: Some("rust".into()),
                text: "let x=1;".into()
            }]
        );
    }

    #[test]
    fn flatten_messages_joins_prose_and_code() {
        let msgs = vec![
            Message {
                role: Role::User,
                blocks: vec![Block::Prose("hi".into())],
            },
            Message {
                role: Role::Agent,
                blocks: vec![
                    Block::Prose("fixed".into()),
                    Block::Code {
                        lang: Some("rust".into()),
                        text: "let x=1;".into(),
                    },
                ],
            },
        ];
        assert_eq!(flatten_messages(&msgs), "hi\nfixed\nlet x=1;");
    }
}
