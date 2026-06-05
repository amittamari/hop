//! Centered help overlay listing the active keymap.

use crate::tui::keymap::Preset;
use crate::tui::theme;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph};
use ratatui::Frame;

pub fn lines(preset: Preset) -> Vec<Line<'static>> {
    let mut out = vec![
        section("Navigation"),
        Line::from("  ↑/↓        move selection"),
        Line::from("  PgUp/PgDn  page list"),
        Line::from("  Ctrl+U/D   scroll preview"),
        Line::from("  Ctrl+N/B   preview matches"),
        Line::from(""),
        section("Preview"),
        Line::from("  Ctrl+P     toggle preview"),
        Line::from("  [ / ]      resize preview"),
        Line::from(""),
        section("Search Editing"),
        Line::from("  ←/→        move cursor"),
        Line::from("  Home/End   jump cursor"),
        Line::from("  Backspace  delete left"),
        Line::from("  Delete     delete at cursor"),
        Line::from("  Ctrl+A/E   start / end"),
        Line::from("  Ctrl+W     delete word"),
        Line::from(""),
        section("Actions"),
        Line::from("  Enter      resume"),
        Line::from("  Ctrl+Y     yolo prompt"),
        Line::from("  Tab        autocomplete keyword"),
        Line::from("  ?          toggle help"),
        Line::from("  `          toggle keymap mode (search/navigate)"),
        Line::from("  Esc        quit"),
        Line::from("  Ctrl+C     quit"),
    ];
    if preset == Preset::Modal {
        out.push(Line::from(""));
        out.push(section("Modal Mode"));
        out.push(Line::from("  Esc        enter NAV"));
        out.push(Line::from("  j/k        move selection"));
        out.push(Line::from("  g/G        top / bottom"));
        out.push(Line::from("  /          search"));
        out.push(Line::from("  p          preview"));
    }
    out
}

fn section(label: &'static str) -> Line<'static> {
    Line::from(Span::styled(
        label,
        Style::default()
            .fg(theme::ACCENT)
            .add_modifier(Modifier::BOLD),
    ))
}

/// Render the overlay centered over the frame.
pub fn render(f: &mut Frame, preset: Preset) {
    let area = f.area();
    if area.width < 8 || area.height < 6 {
        return;
    }

    let body = lines(preset);
    let w = 58u16.min(area.width.saturating_sub(4)).max(8);
    let h = (body.len() as u16 + 4)
        .min(area.height.saturating_sub(2))
        .max(4);
    let rect = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    f.buffer_mut()
        .set_style(area, Style::default().fg(theme::OVERLAY_DIM));
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::ACCENT))
        .title(" help ")
        .title_style(
            Style::default()
                .fg(theme::ACCENT)
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
    fn search_help_lists_core_bindings() {
        let l = lines(Preset::Search);
        let text: String = l
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
        assert!(text.contains("Ctrl+Y"));
        assert!(text.contains("Tab"));
        assert!(!text.contains("Modal mode"));
    }

    #[test]
    fn modal_help_adds_modal_section() {
        let text: String = lines(Preset::Modal)
            .iter()
            .map(|x| {
                x.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Modal Mode"));
    }
}
