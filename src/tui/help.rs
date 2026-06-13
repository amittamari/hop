//! Centered help overlay listing the keymap.

use crate::tui::theme::Theme;
use ratatui::layout::Alignment;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Padding, Paragraph};
use ratatui::Frame;

pub fn lines(theme: &Theme) -> Vec<Line<'static>> {
    vec![
        section("Navigation", theme),
        Line::from("  ↑/↓        move selection"),
        Line::from("  PgUp/PgDn  page list"),
        Line::from("  Ctrl+U/D   scroll preview"),
        Line::from("  Ctrl+N/B   preview matches"),
        Line::from(""),
        section("Preview", theme),
        Line::from("  Ctrl+P     toggle preview"),
        Line::from("  Ctrl+←/→   resize preview"),
        Line::from(""),
        section("Search Editing", theme),
        Line::from("  ←/→        move cursor"),
        Line::from("  Home/End   jump cursor"),
        Line::from("  Backspace  delete left"),
        Line::from("  Delete     delete at cursor"),
        Line::from("  Ctrl+A/E   start / end"),
        Line::from("  Ctrl+W     delete word"),
        Line::from(""),
        section("Actions", theme),
        Line::from("  Enter      resume"),
        Line::from("  Tab        autocomplete keyword"),
        Line::from("  ?          toggle help"),
        Line::from("  Esc        clear query / quit"),
        Line::from("  Ctrl+C     quit"),
    ]
}

fn section(label: &'static str, theme: &Theme) -> Line<'static> {
    Line::from(Span::styled(
        label,
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    ))
}

/// Render the overlay centered over the frame.
pub fn render(f: &mut Frame, theme: &Theme) {
    let area = f.area();
    if area.width < 8 || area.height < 6 {
        return;
    }

    let body = lines(theme);
    let w = 58u16.min(area.width.saturating_sub(4)).max(8);
    let h = (body.len() as u16 + 4)
        .min(area.height.saturating_sub(2))
        .max(4);
    let rect = crate::tui::view::center(area, w, h);
    f.buffer_mut().set_style(
        area,
        Style::default().fg(theme.overlay_fg).bg(theme.overlay_bg),
    );
    f.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_style(Style::default().fg(theme.accent))
        .title(" help ")
        .title_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
        .padding(Padding::symmetric(2, 1));
    f.render_widget(
        Paragraph::new(body)
            .block(block)
            .alignment(Alignment::Left)
            .style(Style::default()),
        rect,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_lists_core_bindings() {
        let text: String = lines(&Theme::default())
            .iter()
            .map(|x| {
                x.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Ctrl+P"));
        assert!(text.contains("Ctrl+N/B"));
        assert!(text.contains("Ctrl+←/→"));
        assert!(text.contains("Tab"));
        assert!(text.contains("clear query / quit"));
        // The removed modal keymap and its toggles are gone.
        assert!(!text.to_lowercase().contains("modal"));
        assert!(!text.contains("Ctrl+Y"));
    }
}
