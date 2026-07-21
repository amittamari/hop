//! Shared render helpers for the `view` test submodules.

use super::*;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

/// Render `app` to a 100x12 test terminal and return the flattened text.
pub(super) fn render_to_text(app: &App) -> String {
    use crate::enrich::Enricher;
    use std::collections::HashMap;
    let enr: Vec<Box<dyn Enricher>> = vec![];
    let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
    let cols = crate::tui::columns::default_columns();
    let backend = TestBackend::new(100, 12);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        render(
            f,
            app,
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
    term.backend().buffer().content().iter().map(|c| c.symbol()).collect()
}

/// Render just to read the footer at a given width, with the preview pane on
/// or off.
pub(super) fn footer_text(width: u16, preview: bool) -> String {
    use crate::enrich::Enricher;
    use std::collections::HashMap;

    let mut app = App::new();
    app.set_preview(preview, 50);
    let enr: Vec<Box<dyn Enricher>> = vec![];
    let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
    let cols = crate::tui::columns::default_columns();
    let backend = TestBackend::new(width, 8);
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
    term.backend().buffer().content().iter().map(|c| c.symbol()).collect()
}
