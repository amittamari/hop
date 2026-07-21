//! Renders the result list as an aligned column grid using the `columns`
//! solver, the fast enrichers, and a resolved-slow-value lookup.

use crate::core::SessionSummary;
use crate::enrich::{EnrichKind, Enricher};
use crate::tui::columns::{Column, display_width, fit, solve_layout_with_desired};
use crate::tui::glyphs::Glyphs;
use crate::tui::theme::Theme;
use crate::tui::view::rel_time;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Row};
use std::collections::HashMap;

fn cell(s: &SessionSummary, col: &Column, ctx: &RowCtx<'_>) -> (String, Style) {
    match col.id {
        "agent" => (agent_mark(s, ctx.glyphs), Style::default().fg(ctx.theme.agent_color(s.agent))),
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
                ctx.glyphs.spinner_frame(ctx.frame).to_string(),
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
    pub glyphs: &'a Glyphs,
}

/// The agent mark for a session: the agent glyph (when icons are on) followed by
/// the agent badge. In ascii mode this is just the badge, unchanged.
fn agent_mark(s: &SessionSummary, glyphs: &Glyphs) -> String {
    let glyph = glyphs.agent(s.agent);
    if glyph.is_empty() {
        s.agent.badge().to_string()
    } else {
        format!("{glyph} {}", s.agent.badge())
    }
}

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
                    ctx.glyphs,
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
/// muted marker prefixed to the title within the same cell width — the `arch `
/// text in ascii mode, or an archive icon when icons are enabled.
fn title_line(
    title: &str,
    width: u16,
    terms: &[String],
    theme: &Theme,
    glyphs: &Glyphs,
    archived: bool,
) -> Line<'static> {
    let marker = glyphs.archived_marker();
    let marker_width = if archived { display_width(marker) as u16 } else { 0 };
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
    let mut spans = vec![Span::styled(marker, Style::default().fg(theme.muted))];
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

// ---------------------------------------------------------------------------
// Card-mode rendering
// ---------------------------------------------------------------------------

/// Height of a card in lines: content (2 or 3) + 1 blank separator.
/// Stable across selection state (accent bar doesn't change height).
pub fn card_height(session: &SessionSummary, _selected: bool) -> u16 {
    let content = if session.snippet.is_some() { 3 } else { 2 };
    content + 1 // +1 blank separator
}

/// Build the lines for a single card. Returns a Vec of Lines that the caller
/// renders as a Paragraph (optionally inside a bordered Block for the selected card).
pub fn card_lines(session: &SessionSummary, width: u16, ctx: &RowCtx<'_>) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Line 1: agent mark (glyph + badge) + bold title + right-aligned time
    let time_str = rel_time(session.timestamp, ctx.now);
    let time_w = display_width(&time_str) as u16;
    let mark = agent_mark(session, ctx.glyphs);
    let mark_w = display_width(&mark) as u16;
    let gap = 2u16; // spaces between badge/title and title/time
    let title_budget = width.saturating_sub(mark_w + gap + time_w + gap);
    let title_fitted = fit(&session.title, title_budget, crate::tui::columns::Align::Left);

    let mut line1_spans = vec![
        Span::styled(mark, Style::default().fg(ctx.theme.agent_color(session.agent))),
        Span::raw("  "),
    ];
    // Highlight title terms if searching
    let title_line =
        Line::from(Span::styled(title_fitted, Style::default().add_modifier(Modifier::BOLD)));
    let highlighted = if ctx.terms.is_empty() {
        title_line
    } else {
        crate::tui::preview::highlight_terms(&title_line, ctx.terms, ctx.theme)
    };
    line1_spans.extend(highlighted.spans);
    // Right-pad to push time to the right edge
    let used: usize = line1_spans.iter().map(|s| display_width(&s.content)).sum();
    let pad = (width as usize).saturating_sub(used + display_width(&time_str));
    if pad > 0 {
        line1_spans.push(Span::raw(" ".repeat(pad)));
    }
    line1_spans.push(Span::styled(time_str, Style::default().fg(ctx.theme.muted)));
    lines.push(Line::from(line1_spans));

    // Line 2: muted dot-separated metadata, each field prefixed with its icon
    // (icons enabled) or its bare value (ascii).
    let g = ctx.glyphs;
    let mut meta_parts: Vec<String> = Vec::new();
    // repo
    if let Some(enr) = ctx.enrichers.iter().find(|e| e.id() == "repo")
        && let Some(v) = enr.resolve(session)
    {
        meta_parts.push(format!("{}{}", g.repo(), v.text));
    }
    // branch
    if let Some(b) = &session.branch {
        meta_parts.push(format!("{}{}", g.branch(), b));
    }
    // PR (slow enricher)
    if let Some(Some(pr)) = ctx.resolved.get(&(session.document_key(), "pr")) {
        meta_parts.push(format!("{}{}", g.pr(), pr));
    }
    // model (no dedicated icon; kept as plain text)
    if let Some(m) = &session.model {
        meta_parts.push(m.clone());
    }
    // message count
    if session.message_count > 0 {
        meta_parts.push(format!("{}{} msgs", g.msgs(), session.message_count));
    }
    let meta_text = meta_parts.join(g.sep());
    let meta_fitted = fit(&meta_text, width, crate::tui::columns::Align::Left);
    let meta_style = if session.archived {
        Style::default().fg(ctx.theme.muted).add_modifier(Modifier::DIM)
    } else {
        Style::default().fg(ctx.theme.muted)
    };
    lines.push(Line::from(Span::styled(meta_fitted, meta_style)));

    // Line 3: snippet (only when present)
    if let Some(snippet) = &session.snippet {
        lines.push(snippet_line(snippet, width, ctx.theme));
    }

    lines
}

/// Parse a Tantivy HTML snippet (`<b>term</b>`) into a styled Line with
/// bold+accent on matched terms and muted for context.
#[allow(unused_assignments)]
fn snippet_line(html: &str, width: u16, theme: &Theme) -> Line<'static> {
    let mut spans = Vec::new();
    let mut remaining = html;
    let muted = Style::default().fg(theme.muted);
    let highlight = Style::default().fg(theme.accent).add_modifier(Modifier::BOLD);
    let mut total_w = 0usize;
    let max_w = width as usize;

    while !remaining.is_empty() && total_w < max_w {
        if let Some(start) = remaining.find("<b>") {
            if start > 0 {
                let text = decode_html_entities(&remaining[..start]);
                let budget = max_w.saturating_sub(total_w);
                let fitted = take_up_to(&text, budget);
                total_w += display_width(&fitted);
                spans.push(Span::styled(fitted, muted));
            }
            remaining = &remaining[start + 3..];
            if let Some(end) = remaining.find("</b>") {
                let term = decode_html_entities(&remaining[..end]);
                let budget = max_w.saturating_sub(total_w);
                let fitted = take_up_to(&term, budget);
                total_w += display_width(&fitted);
                spans.push(Span::styled(fitted, highlight));
                remaining = &remaining[end + 4..];
            } else {
                let text = decode_html_entities(remaining);
                let budget = max_w.saturating_sub(total_w);
                let fitted = take_up_to(&text, budget);
                total_w += display_width(&fitted);
                spans.push(Span::styled(fitted, highlight));
                break;
            }
        } else {
            let text = decode_html_entities(remaining);
            let budget = max_w.saturating_sub(total_w);
            let fitted = take_up_to(&text, budget);
            total_w += display_width(&fitted);
            spans.push(Span::styled(fitted, muted));
            break;
        }
    }

    Line::from(spans)
}

