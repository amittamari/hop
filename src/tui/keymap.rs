//! Keymap for the single live-search interaction model. Typing always edits the
//! query; navigation lives on the arrows and preview actions on Ctrl-chords, so
//! no key ever does double duty and there are no modes to track.
//!
//! The Ctrl-chord actions are configurable from `config.toml`'s `[keybindings]`
//! table (see `Keymap::from_config`). Bindings keep the "Ctrl-chord only"
//! invariant: every chord must include Ctrl so it can never collide with query
//! editing. Cursor movement and deletion within the query use the standard
//! arrow/Home/End/Backspace/Delete keys, handled directly by `App::handle_key`.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Command {
    Quit,
    TogglePreview,
    ScrollPreview(i16),
    JumpPreviewMatch(i16),
    ResizePreview(i8),
}

/// A resolved key chord: the modifier set plus the key code. Char codes are
/// stored lowercase so they match the events crossterm emits for Ctrl+letter.
type Chord = (KeyModifiers, KeyCode);

/// One configurable chord action with a stable config name and a default chord.
struct ChordSpec {
    /// The `[keybindings]` key users write in `config.toml`, e.g. `toggle_preview`.
    name: &'static str,
    default: Chord,
    command: Command,
}

/// The canonical list of configurable chords. The `name` strings are the stable
/// public API for `config.toml`; keep them in sync with the README.
fn chord_specs() -> Vec<ChordSpec> {
    let ctrl = KeyModifiers::CONTROL;
    vec![
        ChordSpec {
            name: "quit",
            default: (ctrl, KeyCode::Char('c')),
            command: Command::Quit,
        },
        ChordSpec {
            name: "toggle_preview",
            default: (ctrl, KeyCode::Char('p')),
            command: Command::TogglePreview,
        },
        ChordSpec {
            name: "scroll_preview_up",
            default: (ctrl, KeyCode::Char('u')),
            command: Command::ScrollPreview(-1),
        },
        ChordSpec {
            name: "scroll_preview_down",
            default: (ctrl, KeyCode::Char('d')),
            command: Command::ScrollPreview(1),
        },
        ChordSpec {
            name: "jump_match_prev",
            default: (ctrl, KeyCode::Char('b')),
            command: Command::JumpPreviewMatch(-1),
        },
        ChordSpec {
            name: "jump_match_next",
            default: (ctrl, KeyCode::Char('n')),
            command: Command::JumpPreviewMatch(1),
        },
        ChordSpec {
            name: "resize_preview_smaller",
            default: (ctrl, KeyCode::Left),
            command: Command::ResizePreview(-1),
        },
        ChordSpec {
            name: "resize_preview_larger",
            default: (ctrl, KeyCode::Right),
            command: Command::ResizePreview(1),
        },
    ]
}

/// Resolved Ctrl-chord bindings. Built from defaults, optionally overlaid with
/// `config.keybindings`. Drives both the action lookup and the displayed key
/// labels, so the help overlay always reflects the active bindings.
#[derive(Debug, Clone)]
pub struct Keymap {
    chords: Vec<(Chord, Command)>,
}

impl Default for Keymap {
    fn default() -> Self {
        Keymap::defaults()
    }
}

impl Keymap {
    /// The hardcoded default bindings.
    pub fn defaults() -> Keymap {
        let chords = chord_specs()
            .into_iter()
            .map(|s| (s.default, s.command))
            .collect();
        Keymap { chords }
    }

