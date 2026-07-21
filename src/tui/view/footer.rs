//! Footer rendering: static key-hints on the left, volatile sync/PR/warning
//! status on the right. Both are built here and measured/placed by `render`.

use super::StatusLine;
use crate::tui::glyphs::Glyphs;
use crate::tui::theme::Theme;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// Static, low-priority hints shown on the left of the footer, built from the
/// `primary` subset of the canonical bindings table. Dropped first (clipped by
/// the SpaceBetween layout) when the terminal is too narrow for both halves.
pub(super) fn footer_hints_line(
    keymap: &crate::tui::keymap::Keymap,
    mode: crate::tui::SearchMode,
    theme: &Theme,
    glyphs: &Glyphs,
) -> Line<'static> {
    let primary: Vec<String> = crate::tui::keymap::bindings(keymap, mode)
        .iter()
        .filter(|b| b.primary)
        .map(|b| {
            if b.keys == "type" {
                format!("type to {}", b.label)
            } else {
                format!("{} {}", b.keys, b.label)
            }
        })
        .collect();

    // Restraint: footer key-hints get no icons. `glyphs` is used only for the
    // shared separator so the glyph vocabulary stays centralized.
    let sep = glyphs.sep();
    let mut spans = Vec::new();
    for (i, hint) in primary.iter().enumerate() {
        if i == 0 {
            spans.push(Span::styled(
                hint.clone(),
                Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(format!("{sep}{hint}"), Style::default().fg(theme.muted)));
        }
    }
    Line::from(spans)
}

/// Volatile, high-priority status shown on the right of the footer. Rendered
/// right-aligned so it survives clipping ahead of the static hints.
pub(super) fn footer_status_line(
    status: &StatusLine,
    theme: &Theme,
    glyphs: &Glyphs,
) -> Line<'static> {
    let sep = glyphs.sep();
    let mut spans = Vec::new();
    let push_sep = |spans: &mut Vec<Span<'static>>| {
        if !spans.is_empty() {
            spans.push(Span::styled(sep.to_string(), Style::default().fg(theme.muted)));
        }
    };
    if let Some(sync) = status.sync.as_deref().filter(|s| !s.is_empty()) {
        push_sep(&mut spans);
        spans.push(Span::styled(sync.to_string(), Style::default().fg(theme.muted)));
    }
    if status.pr_pending > 0 {
        push_sep(&mut spans);
        spans.push(Span::styled(
            format!("pr {} pending", status.pr_pending),
            Style::default().fg(theme.muted),
        ));
    }
    if let Some(warning) = status.warning.as_deref().filter(|s| !s.is_empty()) {
        push_sep(&mut spans);
        // Status glyph (icons on) colored by the warning role; empty in ascii.
        spans.push(Span::styled(
            format!("{}{}", glyphs.warning(), warning),
            Style::default().fg(theme.warning),
        ));
    }
    Line::from(spans)
}
