use crate::columns::Column;
use crate::core::SessionSummary;
use crate::enrich::Enricher;
use crate::tui::{help, results_list, theme, App};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, HighlightSpacing, List, ListItem, ListState, Padding, Paragraph, Wrap,
};
use ratatui::Frame;
use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;

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
    pub preview_lines: &'a [Line<'static>],
    pub status: &'a StatusLine,
    pub modal_command: Option<&'a [String]>,
}

const SELECTION_MARKER: &str = "❯ ";
const SELECTION_MARKER_WIDTH: u16 = 2;

pub fn render(f: &mut Frame, app: &App, model: RenderModel<'_>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(f.area());

    // search input
    let total = app.results().len();
    let pos = if total == 0 { 0 } else { app.selected() + 1 };

    // The query input is always live, so the prompt and query stay bright and
    // the caret is shown whenever no overlay is covering the input.
    let header = Line::from(vec![
        Span::styled(" ❯ ", Style::default().fg(theme::ACCENT)),
        Span::styled(
            app.query().to_string(),
            Style::default().fg(theme::SELECTED_FG),
        ),
        Span::raw(format!("   {}/{}", pos, total)).fg(theme::DIM),
    ]);
    f.render_widget(Paragraph::new(header), chunks[0]);
    if !app.help_open() && !app.modal_open() {
        let query_prefix = app.query().get(..app.query_cursor()).unwrap_or(app.query());
        let x = chunks[0]
            .x
            .saturating_add(crate::columns::display_width(" ❯ ") as u16)
            .saturating_add(crate::columns::display_width(query_prefix) as u16);
        let x = x.min(chunks[0].right().saturating_sub(1));
        f.set_cursor_position(Position::new(x, chunks[0].y));
    }

    // body: list (| preview)
    let (list_area, preview_area) = if app.preview_visible() {
        let pw = app.preview_width_pct();
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100 - pw), Constraint::Percentage(pw)])
            .split(chunks[1]);
        (body[0], Some(body[1]))
    } else {
        (chunks[1], None)
    };

    // column grid
    let cols = model.columns;
    let list_inner_w = list_area.width.saturating_sub(SELECTION_MARKER_WIDTH);
    let (list_header_area, list_rows_area) = split_list_area(list_area);
    let visible = visible_result_range(
        app.results().len(),
        app.selected(),
        list_rows_area.height as usize,
    );
    let visible_results = app.results().get(visible.clone()).unwrap_or_default();
    let layout = results_list::layout_for_rows(
        cols,
        list_inner_w,
        visible_results,
        model.enrichers,
        model.resolved,
        model.now,
    );
    let mut header = results_list::header_line(&layout, cols);
    header
        .spans
        .insert(0, Span::raw(" ".repeat(SELECTION_MARKER_WIDTH as usize)));
    f.render_widget(Paragraph::new(header), list_header_area);

    let items: Vec<ListItem> = visible_results
        .iter()
        .map(|s| {
            ListItem::new(results_list::row_line(
                s,
                &layout,
                cols,
                model.enrichers,
                model.resolved,
                model.now,
            ))
        })
        .collect();
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(app.selected().saturating_sub(visible.start)));
    }
    let list = List::new(items)
        .highlight_symbol(SELECTION_MARKER)
        .highlight_spacing(HighlightSpacing::Always)
        .highlight_style(
            Style::default()
                .fg(theme::SELECTED_FG)
                .bg(theme::SELECTED_BG)
                .add_modifier(Modifier::BOLD),
        );
    f.render_stateful_widget(list, list_rows_area, &mut state);

    // preview (lines are pre-rendered/memoized by the caller per selection+query)
    if let Some(area) = preview_area {
        let preview_block = Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(theme::DIVIDER))
            .padding(Padding::left(1));
        let preview_area = area;
        let area = preview_block.inner(area);
        f.render_widget(preview_block, preview_area);

        let selected = app.results().get(app.selected());
        let (header_area, transcript_area) =
            if app.preview_header_visible() && selected.is_some() && area.height >= 3 {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(2), Constraint::Min(1)])
                    .split(area);
                (Some(chunks[0]), chunks[1])
            } else {
                (None, area)
            };
        if let (Some(header_area), Some(session)) = (header_area, selected) {
            f.render_widget(
                Paragraph::new(preview_header_lines(session, model.now, model.resolved))
                    .style(Style::default().fg(theme::PREVIEW_TEXT)),
                header_area,
            );
        }
        f.render_widget(
            Paragraph::new(model.preview_lines.to_vec())
                .style(Style::default().fg(theme::PREVIEW_TEXT))
                .wrap(Wrap { trim: false })
                .scroll((app.preview_scroll(), 0)),
            transcript_area,
        );
    }

    // footer
    f.render_widget(Paragraph::new(footer_line(model.status, app.theme())), chunks[2]);

    if let Some((index, yolo)) = app.yolo_modal() {
        let session = app.results().get(index);
        render_yolo_modal(f, session, yolo, model.modal_command, app.theme());
    }

    // help overlay (drawn last, on top)
    if app.help_open() {
        help::render(f);
    }
}

