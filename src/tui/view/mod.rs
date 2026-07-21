use crate::config::RowStyle;
use crate::enrich::Enricher;
use crate::tui::columns::Column;
use crate::tui::modal;
use crate::tui::theme::Theme;
use crate::tui::{App, help, results_list};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Position, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Row, Table, TableState, Wrap};
use std::collections::HashMap;
use std::ops::Range;

mod cards;
mod footer;
mod preview_header;

/// Relative-time label from a unix-seconds timestamp.
pub fn rel_time(ts: i64, now: i64) -> String {
    let s = (now - ts).max(0);
    if s >= 86_400 {
        format!("{}d", s / 86_400)
    } else if s >= 3_600 {
        format!("{}h", s / 3_600)
    } else if s >= 60 {
        format!("{}m", s / 60)
    } else {
        format!("{s}s")
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StatusLine {
    pub sync: Option<String>,
    pub pr_pending: usize,
    pub warning: Option<String>,
}

pub struct RenderModel<'a> {
    pub now: i64,
    pub columns: &'a [Column],
    pub enrichers: &'a [Box<dyn Enricher>],
    pub resolved: &'a HashMap<(String, &'static str), Option<String>>,
    pub query_terms: &'a [String],
    pub preview_lines: &'a [Line<'static>],
    pub status: &'a StatusLine,
    pub modal_command: Option<&'a [String]>,
    pub theme: Theme,
    pub row_style: RowStyle,
}

/// Braille throbber frames. The vocabulary now lives in the `glyphs` module
/// (centralized glyph ownership); re-exported here so existing call sites and
/// tests keep referring to `view::SPINNER_FRAMES`.
pub(crate) use crate::tui::glyphs::SPINNER_FRAMES;

/// The current throbber glyph for a given frame counter.
fn spinner_frame(frame: u64) -> &'static str {
    SPINNER_FRAMES[(frame as usize) % SPINNER_FRAMES.len()]
}

