use crate::core::SessionSummary;
use crate::tui::columns;
use crate::tui::theme::Theme;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;

/// A `w` x `h` rect centered within `area` on both axes (clamped to `area`).
pub fn center(area: Rect, w: u16, h: u16) -> Rect {
    let [_, mid, _] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(w.min(area.width)),
        Constraint::Fill(1),
    ])
    .flex(Flex::Center)
    .areas(area);
    let [_, rect, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(h.min(area.height)),
        Constraint::Fill(1),
    ])
    .flex(Flex::Center)
    .areas(mid);
    rect
}

pub fn render_yolo_modal(
    f: &mut Frame,
    session: Option<&SessionSummary>,
    yolo: bool,
    modal_command: Option<&[String]>,
    theme: &Theme,
) {
    let area = f.area();
    if area.width < 4 || area.height < 4 {
        return;
    }
    let archived = session.is_some_and(|s| s.archived);
    let dir_missing = session
        .is_some_and(|s| !s.directory.is_empty() && !std::path::Path::new(&s.directory).is_dir());
    let max_w = area.width.saturating_sub(2);
    let max_h = area.height.saturating_sub(2);
    let min_w = 20.min(max_w);
    let min_h = 6.min(max_h);
    let w = 72u16.min(max_w).max(min_w);
    let extra = u16::from(archived) + u16::from(dir_missing);
    let h = (10u16 + extra).min(max_h).max(min_h);
    let rect = center(area, w, h);

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

    let mut body = vec![
        Line::from(vec![
            Span::styled("Session  ", Style::default().fg(theme.muted)),
            Span::raw(title),
        ]),
        Line::from(vec![
            Span::styled("Directory ", Style::default().fg(theme.muted)),
            if dir_missing {
                Span::styled(directory, Style::default().fg(theme.warning))
            } else {
                Span::raw(directory)
            },
        ]),
        Line::from(vec![
            Span::styled("Command   ", Style::default().fg(theme.muted)),
            Span::raw(command),
        ]),
    ];
    if archived {
        body.push(Line::from(vec![
            Span::styled("Archived  ", Style::default().fg(theme.muted)),
            Span::styled(
                "session is archived; it will be unarchived first",
                Style::default()
                    .fg(theme.warning)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }
    if dir_missing {
        body.push(Line::from(vec![
            Span::styled("Missing   ", Style::default().fg(theme.muted)),
            Span::styled(
                "directory does not exist; agent will start in current dir",
                Style::default()
                    .fg(theme.warning)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }
    body.push(Line::from(""));
    body.push(Line::from(Span::styled(
        danger,
        if yolo {
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.muted)
        },
    )));
    body.push(Line::from(""));
    body.push(Line::from(if archived {
        "Tab toggles yolo · Enter unarchives & resumes · Esc cancels"
    } else {
        "Tab toggles yolo · Enter resumes · Esc cancels"
    }));

    let modal_title = if archived {
        " unarchive & resume "
    } else {
        " confirm resume "
    };
    f.buffer_mut().set_style(
        area,
        Style::default().fg(theme.overlay_fg).bg(theme.overlay_bg),
    );
    f.render_widget(Clear, rect);
    f.render_widget(
        Paragraph::new(body)
            .block(Block::bordered().title(modal_title))
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

pub(crate) fn fit_for_modal(s: &str, width: usize) -> String {
    columns::fit(
        s,
        width.min(u16::MAX as usize) as u16,
        columns::Align::Left,
    )
    .trim_end()
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_centers_on_both_axes() {
        let area = Rect::new(0, 0, 100, 40);
        let rect = center(area, 20, 10);
        assert_eq!(rect.width, 20);
        assert_eq!(rect.height, 10);
        assert_eq!(rect.x, 40); // (100 - 20) / 2
        assert_eq!(rect.y, 15); // (40 - 10) / 2
    }
}