fn split_list_area(area: Rect) -> (Rect, Rect) {
    if area.height == 0 {
        return (area, area);
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
    (chunks[0], chunks[1])
}

const FOOTER_HINTS: &str = "type to search · ↑↓ move · Enter resume · ? help · Esc clear/quit";

fn footer_line(status: &StatusLine, theme: &crate::tui::theme::Theme) -> Line<'static> {
    let mut spans = Vec::new();
    let (label, rest) = FOOTER_HINTS.split_once(" · ").unwrap_or((FOOTER_HINTS, ""));
    spans.push(Span::styled(
        label.to_string(),
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    ));
    if !rest.is_empty() {
        spans.push(Span::styled(
            format!(" · {rest}"),
            Style::default().fg(theme.muted),
        ));
    }
    if let Some(sync) = status.sync.as_deref().filter(|s| !s.is_empty()) {
        spans.push(Span::styled(
            format!(" · {sync}"),
            Style::default().fg(theme.muted),
        ));
    }
    if status.pr_pending > 0 {
        spans.push(Span::styled(
            format!(" · pr {} pending", status.pr_pending),
            Style::default().fg(theme.muted),
        ));
    }
    if let Some(filters) = status.filters.as_deref().filter(|s| !s.is_empty()) {
        spans.push(Span::styled(
            format!(" · filters {filters}"),
            Style::default().fg(theme.muted),
        ));
    }
    if let Some(warning) = status.warning.as_deref().filter(|s| !s.is_empty()) {
        spans.push(Span::styled(
            format!(" · {warning}"),
            Style::default().fg(theme.warning),
        ));
    }
    Line::from(spans)
}

fn render_yolo_modal(
    f: &mut Frame,
    session: Option<&SessionSummary>,
    yolo: bool,
    modal_command: Option<&[String]>,
    theme: &crate::tui::theme::Theme,
) {
    let area = f.area();
    if area.width < 4 || area.height < 4 {
        return;
    }
    let max_w = area.width.saturating_sub(2);
    let max_h = area.height.saturating_sub(2);
    let min_w = 20.min(max_w);
    let min_h = 6.min(max_h);
    let w = 72u16.min(max_w).max(min_w);
    let h = 10u16.min(max_h).max(min_h);
    let rect = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };

    let title = session
        .map(|s| fit_for_modal(&s.title, rect.width.saturating_sub(4) as usize))
        .unwrap_or_else(|| "(no session)".to_string());
    let directory = session
        .map(|s| fit_for_modal(&s.directory, rect.width.saturating_sub(15) as usize))
        .unwrap_or_else(|| "—".to_string());
    let command = modal_command
        .map(shell_join)
        .unwrap_or_else(|| "resume command unavailable".to_string());
    let command = fit_for_modal(&command, rect.width.saturating_sub(13) as usize);
    let danger = if yolo {
        "YOLO on: approvals and sandbox may be bypassed"
    } else {
        "YOLO off: normal resume"
    };

    let body = vec![
        Line::from(vec![
            Span::styled("Session  ", Style::default().fg(theme.muted)),
            Span::raw(title),
        ]),
        Line::from(vec![
            Span::styled("Directory ", Style::default().fg(theme.muted)),
            Span::raw(directory),
        ]),
        Line::from(vec![
            Span::styled("Command   ", Style::default().fg(theme.muted)),
            Span::raw(command),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            danger,
            if yolo {
                Style::default()
                    .fg(theme.warning)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.muted)
            },
        )),
        Line::from(""),
        Line::from("Tab toggles yolo · Enter resumes · Esc cancels"),
    ];

    f.buffer_mut()
        .set_style(area, Style::default().fg(theme.overlay_fg));
    f.render_widget(Clear, rect);
    f.render_widget(
        Paragraph::new(body)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" confirm resume "),
            )
            .alignment(Alignment::Left),
        rect,
    );
}

