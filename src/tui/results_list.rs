//! Renders the result list as an aligned column grid using the `columns`
//! solver, the fast enrichers, and a resolved-slow-value lookup.

use crate::columns::{fit, solve_layout, Column};
use crate::core::Session;
use crate::enrich::{EnrichKind, Enricher};
use crate::tui::{theme, view::rel_time};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use std::collections::HashMap;

/// Build one display line for a session given the resolved layout. `resolved`
/// maps (session_id, enricher_id) -> displayed text for slow enrichers; a
/// missing slow value renders as the pending glyph.
pub fn row_line(
    s: &Session,
    layout: &[(usize, u16)],
    columns: &[Column],
    enrichers: &[Box<dyn Enricher>],
    resolved: &HashMap<(String, &'static str), Option<String>>,
    now: i64,
) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (n, &(ci, width)) in layout.iter().enumerate() {
        if n > 0 {
            spans.push(Span::raw(" "));
        }
        let col = &columns[ci];
        let (text, style) = cell(s, col, enrichers, resolved, now);
        spans.push(Span::styled(fit(&text, width, col.align), style));
    }
    Line::from(spans)
}

fn cell(
    s: &Session,
    col: &Column,
    enrichers: &[Box<dyn Enricher>],
    resolved: &HashMap<(String, &'static str), Option<String>>,
    now: i64,
) -> (String, Style) {
    match col.id {
        "agent" => (s.agent.badge().to_string(), Style::default().fg(theme::agent_color(s.agent))),
        "title" => (s.title.clone(), Style::default()),
        "msgs" => (
            if s.message_count > 0 { s.message_count.to_string() } else { "-".into() },
            Style::default().fg(theme::DIM),
        ),
        "time" => (rel_time(s.timestamp, now), Style::default().fg(theme::DIM)),
        other => enrichment_cell(other, s, enrichers, resolved),
    }
}

fn enrichment_cell(
    id: &str,
    s: &Session,
    enrichers: &[Box<dyn Enricher>],
    resolved: &HashMap<(String, &'static str), Option<String>>,
) -> (String, Style) {
    let Some(enr) = enrichers.iter().find(|e| e.id() == id) else {
        return (String::new(), Style::default());
    };
    match enr.kind() {
        EnrichKind::Fast => (
            enr.resolve(s).map(|v| v.text).unwrap_or_else(|| "—".into()),
            Style::default().fg(theme::DIM),
        ),
        EnrichKind::Slow => match resolved.get(&(s.id.clone(), enr.id())) {
            Some(Some(text)) => (text.clone(), Style::default().fg(theme::ACCENT)),
            Some(None) => ("—".into(), Style::default().fg(theme::DIM)),
            None => ("⟳".into(), Style::default().fg(theme::DIM)),
        },
    }
}

/// Convenience: solve the layout for a given width.
pub fn layout_for(columns: &[Column], width: u16) -> Vec<(usize, u16)> {
    solve_layout(columns, width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::columns::default_columns;
    use crate::core::{AgentId, Session};
    use crate::enrich::{BranchEnricher, RepoEnricher};

    fn sess() -> Session {
        Session {
            id: "a".into(), agent: AgentId::Claude, title: "fix auth".into(),
            directory: "/work/api".into(), timestamp: 0, content: String::new(),
            message_count: 12, mtime: 0, yolo: false,
            branch: Some("feat/auth".into()), repo_url: None,
        }
    }

    #[test]
    fn row_renders_repo_branch_title() {
        let cols = default_columns();
        let layout = layout_for(&cols, 120);
        let enr: Vec<Box<dyn Enricher>> = vec![Box::new(BranchEnricher), Box::new(RepoEnricher)];
        let resolved = HashMap::new();
        let line = row_line(&sess(), &layout, &cols, &enr, &resolved, 3600);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("CLAUDE"));
        assert!(text.contains("api"));        // repo from dir basename
        assert!(text.contains("feat/auth"));  // branch from data
        assert!(text.contains("fix auth"));   // title
        assert!(text.contains("12"));         // msgs
    }

    #[test]
    fn pending_pr_shows_glyph() {
        let cols = default_columns();
        let layout = layout_for(&cols, 120);
        let enr: Vec<Box<dyn Enricher>> =
            vec![Box::new(crate::enrich::gh_pr::GhPrEnricher)];
        let resolved = HashMap::new();
        let line = row_line(&sess(), &layout, &cols, &enr, &resolved, 0);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("⟳"));
    }
}
