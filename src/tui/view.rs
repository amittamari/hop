use crate::tui::columns::Column;
use crate::core::SessionSummary;
use crate::enrich::{BranchEnricher, Enricher};
use crate::tui::theme::Theme;
use crate::tui::{help, results_list, App};
use crate::tui::modal;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Position, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;
use std::collections::HashMap;
use std::ops::Range;

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
    pub filters: Option<String>,
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
}

const SELECTION_MARKER: &str = "❯ ";

/// Braille throbber frames, indexed by the per-redraw frame counter. Hand-rolled
/// to avoid a spinner crate; advances one frame per redraw (the run loop polls
/// every 50ms, so it animates smoothly).
pub(crate) const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// The current throbber glyph for a given frame counter.
fn spinner_frame(frame: u64) -> &'static str {
    SPINNER_FRAMES[(frame as usize) % SPINNER_FRAMES.len()]
}

/// Public throbber glyph for callers outside this module (e.g. the pending
/// enricher cell), reusing the same frame table.
pub(crate) fn spinner_glyph(frame: u64) -> &'static str {
    spinner_frame(frame)
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

    let [header_area, body_area, footer_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);

    // search input
    let total = app.results().len();
    let pos = if total == 0 { 0 } else { app.selected() + 1 };

    // The query input is always live, so the prompt and query stay bright and
    // the caret is shown whenever no overlay is covering the input.
    let mut header = Line::from(vec![
        Span::styled(" ❯ ", Style::default().fg(model.theme.accent)),
        Span::styled(
            app.query().to_string(),
            Style::default().fg(model.theme.selection_fg),
        ),
        Span::raw(format!("   {}/{}", pos, total)).fg(model.theme.muted),
    ]);
    if let Some(count) = app.indexing() {
        header.spans.push(Span::styled(
            format!("   {} indexing {count}…", spinner_frame(app.frame())),
            Style::default().fg(model.theme.muted),
        ));
    }
    f.render_widget(Paragraph::new(header), header_area);
    if !app.help_open() && !app.modal_open() {
        let query_prefix = app.query().get(..app.query_cursor()).unwrap_or(app.query());
        let x = header_area
            .x
            .saturating_add(crate::tui::columns::display_width(" ❯ ") as u16)
            .saturating_add(crate::tui::columns::display_width(query_prefix) as u16);
        let x = x.min(header_area.right().saturating_sub(1));
        f.set_cursor_position(Position::new(x, header_area.y));
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

    // results table (Table owns its header; no separate header pane)
    let cols = model.columns;
    let marker_w = crate::tui::columns::display_width(SELECTION_MARKER) as u16;
    let list_inner_w = list_area.width.saturating_sub(marker_w);
    let visible = visible_result_range(
        app.results().len(),
        app.selected(),
        list_area.height.saturating_sub(1) as usize,
    );
    let visible_start = visible.start;
    let visible_results = app.results().get(visible).unwrap_or(&[]);
    if visible_results.is_empty() {
        let msg = empty_state_message(app.query().is_empty());
        let para = Paragraph::new(msg)
            .style(Style::default().fg(model.theme.muted))
            .alignment(Alignment::Center);
        // Vertically center the single line within the list area.
        let y = list_area.y.saturating_add(list_area.height / 2);
        let centered = Rect {
            x: list_area.x,
            y,
            width: list_area.width,
            height: 1.min(list_area.height),
        };
        f.render_widget(para, centered);
    } else {
        let layout = results_list::layout_for_rows(
            cols,
            list_inner_w,
            visible_results,
            model.enrichers,
            model.resolved,
            model.now,
            app.frame(),
        );
        let ctx = results_list::RowCtx {
            enrichers: model.enrichers,
            resolved: model.resolved,
            now: model.now,
            frame: app.frame(),
            terms: model.query_terms,
            theme: &model.theme,
        };
        let rows: Vec<Row> = visible_results
            .iter()
            .map(|s| results_list::session_row(s, &layout, cols, &ctx))
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
            .highlight_symbol(SELECTION_MARKER);
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
        let (preview_header_area, transcript_area) =
            if app.preview_header_visible() && selected.is_some() && inner.height >= 5 {
                let [head, body] =
                    Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(inner);
                (Some(head), body)
            } else {
                (None, inner)
            };
        if let (Some(preview_header_area), Some(session)) = (preview_header_area, selected) {
            f.render_widget(
                Paragraph::new(preview_header_lines(
                    session,
                    model.now,
                    model.resolved,
                    &model.theme,
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
    // (sync/pr/filters/warning) survives clipping ahead of the static hints.
    let [hints_area, status_area] = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(footer_status_width(model.status, &model.theme)),
    ])
    .flex(Flex::SpaceBetween)
    .areas(footer_area);
    f.render_widget(
        Paragraph::new(footer_hints_line(app.keymap(), &model.theme)),
        hints_area,
    );
    f.render_widget(
        Paragraph::new(footer_status_line(model.status, &model.theme)).alignment(Alignment::Right),
        status_area,
    );

    if let Some((index, yolo)) = app.yolo_modal() {
        let session = app.results().get(index);
        modal::render_yolo_modal(f, session, yolo, model.modal_command, &model.theme);
    }

    // help overlay (drawn last, on top)
    if app.help_open() {
        help::render(f, app.keymap(), &model.theme);
    }
}

/// Static, low-priority hints shown on the left of the footer, built from the
/// `primary` subset of the canonical bindings table. Dropped first (clipped by
/// the SpaceBetween layout) when the terminal is too narrow for both halves.
fn footer_hints_line(keymap: &crate::tui::keymap::Keymap, theme: &Theme) -> Line<'static> {
    let primary: Vec<String> = crate::tui::keymap::bindings(keymap)
        .iter()
        .filter(|b| b.primary)
        .map(|b| {
            if b.keys == "type" {
                format!("type to {}", b.label)
            } else {
                format!("{} {}", b.keys, b.label)
            }
        })
        .collect();

    let mut spans = Vec::new();
    for (i, hint) in primary.iter().enumerate() {
        if i == 0 {
            spans.push(Span::styled(
                hint.clone(),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!(" · {hint}"),
                Style::default().fg(theme.muted),
            ));
        }
    }
    Line::from(spans)
}

/// Volatile, high-priority status shown on the right of the footer. Rendered
/// right-aligned so it survives clipping ahead of the static hints.
fn footer_status_line(status: &StatusLine, theme: &Theme) -> Line<'static> {
    let mut spans = Vec::new();
    let push_sep = |spans: &mut Vec<Span<'static>>| {
        if !spans.is_empty() {
            spans.push(Span::styled(
                " · ".to_string(),
                Style::default().fg(theme.muted),
            ));
        }
    };
    if let Some(sync) = status.sync.as_deref().filter(|s| !s.is_empty()) {
        push_sep(&mut spans);
        spans.push(Span::styled(
            sync.to_string(),
            Style::default().fg(theme.muted),
        ));
    }
    if status.pr_pending > 0 {
        push_sep(&mut spans);
        spans.push(Span::styled(
            format!("pr {} pending", status.pr_pending),
            Style::default().fg(theme.muted),
        ));
    }
    if let Some(filters) = status.filters.as_deref().filter(|s| !s.is_empty()) {
        push_sep(&mut spans);
        spans.push(Span::styled(
            format!("filters {filters}"),
            Style::default().fg(theme.muted),
        ));
    }
    if let Some(warning) = status.warning.as_deref().filter(|s| !s.is_empty()) {
        push_sep(&mut spans);
        spans.push(Span::styled(
            warning.to_string(),
            Style::default().fg(theme.warning),
        ));
    }
    Line::from(spans)
}

