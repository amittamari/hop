//! Renders the result list as an aligned column grid using the `columns`
//! solver, the fast enrichers, and a resolved-slow-value lookup.

use crate::core::SessionSummary;
use crate::enrich::{EnrichKind, Enricher};
use crate::tui::columns::{Column, display_width, fit, solve_layout_with_desired};
use crate::tui::theme::Theme;
use crate::tui::view::rel_time;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Row};
use std::collections::HashMap;

fn cell(s: &SessionSummary, col: &Column, ctx: &RowCtx<'_>) -> (String, Style) {
    match col.id {
        "agent" => {
            (s.agent.badge().to_string(), Style::default().fg(ctx.theme.agent_color(s.agent)))
        }
        "title" => (s.title.clone(), Style::default()),
        "msgs" => (
            if s.message_count > 0 { s.message_count.to_string() } else { "-".into() },
            Style::default().fg(ctx.theme.muted),
        ),
        "time" => (rel_time(s.timestamp, ctx.now), Style::default().fg(ctx.theme.muted)),
        "model" => {
            (s.model.clone().unwrap_or_else(|| "-".into()), Style::default().fg(ctx.theme.muted))
        }
        other => enrichment_cell(other, s, ctx),
    }
}

fn enrichment_cell(id: &str, s: &SessionSummary, ctx: &RowCtx<'_>) -> (String, Style) {
    let Some(enr) = ctx.enrichers.iter().find(|e| e.id() == id) else {
        return (String::new(), Style::default());
    };
    match enr.kind() {
        EnrichKind::Fast => {
            let text = enr.resolve(s).map(|v| v.text).unwrap_or_else(|| "—".into());
            (text, Style::default().fg(ctx.theme.muted))
        }
        EnrichKind::Slow => match ctx.resolved.get(&(s.document_key(), enr.id())) {
            Some(Some(text)) => (text.clone(), Style::default().fg(ctx.theme.accent)),
            Some(None) => ("—".into(), Style::default().fg(ctx.theme.muted)),
            None => (
                crate::tui::view::spinner_glyph(ctx.frame).to_string(),
                Style::default().fg(ctx.theme.muted),
            ),
        },
    }
}

/// Compute the `(text, style)` of every column for the visible rows once per
/// frame. The width solver and the row builder both read this, so each cell's
/// `cell()` (and its per-row `document_key()` probe) runs once per frame instead
/// of twice. The flex (title) column is rendered specially with query-term
/// highlighting, so its slot is left as an empty placeholder here.
pub fn compute_cells(
    columns: &[Column],
    rows: &[SessionSummary],
    ctx: &RowCtx<'_>,
) -> Vec<Vec<(String, Style)>> {
    rows.iter()
        .map(|s| {
            columns
                .iter()
                .map(
                    |col| {
                        if col.flex { (String::new(), Style::default()) } else { cell(s, col, ctx) }
                    },
                )
                .collect()
        })
        .collect()
}

/// Solve the layout from precomputed cell texts (visible rows only). Non-flex
/// column widths come from the cell text; the flex column absorbs the slack.
pub fn layout_from_cells(
    columns: &[Column],
    width: u16,
    cells: &[Vec<(String, Style)>],
) -> Vec<(usize, u16)> {
    let mut widths: Vec<u16> = columns.iter().map(|col| display_width(col.header) as u16).collect();
    for row_cells in cells {
        for (i, col) in columns.iter().enumerate() {
            if col.flex {
                continue;
            }
            widths[i] = widths[i].max(display_width(&row_cells[i].0) as u16);
        }
    }
    solve_layout_with_desired(columns, width, &widths)
}

pub struct RowCtx<'a> {
    pub enrichers: &'a [Box<dyn Enricher>],
    pub resolved: &'a HashMap<(String, &'static str), Option<String>>,
    pub now: i64,
    pub frame: u64,
    pub terms: &'a [String],
    pub theme: &'a Theme,
}

/// Marker prefixed to the title of archived sessions so the state is explicit
/// beyond the row dimming.
const ARCHIVED_MARKER: &str = "arch ";

