//! Centered help overlay listing the keymap.

use crate::tui::SearchMode;
use crate::tui::keymap::Keymap;
use crate::tui::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Alignment;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Padding, Paragraph};

pub fn lines(keymap: &Keymap, mode: SearchMode, theme: &Theme) -> Vec<Line<'static>> {
    let table = crate::tui::keymap::bindings(keymap, mode);
    // Pad the key column to the widest key label (skipping the "type"
    // pseudo-key, which is shown as prose, not a key chord). This replaces the
    // old hand-counted leading spaces.
    let key_w = table
        .iter()
        .filter(|b| b.keys != "type")
        .map(|b| b.keys.chars().count())
        .max()
        .unwrap_or(0);

    // Distinct groups, in first-seen order.
    let mut groups: Vec<&'static str> = Vec::new();
    for b in &table {
        if !groups.contains(&b.group) {
            groups.push(b.group);
        }
    }

    let mut out: Vec<Line<'static>> = Vec::new();
    for (gi, group) in groups.iter().enumerate() {
        if gi > 0 {
            out.push(Line::from(""));
        }
        out.push(section(group, theme));
        for b in table.iter().filter(|b| &b.group == group) {
            if b.keys == "type" {
                // Prose row: "type to <label>", no key chord column.
                out.push(Line::from(format!("  type to {}", b.label)));
                continue;
            }
            let key_col = format!("  {:<width$}  ", b.keys, width = key_w);
            out.push(Line::from(vec![
                Span::styled(key_col, Style::default().fg(theme.accent)),
                Span::raw(b.label.to_string()),
            ]));
        }
    }

    // Query keyword reference. These are not key bindings, so they live here as
    // static help text rather than in the `bindings()` catalog. In simple search
    // mode the toolbar covers the common cases; the keywords apply when typing a
    // raw query (see the Query Syntax section of the README for full grammar).
    out.push(Line::from(""));
    out.push(section("Query Keywords (raw mode)", theme));
    for (kw, desc) in QUERY_KEYWORDS {
        let kw_col = format!("  {kw:<width$}  ", width = key_w);
        out.push(Line::from(vec![
            Span::styled(kw_col, Style::default().fg(theme.accent)),
            Span::raw((*desc).to_string()),
        ]));
    }
    out
}

/// Static query-keyword reference shown in the help overlay. Mirrors the DSL
/// parsed in `src/query.rs`; keep in sync with the README Query Syntax table.
const QUERY_KEYWORDS: &[(&str, &str)] = &[
    ("agent:claude", "filter by agent (! or - excludes)"),
    ("dir:api", "filter by working directory"),
    ("repo:hop", "filter by repo (all worktrees)"),
    ("date:today", "today/yesterday/week/month"),
    ("date:<2d", "within(<)/older(>) by h/d/w"),
];

fn section(label: &'static str, theme: &Theme) -> Line<'static> {
    Line::from(Span::styled(label, Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)))
}

/// Render the overlay centered over the frame.
pub fn render(f: &mut Frame, keymap: &Keymap, mode: SearchMode, theme: &Theme) {
    let area = f.area();
    if area.width < 8 || area.height < 6 {
        return;
    }

    let body = lines(keymap, mode, theme);
    let w = 58u16.min(area.width.saturating_sub(4)).max(8);
    let h = (body.len() as u16 + 4).min(area.height.saturating_sub(2)).max(4);
    let rect = crate::tui::modal::center(area, w, h);
    f.buffer_mut().set_style(area, Style::default().fg(theme.overlay_fg).bg(theme.overlay_bg));
    f.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_style(Style::default().fg(theme.accent))
        .title(" help ")
        .title_style(Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))
        .padding(Padding::symmetric(2, 1));
    f.render_widget(
        Paragraph::new(body).block(block).alignment(Alignment::Left).style(Style::default()),
        rect,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered_text(mode: SearchMode) -> String {
        lines(&Keymap::defaults(), mode, &Theme::default())
            .iter()
            .map(|x| x.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn help_lists_every_binding_from_table() {
        let text = rendered_text(SearchMode::Simple);
        for b in crate::tui::keymap::bindings(&Keymap::defaults(), SearchMode::Simple) {
            assert!(text.contains(b.label), "help overlay missing binding label {:?}", b.label);
            // The "type" pseudo-key has no literal key column in help.
            if b.keys != "type" {
                assert!(text.contains(&b.keys), "help overlay missing binding keys {:?}", b.keys);
            }
        }
        // Group headings still render.
        assert!(text.contains("Navigation"));
        assert!(text.contains("Preview"));
        assert!(text.contains("Search Editing"));
        assert!(text.contains("Actions"));
        // The removed modal keymap and its toggles stay gone.
        assert!(!text.to_lowercase().contains("modal"));
        assert!(!text.contains("Ctrl+Y"));
    }

    #[test]
    fn help_lists_query_keywords() {
        let text = rendered_text(SearchMode::Simple);
        assert!(text.contains("Query Keywords"));
        for (kw, _) in QUERY_KEYWORDS {
            assert!(text.contains(kw), "help overlay missing query keyword {kw:?}");
        }
        // Keyword labels must fit the shared key column so alignment holds.
        let key_w = crate::tui::keymap::bindings(&Keymap::defaults(), SearchMode::Simple)
            .iter()
            .filter(|b| b.keys != "type")
            .map(|b| b.keys.chars().count())
            .max()
            .unwrap();
        for (kw, _) in QUERY_KEYWORDS {
            assert!(
                kw.chars().count() <= key_w,
                "query keyword {kw:?} wider than key column {key_w}"
            );
        }
    }

    #[test]
    fn help_key_column_is_aligned() {
        // Every non-heading, non-blank row pads the key column to a constant
        // width, so the label column starts at the same offset on every line.
        let key_w = crate::tui::keymap::bindings(&Keymap::defaults(), SearchMode::Simple)
            .iter()
            .filter(|b| b.keys != "type")
            .map(|b| b.keys.chars().count())
            .max()
            .unwrap();
        let body = lines(&Keymap::defaults(), SearchMode::Simple, &Theme::default());
        let mut checked = 0usize;
        for line in &body {
            // Rows rendered by the table have exactly two spans: key + label.
            if line.spans.len() == 2 {
                let key_span = line.spans[0].content.as_ref();
                assert_eq!(
                    key_span.chars().count(),
                    // leading "  " indent + padded key column + trailing "  "
                    2 + key_w + 2,
                    "key column not padded to constant width: {key_span:?}"
                );
                checked += 1;
            }
        }
        assert!(checked > 0, "expected at least one table row");
    }

    #[test]
    fn overlay_renders_labels_into_buffer() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let backend = TestBackend::new(64, 40);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| render(f, &Keymap::defaults(), SearchMode::Simple, &Theme::default()))
            .unwrap();
        let text: String = term.backend().buffer().content().iter().map(|c| c.symbol()).collect();
        assert!(text.contains("toggle preview"));
        assert!(text.contains("resume"));
        assert!(text.contains("help"));
    }
}