/// Display width of the rendered status line, used to size the right footer
/// region so the status is never clipped.
fn footer_status_width(status: &StatusLine, theme: &Theme) -> u16 {
    let text: String = footer_status_line(status, theme)
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect();
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

fn preview_header_lines(
    s: &SessionSummary,
    now: i64,
    resolved: &HashMap<(String, &'static str), Option<String>>,
    theme: &Theme,
    width: u16,
) -> Vec<Line<'static>> {
    let w = width as usize;
    let title_line = Line::from(Span::styled(
        modal::fit_for_modal(&s.title, w),
        Style::default().add_modifier(Modifier::BOLD),
    ));

    let sep_style = Style::default().fg(theme.border);
    let muted = Style::default().fg(theme.muted);
    const SEP: &str = " · ";
    let sep_w = crate::tui::columns::display_width(SEP);

    let badge = s.agent.badge();
    let branch = BranchEnricher.resolve(s).map(|v| v.text);
    let pr = resolved
        .get(&(s.document_key(), "pr"))
        .and_then(|v| v.as_deref());
    let msgs = if s.message_count > 0 {
        Some(format!("{} msgs", s.message_count))
    } else {
        None
    };
    let time = rel_time(s.timestamp, now);

    let dw = crate::tui::columns::display_width;
    let fixed_w = dw(badge)
        + sep_w // separator after badge
        + branch.as_ref().map_or(0, |_| sep_w)
        + pr.map_or(0, |p| sep_w + dw(p))
        + msgs.as_ref().map_or(0, |m| sep_w + dw(m))
        + sep_w
        + dw(&time);

    let variable_budget = w.saturating_sub(fixed_w);
    let dir_raw_w = dw(&s.directory);
    let branch_raw_w = branch.as_ref().map_or(0, |b| dw(b));

    let (dir_budget, branch_budget) = if branch.is_some() && variable_budget > 0 {
        let total_raw = dir_raw_w + branch_raw_w;
        if total_raw <= variable_budget {
            (dir_raw_w, branch_raw_w)
        } else {
            let dir_share = variable_budget * 3 / 5;
            let branch_share = variable_budget - dir_share;
            (dir_share.min(dir_raw_w), branch_share.min(branch_raw_w))
        }
    } else {
        (variable_budget.min(dir_raw_w), 0)
    };

    let dir_text = crate::tui::columns::fit_end(&s.directory, dir_budget as u16);
    let branch_text = branch
        .as_ref()
        .map(|b| modal::fit_for_modal(b, branch_budget));

    let push_sep = |spans: &mut Vec<Span<'static>>| {
        spans.push(Span::styled(SEP, sep_style));
    };
    let mut meta: Vec<Span<'static>> = Vec::new();

    meta.push(Span::styled(
        badge,
        Style::default()
            .fg(theme.agent_color(s.agent))
            .add_modifier(Modifier::BOLD),
    ));

    if !dir_text.is_empty() {
        push_sep(&mut meta);
        meta.push(Span::styled(dir_text, muted));
    }

    if let Some(branch_text) = branch_text.filter(|t| !t.is_empty()) {
        push_sep(&mut meta);
        meta.push(Span::styled(branch_text, muted));
    }

    if let Some(pr) = pr {
        push_sep(&mut meta);
        meta.push(Span::styled(
            pr.to_string(),
            Style::default().fg(theme.accent),
        ));
    }

    if let Some(msgs) = msgs {
        push_sep(&mut meta);
        meta.push(Span::styled(msgs, muted));
    }

    push_sep(&mut meta);
    meta.push(Span::styled(time, muted));

    let meta_line = Line::from(meta);
    let rule_line = Line::from(Span::styled(
        "─".repeat(w.max(1)),
        sep_style,
    ));

    vec![title_line, meta_line, rule_line]
}

