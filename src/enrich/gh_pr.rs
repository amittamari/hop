//! GitHub PR enricher: maps (repo, branch) -> PR number via the `gh` CLI.
//! Slow (network); resolved in the background and disk-cached.

use super::{EnrichKind, EnrichValue, Enricher};
use crate::core::SessionSummary;
use std::time::Duration;

pub struct GhPrEnricher;

impl Enricher for GhPrEnricher {
    fn id(&self) -> &'static str {
        "pr"
    }
    fn kind(&self) -> EnrichKind {
        EnrichKind::Slow
    }
    fn resolve(&self, s: &SessionSummary) -> Option<EnrichValue> {
        let branch = s.branch.as_deref()?;
        if branch.is_empty() || branch == "master" || branch == "main" {
            return None;
        }
        let num = gh_pr_number(branch, s.repo_url.as_deref(), &s.directory)?;
        Some(EnrichValue {
            text: format!("#{num}"),
        })
    }
    fn cache_key(&self, s: &SessionSummary) -> String {
        let repo = s
            .repo_url
            .as_deref()
            .and_then(owner_repo_from_url)
            .unwrap_or_else(|| s.directory.clone());
        format!("{}@{}", repo, s.branch.as_deref().unwrap_or(""))
    }
    fn ttl(&self) -> Duration {
        Duration::from_secs(60 * 60) // 1h
    }
}

/// Run `gh pr list --head <branch> ...` and return the first PR number, if any.
/// Uses `--repo owner/repo` when derivable from the URL, else runs in `dir`.
fn gh_pr_number(branch: &str, repo_url: Option<&str>, dir: &str) -> Option<u64> {
    use std::process::Command;
    let mut cmd = Command::new("gh");
    cmd.args([
        "pr", "list", "--head", branch, "--state", "all", "--limit", "1", "--json", "number",
    ]);
    if let Some(slug) = repo_url.and_then(owner_repo_from_url) {
        cmd.args(["--repo", &slug]);
    } else if !dir.is_empty() {
        cmd.current_dir(dir);
    }
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_pr_number(&String::from_utf8_lossy(&out.stdout))
}

/// Open the PR whose resolved label is `pr_text` (e.g. `"#4821"`) in the
/// default browser via `gh pr view <n> --web`. Targets the repo the same way
/// `gh_pr_number` does — `--repo owner/repo` when derivable from the URL, else
/// the session directory. `gh` launches the browser through the OS opener and
/// returns promptly, so this is safe to call while the TUI owns the terminal;
/// stdout/stderr are silenced to keep the alternate screen clean. Returns
/// whether a PR number was parsed and the command launched successfully.
pub fn open_pr_in_browser(pr_text: &str, repo_url: Option<&str>, dir: &str) -> bool {
    use std::process::{Command, Stdio};
    let Some(number) = pr_number_from_label(pr_text) else {
        return false;
    };
    let mut cmd = Command::new("gh");
    cmd.args(["pr", "view", &number.to_string(), "--web"]);
    if let Some(slug) = repo_url.and_then(owner_repo_from_url) {
        cmd.args(["--repo", &slug]);
    } else if !dir.is_empty() {
        cmd.current_dir(dir);
    }
    cmd.stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Parse a resolved PR label such as `"#4821"` back into its number. Returns
/// None for the empty/absent (`"—"`) or otherwise unparseable labels.
pub fn pr_number_from_label(label: &str) -> Option<u64> {
    label.trim().trim_start_matches('#').parse::<u64>().ok()
}

/// Parse `[{"number":4821}]` -> 4821.
pub fn parse_pr_number(json: &str) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v.as_array()?.first()?.get("number")?.as_u64()
}

/// `git@github.com:owner/repo.git` / `https://github.com/owner/repo(/...)` -> `owner/repo`.
/// Requires the host to be exactly github.com (a boundary char `/` or `@` must
/// precede it, so `notgithub.com` is rejected) and returns only the first two
/// path segments, ignoring any trailing path.
pub fn owner_repo_from_url(url: &str) -> Option<String> {
    let t = url.trim().trim_end_matches(".git");
    let idx = t.find("github.com")?;
    if idx > 0 {
        let prev = t[..idx].chars().last()?;
        if prev != '/' && prev != '@' {
            return None;
        }
    }
    let rest = t[idx + "github.com".len()..].trim_start_matches([':', '/']);
    let mut segs = rest.split('/').filter(|s| !s.is_empty());
    let owner = segs.next()?;
    let repo = segs.next()?;
    Some(format!("{owner}/{repo}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pr_number_from_label_round_trips_and_rejects_junk() {
        assert_eq!(pr_number_from_label("#4821"), Some(4821));
        assert_eq!(pr_number_from_label(" 12 "), Some(12));
        assert_eq!(pr_number_from_label("—"), None);
        assert_eq!(pr_number_from_label(""), None);
        assert_eq!(pr_number_from_label("⟳"), None);
    }

    #[test]
    fn open_pr_rejects_unparseable_labels_without_launching() {
        // A non-numeric label can never resolve to a PR, so the helper returns
        // false before it would ever shell out to `gh`.
        assert!(!open_pr_in_browser("—", None, ""));
        assert!(!open_pr_in_browser(
            "",
            Some("git@github.com:me/web.git"),
            "/w"
        ));
    }

    #[test]
    fn parses_pr_number() {
        assert_eq!(parse_pr_number(r#"[{"number":4821}]"#), Some(4821));
        assert_eq!(parse_pr_number("[]"), None);
        assert_eq!(parse_pr_number("garbage"), None);
    }

    #[test]
    fn owner_repo_extraction() {
        assert_eq!(
            owner_repo_from_url("git@github.com:me/web.git").as_deref(),
            Some("me/web")
        );
        assert_eq!(
            owner_repo_from_url("https://github.com/me/web").as_deref(),
            Some("me/web")
        );
        assert_eq!(owner_repo_from_url("file:///tmp/x"), None);
        assert_eq!(
            owner_repo_from_url("https://github.com/owner/repo/tree/main").as_deref(),
            Some("owner/repo")
        );
        assert_eq!(
            owner_repo_from_url("https://notgithub.com/owner/repo"),
            None
        );
    }

    #[test]
    fn skips_default_branches() {
        use crate::core::{AgentId, SessionSummary};
        let s = SessionSummary {
            id: "a".into(),
            agent: AgentId::Claude,
            title: "t".into(),
            directory: "/w".into(),
            timestamp: 1,
            branch: Some("main".into()),
            ..Default::default()
        };
        assert_eq!(GhPrEnricher.resolve(&s), None);
    }
}
