//! The 3-line preview header (title, meta row, rule) shown above the
//! transcript in compact mode. Fits directory/branch into the available width
//! with per-field icon budgets.

use super::rel_time;
use crate::core::SessionSummary;
use crate::enrich::{BranchEnricher, Enricher};
use crate::tui::glyphs::Glyphs;
use crate::tui::modal;
use crate::tui::theme::Theme;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use std::collections::HashMap;

pub(super) fn preview_header_lines(
    s: &SessionSummary,
    now: i64,
    resolved: &HashMap<(String, &'static str), Option<String>>,
    theme: &Theme,
    glyphs: &Glyphs,
    width: u16,
) -> Vec<Line<'static>> {
    let w = width as usize;
    let title_line = Line::from(Span::styled(
        modal::fit_for_modal(&s.title, w),
        Style::default().add_modifier(Modifier::BOLD),
    ));

    let sep_style = Style::default().fg(theme.border);
    let muted = Style::default().fg(theme.muted);
    let sep = glyphs.sep();
    let dw = crate::tui::columns::display_width;
    let sep_w = dw(sep);

    // Field icons (with trailing space) when enabled; empty in ascii mode.
    let agent_glyph = glyphs.agent(s.agent);
    let badge = s.agent.badge();
    let mark =
        if agent_glyph.is_empty() { badge.to_string() } else { format!("{agent_glyph} {badge}") };
    let branch = BranchEnricher.resolve(s).map(|v| v.text);
    let pr = resolved.get(&(s.document_key(), "pr")).and_then(|v| v.as_deref());
    let msgs = if s.message_count > 0 {
        Some(format!("{}{} msgs", glyphs.msgs(), s.message_count))
    } else {
        None
    };
    let time = format!("{}{}", glyphs.time(), rel_time(s.timestamp, now));

    // Per-field icon widths that sit outside the truncation budgets.
    let dir_icon_w = dw(glyphs.repo());
    let branch_icon_w = dw(glyphs.branch());
    let pr_icon_w = dw(glyphs.pr());

    let fixed_w = dw(&mark)
        + sep_w // separator after mark
        + dir_icon_w
        + branch.as_ref().map_or(0, |_| sep_w + branch_icon_w)
        + pr.map_or(0, |p| sep_w + pr_icon_w + dw(p))
        + msgs.as_ref().map_or(0, |m| sep_w + dw(m))
        + sep_w
        + dw(&time);

    let variable_budget = w.saturating_sub(fixed_w);
    let dir_raw_w = dw(&s.directory);
    let branch_raw_w = branch.as_ref().map_or(0, |b| dw(b));

    let (dir_budget, branch_budget) = if branch.is_some() && variable_budget > 0 {
        let total_raw = dir_raw_w + branch_raw_w;
        if total_raw <= variable_budget {
            (dir_raw_w, branch_raw_w)
        } else {
            let dir_share = variable_budget * 3 / 5;
            let branch_share = variable_budget - dir_share;
            (dir_share.min(dir_raw_w), branch_share.min(branch_raw_w))
        }
    } else {
        (variable_budget.min(dir_raw_w), 0)
    };

    let dir_text = crate::tui::columns::fit_end(&s.directory, dir_budget as u16);
    let branch_text = branch.as_ref().map(|b| modal::fit_for_modal(b, branch_budget));

    let push_sep = |spans: &mut Vec<Span<'static>>| {
        spans.push(Span::styled(sep, sep_style));
    };
    let mut meta: Vec<Span<'static>> = Vec::new();

    meta.push(Span::styled(
        mark,
        Style::default().fg(theme.agent_color(s.agent)).add_modifier(Modifier::BOLD),
    ));

    if !dir_text.is_empty() {
        push_sep(&mut meta);
        meta.push(Span::styled(format!("{}{}", glyphs.repo(), dir_text), muted));
    }

    if let Some(branch_text) = branch_text.filter(|t| !t.is_empty()) {
        push_sep(&mut meta);
        meta.push(Span::styled(format!("{}{}", glyphs.branch(), branch_text), muted));
    }

    if let Some(pr) = pr {
        push_sep(&mut meta);
        meta.push(Span::styled(
            format!("{}{}", glyphs.pr(), pr),
            Style::default().fg(theme.accent),
        ));
    }

    if let Some(msgs) = msgs {
        push_sep(&mut meta);
        meta.push(Span::styled(msgs, muted));
    }

    push_sep(&mut meta);
    meta.push(Span::styled(time, muted));

    let meta_line = Line::from(meta);
    let rule_line = Line::from(Span::styled("─".repeat(w.max(1)), sep_style));

    vec![title_line, meta_line, rule_line]
}