pub fn visible_result_range(total: usize, selected: usize, height: usize) -> Range<usize> {
    if total == 0 || height == 0 {
        return 0..0;
    }
    let len = height.min(total);
    let max_start = total - len;
    let start = selected
        .saturating_add(1)
        .saturating_sub(len)
        .min(max_start);
    start..start + len
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{AgentId, SessionSummary};
    use crate::tui::App;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

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
            yolo: false,
            branch: Some("feat/auth".into()),
            repo_url: None,
            source_path: None,
            archived: false,
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
                },
            )
        })
        .unwrap();
        let text: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
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
            yolo: false,
            branch: None,
            repo_url: None,
            source_path: None,
            archived: true,
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
                },
            )
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        let text: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            text.contains("arch fix auth"),
            "archived marker prefixes title"
        );
        // The title cell on the archived row must carry the DIM modifier.
        let dimmed = buf
            .content()
            .iter()
            .any(|c| c.symbol().contains('f') && c.modifier.contains(Modifier::DIM));
        assert!(dimmed, "archived row cells must be dimmed");
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
    fn renders_columns_and_preview() {
        use crate::core::{Block, Message, Role};
        use crate::enrich::{BranchEnricher, Enricher, RepoEnricher};
        use std::collections::HashMap;

        let mut app = App::new();
        app.set_results(vec![SessionSummary {
            id: "a".into(),
            agent: AgentId::Claude,
            title: "fix auth".into(),
            directory: "/work/api".into(),
            timestamp: 0,
            message_count: 3,
            yolo: false,
            branch: Some("feat/auth".into()),
            repo_url: None,
            source_path: None,
            archived: false,
        }]);
        let enr: Vec<Box<dyn Enricher>> = vec![Box::new(RepoEnricher), Box::new(BranchEnricher)];
        let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
        let transcript = vec![Message {
            role: Role::User,
            blocks: vec![Block::Prose("fix auth".into())],
        }];

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
    fn renders_yolo_dialog_and_status_footer() {
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
            yolo: false,
            branch: Some("feat/auth".into()),
            repo_url: None,
            source_path: None,
            archived: false,
        }]);
        app.open_yolo_modal_with(true);

        let enr: Vec<Box<dyn Enricher>> = vec![];
        let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
        let cols = crate::tui::columns::default_columns();
        let status = StatusLine {
            sync: Some("sync complete; parse errors 2".to_string()),
            pr_pending: 1,
            warning: Some("source unavailable".to_string()),
            filters: Some("agent:claude".to_string()),
        };
        let command = vec![
            "claude".to_string(),
            "--dangerously-skip-permissions".to_string(),
            "--resume".to_string(),
            "a".to_string(),
        ];

        let backend = TestBackend::new(180, 16);
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
                    modal_command: Some(&command),
                    theme: Theme::default(),
                },
            )
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        let text: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(text.contains("confirm resume"));
        assert!(text.contains("YOLO on"));
        assert!(text.contains("Session"));
        assert!(text.contains("fix auth"));
        assert!(text.contains("Directory"));
        assert!(text.contains("/work/api"));
        assert!(text.contains("Command"));
        assert!(text.contains("claude"));
        assert!(text.contains("parse errors 2"));
        assert!(text.contains("pr 1 pending"));
        assert!(text.contains("filters agent:claude"));
        assert!(text.contains("source unavailable"));
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
            yolo: false,
            branch: Some("feat/auth".into()),
            repo_url: None,
            source_path: None,
            archived: false,
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
                },
            )
        })
        .unwrap();

        let buf = term.backend().buffer();
        let marker = SELECTION_MARKER.trim();
        let has_marker = buf.content().iter().any(|c| c.symbol() == marker);
        assert!(has_marker, "selection marker should be rendered");
        let has_sel_bg = buf
            .content()
            .iter()
            .any(|c| c.bg == crate::tui::theme::Theme::default().selection_bg);
        assert!(
            has_sel_bg,
            "selected row should carry the selection background"
        );
        let has_sel_fg = buf.content().iter().any(|c| {
            c.fg == crate::tui::theme::Theme::default().selection_fg
                && c.bg == crate::tui::theme::Theme::default().selection_bg
        });
        assert!(
            has_sel_fg,
            "selected row text should use the selection fg over selection bg"
        );
    }

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
                },
            )
        })
        .unwrap();
        let text: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("type to search"));
        // Esc hint derives from the bindings table label ("clear query / quit");
        // assert on a stable substring rather than exact spacing.
        assert!(text.contains("clear"));
        // No mode indicators remain.
        assert!(!text.contains("NAV"));
    }

    fn footer_text(width: u16, preview: bool) -> String {
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
                },
            )
        })
        .unwrap();
        term.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
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
            yolo: false,
            branch: Some("feat/auth".into()),
            repo_url: None,
            source_path: None,
            archived: false,
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
            yolo: false,
            branch: Some("feat/auth".into()),
            repo_url: None,
            source_path: None,
            archived: false,
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
                },
            )
        })
        .unwrap();
        let text: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(
            text.contains("TITLE"),
            "TITLE header must survive narrow width"
        );
        assert!(text.contains("fix auth"), "title value must survive");
        assert!(
            !text.contains("PR"),
            "lowest-priority PR column should be dropped"
        );
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
            yolo: false,
            branch: None,
            repo_url: None,
            source_path: None,
            archived: false,
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
                },
            )
        })
        .unwrap();
        let buf = term.backend().buffer();
        let any_reversed = buf
            .content()
            .iter()
            .any(|c| c.modifier.contains(Modifier::REVERSED));
        assert!(
            any_reversed,
            "matched query term in title should render reversed"
        );
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
    fn yolo_banner_uses_warning_color_not_accent() {
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
            yolo: false,
            branch: None,
            repo_url: None,
            source_path: None,
            archived: false,
        }]);
        app.open_yolo_modal_with(true);

        let enr: Vec<Box<dyn Enricher>> = vec![];
        let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
        let cols = crate::tui::columns::default_columns();
        let backend = TestBackend::new(120, 16);
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
                },
            )
        })
        .unwrap();

        let buf = term.backend().buffer().clone();
        let warning = crate::tui::theme::Theme::default().warning;
        let accent = crate::tui::theme::Theme::default().accent;
        let (w, h) = (buf.area.width, buf.area.height);
        let mut found = false;
        for y in 0..h {
            for x in 0..w {
                let cell = &buf[(x, y)];
                if cell.symbol() == "Y" {
                    if cell.fg == warning {
                        found = true;
                    }
                    assert_ne!(cell.fg, accent, "YOLO banner must not use accent");
                }
            }
        }
        assert!(found, "expected a 'Y' cell painted with the warning color");
    }

    #[test]
    fn yolo_backdrop_dims_background() {
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
            yolo: false,
            branch: None,
            repo_url: None,
            source_path: None,
            archived: false,
        }]);
        app.open_yolo_modal_with(true);

        let enr: Vec<Box<dyn Enricher>> = vec![];
        let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
        let cols = crate::tui::columns::default_columns();
        let backend = TestBackend::new(120, 16);
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
                },
            )
        })
        .unwrap();

        let buf = term.backend().buffer().clone();
        let overlay_bg = crate::tui::theme::Theme::default().overlay_bg;
        assert_eq!(
            buf[(0, 0)].bg,
            overlay_bg,
            "backdrop must set bg, not fg-only"
        );
    }

    #[test]
    fn confirm_modal_for_archived_session_explains_unarchive() {
        use crate::enrich::Enricher;
        use std::collections::HashMap;

        let mut app = App::new();
        app.set_results(vec![SessionSummary {
            id: "a".into(),
            agent: AgentId::Codex,
            title: "fix auth".into(),
            directory: "/work/api".into(),
            timestamp: 0,
            message_count: 3,
            yolo: false,
            branch: None,
            repo_url: None,
            source_path: None,
            archived: true,
        }]);
        app.open_yolo_modal_with(false);

        let enr: Vec<Box<dyn Enricher>> = vec![];
        let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
        let cols = crate::tui::columns::default_columns();
        let backend = TestBackend::new(120, 18);
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
                },
            )
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        let text: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            text.contains("unarchive & resume"),
            "archived modal title should mention unarchive"
        );
        assert!(
            text.contains("it will be unarchived first"),
            "archived modal should explain the unarchive step"
        );
        assert!(
            text.contains("unarchives & resumes"),
            "archived modal legend should reflect the unarchive step"
        );
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
            yolo: false,
            branch: Some("feat/auth".into()),
            repo_url: None,
            source_path: None,
            archived: false,
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
                },
            )
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        let text: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            text.contains("too small"),
            "expected too-small notice, got: {text:?}"
        );
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
            yolo: false,
            branch: Some("feat/auth".into()),
            repo_url: None,
            source_path: None,
            archived: false,
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
            yolo: false,
            branch: Some("feat/auth".into()),
            repo_url: None,
            source_path: None,
            archived: false,
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

    #[test]
    fn footer_warning_survives_narrow_width() {
        use crate::enrich::Enricher;
        use std::collections::HashMap;

        let app = App::new();
        let enr: Vec<Box<dyn Enricher>> = vec![];
        let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
        let cols = crate::tui::columns::default_columns();
        let status = StatusLine {
            sync: None,
            pr_pending: 0,
            warning: Some("WARNTOKEN".to_string()),
            filters: None,
        };

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
                },
            )
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        let text: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            text.contains("WARNTOKEN"),
            "warning must survive narrow footer, got: {text:?}"
        );
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
            yolo: false,
            branch: Some("feat/auth".into()),
            repo_url: None,
            source_path: None,
            archived: false,
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
    fn yolo_modal_warns_when_directory_missing() {
        use crate::enrich::Enricher;
        use std::collections::HashMap;

        let mut app = App::new();
        app.set_results(vec![SessionSummary {
            id: "a".into(),
            agent: AgentId::Claude,
            title: "fix auth".into(),
            directory: "/tmp/nonexistent-hop-test-dir-999999".into(),
            timestamp: 0,
            message_count: 3,
            yolo: false,
            branch: None,
            repo_url: None,
            source_path: None,
            archived: false,
        }]);
        app.open_yolo_modal_with(false);

        let enr: Vec<Box<dyn Enricher>> = vec![];
        let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
        let cols = crate::tui::columns::default_columns();
        let command = vec![
            "claude".to_string(),
            "--resume".to_string(),
            "a".to_string(),
        ];

        let backend = TestBackend::new(180, 16);
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
                    modal_command: Some(&command),
                    theme: Theme::default(),
                },
            )
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        let text: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            text.contains("does not exist"),
            "missing-dir warning should appear: {text:?}"
        );
        assert!(
            text.contains("Missing"),
            "Missing label should appear: {text:?}"
        );
    }

    #[test]
    fn yolo_modal_no_warning_when_directory_exists() {
        use crate::enrich::Enricher;
        use std::collections::HashMap;

        let mut app = App::new();
        app.set_results(vec![SessionSummary {
            id: "a".into(),
            agent: AgentId::Claude,
            title: "fix auth".into(),
            directory: "/tmp".into(),
            timestamp: 0,
            message_count: 3,
            yolo: false,
            branch: None,
            repo_url: None,
            source_path: None,
            archived: false,
        }]);
        app.open_yolo_modal_with(false);

        let enr: Vec<Box<dyn Enricher>> = vec![];
        let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
        let cols = crate::tui::columns::default_columns();
        let command = vec![
            "claude".to_string(),
            "--resume".to_string(),
            "a".to_string(),
        ];

        let backend = TestBackend::new(180, 16);
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
                    modal_command: Some(&command),
                    theme: Theme::default(),
                },
            )
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        let text: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            !text.contains("does not exist"),
            "missing-dir warning should NOT appear for valid dir: {text:?}"
        );
    }

}
