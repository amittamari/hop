//! Keymap presets. The default "search" preset keeps the query always-live and
//! puts actions on arrows/PgUp-Dn/Ctrl-chords. The "modal" preset adds a
//! navigate mode where single letters act.

use crate::tui::Action;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Search,
    Modal,
}

impl Preset {
    pub fn parse(s: &str) -> Preset {
        match s {
            "modal" => Preset::Modal,
            _ => Preset::Search,
        }
    }
}

impl std::str::FromStr for Preset {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Preset::parse(s))
    }
}

/// Resolve a key to an action that is independent of mode/query editing. These
/// chords work in both presets. Returns None if the key isn't a bound chord.
pub fn chord_action(key: &KeyEvent) -> Option<Action> {
    control_chord_action(key).or_else(|| empty_query_chord_action(key))
}

pub fn control_chord_action(key: &KeyEvent) -> Option<Action> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match (key.code, ctrl) {
        (KeyCode::Char('c'), true) => Some(Action::Quit),
        (KeyCode::Char('p'), true) => Some(Action::TogglePreview),
        (KeyCode::Char('u'), true) => Some(Action::ScrollPreview(-1)),
        (KeyCode::Char('d'), true) => Some(Action::ScrollPreview(1)),
        (KeyCode::Char('b'), true) => Some(Action::JumpPreviewMatch(-1)),
        (KeyCode::Char('n'), true) => Some(Action::JumpPreviewMatch(1)),
        (KeyCode::Char('y'), true) => Some(Action::Resume {
            index: 0,
            yolo: true,
        }), // index filled by App
        _ => None,
    }
}

pub fn empty_query_chord_action(key: &KeyEvent) -> Option<Action> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match (key.code, ctrl) {
        (KeyCode::Char('['), false) => Some(Action::ResizePreview(-1)),
        (KeyCode::Char(']'), false) => Some(Action::ResizePreview(1)),
        (KeyCode::Char('?'), false) => Some(Action::Help),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }
    fn plain(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn ctrl_chords_map() {
        assert_eq!(chord_action(&ctrl('p')), Some(Action::TogglePreview));
        assert!(matches!(chord_action(&ctrl('u')), Some(Action::ScrollPreview(n)) if n < 0));
        assert!(matches!(
            chord_action(&ctrl('n')),
            Some(Action::JumpPreviewMatch(n)) if n > 0
        ));
        assert!(matches!(
            chord_action(&ctrl('y')),
            Some(Action::Resume { yolo: true, .. })
        ));
        assert_eq!(chord_action(&ctrl('c')), Some(Action::Quit));
    }

    #[test]
    fn bracket_resizes_and_question_helps() {
        assert_eq!(
            chord_action(&plain(KeyCode::Char('['))),
            Some(Action::ResizePreview(-1))
        );
        assert_eq!(
            chord_action(&plain(KeyCode::Char(']'))),
            Some(Action::ResizePreview(1))
        );
        assert_eq!(chord_action(&plain(KeyCode::Char('?'))), Some(Action::Help));
    }

    #[test]
    fn preset_parsing() {
        assert_eq!(Preset::parse("modal"), Preset::Modal);
        assert_eq!(Preset::parse("search"), Preset::Search);
        assert_eq!(Preset::parse("nonsense"), Preset::Search);
        assert_eq!("modal".parse::<Preset>().unwrap(), Preset::Modal);
    }
}
