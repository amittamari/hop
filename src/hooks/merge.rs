use crate::core::SessionSummary;
use crate::hooks::git_meta::GitMeta;
use crate::hooks::sidecar::{sidecar_dir, sidecar_path_in, Sidecar};
use std::path::Path;

pub fn apply_sidecar(summary: &mut SessionSummary, sidecar: &Sidecar) {
    let Some(event) = sidecar.last_event() else {
        return;
    };
    summary.branch = event.branch.clone();
    summary.repo_url = event.repo_url.clone();
    summary.worktree = event.worktree.clone();
    if let Some(cwd) = &event.cwd {
        summary.directory = cwd.clone();
    }
}

pub fn enrich_from_git_if_needed(summary: &mut SessionSummary) {
    if summary.branch.is_some() && summary.repo_url.is_some() {
        return;
    }
    if summary.directory.is_empty() {
        return;
    }
    if !std::path::Path::new(&summary.directory).is_dir() {
        return;
    }
    let git = GitMeta::collect(&summary.directory);
    if summary.branch.is_none() {
        summary.branch = git.branch;
    }
    if summary.repo_url.is_none() {
        summary.repo_url = git.repo_url;
    }
    if summary.worktree.is_none() {
        summary.worktree = git.worktree;
    }
}

pub fn merge_sidecar(summary: &mut SessionSummary) {
    merge_sidecar_from_dir(summary, &sidecar_dir());
}