fn take_up_to(s: &str, max_width: usize) -> String {
    crate::tui::columns::take_display_width(s, max_width)
}

fn decode_html_entities(s: &str) -> String {
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"")
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
            glyphs: &Glyphs::ascii(),
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
            glyphs: &Glyphs::ascii(),
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
        let g = Glyphs::ascii();
        let mk = |frame| RowCtx {
            enrichers: &enr,
            resolved: &resolved,
            now: 0,
            frame,
            terms: &[],
            theme: &theme,
            glyphs: &g,
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
            glyphs: &Glyphs::ascii(),
        };
        let (text, style) = super::cell(&sess(), pr_col, &ctx);
        assert_eq!(text, "#42");
        assert_eq!(style.fg, Some(Theme::default().accent));
    }

    #[test]
    fn title_line_highlights_query_terms() {
        use ratatui::style::Modifier;
        let terms = vec!["auth".to_string()];
        let line = super::title_line(
            "fix auth bug",
            40,
            &terms,
            &Theme::default(),
            &Glyphs::ascii(),
            false,
        );
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
        let g = Glyphs::ascii();
        let marker = g.archived_marker();
        let line = super::title_line("fix auth", 40, &[], &theme, &g, true);
        let first = line.spans.first().expect("title line has spans");
        assert_eq!(first.content, marker);
        assert_eq!(first.style.fg, Some(theme.muted));
        // Non-archived titles carry no marker.
        let plain = super::title_line("fix auth", 40, &[], &theme, &g, false);
        assert_ne!(plain.spans.first().map(|s| s.content.as_ref()), Some(marker));
    }

    fn card_sess() -> SessionSummary {
        SessionSummary {
            id: "a".into(),
            agent: AgentId::Claude,
            title: "fix auth bug".into(),
            directory: "/work/api".into(),
            timestamp: 0,
            message_count: 12,
            branch: Some("feat/auth".into()),
            ..Default::default()
        }
    }

    #[test]
    fn card_height_without_snippet() {
        let s = card_sess();
        assert!(s.snippet.is_none());
        assert_eq!(super::card_height(&s, false), 3); // 2 content + 1 separator
    }

    #[test]
    fn card_height_with_snippet() {
        let mut s = card_sess();
        s.snippet = Some("the <b>auth</b> token expired".into());
        assert_eq!(super::card_height(&s, false), 4); // 3 content + 1 separator
    }

    #[test]
    fn card_height_is_stable_across_selection() {
        let s = card_sess();
        assert_eq!(super::card_height(&s, false), super::card_height(&s, true));
    }

    #[test]
    fn card_lines_without_snippet_has_two_lines() {
        let theme = Theme::default();
        let enr: Vec<Box<dyn Enricher>> = vec![Box::new(RepoEnricher), Box::new(BranchEnricher)];
        let resolved = HashMap::new();
        let ctx = RowCtx {
            enrichers: &enr,
            resolved: &resolved,
            now: 3600,
            frame: 0,
            terms: &[],
            theme: &theme,
            glyphs: &Glyphs::ascii(),
        };
        let s = card_sess();
        let lines = super::card_lines(&s, 80, &ctx);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn card_lines_with_snippet_has_three_lines() {
        let theme = Theme::default();
        let enr: Vec<Box<dyn Enricher>> = vec![];
        let resolved = HashMap::new();
        let ctx = RowCtx {
            enrichers: &enr,
            resolved: &resolved,
            now: 3600,
            frame: 0,
            terms: &[],
            theme: &theme,
            glyphs: &Glyphs::ascii(),
        };
        let mut s = card_sess();
        s.snippet = Some("the <b>auth</b> token expired".into());
        let lines = super::card_lines(&s, 80, &ctx);
        assert_eq!(lines.len(), 3);
        // The snippet line should contain the matched term
        let snippet_text: String = lines[2].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(snippet_text.contains("auth"), "snippet should contain matched term");
    }

    fn line_text(lines: &[Line<'static>], i: usize) -> String {
        lines[i].spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn has_pua(s: &str) -> bool {
        s.chars().any(|c| ('\u{e000}'..='\u{f8ff}').contains(&c))
    }

    #[test]
    fn card_lines_icons_disabled_match_pre_change_text() {
        let theme = Theme::default();
        let g = Glyphs::ascii();
        let enr: Vec<Box<dyn Enricher>> = vec![Box::new(RepoEnricher), Box::new(BranchEnricher)];
        let resolved = HashMap::new();
        let ctx = RowCtx {
            enrichers: &enr,
            resolved: &resolved,
            now: 3600,
            frame: 0,
            terms: &[],
            theme: &theme,
            glyphs: &g,
        };
        let s = card_sess();
        let lines = super::card_lines(&s, 80, &ctx);
        let l1 = line_text(&lines, 0);
        let l2 = line_text(&lines, 1);
        // Agent mark is the bare badge; metadata joins with the middot separator.
        assert!(l1.starts_with("CLAUDE"), "ascii agent mark is the badge: {l1:?}");
        assert!(l2.contains(" · "), "ascii metadata keeps the middot separator: {l2:?}");
        assert!(l2.contains("feat/auth"), "branch value present: {l2:?}");
        assert!(l2.contains("12 msgs"), "message count present: {l2:?}");
        // No tofu: nothing in the chrome uses a Private Use Area code point.
        assert!(!has_pua(&l1) && !has_pua(&l2), "ascii mode must not emit PUA glyphs");
    }

    #[test]
    fn card_lines_icons_enabled_carry_glyphs() {
        let theme = Theme::default();
        let mut g = Glyphs::nerd();
        g.set_agent_glyph(AgentId::Claude, "\u{f069}");
        let enr: Vec<Box<dyn Enricher>> = vec![Box::new(RepoEnricher), Box::new(BranchEnricher)];
        let resolved = HashMap::new();
        let ctx = RowCtx {
            enrichers: &enr,
            resolved: &resolved,
            now: 3600,
            frame: 0,
            terms: &[],
            theme: &theme,
            glyphs: &g,
        };
        let s = card_sess();
        let lines = super::card_lines(&s, 120, &ctx);
        let l1 = line_text(&lines, 0);
        let l2 = line_text(&lines, 1);
        // Agent mark carries the adapter glyph ahead of the badge.
        assert!(l1.contains("\u{f069}"), "agent glyph should prefix the mark: {l1:?}");
        assert!(l1.contains("CLAUDE"), "text label still present: {l1:?}");
        // Metadata fields carry their icons (branch fork, msgs comments).
        assert!(l2.contains("\u{f126}"), "branch icon should prefix the branch: {l2:?}");
        assert!(l2.contains("\u{f086}"), "msgs icon should prefix the count: {l2:?}");
        assert!(l2.contains("feat/auth"), "branch value still present: {l2:?}");
    }

    #[test]
    fn snippet_line_highlights_bold_tags() {
        let theme = Theme::default();
        let line = super::snippet_line("before <b>match</b> after", 80, &theme);
        let bold = line.spans.iter().any(|s| {
            s.content.contains("match")
                && s.style.add_modifier.contains(Modifier::BOLD)
                && s.style.fg == Some(theme.accent)
        });
        assert!(bold, "matched term should be bold+accent");
    }
}
