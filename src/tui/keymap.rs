//! Keymap presets. The default "search" preset keeps the query always-live and
//! puts actions on arrows/PgUp-Dn/Ctrl-chords. The "modal" preset adds a
//! navigate mode where single letters act.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Command {
    Quit,
    TogglePreview,
    ScrollPreview(i16),
    JumpPreviewMatch(i16),
    ResizePreview(i8),
    Help,
    ResumeSelected { yolo: bool },
}

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

/// Resolve a key to a command that is independent of mode/query editing. These
/// chords work in both presets. Returns None if the key isn't a bound chord.
#[cfg(test)]
fn chord_action(key: &KeyEvent) -> Option<Command> {
    control_chord_action(key).or_else(|| empty_query_chord_action(key))
}

pub(super) fn control_chord_action(key: &KeyEvent) -> Option<Command> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match (key.code, ctrl) {
        (KeyCode::Char('c'), true) => Some(Command::Quit),
        (KeyCode::Char('p'), true) => Some(Command::TogglePreview),
        (KeyCode::Char('u'), true) => Some(Command::ScrollPreview(-1)),
        (KeyCode::Char('d'), true) => Some(Command::ScrollPreview(1)),
        (KeyCode::Char('b'), true) => Some(Command::JumpPreviewMatch(-1)),
        (KeyCode::Char('n'), true) => Some(Command::JumpPreviewMatch(1)),
        (KeyCode::Char('y'), true) => Some(Command::ResumeSelected { yolo: true }),
        _ => None,
    }
}

pub(super) fn empty_query_chord_action(key: &KeyEvent) -> Option<Command> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match (key.code, ctrl) {
        (KeyCode::Char('['), false) => Some(Command::ResizePreview(-1)),
        (KeyCode::Char(']'), false) => Some(Command::ResizePreview(1)),
        (KeyCode::Char('?'), false) => Some(Command::Help),
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
        assert_eq!(chord_action(&ctrl('p')), Some(Command::TogglePreview));
        assert!(matches!(chord_action(&ctrl('u')), Some(Command::ScrollPreview(n)) if n < 0));
        assert!(matches!(
            chord_action(&ctrl('n')),
            Some(Command::JumpPreviewMatch(n)) if n > 0
        ));
        assert!(matches!(
            chord_action(&ctrl('y')),
            Some(Command::ResumeSelected { yolo: true })
        ));
        assert_eq!(chord_action(&ctrl('c')), Some(Command::Quit));
    }

    #[test]
    fn bracket_resizes_and_question_helps() {
        assert_eq!(
            chord_action(&plain(KeyCode::Char('['))),
            Some(Command::ResizePreview(-1))
        );
        assert_eq!(
            chord_action(&plain(KeyCode::Char(']'))),
            Some(Command::ResizePreview(1))
        );
        assert_eq!(
            chord_action(&plain(KeyCode::Char('?'))),
            Some(Command::Help)
        );
    }

    #[test]
    fn preset_parsing() {
        assert_eq!(Preset::parse("modal"), Preset::Modal);
        assert_eq!(Preset::parse("search"), Preset::Search);
        assert_eq!(Preset::parse("nonsense"), Preset::Search);
        assert_eq!("modal".parse::<Preset>().unwrap(), Preset::Modal);
    }
}