pub fn merge_sidecar_from_dir(summary: &mut SessionSummary, sidecar_base: &Path) {
    // Live Git is only a fallback. Apply it before the captured hook snapshot
    // so an authoritative final `None` is not repopulated from later repo state.
    enrich_from_git_if_needed(summary);
    let path = sidecar_path_in(sidecar_base, summary.agent, &summary.id);
    if let Some(sidecar) = Sidecar::read(&path) {
        apply_sidecar(summary, &sidecar);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{AgentId, SessionSummary};
    use crate::hooks::sidecar::{HookEvent, Sidecar, SidecarEvent};

    fn base_summary() -> SessionSummary {
        SessionSummary {
            id: "s1".into(),
            agent: AgentId::Claude,
            title: "test".into(),
            directory: "/vendor/path".into(),
            timestamp: 100,
            message_count: 5,
            ..Default::default()
        }
    }

    fn sidecar_with_events(events: Vec<SidecarEvent>) -> Sidecar {
        Sidecar {
            version: 1,
            session_id: "s1".into(),
            agent: AgentId::Claude,
            events,
        }
    }

    #[test]
    fn merge_fills_missing_branch() {
        let mut summary = base_summary();
        let sidecar = sidecar_with_events(vec![SidecarEvent {
            event: HookEvent::Start,
            timestamp: 100,
            cwd: Some("/project".into()),
            branch: Some("feature".into()),
            repo_url: None,
            worktree: None,
            permission_mode: None,
        }]);
        apply_sidecar(&mut summary, &sidecar);
        assert_eq!(summary.branch.as_deref(), Some("feature"));
    }

    #[test]
    fn merge_sidecar_wins_over_vendor_branch() {
        let mut summary = base_summary();
        summary.branch = Some("old-vendor-branch".into());
        let sidecar = sidecar_with_events(vec![
            SidecarEvent {
                event: HookEvent::Start,
                timestamp: 100,
                cwd: None,
                branch: Some("start-branch".into()),
                repo_url: None,
                worktree: None,
                permission_mode: None,
            },
            SidecarEvent {
                event: HookEvent::Stop,
                timestamp: 200,
                cwd: None,
                branch: Some("final-branch".into()),
                repo_url: None,
                worktree: None,
                permission_mode: None,
            },
        ]);
        apply_sidecar(&mut summary, &sidecar);
        assert_eq!(summary.branch.as_deref(), Some("final-branch"));
    }

    #[test]
    fn merge_fills_worktree() {
        let mut summary = base_summary();
        let sidecar = sidecar_with_events(vec![SidecarEvent {
            event: HookEvent::Start,
            timestamp: 100,
            cwd: None,
            branch: None,
            repo_url: None,
            worktree: Some("/worktrees/feature".into()),
            permission_mode: None,
        }]);
        apply_sidecar(&mut summary, &sidecar);
        assert_eq!(summary.worktree.as_deref(), Some("/worktrees/feature"));
    }

    #[test]
    fn merge_sidecar_cwd_wins() {
        let mut summary = base_summary();
        let sidecar = sidecar_with_events(vec![SidecarEvent {
            event: HookEvent::Start,
            timestamp: 100,
            cwd: Some("/sidecar/path".into()),
            branch: None,
            repo_url: None,
            worktree: None,
            permission_mode: None,
        }]);
        apply_sidecar(&mut summary, &sidecar);
        assert_eq!(summary.directory, "/sidecar/path");
    }

    #[test]
    fn cursor_enrichment_fills_branch_at_index_time() {
        let repo = crate::hooks::git_meta::init_test_repo();
        let mut summary = base_summary();
        summary.agent = AgentId::Cursor;
        summary.branch = None;
        summary.directory = repo.path().to_str().unwrap().into();
        enrich_from_git_if_needed(&mut summary);
        assert_eq!(summary.branch.as_deref(), Some("test-branch"));
    }

    #[test]
    fn enrichment_skips_when_branch_and_repo_url_present() {
        let mut summary = base_summary();
        summary.branch = Some("existing-branch".into());
        summary.repo_url = Some("https://example.com/repo".into());
        summary.directory = ".".into();
        enrich_from_git_if_needed(&mut summary);
        // Values unchanged — should not have shelled out
        assert_eq!(summary.branch.as_deref(), Some("existing-branch"));
        assert_eq!(
            summary.repo_url.as_deref(),
            Some("https://example.com/repo")
        );
    }

    #[test]
    fn enrichment_skips_on_empty_directory() {
        let mut summary = base_summary();
        summary.branch = None;
        summary.directory = String::new();
        enrich_from_git_if_needed(&mut summary);
        assert!(summary.branch.is_none());
    }

    #[test]
    fn enrichment_fills_repo_url_when_only_branch_present() {
        let mut summary = base_summary();
        summary.branch = Some("main".into());
        summary.repo_url = None;
        summary.directory = ".".into();
        enrich_from_git_if_needed(&mut summary);
        // repo_url should now be filled from git
        assert!(summary.repo_url.is_some());
        // branch should remain unchanged
        assert_eq!(summary.branch.as_deref(), Some("main"));
    }

    #[test]
    fn final_null_git_fields_clear_older_and_vendor_values() {
        let mut summary = base_summary();
        summary.branch = Some("vendor-branch".into());
        summary.repo_url = Some("vendor-url".into());
        summary.worktree = Some("/vendor/worktree".into());
        let sidecar = sidecar_with_events(vec![
            SidecarEvent {
                event: HookEvent::Start,
                timestamp: 100,
                cwd: Some("/start/path".into()),
                branch: Some("start-branch".into()),
                repo_url: Some("start-url".into()),
                worktree: Some("/start/worktree".into()),
                permission_mode: None,
            },
            SidecarEvent {
                event: HookEvent::Stop,
                timestamp: 200,
                cwd: None,
                branch: None,
                repo_url: None,
                worktree: None,
                permission_mode: None,
            },
        ]);
        apply_sidecar(&mut summary, &sidecar);
        assert_eq!(summary.branch, None);
        assert_eq!(summary.repo_url, None);
        assert_eq!(summary.worktree, None);
        assert_eq!(summary.directory, "/vendor/path");
    }

    #[test]
    fn empty_sidecar_preserves_vendor_metadata() {
        let mut summary = base_summary();
        summary.branch = Some("vendor-branch".into());
        summary.repo_url = Some("vendor-url".into());
        summary.worktree = Some("/vendor/worktree".into());
        apply_sidecar(&mut summary, &sidecar_with_events(vec![]));
        assert_eq!(summary.branch.as_deref(), Some("vendor-branch"));
        assert_eq!(summary.repo_url.as_deref(), Some("vendor-url"));
        assert_eq!(summary.worktree.as_deref(), Some("/vendor/worktree"));
    }

    #[test]
    fn final_null_snapshot_wins_over_live_git_fallback() {
        use crate::hooks::sidecar::sidecar_path_in;

        let sidecars = tempfile::tempdir().unwrap();
        let mut summary = base_summary();
        summary.directory = ".".into();
        let sidecar = sidecar_with_events(vec![SidecarEvent {
            event: HookEvent::Stop,
            timestamp: 200,
            cwd: None,
            branch: None,
            repo_url: None,
            worktree: None,
            permission_mode: None,
        }]);
        sidecar
            .write(&sidecar_path_in(sidecars.path(), AgentId::Claude, "s1"))
            .unwrap();

        merge_sidecar_from_dir(&mut summary, sidecars.path());

        assert_eq!(summary.branch, None);
        assert_eq!(summary.repo_url, None);
        assert_eq!(summary.worktree, None);
    }
}