    /// Build a keymap from `config.keybindings`: defaults overlaid with any valid
    /// overrides. Invalid binding strings, unknown command names, and duplicate
    /// chords never fail the launch — each yields a warning and the default (or
    /// both conflicting entries) is kept. Returns the keymap plus warnings for
    /// the caller to surface before entering the alternate screen.
    pub fn from_config(overrides: &HashMap<String, String>) -> (Keymap, Vec<String>) {
        let specs = chord_specs();
        let mut warnings = Vec::new();
        let mut chords: Vec<(Chord, Command)> = Vec::with_capacity(specs.len());

        for spec in &specs {
            let chord = match overrides.get(spec.name) {
                Some(raw) => match parse_chord(raw) {
                    Ok(c) => c,
                    Err(e) => {
                        warnings.push(format!(
                            "keybinding `{}` = {raw:?}: {e}; using default",
                            spec.name
                        ));
                        spec.default
                    }
                },
                None => spec.default,
            };
            chords.push((chord, spec.command));
        }

        let known: HashSet<&str> = specs.iter().map(|s| s.name).collect();
        for key in overrides.keys() {
            if !known.contains(key.as_str()) {
                warnings.push(format!("unknown keybinding command `{key}`; ignored"));
            }
        }

        for i in 0..chords.len() {
            for j in (i + 1)..chords.len() {
                if chords[i].0 == chords[j].0 {
                    warnings.push(format!(
                        "keybinding conflict: `{}` and `{}` both map to {}",
                        specs[i].name,
                        specs[j].name,
                        format_chord(chords[i].0)
                    ));
                }
            }
        }

        (Keymap { chords }, warnings)
    }

    /// Resolve a key event to a command. Returns None if the key isn't a bound
    /// chord. Requiring Ctrl keeps these from ever colliding with query editing.
    pub(super) fn chord_action(&self, key: &KeyEvent) -> Option<Command> {
        if !key.modifiers.contains(KeyModifiers::CONTROL) {
            return None;
        }
        let code = normalize_code(key.code);
        self.chords
            .iter()
            .find(|((m, c), _)| key.modifiers.contains(*m) && *c == code)
            .map(|(_, cmd)| *cmd)
    }

    /// The chord currently bound to `command`, for display in help/footer.
    fn chord_for(&self, command: Command) -> Option<Chord> {
        self.chords
            .iter()
            .find(|(_, c)| *c == command)
            .map(|(chord, _)| *chord)
    }
}

/// Normalize a key code so stored (lowercase) chords match incoming events.
fn normalize_code(code: KeyCode) -> KeyCode {
    match code {
        KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
        other => other,
    }
}

/// Parse a binding string like `"ctrl+t"` or `"ctrl+left"` into a chord. Only
/// the Ctrl modifier is supported, and it is required (the chord-only invariant).
fn parse_chord(s: &str) -> Result<Chord, String> {
    let parts: Vec<&str> = s
        .split('+')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    let Some((key_part, mod_parts)) = parts.split_last() else {
        return Err("empty binding".to_string());
    };
    let mut mods = KeyModifiers::NONE;
    for m in mod_parts {
        match m.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods |= KeyModifiers::CONTROL,
            other => {
                return Err(format!(
                    "unsupported modifier `{other}` (only ctrl is allowed)"
                ))
            }
        }
    }
    if !mods.contains(KeyModifiers::CONTROL) {
        return Err("must include ctrl (chord-only invariant)".to_string());
    }
    Ok((mods, parse_keycode(key_part)?))
}

fn parse_keycode(s: &str) -> Result<KeyCode, String> {
    let lower = s.to_ascii_lowercase();
    Ok(match lower.as_str() {
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "tab" => KeyCode::Tab,
        "enter" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "backspace" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        "space" => KeyCode::Char(' '),
        _ => {
            let mut chars = lower.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => KeyCode::Char(c),
                _ => return Err(format!("unrecognized key `{s}`")),
            }
        }
    })
}

/// Human-readable label for a chord, e.g. "Ctrl+P", "Ctrl+←".
fn format_chord((mods, code): Chord) -> String {
    let mut out = String::new();
    if mods.contains(KeyModifiers::CONTROL) {
        out.push_str("Ctrl+");
    }
    out.push_str(&format_keycode(code));
    out
}

