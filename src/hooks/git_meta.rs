use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct GitMeta {
    pub branch: Option<String>,
    pub repo_url: Option<String>,
    pub worktree: Option<String>,
}

impl GitMeta {
    pub fn collect(cwd: &str) -> Self {
        if cwd.is_empty() {
            return Self::default();
        }
        let branch = git_field(cwd, &["rev-parse", "--abbrev-ref", "HEAD"]).filter(|b| b != "HEAD");
        let repo_url = git_field(cwd, &["remote", "get-url", "origin"]);
        let worktree = detect_worktree(cwd);
        Self { branch, repo_url, worktree }
    }
}

fn git_field(dir: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

fn detect_worktree(dir: &str) -> Option<String> {
    let toplevel = git_field(dir, &["rev-parse", "--show-toplevel"])?;
    let common_dir = git_field(dir, &["rev-parse", "--git-common-dir"])?;
    let git_dir = git_field(dir, &["rev-parse", "--git-dir"])?;
    // If git-dir != common-dir, we're in a linked worktree
    if git_dir != common_dir { Some(toplevel) } else { None }
}

/// Build a hermetic git repo in a fresh tempdir with a known branch and origin
/// remote. Used by tests instead of the ambient repo, whose checkout state is
/// non-deterministic (e.g. CI checks out pull requests in detached HEAD).
#[cfg(test)]
pub(crate) fn init_test_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    let run = |args: &[&str]| {
        let status = Command::new("git").arg("-C").arg(path).args(args).status().unwrap();
        assert!(status.success(), "git {args:?} failed");
    };
    run(&["init"]);
    run(&["config", "user.email", "test@example.com"]);
    run(&["config", "user.name", "Test"]);
    run(&["checkout", "-b", "test-branch"]);
    run(&["commit", "--allow-empty", "-m", "init"]);
    run(&["remote", "add", "origin", "https://example.com/test/repo.git"]);
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_from_valid_git_repo() {
        let repo = init_test_repo();
        let meta = GitMeta::collect(repo.path().to_str().unwrap());
        assert_eq!(meta.branch.as_deref(), Some("test-branch"));
        assert!(meta.repo_url.is_some());
    }

    #[test]
    fn collect_from_non_git_dir() {
        let dir = tempfile::tempdir().unwrap();
        let meta = GitMeta::collect(dir.path().to_str().unwrap());
        assert!(meta.branch.is_none());
        assert!(meta.repo_url.is_none());
        assert!(meta.worktree.is_none());
    }

    #[test]
    fn collect_from_empty_string() {
        let meta = GitMeta::collect("");
        assert!(meta.branch.is_none());
    }
}
