pub mod help;
pub mod keymap;
pub mod preview;
pub mod results_list;
pub mod theme;
pub mod view;

use crate::core::SessionSummary;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// What the run loop should do after the app handles a key event.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// The app handled the key locally, or ignored it.
    None,
    Quit,
    /// Query changed; the loop should (debounced) re-search.
    Search,
    /// Resume the selected session.
    Resume {
        index: usize,
        yolo: bool,
    },
}

#[derive(Debug, PartialEq, Eq)]
enum Mode {
    Main,
    /// Yolo confirmation for the pending session index; `yolo` is the toggle.
    YoloModal {
        index: usize,
        yolo: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionMode {
    Search,
    Navigate,
}

impl InteractionMode {
    pub fn label(self) -> &'static str {
        match self {
            InteractionMode::Search => "SEARCH",
            InteractionMode::Navigate => "NAV",
        }
    }
}

pub struct App {
    query: String,
    query_cursor: usize,
    results: Vec<SessionSummary>,
    selected: usize,
    mode: Mode,
    /// Set by the loop so the App knows which agents need a yolo prompt.
    yolo_supported: Vec<bool>,
    preview_visible: bool,
    preview_width_pct: u16,
    preview_header_visible: bool,
    preview_scroll: u16,
    help_open: bool,
    keymap: keymap::Preset,
    navigate: bool,
    list_page_size: usize,
    preview_scroll_step: u16,
    preview_matches: Vec<u16>,
    preview_match_index: usize,
}

impl App {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            query_cursor: 0,
            results: Vec::new(),
            selected: 0,
            mode: Mode::Main,
            yolo_supported: Vec::new(),
            preview_visible: true,
            preview_width_pct: 50,
            preview_header_visible: true,
            preview_scroll: 0,
            help_open: false,
            keymap: keymap::Preset::Search,
            navigate: false,
            list_page_size: 10,
            preview_scroll_step: 8,
            preview_matches: Vec::new(),
            preview_match_index: 0,
        }
    }

    pub fn query(&self) -> &str {
        &self.query
    }
    pub fn query_cursor(&self) -> usize {
        self.query_cursor
    }
    pub fn set_query(&mut self, q: String) {
        self.query = q;
        self.query_cursor = self.query.len();
    }
    pub fn results(&self) -> &[SessionSummary] {
        &self.results
    }
    pub fn selected(&self) -> usize {
        self.selected
    }
    pub fn modal_open(&self) -> bool {
        matches!(self.mode, Mode::YoloModal { .. })
    }
    pub fn interaction_mode(&self) -> InteractionMode {
        if self.keymap == keymap::Preset::Modal && self.navigate {
            InteractionMode::Navigate
        } else {
            InteractionMode::Search
        }
    }
    pub fn yolo_modal(&self) -> Option<(usize, bool)> {
        match self.mode {
            Mode::YoloModal { index, yolo } => Some((index, yolo)),
            Mode::Main => None,
        }
    }

    pub fn set_results(&mut self, results: Vec<SessionSummary>) {
        self.yolo_supported = results.iter().map(|_| false).collect();
        self.results = results;
        if self.selected >= self.results.len() {
            self.selected = self.results.len().saturating_sub(1);
        }
    }

    pub fn set_results_with_yolo(
        &mut self,
        results: Vec<SessionSummary>,
        yolo_supported: Vec<bool>,
    ) {
        self.yolo_supported = yolo_supported;
        self.results = results;
        self.yolo_supported.resize(self.results.len(), false);
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

    pub fn open_yolo_modal_with(&mut self, yolo: bool) {
        self.mode = Mode::YoloModal {
            index: self.selected,
            yolo,
        };
    }

    pub fn preview_visible(&self) -> bool {
        self.preview_visible
    }
    pub fn preview_width_pct(&self) -> u16 {
        self.preview_width_pct
    }
    pub fn preview_header_visible(&self) -> bool {
        self.preview_header_visible
    }
    pub fn preview_scroll(&self) -> u16 {
        self.preview_scroll
    }
    pub fn help_open(&self) -> bool {
        self.help_open
    }
    pub fn keymap_preset(&self) -> keymap::Preset {
        self.keymap
    }
    pub fn set_keymap(&mut self, p: keymap::Preset) {
        self.keymap = p;
    }
    pub fn toggle_keymap(&mut self) {
        match self.keymap {
            keymap::Preset::Search => {
                self.set_keymap(keymap::Preset::Modal);
                self.navigate = true;
            },
            keymap::Preset::Modal => {
                self.set_keymap(keymap::Preset::Search);
                self.navigate = false;
            },
        }
    }
    pub fn set_preview(&mut self, visible: bool, width_pct: u16) {
        self.preview_visible = visible;
        self.preview_width_pct = width_pct.clamp(20, 80);
    }
    pub fn set_preview_header(&mut self, visible: bool) {
        self.preview_header_visible = visible;
    }
    pub fn set_viewport_metrics(&mut self, list_rows_height: u16, preview_height: u16) {
        self.list_page_size = usize::from(list_rows_height.saturating_sub(1).max(1));
        self.preview_scroll_step = preview_height.saturating_sub(1).max(1);
    }
    pub fn set_preview_matches(&mut self, matches: Vec<usize>) {
        self.preview_matches = matches
            .into_iter()
            .map(|line| line.min(u16::MAX as usize) as u16)
            .collect();
        self.preview_match_index = 0;
        self.preview_scroll = self.preview_matches.first().copied().unwrap_or(0);
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.kind == KeyEventKind::Release {
            return Action::None; // ignore key-release (Windows)
        }
        // Ctrl+C always quits, even from overlays and modals.
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit;
        }
        // Help overlay swallows keys (Esc/? close it).
        if self.help_open {
            if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') {
                self.help_open = false;
            }
            return Action::None;
        }
        // Yolo confirmation modal.
        if let Mode::YoloModal { index, yolo } = self.mode {
            return match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::Main;
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
            };
        }
        if let Some(act) = keymap::control_chord_action(&key) {
            return self.apply_command(act);
        }
        // Modal preset: navigate mode consumes letter keys.
        if self.keymap == keymap::Preset::Modal && self.navigate {
            return self.handle_navigate(key);
        }
        // Main search handling.
        match key.code {
            KeyCode::Esc => {
                if self.keymap == keymap::Preset::Modal {
                    self.navigate = true; // leave query → navigate
                    Action::None
                } else {
                    Action::Quit
                }
            }
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
            KeyCode::PageDown => {
                if !self.results.is_empty() {
                    self.selected =
                        (self.selected + self.list_page_size).min(self.results.len() - 1);
                }
                self.preview_scroll = 0;
                Action::None
            }
            KeyCode::PageUp => {
                self.selected = self.selected.saturating_sub(self.list_page_size);
                self.preview_scroll = 0;
                Action::None
            }
            KeyCode::Enter => self.activate(false),
            KeyCode::Tab => {
                if let Some(completed) = crate::query::autocomplete(&self.query) {
                    self.query = completed;
                    self.query_cursor = self.query.len();
                    self.preview_scroll = 0;
                    Action::Search
                } else {
                    Action::None
                }
            }
            KeyCode::Left => {
                self.move_query_cursor_left();
                Action::None
            }
            KeyCode::Right => {
                self.move_query_cursor_right();
                Action::None
            }
            KeyCode::Home => {
                self.query_cursor = 0;
                Action::None
            }
            KeyCode::End => {
                self.query_cursor = self.query.len();
                Action::None
            }
            KeyCode::Delete => self.delete_query_char(),
            KeyCode::Backspace => self.backspace_query_char(),
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.query_cursor = 0;
                Action::None
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.query_cursor = self.query.len();
                Action::None
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.delete_query_word_before_cursor()
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.query.is_empty() {
                    if let Some(act) = keymap::empty_query_chord_action(&key) {
                        return self.apply_command(act);
                    }
                }
                self.insert_query_char(c)
            }
            _ => Action::None,
        }
    }

    fn apply_command(&mut self, command: keymap::Command) -> Action {
        match command {
            keymap::Command::Quit => Action::Quit,
            keymap::Command::TogglePreview => {
                self.preview_visible = !self.preview_visible;
                Action::None
            }
            keymap::Command::ResizePreview(d) => {
                let next = self.preview_width_pct as i32 + (d as i32) * 5;
                self.preview_width_pct = next.clamp(20, 80) as u16;
                Action::None
            }
            keymap::Command::ScrollPreview(d) => {
                let delta = d as i32 * i32::from(self.preview_scroll_step);
                let next = self.preview_scroll as i32 + delta;
                self.preview_scroll = next.max(0) as u16;
                Action::None
            }
            keymap::Command::JumpPreviewMatch(d) => {
                self.jump_preview_match(d);
                Action::None
            }
            keymap::Command::Help => {
                self.help_open = true;
                Action::None
            }
            keymap::Command::ResumeSelected { yolo } => {
                if self.results.is_empty() {
                    Action::None
                } else if yolo {
                    self.open_yolo_modal_with(true);
                    Action::None
                } else {
                    Action::Resume {
                        index: self.selected,
                        yolo,
                    }
                }
            }
            keymap::Command::ToggleKeymapPreset => {
                self.toggle_keymap();
                Action::None
            }
        }
    }

    fn handle_navigate(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => Action::Quit,
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.results.is_empty() {
                    self.selected = (self.selected + 1).min(self.results.len() - 1);
                }
                self.preview_scroll = 0;
                Action::None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                self.preview_scroll = 0;
                Action::None
            }
            KeyCode::Char('g') => {
                self.selected = 0;
                self.preview_scroll = 0;
                Action::None
            }
            KeyCode::Char('G') => {
                self.selected = self.results.len().saturating_sub(1);
                self.preview_scroll = 0;
                Action::None
            }
            KeyCode::Char('p') => {
                self.preview_visible = !self.preview_visible;
                Action::None
            }
            KeyCode::Char('?') => {
                self.help_open = true;
                Action::None
            }
            KeyCode::Char('/') => {
                self.navigate = false; // back to live search
                Action::None
            }
            KeyCode::Char('`') => {
                self.toggle_keymap();
                Action::None
            }
            KeyCode::Enter => self.activate(false),
            _ => Action::None,
        }
    }

    fn insert_query_char(&mut self, c: char) -> Action {
        self.query.insert(self.query_cursor, c);
        self.query_cursor += c.len_utf8();
        self.preview_scroll = 0;
        Action::Search
    }

    fn backspace_query_char(&mut self) -> Action {
        let Some(prev) = prev_boundary(&self.query, self.query_cursor) else {
            return Action::None;
        };
        self.query.drain(prev..self.query_cursor);
        self.query_cursor = prev;
        self.preview_scroll = 0;
        Action::Search
    }

    fn delete_query_char(&mut self) -> Action {
        if self.query_cursor >= self.query.len() {
            return Action::None;
        }
        let next = next_boundary(&self.query, self.query_cursor);
        self.query.drain(self.query_cursor..next);
        self.preview_scroll = 0;
        Action::Search
    }

    fn delete_query_word_before_cursor(&mut self) -> Action {
        let mut start = self.query_cursor;
        while let Some(prev) = prev_boundary(&self.query, start) {
            let ch = self.query[prev..start].chars().next().unwrap();
            if !ch.is_whitespace() {
                break;
            }
            start = prev;
        }
        while let Some(prev) = prev_boundary(&self.query, start) {
            let ch = self.query[prev..start].chars().next().unwrap();
            if ch.is_whitespace() {
                break;
            }
            start = prev;
        }
        if start == self.query_cursor {
            return Action::None;
        }
        self.query.drain(start..self.query_cursor);
        self.query_cursor = start;
        self.preview_scroll = 0;
        Action::Search
    }

    fn move_query_cursor_left(&mut self) {
        if let Some(prev) = prev_boundary(&self.query, self.query_cursor) {
            self.query_cursor = prev;
        }
    }

    fn move_query_cursor_right(&mut self) {
        if self.query_cursor < self.query.len() {
            self.query_cursor = next_boundary(&self.query, self.query_cursor);
        }
    }

    fn jump_preview_match(&mut self, delta: i16) {
        if self.preview_matches.is_empty() {
            return;
        }
        let len = self.preview_matches.len() as i32;
        let next = (self.preview_match_index as i32 + delta as i32).rem_euclid(len);
        self.preview_match_index = next as usize;
        self.preview_scroll = self.preview_matches[self.preview_match_index];
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
            return Action::Resume {
                index: idx,
                yolo: true,
            };
        }
        if supports {
            self.mode = Mode::YoloModal {
                index: idx,
                yolo: false,
            };
            Action::None
        } else {
            Action::Resume {
                index: idx,
                yolo: false,
            }
        }
    }
}

