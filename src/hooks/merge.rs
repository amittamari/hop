use crate::core::SessionSummary;
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

pub fn merge_sidecar(summary: &mut SessionSummary) {
    let path = sidecar_path(summary.agent, &summary.id);
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
