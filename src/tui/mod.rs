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
    list_page_size: usize,
    preview_scroll_step: u16,
    preview_viewport_height: u16,
    preview_line_count: usize,
    preview_matches: Vec<u16>,
    preview_match_index: usize,
    theme: crate::tui::theme::Theme,
    frame: u64,
    indexing: Option<usize>,
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
            list_page_size: 10,
            preview_scroll_step: 8,
            preview_viewport_height: 1,
            preview_line_count: 0,
            preview_matches: Vec::new(),
            preview_match_index: 0,
            theme: crate::tui::theme::Theme::default(),
            frame: 0,
            indexing: None,
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
    pub fn frame(&self) -> u64 {
        self.frame
    }
    /// Advance the spinner clock by one redraw. The run loop calls this once
    /// per iteration; the loop polls every 50ms, so the throbber animates
    /// without a dedicated timer.
    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);
    }
    /// Number of sessions still being indexed, or `None` when idle.
    pub fn indexing(&self) -> Option<usize> {
        self.indexing
    }
    pub fn set_indexing(&mut self, count: Option<usize>) {
        self.indexing = count;
    }
    pub fn modal_open(&self) -> bool {
        matches!(self.mode, Mode::YoloModal { .. })
    }
    pub fn yolo_modal(&self) -> Option<(usize, bool)> {
        match self.mode {
            Mode::YoloModal { index, yolo } => Some((index, yolo)),
            Mode::Main => None,
        }
    }

    pub fn set_results(&mut self, results: Vec<SessionSummary>) {
        self.results = results;
        self.yolo_supported = vec![false; self.results.len()];
        self.clamp_selection();
    }

    pub fn set_results_with_yolo(
        &mut self,
        results: Vec<SessionSummary>,
        yolo_supported: Vec<bool>,
    ) {
        self.results = results;
        self.yolo_supported = yolo_supported;
        self.yolo_supported.resize(self.results.len(), false);
        self.clamp_selection();
    }

    fn clamp_selection(&mut self) {
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
    pub fn theme(&self) -> &crate::tui::theme::Theme {
        &self.theme
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
        self.preview_viewport_height = preview_height.max(1);
    }

    /// Number of source lines in the current preview transcript; used to clamp
    /// `preview_scroll` so it can't run past the end into blank space.
    pub fn set_preview_line_count(&mut self, count: usize) {
        self.preview_line_count = count;
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
        // Main search handling.
        match key.code {
            // Esc clears a non-empty query, then quits when already empty.
            KeyCode::Esc => {
                if self.query.is_empty() {
                    Action::Quit
                } else {
                    self.query.clear();
                    self.query_cursor = 0;
                    self.preview_scroll = 0;
                    Action::Search
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
            KeyCode::Enter => self.activate(),
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
            // `?` is reserved for help in every state, so it never types.
            KeyCode::Char('?') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.help_open = true;
                Action::None
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.insert_query_char(c)
            }
            _ => Action::None,
        }
    }

    fn apply_command(&mut self, command: keymap::Command) -> Action {
        match command {
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
                let next = (self.preview_scroll as i32 + delta).max(0) as usize;
                let max_scroll = self
                    .preview_line_count
                    .saturating_sub(self.preview_viewport_height as usize);
                self.preview_scroll = next.min(max_scroll).min(u16::MAX as usize) as u16;
                Action::None
            }
            keymap::Command::JumpPreviewMatch(d) => {
                self.jump_preview_match(d);
                Action::None
            }
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

    /// Enter on a yolo-capable agent opens the confirmation modal; otherwise resume.
    fn activate(&mut self) -> Action {
        if self.results.is_empty() {
            return Action::None;
        }
        let idx = self.selected;
        if self.yolo_supported.get(idx).copied().unwrap_or(false) {
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
    fn frame_starts_at_zero_and_advances() {
        let mut app = App::new();
        assert_eq!(app.frame(), 0);
        app.tick();
        app.tick();
        assert_eq!(app.frame(), 2);
    }

    #[test]
    fn indexing_state_round_trips() {
        let mut app = App::new();
        assert_eq!(app.indexing(), None);
        app.set_indexing(Some(42));
        assert_eq!(app.indexing(), Some(42));
        app.set_indexing(None);
        assert_eq!(app.indexing(), None);
    }

    #[test]
    fn app_exposes_default_theme() {
        assert_eq!(*App::new().theme(), crate::tui::theme::Theme::default());
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
    fn question_toggles_help_and_esc_closes_it() {
        let mut app = app_with(1);
        app.handle_key(key(KeyCode::Char('?')));
        assert!(app.help_open());
        assert_eq!(app.handle_key(key(KeyCode::Esc)), Action::None);
        assert!(!app.help_open());
    }

    #[test]
    fn bracket_chars_type_into_query() {
        // `[` and `]` no longer resize; they are ordinary query characters now.
        let mut app = app_with(1);
        let before = app.preview_width_pct();
        assert_eq!(app.handle_key(key(KeyCode::Char('['))), Action::Search);
        assert_eq!(app.handle_key(key(KeyCode::Char(']'))), Action::Search);
        assert_eq!(app.query(), "[]");
        assert_eq!(app.preview_width_pct(), before);
    }

    #[test]
    fn question_opens_help_and_never_types() {
        // `?` is reserved for help in every state, even mid-query.
        let mut app = app_with(1);
        app.handle_key(key(KeyCode::Char('a')));
        assert_eq!(app.handle_key(key(KeyCode::Char('?'))), Action::None);
        assert!(app.help_open());
        assert_eq!(app.query(), "a");
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
    fn esc_clears_query_then_quits() {
        let mut app = app_with(3);
        for c in "abc".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        assert_eq!(app.query(), "abc");
        // First Esc clears the query and re-searches.
        assert_eq!(app.handle_key(key(KeyCode::Esc)), Action::Search);
        assert_eq!(app.query(), "");
        // Second Esc (empty query) quits.
        assert_eq!(app.handle_key(key(KeyCode::Esc)), Action::Quit);
    }

    #[test]
    fn ctrl_arrows_resize_preview() {
        let mut app = app_with(1);
        let before = app.preview_width_pct();
        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL));
        assert!(app.preview_width_pct() > before);
        app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL));
        app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL));
        assert!(app.preview_width_pct() < before);
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
        app.set_preview_line_count(100);
        app.handle_key(key(KeyCode::PageDown));
        assert_eq!(app.selected(), 5);
        app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
        assert_eq!(app.preview_scroll(), 3);
    }

    #[test]
    fn preview_scroll_clamps_at_bottom() {
        let mut app = app_with(1);
        // viewport: list_rows_height irrelevant here; preview_height = 5 rows.
        app.set_viewport_metrics(6, 5);
        app.set_preview_line_count(30);
        // Scroll down far more than the content allows.
        for _ in 0..20 {
            app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
        }
        // Max top line = 30 - 5 = 25; never past it into blank space.
        assert_eq!(app.preview_scroll(), 25);
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
    fn backtick_types_into_query() {
        // `` ` `` no longer toggles a keymap mode; it is an ordinary character.
        let mut app = app_with(0);
        assert_eq!(app.handle_key(key(KeyCode::Char('`'))), Action::Search);
        assert_eq!(app.query(), "`");
    }
}