fn prev_boundary(s: &str, index: usize) -> Option<usize> {
    s.get(..index)?.char_indices().last().map(|(i, _)| i)
}

fn next_boundary(s: &str, index: usize) -> usize {
    s.get(index..)
        .and_then(|tail| tail.chars().next().map(|c| index + c.len_utf8()))
        .unwrap_or(s.len())
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{AgentId, SessionSummary};
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn sess(id: &str) -> SessionSummary {
        SessionSummary {
            id: id.into(),
            agent: AgentId::Claude,
            title: id.into(),
            directory: "/d".into(),
            timestamp: 1,
            message_count: 0,
            yolo: false,
            branch: None,
            repo_url: None,
            source_path: None,
        }
    }

    fn app_with(n: usize) -> App {
        let mut app = App::new();
        app.set_results((0..n).map(|i| sess(&format!("s{i}"))).collect());
        app.set_yolo_supported((0..n).map(|_| true).collect());
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
    fn yolo_modal_confirms_original_selected_index() {
        let mut app = app_with(3);
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.selected(), 2);

        assert_eq!(app.handle_key(key(KeyCode::Enter)), Action::None);
        assert!(app.modal_open());
        match app.handle_key(key(KeyCode::Enter)) {
            Action::Resume { index, yolo } => {
                assert_eq!(index, 2);
                assert!(!yolo);
            }
            other => panic!("expected resume for selected row, got {other:?}"),
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
    fn ctrl_y_opens_yolo_modal_for_selected_row() {
        let mut app = app_with(2);
        app.handle_key(key(KeyCode::Down)); // select index 1
        assert_eq!(
            app.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL)),
            Action::None
        );
        assert_eq!(app.yolo_modal(), Some((1, true)));

        match app.handle_key(key(KeyCode::Enter)) {
            Action::Resume { index, yolo } => {
                assert_eq!(index, 1);
                assert!(yolo);
            }
            other => panic!("expected yolo resume, got {other:?}"),
        }
    }

    #[test]
    fn plain_command_chars_type_when_query_nonempty() {
        let mut app = app_with(1);
        app.handle_key(key(KeyCode::Char('a')));
        let before = app.preview_width_pct();
        assert_eq!(app.handle_key(key(KeyCode::Char('['))), Action::Search);
        assert_eq!(app.preview_width_pct(), before);
        assert_eq!(app.query(), "a[");
        assert_eq!(app.handle_key(key(KeyCode::Char('?'))), Action::Search);
        assert!(!app.help_open());
        assert_eq!(app.query(), "a[?");
    }

    #[test]
    fn plain_command_chars_still_work_when_query_empty() {
        let mut app = app_with(1);
        let before = app.preview_width_pct();
        assert_eq!(app.handle_key(key(KeyCode::Char('['))), Action::None);
        assert!(app.preview_width_pct() < before);
        assert_eq!(app.handle_key(key(KeyCode::Char('?'))), Action::None);
        assert!(app.help_open());
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

    #[test]
    fn search_preset_esc_still_quits() {
        let mut app = app_with(3); // default = search preset
        assert_eq!(app.handle_key(key(KeyCode::Esc)), Action::Quit);
    }

    #[test]
    fn modal_esc_enters_navigate_then_letters_move() {
        let mut app = app_with(3);
        app.set_keymap(keymap::Preset::Modal);
        // Esc enters navigate mode instead of quitting
        assert_eq!(app.handle_key(key(KeyCode::Esc)), Action::None);
        // letters now navigate
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.selected(), 1);
        app.handle_key(key(KeyCode::Char('k')));
        assert_eq!(app.selected(), 0);
        app.handle_key(key(KeyCode::Char('G')));
        assert_eq!(app.selected(), 2);
        // '/' returns to search so letters type again
        app.handle_key(key(KeyCode::Char('/')));
        assert_eq!(app.handle_key(key(KeyCode::Char('a'))), Action::Search);
        assert_eq!(app.query(), "a");
    }

    #[test]
    fn ctrl_c_quits_from_help_and_modal() {
        let mut app = app_with(1);
        app.handle_key(key(KeyCode::Char('?')));
        assert!(app.help_open());
        assert_eq!(
            app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Action::Quit
        );

        let mut app = app_with(1);
        app.open_yolo_modal();
        assert_eq!(
            app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Action::Quit
        );
    }

    #[test]
    fn query_cursor_editing_works() {
        let mut app = app_with(0);
        for c in "abcd".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Left));
        app.handle_key(key(KeyCode::Left));
        assert_eq!(app.query_cursor(), 2);
        assert_eq!(app.handle_key(key(KeyCode::Char('X'))), Action::Search);
        assert_eq!(app.query(), "abXcd");
        assert_eq!(app.handle_key(key(KeyCode::Backspace)), Action::Search);
        assert_eq!(app.query(), "abcd");
        app.handle_key(key(KeyCode::Home));
        assert_eq!(app.handle_key(key(KeyCode::Delete)), Action::Search);
        assert_eq!(app.query(), "bcd");
        app.handle_key(key(KeyCode::End));
        assert_eq!(
            app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL)),
            Action::Search
        );
        assert_eq!(app.query(), "");
    }

    #[test]
    fn viewport_metrics_drive_paging_and_preview_scroll() {
        let mut app = app_with(50);
        app.set_viewport_metrics(6, 4);
        app.handle_key(key(KeyCode::PageDown));
        assert_eq!(app.selected(), 5);
        app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
        assert_eq!(app.preview_scroll(), 3);
    }

    #[test]
    fn preview_match_navigation_wraps() {
        let mut app = app_with(1);
        app.set_preview_matches(vec![2, 8]);
        assert_eq!(app.preview_scroll(), 2);
        app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
        assert_eq!(app.preview_scroll(), 8);
        app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
        assert_eq!(app.preview_scroll(), 2);
        app.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL));
        assert_eq!(app.preview_scroll(), 8);
    }

    #[test]
    fn backtick_toggles_keymap_preset_mode() {
        let mut app = app_with(0);
        assert_eq!(app.keymap_preset(), keymap::Preset::Search);
        assert!(!app.navigate);

        app.handle_key(key(KeyCode::Char('`')));
        assert_eq!(app.keymap_preset(), keymap::Preset::Modal);
        assert!(app.navigate);

        app.handle_key(key(KeyCode::Char('`')));
        assert_eq!(app.keymap_preset(), keymap::Preset::Search);
        assert!(!app.navigate);
    }
}