fn format_keycode(code: KeyCode) -> String {
    match code {
        KeyCode::Left => "←".to_string(),
        KeyCode::Right => "→".to_string(),
        KeyCode::Up => "↑".to_string(),
        KeyCode::Down => "↓".to_string(),
        KeyCode::PageUp => "PgUp".to_string(),
        KeyCode::PageDown => "PgDn".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => c.to_ascii_uppercase().to_string(),
        other => format!("{other:?}"),
    }
}

/// One user-facing keybinding row. This is the single source of truth that
/// drives both the help overlay and the main-view footer hints, so adding a
/// binding here surfaces it everywhere without hand-editing strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// Display label for the key(s), e.g. "Ctrl+P", "↑/↓", "Ctrl+←/Ctrl+→".
    pub keys: String,
    /// Section heading the row belongs to.
    pub group: &'static str,
    /// Human-readable description of what the key does.
    pub label: &'static str,
    /// Whether this row appears in the compact main-view footer.
    pub primary: bool,
}

/// The canonical keybinding catalog, resolved against `keymap`. Ordered for
/// display: rows are grouped by `group` in the order they first appear. Rows for
/// configurable chords derive their key label from the active keymap, so the
/// help overlay always reflects user overrides. The reachability test in
/// `tui::mod` guards against drift between this catalog and `chord_action`.
pub fn bindings(keymap: &Keymap) -> Vec<Binding> {
    let chord = |cmd: Command| keymap.chord_for(cmd).map(format_chord).unwrap_or_default();
    let pair = |a: Command, b: Command| format!("{}/{}", chord(a), chord(b));
    let row = |keys: String, group, label, primary| Binding {
        keys,
        group,
        label,
        primary,
    };
    vec![
        // Navigation
        row("↑/↓".into(), "Navigation", "move selection", false),
        row("PgUp/PgDn".into(), "Navigation", "page list", false),
        row(
            pair(Command::ScrollPreview(-1), Command::ScrollPreview(1)),
            "Navigation",
            "scroll preview",
            false,
        ),
        row(
            pair(Command::JumpPreviewMatch(1), Command::JumpPreviewMatch(-1)),
            "Navigation",
            "preview matches",
            false,
        ),
        // Preview
        row(
            chord(Command::TogglePreview),
            "Preview",
            "toggle preview",
            false,
        ),
        row(
            pair(Command::ResizePreview(-1), Command::ResizePreview(1)),
            "Preview",
            "resize preview",
            false,
        ),
        // Search Editing
        row("←/→".into(), "Search Editing", "move cursor", false),
        row("Home/End".into(), "Search Editing", "jump cursor", false),
        row("Backspace".into(), "Search Editing", "delete left", false),
        row("Delete".into(), "Search Editing", "delete at cursor", false),
        // Actions
        row("type".into(), "Actions", "search", true),
        row("Enter".into(), "Actions", "resume", true),
        row("Tab".into(), "Actions", "autocomplete keyword", false),
        row("?".into(), "Actions", "toggle help", true),
        row("Esc".into(), "Actions", "clear query / quit", true),
        row(chord(Command::Quit), "Actions", "quit", false),
    ]
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
        let km = Keymap::defaults();
        assert_eq!(
            km.chord_action(&ctrl(KeyCode::Char('p'))),
            Some(Command::TogglePreview)
        );
        assert!(matches!(
            km.chord_action(&ctrl(KeyCode::Char('u'))),
            Some(Command::ScrollPreview(n)) if n < 0
        ));
        assert!(matches!(
            km.chord_action(&ctrl(KeyCode::Char('n'))),
            Some(Command::JumpPreviewMatch(n)) if n > 0
        ));
    }

    #[test]
    fn ctrl_arrows_resize_preview() {
        let km = Keymap::defaults();
        assert_eq!(
            km.chord_action(&ctrl(KeyCode::Left)),
            Some(Command::ResizePreview(-1))
        );
        assert_eq!(
            km.chord_action(&ctrl(KeyCode::Right)),
            Some(Command::ResizePreview(1))
        );
    }

    #[test]
    fn uppercase_char_events_still_match() {
        // Some terminals report Ctrl+letter with an uppercase code; the lookup
        // normalizes before matching the lowercase-stored chord.
        let km = Keymap::defaults();
        assert_eq!(
            km.chord_action(&ctrl(KeyCode::Char('P'))),
            Some(Command::TogglePreview)
        );
    }

    #[test]
    fn plain_keys_are_not_chords() {
        let km = Keymap::defaults();
        assert_eq!(km.chord_action(&plain(KeyCode::Char('p'))), None);
        assert_eq!(km.chord_action(&plain(KeyCode::Left)), None);
        assert_eq!(km.chord_action(&plain(KeyCode::Char('?'))), None);
    }

    #[test]
    fn parse_chord_accepts_letters_and_named_keys() {
        assert_eq!(
            parse_chord("ctrl+t"),
            Ok((KeyModifiers::CONTROL, KeyCode::Char('t')))
        );
        assert_eq!(
            parse_chord("Ctrl+Left"),
            Ok((KeyModifiers::CONTROL, KeyCode::Left))
        );
        assert_eq!(
            parse_chord("ctrl + j"),
            Ok((KeyModifiers::CONTROL, KeyCode::Char('j')))
        );
    }

    #[test]
    fn parse_chord_rejects_non_ctrl_and_garbage() {
        assert!(parse_chord("t").is_err());
        assert!(parse_chord("alt+t").is_err());
        assert!(parse_chord("ctrl+nope").is_err());
        assert!(parse_chord("").is_err());
    }

    #[test]
    fn from_config_overlays_valid_binding() {
        let mut overrides = HashMap::new();
        overrides.insert("toggle_preview".to_string(), "ctrl+t".to_string());
        let (km, warnings) = Keymap::from_config(&overrides);
        assert!(
            warnings.is_empty(),
            "valid override should not warn: {warnings:?}"
        );
        assert_eq!(
            km.chord_action(&ctrl(KeyCode::Char('t'))),
            Some(Command::TogglePreview)
        );
        // The default chord no longer triggers toggle.
        assert_eq!(km.chord_action(&ctrl(KeyCode::Char('p'))), None);
    }

    #[test]
    fn from_config_falls_back_and_warns_on_bad_input() {
        let mut overrides = HashMap::new();
        overrides.insert("toggle_preview".to_string(), "t".to_string()); // missing ctrl
        overrides.insert("bogus_command".to_string(), "ctrl+x".to_string());
        let (km, warnings) = Keymap::from_config(&overrides);
        assert_eq!(warnings.len(), 2, "got: {warnings:?}");
        assert!(warnings.iter().any(|w| w.contains("toggle_preview")));
        assert!(warnings.iter().any(|w| w.contains("bogus_command")));
        // Falls back to the default chord.
        assert_eq!(
            km.chord_action(&ctrl(KeyCode::Char('p'))),
            Some(Command::TogglePreview)
        );
    }

    #[test]
    fn from_config_detects_conflicts() {
        let mut overrides = HashMap::new();
        // Bind toggle_preview onto scroll_preview_down's default chord (ctrl+d).
        overrides.insert("toggle_preview".to_string(), "ctrl+d".to_string());
        let (_km, warnings) = Keymap::from_config(&overrides);
        assert!(
            warnings.iter().any(|w| w.contains("conflict")),
            "expected a conflict warning, got: {warnings:?}"
        );
    }

    #[test]
    fn bindings_reflect_overrides() {
        let mut overrides = HashMap::new();
        overrides.insert("toggle_preview".to_string(), "ctrl+t".to_string());
        let (km, _) = Keymap::from_config(&overrides);
        let table = bindings(&km);
        let toggle = table.iter().find(|b| b.label == "toggle preview").unwrap();
        assert_eq!(toggle.keys, "Ctrl+T");
    }

    #[test]
    fn bindings_table_is_well_formed() {
        let table = bindings(&Keymap::defaults());
        assert!(!table.is_empty(), "bindings table must not be empty");
        for b in &table {
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
