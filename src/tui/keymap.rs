//! Keymap for the single live-search interaction model. Typing always edits the
//! query; navigation lives on the arrows and preview actions on Ctrl-chords, so
//! no key ever does double duty and there are no modes to track.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Command {
    TogglePreview,
    ScrollPreview(i16),
    JumpPreviewMatch(i16),
    ResizePreview(i8),
}

/// One user-facing keybinding row. This is the single source of truth that
/// drives both the help overlay and the main-view footer hints, so adding a
/// binding here surfaces it everywhere without hand-editing strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    /// Display label for the key(s), e.g. "Ctrl+P", "↑/↓", "Ctrl+←/→".
    pub keys: &'static str,
    /// Section heading the row belongs to.
    pub group: &'static str,
    /// Human-readable description of what the key does.
    pub label: &'static str,
    /// Whether this row appears in the compact main-view footer.
    pub primary: bool,
}

/// The canonical keybinding catalog. Ordered for display: rows are grouped by
/// `group` in the order they first appear. Keep this in sync with
/// `control_chord_action` above and `App::handle_key`; the reachability test in
/// `tui::mod` guards against drift.
pub fn bindings() -> &'static [Binding] {
    const TABLE: &[Binding] = &[
        // Navigation
        Binding {
            keys: "↑/↓",
            group: "Navigation",
            label: "move selection",
            primary: false,
        },
        Binding {
            keys: "PgUp/PgDn",
            group: "Navigation",
            label: "page list",
            primary: false,
        },
        Binding {
            keys: "Ctrl+U/D",
            group: "Navigation",
            label: "scroll preview",
            primary: false,
        },
        Binding {
            keys: "Ctrl+N/B",
            group: "Navigation",
            label: "preview matches",
            primary: false,
        },
        // Preview
        Binding {
            keys: "Ctrl+P",
            group: "Preview",
            label: "toggle preview",
            primary: false,
        },
        Binding {
            keys: "Ctrl+←/→",
            group: "Preview",
            label: "resize preview",
            primary: false,
        },
        // Search Editing
        Binding {
            keys: "←/→",
            group: "Search Editing",
            label: "move cursor",
            primary: false,
        },
        Binding {
            keys: "Home/End",
            group: "Search Editing",
            label: "jump cursor",
            primary: false,
        },
        Binding {
            keys: "Backspace",
            group: "Search Editing",
            label: "delete left",
            primary: false,
        },
        Binding {
            keys: "Delete",
            group: "Search Editing",
            label: "delete at cursor",
            primary: false,
        },
        Binding {
            keys: "Ctrl+A/E",
            group: "Search Editing",
            label: "start / end",
            primary: false,
        },
        Binding {
            keys: "Ctrl+W",
            group: "Search Editing",
            label: "delete word",
            primary: false,
        },
        // Actions
        Binding {
            keys: "type",
            group: "Actions",
            label: "search",
            primary: true,
        },
        Binding {
            keys: "Enter",
            group: "Actions",
            label: "resume",
            primary: true,
        },
        Binding {
            keys: "Tab",
            group: "Actions",
            label: "autocomplete keyword",
            primary: false,
        },
        Binding {
            keys: "?",
            group: "Actions",
            label: "toggle help",
            primary: true,
        },
        Binding {
            keys: "Esc",
            group: "Actions",
            label: "clear query / quit",
            primary: true,
        },
        Binding {
            keys: "Ctrl+C",
            group: "Actions",
            label: "quit",
            primary: false,
        },
    ];
    TABLE
}

/// Resolve a Ctrl-chord to a command. Returns None if the key isn't a bound
/// chord. Requiring Ctrl keeps these from ever colliding with query editing.
/// Line-editing chords (Ctrl+A/E/W) are handled directly by `App::handle_key`.
pub(super) fn control_chord_action(key: &KeyEvent) -> Option<Command> {
    if !key.modifiers.contains(KeyModifiers::CONTROL) {
        return None;
    }
    match key.code {
        KeyCode::Char('p') => Some(Command::TogglePreview),
        KeyCode::Char('u') => Some(Command::ScrollPreview(-1)),
        KeyCode::Char('d') => Some(Command::ScrollPreview(1)),
        KeyCode::Char('b') => Some(Command::JumpPreviewMatch(-1)),
        KeyCode::Char('n') => Some(Command::JumpPreviewMatch(1)),
        KeyCode::Left => Some(Command::ResizePreview(-1)),
        KeyCode::Right => Some(Command::ResizePreview(1)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }
    fn plain(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn ctrl_chords_map() {
        assert_eq!(
            control_chord_action(&ctrl(KeyCode::Char('p'))),
            Some(Command::TogglePreview)
        );
        assert!(matches!(
            control_chord_action(&ctrl(KeyCode::Char('u'))),
            Some(Command::ScrollPreview(n)) if n < 0
        ));
        assert!(matches!(
            control_chord_action(&ctrl(KeyCode::Char('n'))),
            Some(Command::JumpPreviewMatch(n)) if n > 0
        ));
    }

    #[test]
    fn ctrl_arrows_resize_preview() {
        assert_eq!(
            control_chord_action(&ctrl(KeyCode::Left)),
            Some(Command::ResizePreview(-1))
        );
        assert_eq!(
            control_chord_action(&ctrl(KeyCode::Right)),
            Some(Command::ResizePreview(1))
        );
    }

    #[test]
    fn plain_keys_are_not_chords() {
        assert_eq!(control_chord_action(&plain(KeyCode::Char('p'))), None);
        assert_eq!(control_chord_action(&plain(KeyCode::Left)), None);
        assert_eq!(control_chord_action(&plain(KeyCode::Char('?'))), None);
    }

    #[test]
    fn bindings_table_is_well_formed() {
        let table = bindings();
        assert!(!table.is_empty(), "bindings table must not be empty");
        for b in table {
            assert!(!b.keys.is_empty(), "binding keys must be non-empty");
            assert!(!b.label.is_empty(), "binding label must be non-empty");
            assert!(!b.group.is_empty(), "binding group must be non-empty");
        }
        // At least one binding is flagged primary (footer subset).
        assert!(
            table.iter().any(|b| b.primary),
            "need at least one primary binding"
        );
    }
}
