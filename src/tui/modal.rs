use crate::core::SessionSummary;
use crate::tui::columns;
use crate::tui::glyphs::Glyphs;
use crate::tui::theme::Theme;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Padding, Paragraph};

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
    glyphs: &Glyphs,
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
    // +4 for padding (top 1 + bottom 1 + border top/bottom 2 already in bordered)
    let h = (12u16 + extra).min(max_h).max(min_h);
    let rect = center(area, w, h);

    // Inner width after border (1+1) and horizontal padding (2+2).
    let inner_w = rect.width.saturating_sub(6) as usize;
    let label_w = 10;
    let value_budget = inner_w.saturating_sub(label_w);

    let title = session
        .map(|s| fit_for_modal(&s.title, value_budget))
        .unwrap_or_else(|| "(no session)".to_string());
    let directory = session
        .map(|s| fit_for_modal(&s.directory, value_budget))
        .unwrap_or_else(|| "—".to_string());
    let command =
        modal_command.map(shell_join).unwrap_or_else(|| "resume command unavailable".to_string());
    let command = fit_for_modal(&command, value_budget);

    let label_style = Style::default().fg(theme.muted).add_modifier(Modifier::BOLD);

    let mut body = vec![
        Line::from(vec![
            Span::styled(format!("{:<label_w$}", "Session"), label_style),
            Span::raw(title),
        ]),
        Line::from(vec![
            Span::styled(format!("{:<label_w$}", "Directory"), label_style),
            if dir_missing {
                Span::styled(directory, Style::default().fg(theme.warning))
            } else {
                Span::raw(directory)
            },
        ]),
        Line::from(vec![
            Span::styled(format!("{:<label_w$}", "Command"), label_style),
            Span::raw(command),
        ]),
    ];
    let warn_style = Style::default().fg(theme.warning).add_modifier(Modifier::BOLD);
    let warn_glyph = glyphs.warning(); // icon + trailing space when enabled, else ""
    if archived {
        body.push(Line::from(vec![
            Span::styled(format!("{:<label_w$}", "Archived"), label_style),
            Span::styled(
                format!("{warn_glyph}session is archived; it will be unarchived first"),
                warn_style,
            ),
        ]));
    }
    if dir_missing {
        body.push(Line::from(vec![
            Span::styled(format!("{:<label_w$}", "Missing"), label_style),
            Span::styled(
                format!("{warn_glyph}directory does not exist; agent will start in current dir"),
                warn_style,
            ),
        ]));
    }
    body.push(Line::from(""));
    body.push(if yolo {
        Line::from(Span::styled(
            format!("{warn_glyph}YOLO on: approvals and sandbox may be bypassed"),
            warn_style,
        ))
    } else {
        Line::from(Span::styled("YOLO off: normal resume", Style::default().fg(theme.muted)))
    });
    body.push(Line::from(""));

    let key_style = Style::default().fg(theme.accent);
    let sep_style = Style::default().fg(theme.border);
    let hint_style = Style::default().fg(theme.muted);
    let sep = Span::styled(glyphs.sep(), sep_style);
    let confirm_label = if archived { "unarchive & resume" } else { "resume" };
    body.push(Line::from(vec![
        Span::styled("Tab", key_style),
        Span::styled(" toggle yolo", hint_style),
        sep.clone(),
        Span::styled("Enter", key_style),
        Span::styled(format!(" {confirm_label}"), hint_style),
        sep,
        Span::styled("Esc", key_style),
        Span::styled(" cancel", hint_style),
    ]));

    let modal_title = if archived { " unarchive & resume " } else { " confirm resume " };
    f.buffer_mut().set_style(area, Style::default().fg(theme.overlay_fg).bg(theme.overlay_bg));
    f.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_style(Style::default().fg(theme.accent))
        .title(modal_title)
        .title_style(Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))
        .padding(Padding::symmetric(2, 1));
    f.render_widget(Paragraph::new(body).block(block).alignment(Alignment::Left), rect);
}

fn shell_join(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| {
            if arg.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:@".contains(c)) {
                arg.clone()
            } else {
                format!("'{}'", arg.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn fit_for_modal(s: &str, width: usize) -> String {
    columns::fit(s, width.min(u16::MAX as usize) as u16, columns::Align::Left)
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
