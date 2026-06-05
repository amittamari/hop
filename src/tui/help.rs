//! Centered help overlay listing the active keymap.

use crate::tui::keymap::Preset;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

pub fn lines(preset: Preset) -> Vec<Line<'static>> {
    let mut out = vec![
        Line::from(Span::raw("Navigation")),
        Line::from("  ↑/↓        move selection"),
        Line::from("  PgUp/PgDn  page list by viewport"),
        Line::from("  Ctrl+U/D   scroll preview by viewport"),
        Line::from("  Ctrl+N/B   next / previous preview match"),
        Line::from(""),
        Line::from(Span::raw("Preview")),
        Line::from("  Ctrl+P     toggle preview"),
        Line::from("  [ / ]      shrink / grow preview when query empty"),
        Line::from(""),
        Line::from(Span::raw("Search editing")),
        Line::from("  ←/→ Home/End move cursor"),
        Line::from("  Backspace/Delete edit at cursor"),
        Line::from("  Ctrl+A/E/W start / end / delete word"),
        Line::from(""),
        Line::from(Span::raw("Actions")),
        Line::from("  Enter      resume"),
        Line::from("  Ctrl+Y     yolo resume prompt"),
        Line::from("  Tab        autocomplete keyword"),
        Line::from("  ?          toggle this help when query empty"),
        Line::from("  Esc/Ctrl+C quit"),
    ];
    if preset == Preset::Modal {
        out.push(Line::from(""));
        out.push(Line::from(Span::raw("Modal mode")));
        out.push(Line::from("  Esc        leave query → navigate"));
        out.push(Line::from("  j/k g/G    move / top-bottom"));
        out.push(Line::from("  / p        search / preview"));
    }
    out
}

/// Render the overlay centered over the frame.
pub fn render(f: &mut Frame, preset: Preset) {
    let area = f.area();
    let w = 44u16.min(area.width.saturating_sub(2));
    let body = lines(preset);
    let h = (body.len() as u16 + 2).min(area.height.saturating_sub(2));
    let rect = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);
    let block = Block::default().borders(Borders::ALL).title(" help ");
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
        assert!(text.contains("Modal mode"));
    }
}
