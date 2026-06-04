use std::path::PathBuf;

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
}

/// Collapse whitespace runs to single spaces and truncate to `max` chars
/// (counting the ellipsis), trimming ends.
pub fn truncate_title(raw: &str, max: usize) -> String {
    let collapsed: String = {
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
    };
    let count = collapsed.chars().count();
    if count <= max {
        return collapsed;
    }
    let keep = max.saturating_sub(1);
    let mut s: String = collapsed.chars().take(keep).collect();
    s.push('…');
    s
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
}