fn shell_join(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| {
            if arg
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_./:@".contains(c))
            {
                arg.clone()
            } else {
                format!("'{}'", arg.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn fit_for_modal(s: &str, width: usize) -> String {
    crate::columns::fit(
        s,
        width.min(u16::MAX as usize) as u16,
        crate::columns::Align::Left,
    )
    .trim_end()
    .to_string()
}

fn preview_header_lines(
    s: &SessionSummary,
    now: i64,
    resolved: &HashMap<(String, &'static str), Option<String>>,
) -> Vec<Line<'static>> {
    let repo = repo_label(s);
    let branch = s.branch.as_deref().unwrap_or("—");
    let pr = resolved
        .get(&(s.document_key(), "pr"))
        .and_then(|v| v.as_deref());
    let msgs = if s.message_count == 0 {
        "— msgs".to_string()
    } else {
        format!("{} msgs", s.message_count)
    };

    let mut first = vec![
        Span::styled(
            s.agent.badge(),
            Style::default()
                .fg(theme::agent_color(s.agent))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(repo, Style::default().fg(theme::DIM)),
        Span::raw("  "),
        Span::styled(branch.to_string(), Style::default().fg(theme::DIM)),
        Span::raw("  "),
    ];
    if let Some(pr) = pr {
        first.push(Span::styled(
            pr.to_string(),
            Style::default().fg(theme::ACCENT),
        ));
        first.push(Span::raw("  "));
    }
    first.extend([
        Span::styled(msgs, Style::default().fg(theme::DIM)),
        Span::raw("  "),
        Span::styled(rel_time(s.timestamp, now), Style::default().fg(theme::DIM)),
    ]);

    vec![
        Line::from(first),
        Line::from(vec![
            Span::raw(s.title.clone()),
            Span::styled(
                format!(" · {}", s.directory),
                Style::default().fg(theme::DIM),
            ),
        ]),
    ]
}

fn repo_label(s: &SessionSummary) -> String {
    if let Some(name) = s
        .repo_url
        .as_deref()
        .and_then(crate::enrich::repo_name_from_url)
    {
        return name;
    }
    Path::new(&s.directory)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "—".to_string())
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
        }]);
        let enr: Vec<Box<dyn Enricher>> = vec![Box::new(RepoEnricher), Box::new(BranchEnricher)];
        let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
        let transcript = vec![Message {
            role: Role::User,
            blocks: vec![Block::Prose("fix auth".into())],
        }];

        let lines =
            crate::tui::preview::render_transcript(&transcript, app.query(), AgentId::Claude);

        let cols = crate::columns::default_columns();
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
                    preview_lines: &lines,
                    status: &StatusLine::default(),
                    modal_command: None,
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
        }]);
        app.open_yolo_modal_with(true);

        let enr: Vec<Box<dyn Enricher>> = vec![];
        let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
        let cols = crate::columns::default_columns();
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
                    preview_lines: &[],
                    status: &status,
                    modal_command: Some(&command),
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
        }]);

        let enr: Vec<Box<dyn Enricher>> = vec![];
        let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
        let cols = crate::columns::default_columns();
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
                    preview_lines: &[],
                    status: &StatusLine::default(),
                    modal_command: None,
                },
            )
        })
        .unwrap();

        let buf = term.backend().buffer();
        assert_eq!(buf[(0, 2)].symbol(), SELECTION_MARKER.trim());
        assert_eq!(buf[(0, 2)].bg, theme::SELECTED_BG);
        assert_eq!(buf[(2, 2)].fg, theme::SELECTED_FG);
        assert_eq!(buf[(2, 2)].bg, theme::SELECTED_BG);
    }

    #[test]
    fn renders_single_mode_footer_hints() {
        use crate::enrich::Enricher;
        use std::collections::HashMap;

        let app = App::new();
        let enr: Vec<Box<dyn Enricher>> = vec![];
        let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
        let cols = crate::columns::default_columns();

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
                    preview_lines: &[],
                    status: &StatusLine::default(),
                    modal_command: None,
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
        assert!(text.contains("Esc clear/quit"));
        // No mode indicators remain.
        assert!(!text.contains("NAV"));
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
        }]);

        let enr: Vec<Box<dyn Enricher>> = vec![];
        let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
        let cols = crate::columns::default_columns();
        let preview_lines = vec![Line::from(
            "wrap-start one two three four five six seven eight nine ten wrap-end",
        )];

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
                    preview_lines: &preview_lines,
                    status: &StatusLine::default(),
                    modal_command: None,
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
        }]);
        app.open_yolo_modal_with(true);

        let enr: Vec<Box<dyn Enricher>> = vec![];
        let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
        let cols = crate::columns::default_columns();
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
                    preview_lines: &[],
                    status: &StatusLine::default(),
                    modal_command: None,
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
}
