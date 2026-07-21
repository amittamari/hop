//! Compact (table) list rendering tests: rows, archived markers, selection
//! styling, narrow-width column dropping, and query-match highlighting.

use super::*;
use crate::core::{AgentId, SessionSummary};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

#[test]
fn non_empty_results_render_rows_not_empty_message() {
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
    let enr: Vec<Box<dyn Enricher>> = vec![];
    let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
    let cols = crate::tui::columns::default_columns();
    let backend = TestBackend::new(100, 12);
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
    let text: String = term.backend().buffer().content().iter().map(|c| c.symbol()).collect();
    assert!(text.contains("fix auth"));
    assert!(!text.contains("Type to search"));
    assert!(!text.contains("No sessions match"));
}

#[test]
fn archived_row_renders_marker_and_dims_cells() {
    use crate::enrich::Enricher;
    use ratatui::style::Modifier;
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
    let enr: Vec<Box<dyn Enricher>> = vec![];
    let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
    let cols = crate::tui::columns::default_columns();
    let backend = TestBackend::new(100, 12);
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
    assert!(text.contains("arch fix auth"), "archived marker prefixes title");
    // The title cell on the archived row must carry the DIM modifier.
    let dimmed = buf
        .content()
        .iter()
        .any(|c| c.symbol().contains('f') && c.modifier.contains(Modifier::DIM));
    assert!(dimmed, "archived row cells must be dimmed");
}

#[test]
fn selected_result_has_marker_and_focus_style() {
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

    let enr: Vec<Box<dyn Enricher>> = vec![];
    let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
    let cols = crate::tui::columns::default_columns();
    let backend = TestBackend::new(80, 8);
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

    let buf = term.backend().buffer();
    let marker = crate::tui::glyphs::Glyphs::ascii().selection_marker().trim();
    let has_marker = buf.content().iter().any(|c| c.symbol() == marker);
    assert!(has_marker, "selection marker should be rendered");
    let has_sel_bg =
        buf.content().iter().any(|c| c.bg == crate::tui::theme::Theme::default().selection_bg);
    assert!(has_sel_bg, "selected row should carry the selection background");
    let has_sel_fg = buf.content().iter().any(|c| {
        c.fg == crate::tui::theme::Theme::default().selection_fg
            && c.bg == crate::tui::theme::Theme::default().selection_bg
    });
    assert!(has_sel_fg, "selected row text should use the selection fg over selection bg");
}

#[test]
fn narrow_terminal_drops_low_priority_columns_but_keeps_title() {
    use crate::enrich::Enricher;
    use std::collections::HashMap;

    let mut app = App::new();
    app.set_preview(false, 50);
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
    let enr: Vec<Box<dyn Enricher>> = vec![];
    let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
    let cols = crate::tui::columns::default_columns();
    let backend = TestBackend::new(30, 8);
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
    let text: String = term.backend().buffer().content().iter().map(|c| c.symbol()).collect();
    assert!(text.contains("TITLE"), "TITLE header must survive narrow width");
    assert!(text.contains("fix auth"), "title value must survive");
    assert!(!text.contains("PR"), "lowest-priority PR column should be dropped");
}

#[test]
fn query_match_is_highlighted_in_rendered_title() {
    use crate::enrich::Enricher;
    use ratatui::style::Modifier;
    use std::collections::HashMap;

    let mut app = App::new();
    app.set_preview(false, 50);
    app.set_query("auth".to_string());
    app.set_results(vec![SessionSummary {
        id: "a".into(),
        agent: AgentId::Claude,
        title: "fix auth bug".into(),
        directory: "/work/api".into(),
        timestamp: 0,
        message_count: 3,
        ..Default::default()
    }]);
    let enr: Vec<Box<dyn Enricher>> = vec![];
    let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
    let cols = crate::tui::columns::default_columns();
    let terms = vec!["auth".to_string()];
    let backend = TestBackend::new(100, 8);
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
                query_terms: &terms,
                preview_lines: &[],
                status: &StatusLine::default(),
                modal_command: None,
                theme: Theme::default(),
                row_style: RowStyle::Compact,
            },
        )
    })
    .unwrap();
    let buf = term.backend().buffer();
    let any_reversed = buf.content().iter().any(|c| c.modifier.contains(Modifier::REVERSED));
    assert!(any_reversed, "matched query term in title should render reversed");
}