/// Build one Table row for a session across the kept (visible) columns, reusing
/// the cells computed for this row by [`compute_cells`] (indexed by column).
pub fn session_row(
    session: &SessionSummary,
    row_cells: &[(String, Style)],
    layout: &[(usize, u16)],
    columns: &[Column],
    ctx: &RowCtx<'_>,
) -> Row<'static> {
    let cells: Vec<Cell<'static>> = layout
        .iter()
        .map(|&(ci, width)| {
            let col = &columns[ci];
            if col.id == "title" {
                Cell::from(title_line(
                    &session.title,
                    width,
                    ctx.terms,
                    ctx.theme,
                    session.archived,
                ))
            } else {
                let (text, style) = &row_cells[ci];
                Cell::from(Span::styled(fit(text, width, col.align), *style))
            }
        })
        .collect();
    let row = Row::new(cells).height(1);
    // Dim the whole row for archived sessions; the selection highlight still
    // layers on top via the Table's row_highlight_style.
    if session.archived { row.style(Style::default().add_modifier(Modifier::DIM)) } else { row }
}

/// Build the TITLE line, reverse-highlighting any query-term matches by
/// reusing the preview's multi-byte-safe highlighter. Archived sessions get a
/// muted `arch` marker prefixed to the title within the same cell width.
fn title_line(
    title: &str,
    width: u16,
    terms: &[String],
    theme: &Theme,
    archived: bool,
) -> Line<'static> {
    let marker_width = if archived { ARCHIVED_MARKER.len() as u16 } else { 0 };
    let title_width = width.saturating_sub(marker_width);
    let base = Line::from(Span::raw(fit(title, title_width, crate::tui::columns::Align::Left)));
    let highlighted = if terms.is_empty() {
        base
    } else {
        crate::tui::preview::highlight_terms(&base, terms, theme)
    };
    if !archived {
        return highlighted;
    }
    let mut spans = vec![Span::styled(ARCHIVED_MARKER, Style::default().fg(theme.muted))];
    spans.extend(highlighted.spans);
    Line::from(spans)
}

