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
}

impl App {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            mode: Mode::Main,
            yolo_supported: Vec::new(),
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

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.kind == KeyEventKind::Release {
            return Action::None; // ignore key-release (Windows)
        }
        // Ctrl+C always quits, in any mode.
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit;
        }

        match self.mode {
            Mode::YoloModal { index, yolo } => match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::Main; // close, choose nothing
                    Action::None
                }
                KeyCode::Tab => {
                    self.mode = Mode::YoloModal { index, yolo: !yolo };
                    Action::None
                }
                KeyCode::Enter => {
                    self.mode = Mode::Main;
                    Action::Resume { index, yolo }
                }
                _ => Action::None,
            },
            Mode::Main => match (key.code, key.modifiers) {
                (KeyCode::Esc, _) => Action::Quit,
                (KeyCode::Down, _) => {
                    if !self.results.is_empty() {
                        self.selected = (self.selected + 1).min(self.results.len() - 1);
                    }
                    Action::None
                }
                (KeyCode::Up, _) => {
                    self.selected = self.selected.saturating_sub(1);
                    Action::None
                }
                (KeyCode::Enter, _) => self.activate(false),
                (KeyCode::Tab, _) => self.activate(true),
                (KeyCode::Backspace, _) => {
                    self.query.pop();
                    Action::Search
                }
                (KeyCode::Char(c), m) if !m.contains(KeyModifiers::CONTROL) => {
                    self.query.push(c);
                    Action::Search
                }
                _ => Action::None,
            },
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
}
