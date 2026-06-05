//! Keymap for the single live-search interaction model. Typing always edits the
//! query; navigation lives on the arrows and preview actions on Ctrl-chords, so
//! no key ever does double duty and there are no modes to track.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Command {
    Quit,
    TogglePreview,
    ScrollPreview(i16),
    JumpPreviewMatch(i16),
    ResizePreview(i8),
}

/// Resolve a Ctrl-chord to a command. Returns None if the key isn't a bound
/// chord. Requiring Ctrl keeps these from ever colliding with query editing.
/// Line-editing chords (Ctrl+A/E/W) are handled directly by `App::handle_key`.
pub(super) fn control_chord_action(key: &KeyEvent) -> Option<Command> {
    if !key.modifiers.contains(KeyModifiers::CONTROL) {
        return None;
    }
    match key.code {
        KeyCode::Char('c') => Some(Command::Quit),
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
        assert_eq!(
            control_chord_action(&ctrl(KeyCode::Char('c'))),
            Some(Command::Quit)
        );
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
}