/// Build the muted header row for the kept columns. Styled at the Row level so
/// every header cell shares the muted color.
pub fn header_row(layout: &[(usize, u16)], columns: &[Column], theme: &Theme) -> Row<'static> {
    let cells: Vec<Cell<'static>> =
        layout.iter().map(|&(ci, _)| Cell::from(columns[ci].header)).collect();
    Row::new(cells).style(Style::default().fg(theme.muted))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{AgentId, SessionSummary};
    use crate::enrich::{BranchEnricher, RepoEnricher};
    use crate::tui::columns::{default_columns, solve_layout};

    fn sess() -> SessionSummary {
        SessionSummary {
            id: "a".into(),
            agent: AgentId::Claude,
            title: "fix auth".into(),
            directory: "/work/api".into(),
            timestamp: 0,
            message_count: 12,
            branch: Some("feat/auth".into()),
            ..Default::default()
        }
    }

    #[test]
    fn session_row_has_one_cell_per_kept_column_with_values() {
        let cols = default_columns();
        let enr: Vec<Box<dyn Enricher>> = vec![Box::new(BranchEnricher), Box::new(RepoEnricher)];
        let resolved = HashMap::new();
        let row_data = sess();
        let theme = Theme::default();
        let ctx = RowCtx {
            enrichers: &enr,
            resolved: &resolved,
            now: 3600,
            frame: 0,
            terms: &[],
            theme: &theme,
        };
        let grid = compute_cells(&cols, std::slice::from_ref(&row_data), &ctx);
        let layout = layout_from_cells(&cols, 120, &grid);
        session_row(&row_data, &grid[0], &layout, &cols, &ctx);
        let (agent_text, _) =
            super::cell(&row_data, cols.iter().find(|c| c.id == "agent").unwrap(), &ctx);
        assert_eq!(agent_text, "CLAUDE");
    }

    #[test]
    fn header_row_constructs_for_visible_columns() {
        let cols = default_columns();
        let layout = solve_layout(&cols, 120);
        let _row = header_row(&layout, &cols, &Theme::default());
        assert_eq!(layout.len(), 8);
    }

    #[test]
    fn visible_row_content_sizes_repo_and_branch_before_title_flexes() {
        let cols = default_columns();
        let mut row = sess();
        row.directory = "/work/responsive-editor".into();
        row.branch = Some("workflow/ghostty-terminal".into());
        let enr: Vec<Box<dyn Enricher>> = vec![Box::new(BranchEnricher), Box::new(RepoEnricher)];
        let resolved = HashMap::new();
        let theme = Theme::default();
        let ctx = RowCtx {
            enrichers: &enr,
            resolved: &resolved,
            now: 0,
            frame: 0,
            terms: &[],
            theme: &theme,
        };
        let grid = compute_cells(&cols, &[row], &ctx);
        let layout = layout_from_cells(&cols, 120, &grid);
        let width = |id| layout.iter().find(|&&(i, _)| cols[i].id == id).map(|&(_, w)| w).unwrap();

        assert_eq!(width("repo"), "responsive-editor".len() as u16);
        assert_eq!(width("branch"), "workflow/ghostty-terminal".len() as u16);
        assert!(width("title") > 10);
    }

    #[test]
    fn pending_pr_cell_shows_animated_spinner_glyph() {
        let cols = default_columns();
        let enr: Vec<Box<dyn Enricher>> = vec![Box::new(crate::enrich::gh_pr::GhPrEnricher)];
        let resolved = HashMap::new();
        let pr_col = cols.iter().find(|c| c.id == "pr").unwrap();
        let theme = Theme::default();
        let mk = |frame| RowCtx {
            enrichers: &enr,
            resolved: &resolved,
            now: 0,
            frame,
            terms: &[],
            theme: &theme,
        };
        // frame=0 -> first braille frame; frame=3 -> fourth.
        let (t0, _) = super::cell(&sess(), pr_col, &mk(0));
        assert_eq!(t0, crate::tui::view::SPINNER_FRAMES[0]);
        let (t3, _) = super::cell(&sess(), pr_col, &mk(3));
        assert_eq!(t3, crate::tui::view::SPINNER_FRAMES[3]);
    }

    #[test]
    fn resolved_pr_cell_reads_resolved() {
        let cols = default_columns();
        let enr: Vec<Box<dyn Enricher>> = vec![
            Box::new(RepoEnricher),
            Box::new(BranchEnricher),
            Box::new(crate::enrich::gh_pr::GhPrEnricher),
        ];
        let mut resolved = HashMap::new();
        resolved.insert(("claude:a".to_string(), "pr"), Some("#42".to_string()));
        let pr_col = cols.iter().find(|c| c.id == "pr").unwrap();
        let theme = Theme::default();
        let ctx = RowCtx {
            enrichers: &enr,
            resolved: &resolved,
            now: 0,
            frame: 0,
            terms: &[],
            theme: &theme,
        };
        let (text, style) = super::cell(&sess(), pr_col, &ctx);
        assert_eq!(text, "#42");
        assert_eq!(style.fg, Some(Theme::default().accent));
    }

    #[test]
    fn title_line_highlights_query_terms() {
        use ratatui::style::Modifier;
        let terms = vec!["auth".to_string()];
        let line = super::title_line("fix auth bug", 40, &terms, &Theme::default(), false);
        assert!(
            line.spans.iter().any(|s| {
                s.content.contains("auth") && s.style.add_modifier.contains(Modifier::REVERSED)
            }),
            "matched term in title must be reverse-highlighted"
        );
    }

    #[test]
    fn archived_title_gets_muted_marker_prefix() {
        let theme = Theme::default();
        let line = super::title_line("fix auth", 40, &[], &theme, true);
        let first = line.spans.first().expect("title line has spans");
        assert_eq!(first.content, super::ARCHIVED_MARKER);
        assert_eq!(first.style.fg, Some(theme.muted));
        // Non-archived titles carry no marker.
        let plain = super::title_line("fix auth", 40, &[], &theme, false);
        assert_ne!(plain.spans.first().map(|s| s.content.as_ref()), Some(super::ARCHIVED_MARKER));
    }
}
