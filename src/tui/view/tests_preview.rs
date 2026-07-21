//! Preview-pane render tests: header + transcript, prose wrapping, and the
//! width thresholds that show or drop the preview while flooring the list.

use super::*;
use crate::core::{AgentId, SessionSummary};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

#[test]
fn renders_columns_and_preview() {
    use crate::core::{Block, Message, Role};
    use crate::enrich::{BranchEnricher, Enricher, RepoEnricher};
    use std::collections::HashMap;

    let mut app = App::new();
    app.set_preview(true, 50);
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
    let enr: Vec<Box<dyn Enricher>> = vec![Box::new(RepoEnricher), Box::new(BranchEnricher)];
    let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
    let transcript =
        vec![Message { role: Role::User, blocks: vec![Block::Prose("fix auth".into())] }];

    let lines = crate::tui::preview::render_transcript(
        &transcript,
        app.query(),
        AgentId::Claude,
        app.theme(),
    );

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
                preview_lines: &lines,
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
    assert!(text.contains("AGENT"));
    assert!(text.contains("REPO"));
    assert!(text.contains("CLAUDE"));
    assert!(text.contains("fix auth"));
    assert!(text.contains("feat/auth"));
    assert!(text.contains("/work/api"));
}

#[test]
fn wraps_long_preview_prose() {
    use crate::enrich::Enricher;
    use std::collections::HashMap;

    let mut app = App::new();
    app.set_preview(true, 50);
    app.set_preview_header(false);
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
    let preview_lines = vec![Line::from(
        "wrap-start one two three four five six seven eight nine ten wrap-end",
    )];

    let backend = TestBackend::new(140, 8);
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
                preview_lines: &preview_lines,
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
    assert!(text.contains("wrap-start"));
    assert!(text.contains("wrap-end"));
}

#[test]
fn narrow_width_drops_preview() {
    use crate::core::{Block, Message, Role};
    use crate::enrich::Enricher;
    use std::collections::HashMap;

    let mut app = App::new();
    app.set_preview(true, 50); // preview requested ON
    app.set_preview_header(false);
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
    let transcript = vec![Message {
        role: Role::User,
        blocks: vec![Block::Prose("PREVIEWBODYTOKEN".into())],
    }];
    let lines = crate::tui::preview::render_transcript(
        &transcript,
        app.query(),
        AgentId::Claude,
        app.theme(),
    );

    let backend = TestBackend::new(40, 15);
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
                preview_lines: &lines,
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
    // At 40 cols the preview is dropped entirely.
    assert!(
        !text.contains("PREVIEWBODYTOKEN"),
        "preview should be hidden at narrow width, got: {text:?}"
    );
}

#[test]
fn wide_width_keeps_preview_and_list_floor() {
    use crate::core::{Block, Message, Role};
    use crate::enrich::{BranchEnricher, Enricher, RepoEnricher};
    use std::collections::HashMap;

    let mut app = App::new();
    app.set_preview(true, 80); // even maxed preview pct must not starve the list
    app.set_preview_header(false);
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
    let enr: Vec<Box<dyn Enricher>> = vec![Box::new(RepoEnricher), Box::new(BranchEnricher)];
    let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
    let cols = crate::tui::columns::default_columns();
    let transcript = vec![Message {
        role: Role::User,
        blocks: vec![Block::Prose("PREVIEWBODYTOKEN".into())],
    }];
    let lines = crate::tui::preview::render_transcript(
        &transcript,
        app.query(),
        AgentId::Claude,
        app.theme(),
    );

    let backend = TestBackend::new(140, 15);
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
                preview_lines: &lines,
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
    // Preview is present at wide width...
    assert!(
        text.contains("PREVIEWBODYTOKEN"),
        "preview should be shown at wide width, got: {text:?}"
    );
    // ...and the list still shows its columns (grid not starved). The branch
    // column survives even though the floored 48-col list truncates its value.
    assert!(text.contains("fix auth"), "list content missing: {text:?}");
    assert!(text.contains("BRANCH"), "branch column missing: {text:?}");
    assert!(text.contains("feat/a"), "list branch missing: {text:?}");
}
