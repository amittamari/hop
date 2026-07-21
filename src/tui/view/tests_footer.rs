//! Footer render tests: primary key-hints, mode-aware Tab hint, and the
//! right-aligned status surviving narrow widths.

use super::test_support::{footer_text, render_to_text};
use super::*;
use crate::tui::SearchMode;
use crate::tui::toolbar::Scope;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

#[test]
fn renders_single_mode_footer_hints() {
    use crate::enrich::Enricher;
    use std::collections::HashMap;

    let app = App::new();
    let enr: Vec<Box<dyn Enricher>> = vec![];
    let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
    let cols = crate::tui::columns::default_columns();

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
    assert!(text.contains("type to search"));
    // Esc hint derives from the bindings table label ("clear query / quit");
    // assert on a stable substring rather than exact spacing.
    assert!(text.contains("clear"));
    // Simple mode surfaces the mode-aware Tab hint in the footer.
    assert!(text.contains("Tab focus toolbar"));
    // No mode indicators remain.
    assert!(!text.contains("NAV"));
}

#[test]
fn footer_tab_hint_is_mode_aware() {
    let mut simple = App::new(); // simple by default
    simple.init_search(SearchMode::Simple, Scope::All, None, String::new());
    assert!(render_to_text(&simple).contains("Tab focus toolbar"));

    let mut raw = App::new();
    raw.init_search(SearchMode::Raw, Scope::All, None, String::new());
    let raw_text = render_to_text(&raw);
    assert!(raw_text.contains("Tab autocomplete keyword"));
    assert!(!raw_text.contains("focus toolbar"));
}

#[test]
fn footer_shows_only_primary_actions() {
    // The footer is the compact primary subset of the bindings table; it must
    // not spill preview vocabulary or navigation chords, even when the preview
    // pane is visible on a wide terminal.
    let wide = footer_text(160, true);
    assert!(wide.contains("type to search"));
    assert!(wide.contains("Enter resume"));
    assert!(wide.contains("clear"));
    assert!(
        !wide.contains("Ctrl+P") && !wide.contains("toggle preview"),
        "footer must not show preview vocabulary: {wide:?}"
    );
    assert!(
        !wide.contains("move selection"),
        "footer must not show non-primary navigation hints: {wide:?}"
    );
}

#[test]
fn footer_warning_survives_narrow_width() {
    use crate::enrich::Enricher;
    use std::collections::HashMap;

    let app = App::new();
    let enr: Vec<Box<dyn Enricher>> = vec![];
    let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
    let cols = crate::tui::columns::default_columns();
    let status = StatusLine { sync: None, pr_pending: 0, warning: Some("WARNTOKEN".to_string()) };

    // 50 cols is too narrow for the full static hint + warning on one line;
    // the warning must still be present.
    let backend = TestBackend::new(50, 8);
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
                modal_command: None,
                theme: Theme::default(),
                row_style: RowStyle::Compact,
            },
        )
    })
    .unwrap();
    let buf = term.backend().buffer().clone();
    let text: String = buf.content().iter().map(|c| c.symbol()).collect();
    assert!(text.contains("WARNTOKEN"), "warning must survive narrow footer, got: {text:?}");
}
