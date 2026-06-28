use crate::core::SessionSummary;
use crate::hooks::git_meta::GitMeta;
use crate::hooks::sidecar::{sidecar_path, Sidecar};

pub fn apply_sidecar(summary: &mut SessionSummary, sidecar: &Sidecar) {
    if let Some(branch) = sidecar.last_branch() {
        summary.branch = Some(branch.to_string());
    }
    if let Some(repo_url) = sidecar.last_repo_url() {
        summary.repo_url = Some(repo_url.to_string());
    }
    if let Some(cwd) = sidecar.last_cwd() {
        summary.directory = cwd.to_string();
    }
    if let Some(worktree) = sidecar.last_worktree() {
        summary.worktree = Some(worktree.to_string());
    }
}

pub fn enrich_from_git_if_needed(summary: &mut SessionSummary) {
    if summary.branch.is_some() && summary.repo_url.is_some() {
        return;
    }
    if summary.directory.is_empty() {
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
    let path = sidecar_path(summary.agent, &summary.id);
    if let Some(sidecar) = Sidecar::read(&path) {
        apply_sidecar(summary, &sidecar);
    }
    enrich_from_git_if_needed(summary);
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
            yolo: false,
            branch: None,
            repo_url: None,
            source_path: None,
            archived: false,
            worktree: None,
            permission_mode: None,
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
        let mut summary = base_summary();
        summary.agent = AgentId::Cursor;
        summary.branch = None;
        summary.directory = ".".into(); // current dir is a git repo
        enrich_from_git_if_needed(&mut summary);
        assert!(summary.branch.is_some());
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
        assert_eq!(summary.repo_url.as_deref(), Some("https://example.com/repo"));
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
    fn merge_preserves_vendor_when_sidecar_field_is_none() {
        let mut summary = base_summary();
        summary.repo_url = Some("vendor-url".into());
        let sidecar = sidecar_with_events(vec![SidecarEvent {
            event: HookEvent::Start,
            timestamp: 100,
            cwd: None,
            branch: None,
            repo_url: None,
            worktree: None,
            permission_mode: None,
        }]);
        apply_sidecar(&mut summary, &sidecar);
        assert_eq!(summary.repo_url.as_deref(), Some("vendor-url"));
    }
}
