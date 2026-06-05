pub mod help;
pub mod keymap;
pub mod preview;
pub mod results_list;
pub mod theme;
pub mod view;

use crate::core::Session;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// What the run loop should do after a key event.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
    /// Query changed; the loop should (debounced) re-search.
    Search,
    /// Resume the selected session.
    Resume { index: usize, yolo: bool },
    /// Scroll the preview pane.
    ScrollPreview(i16),
    /// Grow/shrink the preview split (+1 grow, -1 shrink).
    ResizePreview(i8),
    /// Toggle preview visibility.
    TogglePreview,
    /// Toggle the help overlay.
    Help,
}

#[derive(Debug, PartialEq, Eq)]
enum Mode {
    Main,
    /// Yolo confirmation for the pending session index; `yolo` is the toggle.
    YoloModal { index: usize, yolo: bool },
}

pub struct App {
    query: String,
    results: Vec<Session>,
    selected: usize,
    mode: Mode,
    /// Set by the loop so the App knows which agents need a yolo prompt.
    yolo_supported: Vec<bool>,
    preview_visible: bool,
    preview_width_pct: u16,
    preview_scroll: u16,
    help_open: bool,
    keymap: keymap::Preset,
}

impl App {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            mode: Mode::Main,
            yolo_supported: Vec::new(),
            preview_visible: true,
            preview_width_pct: 50,
            preview_scroll: 0,
            help_open: false,
            keymap: keymap::Preset::Search,
        }
    }

    pub fn query(&self) -> &str {
        &self.query
    }
    pub fn set_query(&mut self, q: String) {
        self.query = q;
    }
    pub fn results(&self) -> &[Session] {
        &self.results
    }
    pub fn selected(&self) -> usize {
        self.selected
    }
    pub fn modal_open(&self) -> bool {
        matches!(self.mode, Mode::YoloModal { .. })
    }

    pub fn set_results(&mut self, results: Vec<Session>) {
        // mark which rows support yolo (test default: Claude/Codex both do)
        self.yolo_supported = results.iter().map(|_| true).collect();
        self.results = results;
        if self.selected >= self.results.len() {
            self.selected = self.results.len().saturating_sub(1);
        }
    }

    /// Test/helper: directly mark whether the row's agent supports yolo.
    pub fn set_yolo_supported(&mut self, flags: Vec<bool>) {
        self.yolo_supported = flags;
    }

    pub fn open_yolo_modal(&mut self) {
        self.mode = Mode::YoloModal {
            index: self.selected,
            yolo: false,
        };
    }

    pub fn preview_visible(&self) -> bool { self.preview_visible }
    pub fn preview_width_pct(&self) -> u16 { self.preview_width_pct }
    pub fn preview_scroll(&self) -> u16 { self.preview_scroll }
    pub fn help_open(&self) -> bool { self.help_open }
    pub fn keymap_preset(&self) -> keymap::Preset { self.keymap }
    pub fn set_keymap(&mut self, p: keymap::Preset) { self.keymap = p; }
    pub fn set_preview(&mut self, visible: bool, width_pct: u16) {
        self.preview_visible = visible;
        self.preview_width_pct = width_pct.clamp(20, 80);
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.kind == KeyEventKind::Release {
            return Action::None; // ignore key-release (Windows)
        }
        // Help overlay swallows keys (Esc/? close it).
        if self.help_open {
            if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') {
                self.help_open = false;
            }
            return Action::None;
        }
        // Ctrl+C always quits.
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit;
        }
        // Yolo confirmation modal.
        if let Mode::YoloModal { index, yolo } = self.mode {
            return match key.code {
                KeyCode::Esc => { self.mode = Mode::Main; Action::None }
                KeyCode::Tab => { self.mode = Mode::YoloModal { index, yolo: !yolo }; Action::None }
                KeyCode::Enter => { self.mode = Mode::Main; Action::Resume { index, yolo } }
                _ => Action::None,
            };
        }
        // `[`, `]`, `?` act as chords only when the query is empty (else they type).
        let ambiguous = matches!(key.code, KeyCode::Char('[' | ']' | '?'))
            && !key.modifiers.contains(KeyModifiers::CONTROL);
        if ambiguous && !self.query.is_empty() {
            // fall through to query editing below
        } else if let Some(act) = keymap::chord_action(&key) {
            // Unambiguous chords (Ctrl-chords, paging, and brackets/? when query empty).
            return self.apply_chord(act);
        }
        // Main search handling.
        match key.code {
            KeyCode::Esc => Action::Quit,
            KeyCode::Down => {
                if !self.results.is_empty() {
                    self.selected = (self.selected + 1).min(self.results.len() - 1);
                }
                self.preview_scroll = 0;
                Action::None
            }
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                self.preview_scroll = 0;
                Action::None
            }
            KeyCode::Enter => self.activate(false),
            KeyCode::Tab => {
                if let Some(completed) = crate::query::autocomplete(&self.query) {
                    self.query = completed;
                    self.preview_scroll = 0;
                    Action::Search
                } else {
                    Action::None
                }
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.preview_scroll = 0;
                Action::Search
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.query.push(c);
                self.preview_scroll = 0;
                Action::Search
            }
            _ => Action::None,
        }
    }

    fn apply_chord(&mut self, act: Action) -> Action {
        match act {
            Action::TogglePreview => {
                self.preview_visible = !self.preview_visible;
                Action::None
            }
            Action::ResizePreview(d) => {
                let next = self.preview_width_pct as i32 + (d as i32) * 5;
                self.preview_width_pct = next.clamp(20, 80) as u16;
                Action::None
            }
            Action::ScrollPreview(d) => {
                let next = self.preview_scroll as i32 + d as i32;
                self.preview_scroll = next.max(0) as u16;
                Action::None
            }
            Action::Help => { self.help_open = true; Action::None }
            Action::Resume { yolo, .. } => {
                if self.results.is_empty() { Action::None }
                else { Action::Resume { index: self.selected, yolo } }
            }
            other => other,
        }
    }

    /// Enter (yolo=false) or Tab (yolo=true). If the agent supports yolo and the
    /// caller didn't force it, open the confirmation modal; else resume directly.
    fn activate(&mut self, force_yolo: bool) -> Action {
        if self.results.is_empty() {
            return Action::None;
        }
        let idx = self.selected;
        let supports = self.yolo_supported.get(idx).copied().unwrap_or(false);
        if force_yolo {
            return Action::Resume { index: idx, yolo: true };
        }
        if supports {
            self.mode = Mode::YoloModal { index: idx, yolo: false };
            Action::None
        } else {
            Action::Resume { index: idx, yolo: false }
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{AgentId, Session};
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn sess(id: &str) -> Session {
        Session {
            id: id.into(), agent: AgentId::Claude, title: id.into(),
            directory: "/d".into(), timestamp: 1, content: String::new(),
            message_count: 0, mtime: 0, yolo: false,
            branch: None, repo_url: None,
        }
    }

    fn app_with(n: usize) -> App {
        let mut app = App::new();
        app.set_results((0..n).map(|i| sess(&format!("s{i}"))).collect());
        app
    }

    #[test]
    fn esc_quits_main_view() {
        let mut app = app_with(3);
        assert_eq!(app.handle_key(key(KeyCode::Esc)), Action::Quit);
    }

    #[test]
    fn ctrl_c_quits() {
        let mut app = app_with(3);
        let k = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(app.handle_key(k), Action::Quit);
    }

    #[test]
    fn esc_closes_modal_without_quitting() {
        let mut app = app_with(3);
        app.open_yolo_modal();
        assert!(app.modal_open());
        assert_eq!(app.handle_key(key(KeyCode::Esc)), Action::None);
        assert!(!app.modal_open());
    }

    #[test]
    fn down_moves_selection() {
        let mut app = app_with(3);
        assert_eq!(app.selected(), 0);
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.selected(), 1);
        // clamps at the end
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.selected(), 2);
    }

    #[test]
    fn typing_updates_query_and_requests_search() {
        let mut app = app_with(0);
        assert_eq!(app.handle_key(key(KeyCode::Char('a'))), Action::Search);
        assert_eq!(app.handle_key(key(KeyCode::Char('b'))), Action::Search);
        assert_eq!(app.query(), "ab");
        assert_eq!(app.handle_key(key(KeyCode::Backspace)), Action::Search);
        assert_eq!(app.query(), "a");
    }

    #[test]
    fn enter_on_yolo_agent_opens_modal_then_confirms_resume() {
        let mut app = app_with(1); // Claude supports yolo
        assert_eq!(app.handle_key(key(KeyCode::Enter)), Action::None);
        assert!(app.modal_open());
        // Tab toggles yolo, Enter confirms
        app.handle_key(key(KeyCode::Tab));
        match app.handle_key(key(KeyCode::Enter)) {
            Action::Resume { yolo, .. } => assert!(yolo),
            other => panic!("expected resume, got {other:?}"),
        }
    }

    #[test]
    fn ctrl_p_toggles_preview() {
        let mut app = app_with(1);
        assert!(app.preview_visible());
        app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
        assert!(!app.preview_visible());
    }

    #[test]
    fn brackets_resize_preview_width() {
        let mut app = app_with(1);
        let before = app.preview_width_pct();
        app.handle_key(key(KeyCode::Char(']')));
        assert!(app.preview_width_pct() > before);
        app.handle_key(key(KeyCode::Char('[')));
        app.handle_key(key(KeyCode::Char('[')));
        assert!(app.preview_width_pct() < before);
    }

    #[test]
    fn question_toggles_help_and_esc_closes_it() {
        let mut app = app_with(1);
        app.handle_key(key(KeyCode::Char('?')));
        assert!(app.help_open());
        assert_eq!(app.handle_key(key(KeyCode::Esc)), Action::None);
        assert!(!app.help_open());
    }

    #[test]
    fn ctrl_y_resumes_selected_with_yolo() {
        let mut app = app_with(2);
        app.handle_key(key(KeyCode::Down)); // select index 1
        match app.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL)) {
            Action::Resume { index, yolo } => { assert_eq!(index, 1); assert!(yolo); }
            other => panic!("expected yolo resume, got {other:?}"),
        }
    }

    #[test]
    fn brackets_type_into_query_when_query_nonempty() {
        let mut app = app_with(1);
        app.handle_key(key(KeyCode::Char('a')));
        let act = app.handle_key(key(KeyCode::Char('[')));
        assert_eq!(act, Action::Search);
        assert_eq!(app.query(), "a[");
    }

    #[test]
    fn tab_autocompletes_keyword_value() {
        let mut app = app_with(1);
        for c in "agent:cl".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        assert_eq!(app.handle_key(key(KeyCode::Tab)), Action::Search);
        assert_eq!(app.query(), "agent:claude");
    }
}
