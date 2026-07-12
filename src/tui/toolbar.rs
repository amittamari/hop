//! Simple-mode search toolbar: guided Scope + Sort controls shown under the
//! query input, in the spirit of the Codex `/resume` picker's filter/sort bar.
//!
//! The toolbar is a structured surface over the same search the raw DSL drives:
//! Scope manages the `repo:` filter and Sort selects the result ordering, so a
//! user never has to type field syntax for the common cases. Typing always edits
//! the query (see `App::handle_key`); Tab moves `Focus` between the query cursor
//! and each control, and Left/Right adjust the focused control.

use crate::query::SortOrder;
use crate::tui::theme::Theme;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// Repo scope for simple mode. `ThisRepo` injects the launch repo's `repo:` slug
/// (when known); `All` searches every repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    ThisRepo,
    All,
}

impl Scope {
    pub fn label(self) -> &'static str {
        match self {
            Scope::ThisRepo => "This repo",
            Scope::All => "All",
        }
    }

    /// Toggle between the two scopes (Left/Right behave identically with two
    /// values).
    pub fn toggled(self) -> Scope {
        match self {
            Scope::ThisRepo => Scope::All,
            Scope::All => Scope::ThisRepo,
        }
    }
}

/// What Left/Right currently act on in simple mode. `Query` means the text cursor
/// (the default); the others focus a toolbar control. Tab cycles through them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Query,
    Scope,
    Sort,
}

impl Focus {
    /// Next focus target (Tab). When the launch repo is unknown the Scope control
    /// is hidden, so it is skipped.
    pub fn next(self, has_repo: bool) -> Focus {
        match self {
            Focus::Query if has_repo => Focus::Scope,
            Focus::Query => Focus::Sort,
            Focus::Scope => Focus::Sort,
            Focus::Sort => Focus::Query,
        }
    }

    /// Previous focus target (Shift+Tab).
    pub fn prev(self, has_repo: bool) -> Focus {
        match self {
            Focus::Query => Focus::Sort,
            Focus::Scope => Focus::Query,
            Focus::Sort if has_repo => Focus::Scope,
            Focus::Sort => Focus::Query,
        }
    }
}

/// Build the toolbar line. The focused control is rendered reversed/bold so the
/// user can see what Left/Right will change. `has_repo` hides the Scope control
/// when the launch directory has no resolvable repo.
pub fn line(
    scope: Scope,
    sort: SortOrder,
    focus: Focus,
    has_repo: bool,
    theme: &Theme,
) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = vec![Span::raw("  ")];
    if has_repo {
        push_control(&mut spans, "Scope", scope.label(), focus == Focus::Scope, theme);
        spans.push(Span::raw("   "));
    }
    push_control(&mut spans, "Sort", sort.label(), focus == Focus::Sort, theme);
    Line::from(spans)
}

fn push_control(
    spans: &mut Vec<Span<'static>>,
    name: &'static str,
    value: &str,
    focused: bool,
    theme: &Theme,
) {
    spans.push(Span::styled(format!("{name}: "), Style::default().fg(theme.muted)));
    let value_style = if focused {
        Style::default().fg(theme.selection_fg).bg(theme.accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.accent)
    };
    spans.push(Span::styled(format!(" {value} "), value_style));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_toggles() {
        assert_eq!(Scope::ThisRepo.toggled(), Scope::All);
        assert_eq!(Scope::All.toggled(), Scope::ThisRepo);
    }

    #[test]
    fn focus_cycles_with_repo() {
        assert_eq!(Focus::Query.next(true), Focus::Scope);
        assert_eq!(Focus::Scope.next(true), Focus::Sort);
        assert_eq!(Focus::Sort.next(true), Focus::Query);
        // Round-trip via prev.
        assert_eq!(Focus::Query.prev(true), Focus::Sort);
        assert_eq!(Focus::Sort.prev(true), Focus::Scope);
        assert_eq!(Focus::Scope.prev(true), Focus::Query);
    }

    #[test]
    fn focus_skips_scope_without_repo() {
        assert_eq!(Focus::Query.next(false), Focus::Sort);
        assert_eq!(Focus::Sort.next(false), Focus::Query);
        assert_eq!(Focus::Sort.prev(false), Focus::Query);
    }

    #[test]
    fn line_hides_scope_without_repo() {
        let theme = Theme::default();
        let with =
            render_text(line(Scope::ThisRepo, SortOrder::Recent, Focus::Query, true, &theme));
        let without =
            render_text(line(Scope::ThisRepo, SortOrder::Recent, Focus::Query, false, &theme));
        assert!(with.contains("Scope"));
        assert!(with.contains("This repo"));
        assert!(!without.contains("Scope"));
        assert!(without.contains("Sort"));
    }

    fn render_text(line: Line<'static>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }
}
