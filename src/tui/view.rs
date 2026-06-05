use crate::columns::Column;
use crate::core::Session;
use crate::enrich::Enricher;
use crate::tui::{help, results_list, theme, App};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
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

pub fn render(
    f: &mut Frame,
    app: &App,
    now: i64,
    columns: &[Column],
    enrichers: &[Box<dyn Enricher>],
    fast_cache: &mut HashMap<(String, &'static str), Option<String>>,
    resolved: &HashMap<(String, &'static str), Option<String>>,
    preview_lines: &[Line<'static>],
    match_base: u16,
) {
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
    let header = Line::from(vec![
        Span::raw("❯ "),
        Span::raw(app.query().to_string()),
        Span::raw(format!("   {}/{}", pos, total)).fg(theme::DIM),
    ]);
    f.render_widget(Paragraph::new(header), chunks[0]);

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
    let cols = columns;
    let list_inner_w = list_area
        .width
        .saturating_sub(if preview_area.is_some() { 1 } else { 0 });
    let layout = results_list::layout_for(&cols, list_inner_w);
    let visible = visible_result_range(
        app.results().len(),
        app.selected(),
        list_area.height as usize,
    );
    let items: Vec<ListItem> = app
        .results()
        .get(visible.clone())
        .unwrap_or_default()
        .iter()
        .map(|s| {
            ListItem::new(results_list::row_line(
                s, &layout, &cols, enrichers, fast_cache, resolved, now,
            ))
        })
        .collect();
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(app.selected().saturating_sub(visible.start)));
    }
    let list_block = if preview_area.is_some() {
        Block::default().borders(Borders::RIGHT)
    } else {
        Block::default()
    };
    let list = List::new(items)
        .block(list_block)
        .highlight_style(ratatui::style::Style::default().bg(theme::ACCENT));
    f.render_stateful_widget(list, list_area, &mut state);

    // preview (lines are pre-rendered/memoized by the caller per selection+query)
    if let Some(area) = preview_area {
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
                Paragraph::new(preview_header_lines(session, now, resolved)),
                header_area,
            );
        }
        let scroll = match_base.saturating_add(app.preview_scroll());
        f.render_widget(
            Paragraph::new(preview_lines.to_vec()).scroll((scroll, 0)),
            transcript_area,
        );
    }

    // footer
    let footer = if app.modal_open() {
        "tab toggle yolo · enter confirm · esc cancel"
    } else {
        "↑↓ move · enter resume · ctrl+y yolo · ctrl+p preview · ctrl+u/d scroll · [ ] size · ? help · esc quit"
    };
    f.render_widget(Paragraph::new(footer).fg(theme::DIM), chunks[2]);

    // help overlay (drawn last, on top)
    if app.help_open() {
        help::render(f, app.keymap_preset());
    }
}

fn preview_header_lines(
    s: &Session,
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

fn repo_label(s: &Session) -> String {
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
    use crate::core::{AgentId, Session};
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
        app.set_results(vec![Session {
            id: "a".into(),
            agent: AgentId::Claude,
            title: "fix auth".into(),
            directory: "/work/api".into(),
            timestamp: 0,
            content: "hello".into(),
            message_count: 3,
            mtime: 0,
            yolo: false,
            branch: Some("feat/auth".into()),
            repo_url: None,
            source_path: None,
        }]);
        let enr: Vec<Box<dyn Enricher>> = vec![Box::new(RepoEnricher), Box::new(BranchEnricher)];
        let mut fast_cache = HashMap::new();
        let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
        let transcript = vec![Message {
            role: Role::User,
            blocks: vec![Block::Prose("fix auth".into())],
        }];

        let lines =
            crate::tui::preview::render_transcript(&transcript, app.query(), AgentId::Claude);
        let base = crate::tui::preview::first_match_line(&lines, app.query()).unwrap_or(0) as u16;

        let cols = crate::columns::default_columns();
        let backend = TestBackend::new(100, 12);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            render(
                f,
                &app,
                100,
                &cols,
                &enr,
                &mut fast_cache,
                &resolved,
                &lines,
                base,
            )
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        let text: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(text.contains("CLAUDE"));
        assert!(text.contains("fix auth"));
        assert!(text.contains("feat/auth"));
        assert!(text.contains("/work/api"));
    }

    #[test]
    fn visible_range_keeps_selection_in_view() {
        assert_eq!(visible_result_range(0, 0, 10), 0..0);
        assert_eq!(visible_result_range(100, 0, 10), 0..10);
        assert_eq!(visible_result_range(100, 9, 10), 0..10);
        assert_eq!(visible_result_range(100, 10, 10), 1..11);
        assert_eq!(visible_result_range(100, 99, 10), 90..100);
    }
}
