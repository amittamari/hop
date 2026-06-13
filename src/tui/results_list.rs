//! Renders the result list as an aligned column grid using the `columns`
//! solver, the fast enrichers, and a resolved-slow-value lookup.

use crate::columns::{display_width, fit, solve_layout, solve_layout_with_desired, Column};
use crate::core::SessionSummary;
use crate::enrich::{EnrichKind, Enricher};
use crate::tui::theme::Theme;
use crate::tui::view::rel_time;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use std::collections::HashMap;

/// Build one display line for a session given the resolved layout. `resolved`
/// maps (document_key, enricher_id) -> displayed text for slow enrichers; a
/// missing slow value renders as the pending glyph.
pub fn row_line(
    s: &SessionSummary,
    layout: &[(usize, u16)],
    columns: &[Column],
    enrichers: &[Box<dyn Enricher>],
    resolved: &HashMap<(String, &'static str), Option<String>>,
    now: i64,
    theme: &Theme,
) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (n, &(ci, width)) in layout.iter().enumerate() {
        if n > 0 {
            spans.push(Span::raw(" "));
        }
        let col = &columns[ci];
        let (text, style) = cell(s, col, enrichers, resolved, now, theme);
        spans.push(Span::styled(fit(&text, width, col.align), style));
    }
    Line::from(spans)
}

pub fn header_line(layout: &[(usize, u16)], columns: &[Column], theme: &Theme) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (n, &(ci, width)) in layout.iter().enumerate() {
        if n > 0 {
            spans.push(Span::raw(" "));
        }
        let col = &columns[ci];
        spans.push(Span::styled(
            fit(col.header, width, col.align),
            Style::default().fg(theme.muted),
        ));
    }
    Line::from(spans)
}

