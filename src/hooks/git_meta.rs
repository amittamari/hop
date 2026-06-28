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
        Self {
            branch,
            repo_url,
            worktree,
        }
    }
}

fn git_field(dir: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn detect_worktree(dir: &str) -> Option<String> {
    let toplevel = git_field(dir, &["rev-parse", "--show-toplevel"])?;
    let common_dir = git_field(dir, &["rev-parse", "--git-common-dir"])?;
    let git_dir = git_field(dir, &["rev-parse", "--git-dir"])?;
    // If git-dir != common-dir, we're in a linked worktree
    if git_dir != common_dir {
        Some(toplevel)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_from_valid_git_repo() {
        // Use the hop repo itself as the test subject
        let meta = GitMeta::collect(".");
        // We're in a git repo, so branch and repo_url should be present
        assert!(meta.branch.is_some());
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
