//! Layout, header, empty-state, and size-guard render tests, plus the
//! `rel_time` / `visible_result_range` unit checks.

use super::test_support::render_to_text;
use super::*;
use crate::core::{AgentId, SessionSummary};
use crate::tui::SearchMode;
use crate::tui::toolbar::Scope;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

#[test]
fn simple_mode_renders_scope_and_sort_toolbar() {
    let mut app = App::new();
    app.init_search(SearchMode::Simple, Scope::ThisRepo, Some("me/web".to_string()), String::new());
    let text = render_to_text(&app);
    assert!(text.contains("Scope"), "toolbar Scope control missing");
    assert!(text.contains("This repo"), "Scope value missing");
    assert!(text.contains("Sort"), "toolbar Sort control missing");
    assert!(text.contains("Relevance"), "default Sort value missing");
}

#[test]
fn raw_mode_hides_toolbar() {
    let mut app = App::new();
    app.init_search(SearchMode::Raw, Scope::All, None, String::new());
    let text = render_to_text(&app);
    assert!(!text.contains("Scope"), "raw mode should hide the toolbar");
    assert!(!text.contains("Sort"), "raw mode should hide the toolbar");
}

#[test]
fn empty_results_empty_query_shows_prompt() {
    use crate::enrich::Enricher;
    use std::collections::HashMap;

    let app = App::new(); // empty results, empty query
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
    assert!(text.contains("Type to search"));
}

#[test]
fn empty_results_with_query_shows_no_match() {
    use crate::enrich::Enricher;
    use std::collections::HashMap;

    let mut app = App::new();
    app.set_query("nope".to_string()); // results stay empty
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
    assert!(text.contains("No sessions match"));
    assert!(!text.contains("Type to search"));
}

#[test]
fn indexing_state_shows_spinner_and_label() {
    use crate::enrich::Enricher;
    use std::collections::HashMap;

    let mut app = App::new();
    app.set_indexing(Some(7));
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
    // The braille frame at frame=0 is the first table entry.
    assert!(text.contains(SPINNER_FRAMES[0]));
    assert!(text.contains("indexing 7"));
}

#[test]
fn rel_time_units() {
    assert_eq!(rel_time(0, 30), "30s");
    assert_eq!(rel_time(0, 120), "2m");
    assert_eq!(rel_time(0, 7200), "2h");
    assert_eq!(rel_time(0, 2 * 86400), "2d");
}

#[test]
fn visible_range_keeps_selection_in_view() {
    assert_eq!(visible_result_range(0, 0, 10), 0..0);
    assert_eq!(visible_result_range(100, 0, 10), 0..10);
    assert_eq!(visible_result_range(100, 9, 10), 0..10);
    assert_eq!(visible_result_range(100, 10, 10), 1..11);
    assert_eq!(visible_result_range(100, 99, 10), 90..100);
}

#[test]
fn tiny_terminal_shows_too_small_message() {
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

    let backend = TestBackend::new(20, 4);
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
    assert!(text.contains("too small"), "expected too-small notice, got: {text:?}");
}

#[test]
fn min_height_keeps_header_body_footer() {
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

    let backend = TestBackend::new(80, 6); // exactly the guard floor
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
    // Header query position marker, list row, and footer hint all present.
    assert!(text.contains("/1"), "header count missing: {text:?}");
    assert!(text.contains("fix auth"), "list row missing: {text:?}");
    assert!(text.contains("type to search"), "footer missing: {text:?}");
}

#[test]
fn preview_inner_width_normal_split() {
    // 200 cols at 50%: layout gives 100 to preview, minus 2 for border+padding.
    let w = preview_inner_width(200, 50);
    assert_eq!(w, 98);
}

#[test]
fn preview_inner_width_list_min_dominates() {
    // 100 cols at 70%: layout would give 70 to preview, but LIST_MIN_WIDTH (48)
    // forces the list to keep 48, leaving only 52 for preview → inner 50.
    let w = preview_inner_width(100, 70);
    assert_eq!(w, 50);
}

#[test]
fn preview_inner_width_below_threshold_returns_zero() {
    let w = preview_inner_width(80, 50);
    assert_eq!(w, 0);
}

#[test]
fn preview_inner_width_border_padding_deduction() {
    // Verify the deduction is exactly 2 (1 border + 1 padding).
    let w_full = {
        let body = Rect { x: 0, y: 0, width: 140, height: 1 };
        let [_, preview] =
            Layout::horizontal([Constraint::Min(LIST_MIN_WIDTH), Constraint::Percentage(50)])
                .areas(body);
        preview.width
    };
    let w_inner = preview_inner_width(140, 50);
    assert_eq!(w_full - w_inner, 2);
}
