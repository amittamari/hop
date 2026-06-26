use hop::adapters::git_remote_url;
use std::process::Command;
use tempfile::TempDir;

fn init_repo_with_remote(dir: &std::path::Path, remote: &str) {
    Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["remote", "add", "origin", remote])
        .current_dir(dir)
        .output()
        .unwrap();
}

#[test]
fn resolves_remote_in_existing_repo() {
    let tmp = TempDir::new().unwrap();
    let url = "https://github.com/test/repo.git";
    init_repo_with_remote(tmp.path(), url);
    assert_eq!(
        git_remote_url(&tmp.path().to_string_lossy()),
        Some(url.to_string()),
    );
}

#[test]
fn walks_up_from_missing_subdirectory() {
    let tmp = TempDir::new().unwrap();
    let url = "https://github.com/test/repo.git";
    init_repo_with_remote(tmp.path(), url);

    // Simulate a deleted worktree: path does not exist on disk but its
    // ancestor is a real git repo.
    let missing = tmp.path().join(".worktrees").join("wt-1");
    assert!(!missing.exists());

    assert_eq!(
        git_remote_url(&missing.to_string_lossy()),
        Some(url.to_string()),
    );
}

#[test]
fn returns_none_for_totally_missing_path() {
    let missing = "/tmp/hop-test-nonexistent-abc123/x/y/z";
    assert_eq!(git_remote_url(missing), None);
}
