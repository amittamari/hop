use crate::tui::{theme, App};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

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

pub fn render(f: &mut Frame, app: &App, now: i64) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // search input
            Constraint::Min(1),    // body (list | preview)
            Constraint::Length(1), // footer
        ])
        .split(f.area());

    // --- search input ---
    let header = Line::from(vec![
        Span::raw("❯ "),
        Span::raw(app.query()),
        Span::raw(format!("   {}/{}", app.results().len(), app.results().len())).fg(theme::DIM),
    ]);
    f.render_widget(Paragraph::new(header), chunks[0]);

    // --- body: list | preview ---
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    let items: Vec<ListItem> = app
        .results()
        .iter()
        .map(|s| {
            ListItem::new(Line::from(vec![
                Span::raw(s.agent.badge()).fg(theme::agent_color(s.agent)),
                Span::raw(" "),
                Span::raw(s.title.clone()),
                Span::raw(format!("  · {} · {}", s.directory, rel_time(s.timestamp, now))).fg(theme::DIM),
            ]))
        })
        .collect();

    let mut state = ListState::default();
    if !app.results().is_empty() {
        state.select(Some(app.selected()));
    }
    let list = List::new(items)
        .block(Block::default().borders(Borders::RIGHT))
        .highlight_style(Style::default().bg(theme::ACCENT));
    f.render_stateful_widget(list, body[0], &mut state);

    // --- preview ---
    let preview_text = app
        .results()
        .get(app.selected())
        .map(|s| s.content.clone())
        .unwrap_or_default();
    f.render_widget(Paragraph::new(preview_text), body[1]);

    // --- footer ---
    let footer = if app.modal_open() {
        "tab toggle yolo · enter confirm · esc cancel"
    } else {
        "↑↓ move · enter resume · tab yolo · esc quit"
    };
    f.render_widget(Paragraph::new(footer).fg(theme::DIM), chunks[2]);
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
    fn renders_badge_and_title() {
        let mut app = App::new();
        app.set_results(vec![Session {
            id: "a".into(), agent: AgentId::Claude, title: "fix auth".into(),
            directory: "/w".into(), timestamp: 0, content: "hello".into(),
            message_count: 1, mtime: 0, yolo: false,
            branch: None, repo_url: None,
        }]);
        let backend = TestBackend::new(60, 6);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| render(f, &app, 100)).unwrap();
        let buf = term.backend().buffer().clone();
        let text: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(text.contains("CLAUDE"));
        assert!(text.contains("fix auth"));
    }
}