fn cell(
    s: &SessionSummary,
    col: &Column,
    enrichers: &[Box<dyn Enricher>],
    resolved: &HashMap<(String, &'static str), Option<String>>,
    now: i64,
    theme: &Theme,
) -> (String, Style) {
    match col.id {
        "agent" => (
            s.agent.badge().to_string(),
            Style::default().fg(theme.agent_color(s.agent)),
        ),
        "title" => (s.title.clone(), Style::default()),
        "msgs" => (
            if s.message_count > 0 {
                s.message_count.to_string()
            } else {
                "-".into()
            },
            Style::default().fg(theme.muted),
        ),
        "time" => (rel_time(s.timestamp, now), Style::default().fg(theme.muted)),
        other => enrichment_cell(other, s, enrichers, resolved, theme),
    }
}

fn enrichment_cell(
    id: &str,
    s: &SessionSummary,
    enrichers: &[Box<dyn Enricher>],
    resolved: &HashMap<(String, &'static str), Option<String>>,
    theme: &Theme,
) -> (String, Style) {
    let Some(enr) = enrichers.iter().find(|e| e.id() == id) else {
        return (String::new(), Style::default());
    };
    match enr.kind() {
        EnrichKind::Fast => {
            let text = enr.resolve(s).map(|v| v.text).unwrap_or_else(|| "—".into());
            (text, Style::default().fg(theme.muted))
        }
        EnrichKind::Slow => match resolved.get(&(s.document_key(), enr.id())) {
            Some(Some(text)) => (text.clone(), Style::default().fg(theme.accent)),
            Some(None) => ("—".into(), Style::default().fg(theme.muted)),
            None => ("⟳".into(), Style::default().fg(theme.muted)),
        },
    }
}

/// Convenience: solve the layout for a given width.
pub fn layout_for(columns: &[Column], width: u16) -> Vec<(usize, u16)> {
    solve_layout(columns, width)
}

/// Solve the layout using only the rows currently visible in the viewport.
pub fn layout_for_rows(
    columns: &[Column],
    width: u16,
    rows: &[SessionSummary],
    enrichers: &[Box<dyn Enricher>],
    resolved: &HashMap<(String, &'static str), Option<String>>,
    now: i64,
) -> Vec<(usize, u16)> {
    let desired = desired_widths(columns, rows, enrichers, resolved, now);
    solve_layout_with_desired(columns, width, &desired)
}

fn desired_widths(
    columns: &[Column],
    rows: &[SessionSummary],
    enrichers: &[Box<dyn Enricher>],
    resolved: &HashMap<(String, &'static str), Option<String>>,
    now: i64,
) -> Vec<u16> {
    let mut widths: Vec<u16> = columns
        .iter()
        .map(|col| display_width(col.header) as u16)
        .collect();

    // Widths depend only on cell text, not style, so the theme is irrelevant
    // here; build one default instead of one per cell.
    let theme = Theme::default();
    for row in rows {
        for (i, col) in columns.iter().enumerate() {
            if col.flex {
                continue;
            }
            let (text, _) = cell(row, col, enrichers, resolved, now, &theme);
            widths[i] = widths[i].max(display_width(&text) as u16);
        }
    }

    widths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::columns::default_columns;
    use crate::core::{AgentId, SessionSummary};
    use crate::enrich::{BranchEnricher, RepoEnricher};

    fn sess() -> SessionSummary {
        SessionSummary {
            id: "a".into(),
            agent: AgentId::Claude,
            title: "fix auth".into(),
            directory: "/work/api".into(),
            timestamp: 0,
            message_count: 12,
            yolo: false,
            branch: Some("feat/auth".into()),
            repo_url: None,
            source_path: None,
        }
    }

    #[test]
    fn row_renders_repo_branch_title() {
        let cols = default_columns();
        let enr: Vec<Box<dyn Enricher>> = vec![Box::new(BranchEnricher), Box::new(RepoEnricher)];
        let resolved = HashMap::new();
        let row = sess();
        let layout = layout_for_rows(
            &cols,
            120,
            std::slice::from_ref(&row),
            &enr,
            &resolved,
            3600,
        );
        let line = row_line(&row, &layout, &cols, &enr, &resolved, 3600, &Theme::default());
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("CLAUDE"));
        assert!(text.contains("api")); // repo from dir basename
        assert!(text.contains("feat/auth")); // branch from data
        assert!(text.contains("fix auth")); // title
        assert!(text.contains("12")); // msgs
    }

    #[test]
    fn header_renders_visible_column_labels() {
        let cols = default_columns();
        let layout = layout_for(&cols, 120);
        let line = header_line(&layout, &cols, &Theme::default());
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("AGENT"));
        assert!(text.contains("REPO"));
        assert!(text.contains("BRANCH"));
        assert!(text.contains("TITLE"));
        assert!(text.contains("MSGS"));
        assert!(text.contains("PR"));
        assert!(text.contains("TIME"));
    }

    #[test]
    fn visible_row_content_sizes_repo_and_branch_before_title_flexes() {
        let cols = default_columns();
        let mut row = sess();
        row.directory = "/work/responsive-editor".into();
        row.branch = Some("workflow/ghostty-terminal".into());
        let enr: Vec<Box<dyn Enricher>> = vec![Box::new(BranchEnricher), Box::new(RepoEnricher)];
        let resolved = HashMap::new();
        let layout = layout_for_rows(&cols, 120, &[row], &enr, &resolved, 0);
        let width = |id| {
            layout
                .iter()
                .find(|&&(i, _)| cols[i].id == id)
                .map(|&(_, w)| w)
                .unwrap()
        };

        assert_eq!(width("repo"), "responsive-editor".len() as u16);
        assert_eq!(width("branch"), "workflow/ghostty-terminal".len() as u16);
        assert!(width("title") > 10);
    }

    #[test]
    fn pending_pr_shows_glyph() {
        let cols = default_columns();
        let layout = layout_for(&cols, 120);
        let enr: Vec<Box<dyn Enricher>> = vec![Box::new(crate::enrich::gh_pr::GhPrEnricher)];
        let resolved = HashMap::new();
        let line = row_line(&sess(), &layout, &cols, &enr, &resolved, 0, &Theme::default());
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("⟳"));
    }

    #[test]
    fn pr_cell_reads_resolved_with_full_enricher_list() {
        let cols = default_columns();
        let enr: Vec<Box<dyn Enricher>> = vec![
            Box::new(RepoEnricher),
            Box::new(BranchEnricher),
            Box::new(crate::enrich::gh_pr::GhPrEnricher),
        ];
        let mut resolved = HashMap::new();
        resolved.insert(("claude:a".to_string(), "pr"), Some("#42".to_string()));
        let row = sess();
        let layout = layout_for_rows(&cols, 120, std::slice::from_ref(&row), &enr, &resolved, 0);
        let line = row_line(&row, &layout, &cols, &enr, &resolved, 0, &Theme::default());
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("#42")); // resolved PR rendered
        assert!(text.contains("feat/auth")); // fast branch still rendered
    }
}