pub fn render(f: &mut Frame, app: &App, model: RenderModel<'_>) {
    let area = f.area();
    if area.width < 30 || area.height < 6 {
        let msg = Paragraph::new("terminal too small")
            .alignment(Alignment::Center)
            .style(Style::default().fg(model.theme.muted));
        f.render_widget(msg, area);
        return;
    }

    let [header_area, body_area, footer_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
            .areas(area);

    // Build the toolbar line first so we can measure it and split the header.
    let toolbar_line = if app.search_mode() == crate::tui::SearchMode::Simple {
        crate::tui::toolbar::line(
            app.scope(),
            app.sort(),
            app.toolbar_focus(),
            app.has_repo(),
            &model.theme,
        )
    } else {
        Line::default()
    };
    let toolbar_w = line_display_width(&toolbar_line);
    let [query_area, toolbar_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(toolbar_w)]).areas(header_area);

    // search input
    let total = app.results().len();
    let pos = if total == 0 { 0 } else { app.selected() + 1 };

    let mut header = Line::from(vec![
        Span::styled(" ❯ ", Style::default().fg(model.theme.accent)),
        Span::styled(app.query().to_string(), Style::default().fg(model.theme.selection_fg)),
        Span::raw(format!("   {}/{}", pos, total)).fg(model.theme.muted),
    ]);
    if let Some(count) = app.indexing() {
        header.spans.push(Span::styled(
            format!("   {} indexing {count}…", spinner_frame(app.frame())),
            Style::default().fg(model.theme.muted),
        ));
    }
    f.render_widget(Paragraph::new(header), query_area);
    if toolbar_w > 0 {
        f.render_widget(Paragraph::new(toolbar_line), toolbar_area);
    }
    if !app.help_open() && !app.modal_open() {
        let query_prefix = app.query().get(..app.query_cursor()).unwrap_or(app.query());
        let x = query_area
            .x
            .saturating_add(crate::tui::columns::display_width(" ❯ ") as u16)
            .saturating_add(crate::tui::columns::display_width(query_prefix) as u16);
        let x = x.min(query_area.right().saturating_sub(1));
        f.set_cursor_position(Position::new(x, query_area.y));
    }

    // body: list (| preview). The preview only appears when both requested AND
    // there is room for it without starving the list grid. Below the width
    // threshold the list takes the whole body. When shown, the list side is
    // floored at Min(48) so its columns never collapse.
    const PREVIEW_MIN_WIDTH: u16 = 100;
    const LIST_MIN_WIDTH: u16 = 48;
    let (list_area, preview_area) = if app.preview_visible() && body_area.width >= PREVIEW_MIN_WIDTH
    {
        let pw = app.preview_width_pct();
        let [list, preview] =
            Layout::horizontal([Constraint::Min(LIST_MIN_WIDTH), Constraint::Percentage(pw)])
                .areas(body_area);
        (list, Some(preview))
    } else {
        (body_area, None)
    };

    // results list: card mode or compact (table) mode
    let cols = model.columns;
    if app.results().is_empty() {
        let msg = empty_state_message(app.query().is_empty());
        let para = Paragraph::new(msg)
            .style(Style::default().fg(model.theme.muted))
            .alignment(Alignment::Center);
        let y = list_area.y.saturating_add(list_area.height / 2);
        let centered =
            Rect { x: list_area.x, y, width: list_area.width, height: 1.min(list_area.height) };
        f.render_widget(para, centered);
    } else if model.row_style == RowStyle::Card {
        let ctx = results_list::RowCtx {
            enrichers: model.enrichers,
            resolved: model.resolved,
            now: model.now,
            frame: app.frame(),
            terms: model.query_terms,
            theme: &model.theme,
            glyphs: app.glyphs(),
        };
        let visible =
            cards::card_visible_range(app.results(), app.selected(), list_area.height as usize);
        let visible_start = visible.start;
        let visible_results = app.results().get(visible).unwrap_or(&[]);
        let sel_in_view = app.selected().saturating_sub(visible_start);

        let mut y = list_area.y;
        for (vi, session) in visible_results.iter().enumerate() {
            let is_selected = vi == sel_in_view;
            let h = results_list::card_height(session, is_selected);
            if y + h > list_area.y + list_area.height {
                break;
            }
            let card_area = Rect { x: list_area.x, y, width: list_area.width, height: h };
            cards::render_card(f, session, card_area, is_selected, &ctx);
            y += h;
        }
    } else {
        // Compact (legacy table) mode
        let selection_marker = app.glyphs().selection_marker();
        let marker_w = crate::tui::columns::display_width(selection_marker) as u16;
        let list_inner_w = list_area.width.saturating_sub(marker_w);
        let visible = visible_result_range(
            app.results().len(),
            app.selected(),
            list_area.height.saturating_sub(1) as usize,
        );
        let visible_start = visible.start;
        let visible_results = app.results().get(visible).unwrap_or(&[]);
        let ctx = results_list::RowCtx {
            enrichers: model.enrichers,
            resolved: model.resolved,
            now: model.now,
            frame: app.frame(),
            terms: model.query_terms,
            theme: &model.theme,
            glyphs: app.glyphs(),
        };
        let grid = results_list::compute_cells(cols, visible_results, &ctx);
        let layout = results_list::layout_from_cells(cols, list_inner_w, &grid);
        let rows: Vec<Row> = visible_results
            .iter()
            .zip(&grid)
            .map(|(s, row_cells)| results_list::session_row(s, row_cells, &layout, cols, &ctx))
            .collect();
        let mut state = TableState::default();
        state.select(Some(app.selected().saturating_sub(visible_start)));
        let col_widths: Vec<Constraint> =
            layout.iter().map(|&(_, w)| Constraint::Length(w)).collect();
        let table = Table::new(rows, col_widths)
            .header(results_list::header_row(&layout, cols, &model.theme))
            .column_spacing(1)
            .row_highlight_style(
                Style::default()
                    .fg(model.theme.selection_fg)
                    .bg(model.theme.selection_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(selection_marker);
        f.render_stateful_widget(table, list_area, &mut state);
    }

    // preview (lines are pre-rendered/memoized by the caller per selection+query)
    if let Some(area) = preview_area {
        let preview_block = Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(model.theme.border))
            .padding(Padding::left(1));
        let inner = preview_block.inner(area);
        f.render_widget(preview_block, area);

        let selected = app.results().get(app.selected());
        let show_header = app.preview_header_visible() && model.row_style != RowStyle::Card;
        let (preview_header_area, transcript_area) =
            if show_header && selected.is_some() && inner.height >= 5 {
                let [head, body] =
                    Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(inner);
                (Some(head), body)
            } else {
                (None, inner)
            };
        if let (Some(preview_header_area), Some(session)) = (preview_header_area, selected) {
            f.render_widget(
                Paragraph::new(preview_header::preview_header_lines(
                    session,
                    model.now,
                    model.resolved,
                    &model.theme,
                    app.glyphs(),
                    preview_header_area.width,
                ))
                .style(Style::default().fg(model.theme.preview_text)),
                preview_header_area,
            );
        }
        f.render_widget(
            Paragraph::new(model.preview_lines.to_vec())
                .style(Style::default().fg(model.theme.preview_text))
                .wrap(Wrap { trim: false })
                .scroll((app.preview_scroll(), 0)),
            transcript_area,
        );
    }

    // footer: static hints on the left, volatile status on the right. The two
    // halves share the footer row via SpaceBetween so right-aligned status
    // (sync/pr/warning) survives clipping ahead of the static hints.
    // Build the status line once and size its region from it, rather than
    // building it a second time just to measure the width.
    let status_line = footer::footer_status_line(model.status, &model.theme, app.glyphs());
    let [hints_area, status_area] = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(line_display_width(&status_line)),
    ])
    .flex(Flex::SpaceBetween)
    .areas(footer_area);
    f.render_widget(
        Paragraph::new(footer::footer_hints_line(
            app.keymap(),
            app.search_mode(),
            &model.theme,
            app.glyphs(),
        )),
        hints_area,
    );
    f.render_widget(Paragraph::new(status_line).alignment(Alignment::Right), status_area);

    if let Some((index, yolo)) = app.yolo_modal() {
        let session = app.results().get(index);
        modal::render_yolo_modal(f, session, yolo, model.modal_command, &model.theme, app.glyphs());
    }

    // help overlay (drawn last, on top)
    if app.help_open() {
        help::render(f, app.keymap(), app.search_mode(), &model.theme);
    }
}

/// Display width of a rendered line, used to size the right footer region so
/// the status is never clipped.
fn line_display_width(line: &Line) -> u16 {
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    crate::tui::columns::display_width(&text).min(u16::MAX as usize) as u16
}

/// Message shown in the body area when there are no results.
fn empty_state_message(query_is_empty: bool) -> &'static str {
    if query_is_empty {
        "Type to search your Claude Code / Codex sessions."
    } else {
        "No sessions match. Press Esc to clear the query."
    }
}

pub fn visible_result_range(total: usize, selected: usize, height: usize) -> Range<usize> {
    if total == 0 || height == 0 {
        return 0..0;
    }
    let len = height.min(total);
    let max_start = total - len;
    let start = selected.saturating_add(1).saturating_sub(len).min(max_start);
    start..start + len
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests_footer;
#[cfg(test)]
mod tests_layout;
#[cfg(test)]
mod tests_list;
#[cfg(test)]
mod tests_modal;
#[cfg(test)]
mod tests_preview;
