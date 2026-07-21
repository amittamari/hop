//! Card-mode list rendering: drawing a single result card (with selection
//! accent bar) and computing which variable-height cards fit the viewport.

use crate::core::SessionSummary;
use crate::tui::results_list;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use std::ops::Range;

/// Render a single card. Selected cards get a left accent bar; unselected
/// cards get a space in the same column. Content is inset by the bar width.
pub(super) fn render_card(
    f: &mut Frame,
    session: &SessionSummary,
    area: Rect,
    selected: bool,
    ctx: &results_list::RowCtx<'_>,
) {
    let content_h = area.height.saturating_sub(1); // strip trailing separator
    let bar_w = 2u16; // accent bar + 1 space
    let content_area = Rect {
        x: area.x + bar_w,
        y: area.y,
        width: area.width.saturating_sub(bar_w),
        height: content_h,
    };
    if selected {
        for row in 0..content_h {
            let bar_area = Rect { x: area.x, y: area.y + row, width: bar_w, height: 1 };
            f.render_widget(
                Paragraph::new(Span::styled(
                    ctx.glyphs.accent_bar(),
                    Style::default().fg(ctx.theme.accent),
                )),
                bar_area,
            );
        }
    }
    let lines = results_list::card_lines(session, content_area.width, ctx);
    f.render_widget(Paragraph::new(lines), content_area);
}

/// Compute the visible range of cards that fit in `height` rows, keeping
/// the selected card in view. Cards have variable height (2 or 3 content
/// lines + border + separator).
pub(super) fn card_visible_range(
    results: &[SessionSummary],
    selected: usize,
    height: usize,
) -> Range<usize> {
    if results.is_empty() || height == 0 {
        return 0..0;
    }
    let sel = selected.min(results.len() - 1);

    // Try starting from the selected card and expanding backward as far as
    // we can fill the viewport. First, ensure the selected card fits.
    let sel_h = results_list::card_height(&results[sel], true) as usize;
    if sel_h > height {
        return sel..sel + 1;
    }

    // Greedily add cards before the selected card.
    let mut start = sel;
    let mut used = sel_h;
    while start > 0 {
        let h = results_list::card_height(&results[start - 1], false) as usize;
        if used + h > height {
            break;
        }
        start -= 1;
        used += h;
    }

    // Greedily add cards after the selected card.
    let mut end = sel + 1;
    while end < results.len() {
        let h = results_list::card_height(&results[end], false) as usize;
        if used + h > height {
            break;
        }
        end += 1;
        used += h;
    }

    start..end
}
