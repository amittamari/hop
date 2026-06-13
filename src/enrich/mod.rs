//! Pluggable per-session enrichment. Fast enrichers resolve inline for visible
//! rows; slow enrichers resolve in the background (see `service`).

pub mod gh_pr;
pub mod service;

use crate::core::SessionSummary;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrichKind {
    /// Pure/cheap; safe to call synchronously while rendering visible rows.
    Fast,
    /// May block or hit the network; must run off the UI thread.
    Slow,
}

/// A resolved enrichment value for one session, ready to display in a cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrichValue {
    pub text: String,
}

pub trait Enricher: Send + Sync {
    fn id(&self) -> &'static str;
    fn kind(&self) -> EnrichKind;
    fn resolve(&self, s: &SessionSummary) -> Option<EnrichValue>;
    /// Cache key for slow enrichers; unused for fast ones.
    fn cache_key(&self, _s: &SessionSummary) -> String {
        String::new()
    }
    fn ttl(&self) -> Duration {
        Duration::from_secs(0)
    }
}

/// Branch: from indexed session metadata.
pub struct BranchEnricher;

impl Enricher for BranchEnricher {
    fn id(&self) -> &'static str {
        "branch"
    }
    fn kind(&self) -> EnrichKind {
        EnrichKind::Fast
    }
    fn resolve(&self, s: &SessionSummary) -> Option<EnrichValue> {
        let b = s.branch.clone()?;
        Some(EnrichValue { text: b })
    }
}

/// Repo: `repo_url` basename when present, else the directory basename.
pub struct RepoEnricher;

impl Enricher for RepoEnricher {
    fn id(&self) -> &'static str {
        "repo"
    }
    fn kind(&self) -> EnrichKind {
        EnrichKind::Fast
    }
    fn resolve(&self, s: &SessionSummary) -> Option<EnrichValue> {
        if let Some(url) = &s.repo_url {
            if let Some(name) = repo_name_from_url(url) {
                return Some(EnrichValue { text: name });
            }
        }
        let base = Path::new(&s.directory)
            .file_name()?
            .to_string_lossy()
            .to_string();
        if base.is_empty() {
            None
        } else {
            Some(EnrichValue { text: base })
        }
    }
}

/// `git@github.com:owner/repo.git` or `https://github.com/owner/repo(.git)` -> `repo`.
pub fn repo_name_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches(".git");
    let last = trimmed.rsplit(['/', ':']).next()?;
    if last.is_empty() {
        None
    } else {
        Some(last.to_string())
    }
}

/// `git@github.com:owner/repo.git` or `https://host/owner/repo(.git)` -> `owner/repo`.
/// Unlike `repo_name_from_url`, this keeps the owner so the slug stays unique across
/// repos that share a basename. Used to auto-scope `hop` to the current repo.
pub fn repo_slug_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches(".git");
    let parts: Vec<&str> = trimmed
        .split(['/', ':'])
        .filter(|s| !s.is_empty())
        .collect();
    match parts.as_slice() {
        [.., owner, name] => Some(format!("{owner}/{name}")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{AgentId, SessionSummary};

    fn sess(branch: Option<&str>, repo_url: Option<&str>, dir: &str) -> SessionSummary {
        SessionSummary {
            id: "a".into(),
            agent: AgentId::Claude,
            title: "t".into(),
            directory: dir.into(),
            timestamp: 1,
            message_count: 0,
            yolo: false,
            branch: branch.map(|s| s.to_string()),
            repo_url: repo_url.map(|s| s.to_string()),
            source_path: None,
            archived: false,
        }
    }

    #[test]
    fn branch_from_data() {
        assert_eq!(
            BranchEnricher
                .resolve(&sess(Some("feat/x"), None, "/w"))
                .unwrap()
                .text,
            "feat/x"
        );
    }

    #[test]
    fn repo_from_url_then_dir() {
        assert_eq!(
            repo_name_from_url("git@github.com:me/web.git").as_deref(),
            Some("web")
        );
        assert_eq!(
            repo_name_from_url("https://github.com/me/web").as_deref(),
            Some("web")
        );
        assert_eq!(
            RepoEnricher
                .resolve(&sess(None, Some("git@github.com:me/web.git"), "/a/b"))
                .unwrap()
                .text,
            "web"
        );
        assert_eq!(
            RepoEnricher
                .resolve(&sess(None, None, "/a/myproj"))
                .unwrap()
                .text,
            "myproj"
        );
    }

    #[test]
    fn slug_keeps_owner() {
        assert_eq!(
            repo_slug_from_url("git@github.com:me/web.git").as_deref(),
            Some("me/web")
        );
        assert_eq!(
            repo_slug_from_url("https://github.com/me/web.git").as_deref(),
            Some("me/web")
        );
        assert_eq!(
            repo_slug_from_url("https://github.com/me/web").as_deref(),
            Some("me/web")
        );
        assert_eq!(repo_slug_from_url("").as_deref(), None);
    }
}
