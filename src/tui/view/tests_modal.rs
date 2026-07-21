//! Yolo / confirm-resume modal render tests: dialog content, warning-vs-accent
//! coloring, backdrop dimming, the archived-session variant, and the
//! missing-directory warning.

use super::*;
use crate::core::{AgentId, SessionSummary};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

#[test]
fn renders_yolo_dialog_and_status_footer() {
    use crate::enrich::Enricher;
    use std::collections::HashMap;

    let mut app = App::new();
    app.set_results(vec![SessionSummary {
        id: "a".into(),
        agent: AgentId::Claude,
        title: "fix auth".into(),
        directory: "/work/api".into(),
        timestamp: 0,
        message_count: 3,
        branch: Some("feat/auth".into()),
        ..Default::default()
    }]);
    app.open_yolo_modal_with(true);

    let enr: Vec<Box<dyn Enricher>> = vec![];
    let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
    let cols = crate::tui::columns::default_columns();
    let status = StatusLine {
        sync: Some("sync complete; parse errors 2".to_string()),
        pr_pending: 1,
        warning: Some("source unavailable".to_string()),
    };
    let command = vec![
        "claude".to_string(),
        "--dangerously-skip-permissions".to_string(),
        "--resume".to_string(),
        "a".to_string(),
    ];

    let backend = TestBackend::new(180, 16);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        render(
            f,
            &app,
            RenderModel {
                now: 100,
                columns: &cols,
                enrichers: &enr,
                resolved: &resolved,
                query_terms: &[],
                preview_lines: &[],
                status: &status,
                modal_command: Some(&command),
                theme: Theme::default(),
                row_style: RowStyle::Compact,
            },
        )
    })
    .unwrap();
    let buf = term.backend().buffer().clone();
    let text: String = buf.content().iter().map(|c| c.symbol()).collect();
    assert!(text.contains("confirm resume"));
    assert!(text.contains("YOLO on"));
    assert!(text.contains("Session"));
    assert!(text.contains("fix auth"));
    assert!(text.contains("Directory"));
    assert!(text.contains("/work/api"));
    assert!(text.contains("Command"));
    assert!(text.contains("claude"));
    assert!(text.contains("parse errors 2"));
    assert!(text.contains("pr 1 pending"));
    assert!(text.contains("source unavailable"));
}

#[test]
fn yolo_banner_uses_warning_color_not_accent() {
    use crate::enrich::Enricher;
    use std::collections::HashMap;

    let mut app = App::new();
    app.set_results(vec![SessionSummary {
        id: "a".into(),
        agent: AgentId::Claude,
        title: "fix auth".into(),
        directory: "/work/api".into(),
        timestamp: 0,
        message_count: 3,
        ..Default::default()
    }]);
    app.open_yolo_modal_with(true);

    let enr: Vec<Box<dyn Enricher>> = vec![];
    let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
    let cols = crate::tui::columns::default_columns();
    let backend = TestBackend::new(120, 16);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        render(
            f,
            &app,
            RenderModel {
                now: 100,
                columns: &cols,
                enrichers: &enr,
                resolved: &resolved,
                query_terms: &[],
                preview_lines: &[],
                status: &StatusLine::default(),
                modal_command: None,
                theme: Theme::default(),
                row_style: RowStyle::Compact,
            },
        )
    })
    .unwrap();

    let buf = term.backend().buffer().clone();
    let warning = crate::tui::theme::Theme::default().warning;
    let accent = crate::tui::theme::Theme::default().accent;
    let (w, h) = (buf.area.width, buf.area.height);
    let mut found = false;
    for y in 0..h {
        for x in 0..w {
            let cell = &buf[(x, y)];
            if cell.symbol() == "Y" {
                if cell.fg == warning {
                    found = true;
                }
                assert_ne!(cell.fg, accent, "YOLO banner must not use accent");
            }
        }
    }
    assert!(found, "expected a 'Y' cell painted with the warning color");
}

#[test]
fn yolo_backdrop_dims_background() {
    use crate::enrich::Enricher;
    use std::collections::HashMap;

    let mut app = App::new();
    app.set_results(vec![SessionSummary {
        id: "a".into(),
        agent: AgentId::Claude,
        title: "fix auth".into(),
        directory: "/work/api".into(),
        timestamp: 0,
        message_count: 3,
        ..Default::default()
    }]);
    app.open_yolo_modal_with(true);

    let enr: Vec<Box<dyn Enricher>> = vec![];
    let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
    let cols = crate::tui::columns::default_columns();
    let backend = TestBackend::new(120, 16);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        render(
            f,
            &app,
            RenderModel {
                now: 100,
                columns: &cols,
                enrichers: &enr,
                resolved: &resolved,
                query_terms: &[],
                preview_lines: &[],
                status: &StatusLine::default(),
                modal_command: None,
                theme: Theme::default(),
                row_style: RowStyle::Compact,
            },
        )
    })
    .unwrap();

    let buf = term.backend().buffer().clone();
    let overlay_bg = crate::tui::theme::Theme::default().overlay_bg;
    assert_eq!(buf[(0, 0)].bg, overlay_bg, "backdrop must set bg, not fg-only");
}

#[test]
fn confirm_modal_for_archived_session_explains_unarchive() {
    use crate::enrich::Enricher;
    use std::collections::HashMap;

    let mut app = App::new();
    app.set_results(vec![SessionSummary {
        id: "a".into(),
        agent: AgentId::Codex,
        title: "fix auth".into(),
        directory: "/work/api".into(),
        timestamp: 0,
        message_count: 3,
        archived: true,
        ..Default::default()
    }]);
    app.open_yolo_modal_with(false);

    let enr: Vec<Box<dyn Enricher>> = vec![];
    let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
    let cols = crate::tui::columns::default_columns();
    let backend = TestBackend::new(120, 18);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        render(
            f,
            &app,
            RenderModel {
                now: 100,
                columns: &cols,
                enrichers: &enr,
                resolved: &resolved,
                query_terms: &[],
                preview_lines: &[],
                status: &StatusLine::default(),
                modal_command: None,
                theme: Theme::default(),
                row_style: RowStyle::Compact,
            },
        )
    })
    .unwrap();
    let buf = term.backend().buffer().clone();
    let text: String = buf.content().iter().map(|c| c.symbol()).collect();
    assert!(
        text.contains("unarchive & resume"),
        "archived modal title should mention unarchive"
    );
    assert!(
        text.contains("it will be unarchived first"),
        "archived modal should explain the unarchive step"
    );
    assert!(
        text.contains("unarchive & resume"),
        "archived modal legend should reflect the unarchive step"
    );
}

#[test]
fn yolo_modal_warns_when_directory_missing() {
    use crate::enrich::Enricher;
    use std::collections::HashMap;

    let mut app = App::new();
    app.set_results(vec![SessionSummary {
        id: "a".into(),
        agent: AgentId::Claude,
        title: "fix auth".into(),
        directory: "/tmp/nonexistent-hop-test-dir-999999".into(),
        timestamp: 0,
        message_count: 3,
        ..Default::default()
    }]);
    app.open_yolo_modal_with(false);

    let enr: Vec<Box<dyn Enricher>> = vec![];
    let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
    let cols = crate::tui::columns::default_columns();
    let command = vec!["claude".to_string(), "--resume".to_string(), "a".to_string()];

    let backend = TestBackend::new(180, 16);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        render(
            f,
            &app,
            RenderModel {
                now: 100,
                columns: &cols,
                enrichers: &enr,
                resolved: &resolved,
                query_terms: &[],
                preview_lines: &[],
                status: &StatusLine::default(),
                modal_command: Some(&command),
                theme: Theme::default(),
                row_style: RowStyle::Compact,
            },
        )
    })
    .unwrap();
    let buf = term.backend().buffer().clone();
    let text: String = buf.content().iter().map(|c| c.symbol()).collect();
    assert!(text.contains("does not exist"), "missing-dir warning should appear: {text:?}");
    assert!(text.contains("Missing"), "Missing label should appear: {text:?}");
}

#[test]
fn yolo_modal_no_warning_when_directory_exists() {
    use crate::enrich::Enricher;
    use std::collections::HashMap;

    let mut app = App::new();
    app.set_results(vec![SessionSummary {
        id: "a".into(),
        agent: AgentId::Claude,
        title: "fix auth".into(),
        directory: "/tmp".into(),
        timestamp: 0,
        message_count: 3,
        ..Default::default()
    }]);
    app.open_yolo_modal_with(false);

    let enr: Vec<Box<dyn Enricher>> = vec![];
    let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
    let cols = crate::tui::columns::default_columns();
    let command = vec!["claude".to_string(), "--resume".to_string(), "a".to_string()];

    let backend = TestBackend::new(180, 16);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        render(
            f,
            &app,
            RenderModel {
                now: 100,
                columns: &cols,
                enrichers: &enr,
                resolved: &resolved,
                query_terms: &[],
                preview_lines: &[],
                status: &StatusLine::default(),
                modal_command: Some(&command),
                theme: Theme::default(),
                row_style: RowStyle::Compact,
            },
        )
    })
    .unwrap();
    let buf = term.backend().buffer().clone();
    let text: String = buf.content().iter().map(|c| c.symbol()).collect();
    assert!(
        !text.contains("does not exist"),
        "missing-dir warning should NOT appear for valid dir: {text:?}"
    );
}
