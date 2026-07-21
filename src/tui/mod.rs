pub mod columns;
pub mod help;
pub mod keymap;
pub mod modal;
pub mod preview;
pub mod results_list;
pub mod theme;
pub mod toolbar;
pub mod view;

use crate::core::SessionSummary;
use crate::query::SortOrder;
use crate::tui::toolbar::{Focus, Scope};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// How the query line is interpreted. `Simple` shows a guided Scope/Sort toolbar
/// and treats the input as plain free text; `Raw` accepts the full query DSL.
/// This is a search-input mode, not a keymap/vim mode (see architecture I-011).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Simple,
    Raw,
}

impl SearchMode {
    /// Resolve the configured `search_mode` string; unknown/empty => simple.
    pub fn from_config(s: &str) -> SearchMode {
        match s.trim().to_ascii_lowercase().as_str() {
            "raw" => SearchMode::Raw,
            _ => SearchMode::Simple,
        }
    }
}

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
    /// Open the selected session's associated GitHub PR in the browser. The run
    /// loop resolves the PR from its enrichment state, so this is a no-op when no
    /// PR has been resolved for the row.
    OpenPr {
        index: usize,
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
    /// Simple (guided toolbar) vs raw (DSL) interpretation of `query`.
    search_mode: SearchMode,
    /// Result ordering, driven by the toolbar Sort control.
    sort: SortOrder,
    /// Simple-mode repo scope (`repo:` injection on/off).
    scope: Scope,
    /// The launch repo slug used by `Scope::ThisRepo`; `None` outside a repo.
    repo_slug: Option<String>,
    /// Which control Left/Right act on in simple mode (Tab cycles it).
    toolbar_focus: Focus,
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
    preview_matches: Vec<u16>,
    preview_match_index: usize,
    theme: crate::tui::theme::Theme,
    keymap: keymap::Keymap,
    frame: u64,
    indexing: Option<usize>,
}

impl App {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            query_cursor: 0,
            search_mode: SearchMode::Simple,
            sort: SortOrder::default(),
            scope: Scope::All,
            repo_slug: None,
            toolbar_focus: Focus::Query,
            results: Vec::new(),
            selected: 0,
            mode: Mode::Main,
            yolo_supported: Vec::new(),
            preview_visible: false,
            preview_width_pct: 50,
            preview_header_visible: true,
            preview_scroll: 0,
            help_open: false,
            list_page_size: 10,
            preview_scroll_step: 8,
            preview_matches: Vec::new(),
            preview_match_index: 0,
            theme: crate::tui::theme::Theme::default(),
            keymap: keymap::Keymap::defaults(),
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

    pub fn search_mode(&self) -> SearchMode {
        self.search_mode
    }
    pub fn sort(&self) -> SortOrder {
        self.sort
    }
    pub fn scope(&self) -> Scope {
        self.scope
    }
    pub fn toolbar_focus(&self) -> Focus {
        self.toolbar_focus
    }
    /// Whether the launch directory resolved to a repo, i.e. whether the Scope
    /// control is meaningful/shown.
    pub fn has_repo(&self) -> bool {
        self.repo_slug.is_some()
    }

    /// Initialize the search state from CLI/config resolution. `input` is what the
    /// query line shows (free text in simple mode, full DSL in raw mode).
    pub fn init_search(
        &mut self,
        mode: SearchMode,
        scope: Scope,
        repo_slug: Option<String>,
        input: String,
    ) {
        self.search_mode = mode;
        self.scope = scope;
        self.repo_slug = repo_slug;
        self.toolbar_focus = Focus::Query;
        self.set_query(input);
    }

    /// The repo slug that `Scope::ThisRepo` injects, or `None` when scoped to All.
    fn active_repo_scope(&self) -> Option<&str> {
        match self.scope {
            Scope::ThisRepo => self.repo_slug.as_deref(),
            Scope::All => None,
        }
    }

    /// The query string handed to the engine: raw mode passes the input through;
    /// simple mode composes the scope token with the free text.
    pub fn effective_query(&self) -> String {
        match self.search_mode {
            SearchMode::Raw => self.query.clone(),
            SearchMode::Simple => {
                crate::query::compose_simple(&self.query, self.active_repo_scope())
            }
        }
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
        self.mode = Mode::YoloModal { index: self.selected, yolo: false };
    }

    pub fn open_yolo_modal_with(&mut self, yolo: bool) {
        self.mode = Mode::YoloModal { index: self.selected, yolo };
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
    pub fn keymap(&self) -> &keymap::Keymap {
        &self.keymap
    }
    /// Replace the active keymap (e.g. with one resolved from `config.toml`).
    pub fn set_keymap(&mut self, keymap: keymap::Keymap) {
        self.keymap = keymap;
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
        self.preview_matches =
            matches.into_iter().map(|line| line.min(u16::MAX as usize) as u16).collect();
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
        if let Some(act) = self.keymap.chord_action(&key) {
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
            // Tab focuses the toolbar in simple mode; in raw mode it autocompletes
            // query keywords (the toolbar hides the keyword grammar simple mode
            // would otherwise complete).
            KeyCode::Tab if self.search_mode == SearchMode::Simple => {
                self.toolbar_focus = self.toolbar_focus.next(self.has_repo());
                Action::None
            }
            KeyCode::BackTab if self.search_mode == SearchMode::Simple => {
                self.toolbar_focus = self.toolbar_focus.prev(self.has_repo());
                Action::None
            }
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
            // Left/Right adjust the focused toolbar control in simple mode;
            // otherwise they move the query cursor.
            KeyCode::Left if self.toolbar_control_focused() => self.adjust_toolbar(-1),
            KeyCode::Right if self.toolbar_control_focused() => self.adjust_toolbar(1),
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
            keymap::Command::OpenPr => {
                if self.results.is_empty() {
                    Action::None
                } else {
                    Action::OpenPr { index: self.selected }
                }
            }
            keymap::Command::ToggleSearchMode => self.toggle_search_mode(),
        }
    }

    /// True when a toolbar control (not the query cursor) currently has focus, so
    /// Left/Right adjust it. Only ever true in simple mode.
    fn toolbar_control_focused(&self) -> bool {
        self.search_mode == SearchMode::Simple && self.toolbar_focus != Focus::Query
    }

    /// Adjust the focused toolbar control. `dir` is -1 for Left, +1 for Right.
    fn adjust_toolbar(&mut self, dir: i8) -> Action {
        match self.toolbar_focus {
            Focus::Scope => self.scope = self.scope.toggled(),
            Focus::Sort => {
                self.sort = if dir < 0 { self.sort.prev() } else { self.sort.next() };
            }
            Focus::Query => return Action::None,
        }
        self.preview_scroll = 0;
        Action::Search
    }

    /// Switch between simple and raw search modes, preserving as much intent as
    /// each mode can express. Simple->raw expands the toolbar scope into an
    /// editable DSL string; raw->simple lifts a `repo:` token into the Scope
    /// control and keeps the free text (other DSL filters are dropped, since the
    /// simple toolbar cannot yet represent them).
    fn toggle_search_mode(&mut self) -> Action {
        match self.search_mode {
            SearchMode::Simple => {
                self.query = self.effective_query();
                self.query_cursor = self.query.len();
                self.search_mode = SearchMode::Raw;
            }
            SearchMode::Raw => {
                let parsed = crate::query::parse(&self.query);
                if let Some(first) = parsed.repos.include.first() {
                    self.repo_slug = Some(first.clone());
                }
                self.scope = if self.repo_slug.is_some() && !parsed.repos.include.is_empty() {
                    Scope::ThisRepo
                } else {
                    Scope::All
                };
                self.query = parsed.free_text;
                self.query_cursor = self.query.len();
                self.search_mode = SearchMode::Simple;
            }
        }
        self.toolbar_focus = Focus::Query;
        self.preview_scroll = 0;
        Action::Search
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

    /// Enter opens the confirmation modal when the session needs confirmation
    /// (a yolo-capable agent, or an archived session that must be unarchived
    /// first); otherwise it resumes directly.
    fn activate(&mut self) -> Action {
        if self.results.is_empty() {
            return Action::None;
        }
        let idx = self.selected;
        let yolo_capable = self.yolo_supported.get(idx).copied().unwrap_or(false);
        let archived = self.results.get(idx).is_some_and(|s| s.archived);
        if yolo_capable || archived {
            self.mode = Mode::YoloModal { index: idx, yolo: false };
            Action::None
        } else {
            Action::Resume { index: idx, yolo: false }
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
            ..Default::default()
        }
    }

    fn app_with(n: usize) -> App {
        let mut app = App::new();
        app.set_results((0..n).map(|i| sess(&format!("s{i}"))).collect());
        app.set_yolo_supported((0..n).map(|_| true).collect());
        app
    }

    /// App forced into raw search mode, for exercising DSL/autocomplete behavior
    /// that the simple-mode toolbar otherwise replaces.
    fn raw_app_with(n: usize) -> App {
        let mut app = app_with(n);
        app.init_search(SearchMode::Raw, Scope::All, None, String::new());
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
    fn enter_on_archived_session_opens_confirm_modal_even_without_yolo() {
        let mut app = App::new();
        let mut s = sess("arch");
        s.archived = true;
        app.set_results(vec![s]);
        app.set_yolo_supported(vec![false]); // agent does not support yolo
        assert_eq!(app.handle_key(key(KeyCode::Enter)), Action::None);
        assert!(app.modal_open(), "archived sessions must be confirmed before resume");
        match app.handle_key(key(KeyCode::Enter)) {
            Action::Resume { index, .. } => assert_eq!(index, 0),
            other => panic!("expected resume after confirm, got {other:?}"),
        }
    }

    #[test]
    fn ctrl_p_toggles_preview() {
        let mut app = app_with(1);
        assert!(!app.preview_visible());
        app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
        assert!(app.preview_visible());
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
        // Autocomplete is a raw-mode feature; simple mode uses Tab for the toolbar.
        let mut app = raw_app_with(1);
        for c in "agent:cl".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        assert_eq!(app.handle_key(key(KeyCode::Tab)), Action::Search);
        assert_eq!(app.query(), "agent:claude");
    }

    #[test]
    fn search_mode_from_config_defaults_to_simple() {
        assert_eq!(SearchMode::from_config("raw"), SearchMode::Raw);
        assert_eq!(SearchMode::from_config("RAW"), SearchMode::Raw);
        assert_eq!(SearchMode::from_config("simple"), SearchMode::Simple);
        assert_eq!(SearchMode::from_config(""), SearchMode::Simple);
        assert_eq!(SearchMode::from_config("nonsense"), SearchMode::Simple);
    }

    /// App in simple mode scoped to a known repo, for toolbar tests.
    fn simple_app_with_repo(n: usize) -> App {
        let mut app = app_with(n);
        app.init_search(
            SearchMode::Simple,
            Scope::ThisRepo,
            Some("me/web".to_string()),
            String::new(),
        );
        app
    }

    #[test]
    fn simple_mode_tab_cycles_toolbar_focus() {
        let mut app = simple_app_with_repo(3);
        assert_eq!(app.toolbar_focus(), Focus::Query);
        // Tab does not autocomplete or search in simple mode; it moves focus.
        assert_eq!(app.handle_key(key(KeyCode::Tab)), Action::None);
        assert_eq!(app.toolbar_focus(), Focus::Scope);
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.toolbar_focus(), Focus::Sort);
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.toolbar_focus(), Focus::Query);
        // BackTab walks backwards.
        app.handle_key(key(KeyCode::BackTab));
        assert_eq!(app.toolbar_focus(), Focus::Sort);
    }

    #[test]
    fn simple_mode_arrows_adjust_focused_control() {
        let mut app = simple_app_with_repo(3);
        // Focus Scope, then Left toggles it and requests a re-search.
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.scope(), Scope::ThisRepo);
        assert_eq!(app.handle_key(key(KeyCode::Left)), Action::Search);
        assert_eq!(app.scope(), Scope::All);
        // Focus Sort, Right cycles ordering.
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.toolbar_focus(), Focus::Sort);
        let before = app.sort();
        assert_eq!(app.handle_key(key(KeyCode::Right)), Action::Search);
        assert_ne!(app.sort(), before);
    }

    #[test]
    fn simple_mode_left_right_move_cursor_when_query_focused() {
        let mut app = simple_app_with_repo(1);
        for c in "ab".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        assert_eq!(app.query_cursor(), 2);
        // Focus is on the query, so Left moves the text cursor, not a control.
        assert_eq!(app.handle_key(key(KeyCode::Left)), Action::None);
        assert_eq!(app.query_cursor(), 1);
        assert_eq!(app.scope(), Scope::ThisRepo);
    }

    #[test]
    fn effective_query_composes_scope_in_simple_mode() {
        let mut app = simple_app_with_repo(1);
        for c in "auth".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        // ThisRepo injects the repo token ahead of the free text.
        assert_eq!(app.effective_query(), "repo:me/web auth");
        // Switching scope to All drops the token.
        app.handle_key(key(KeyCode::Tab)); // focus Scope
        app.handle_key(key(KeyCode::Left)); // toggle to All
        assert_eq!(app.effective_query(), "auth");
    }

    #[test]
    fn toggle_search_mode_expands_then_collapses() {
        let mut app = simple_app_with_repo(1);
        for c in "auth".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        // Simple -> raw expands the scope token into an editable DSL string.
        assert_eq!(
            app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)),
            Action::Search
        );
        assert_eq!(app.search_mode(), SearchMode::Raw);
        assert_eq!(app.query(), "repo:me/web auth");
        // Raw -> simple lifts the repo token back into the Scope control and keeps
        // the free text in the input.
        app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert_eq!(app.search_mode(), SearchMode::Simple);
        assert_eq!(app.query(), "auth");
        assert_eq!(app.scope(), Scope::ThisRepo);
        assert_eq!(app.effective_query(), "repo:me/web auth");
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
    fn backtick_types_into_query() {
        // `` ` `` no longer toggles a keymap mode; it is an ordinary character.
        let mut app = app_with(0);
        assert_eq!(app.handle_key(key(KeyCode::Char('`'))), Action::Search);
        assert_eq!(app.query(), "`");
    }

    /// Map a Binding's display key label to a representative KeyEvent.
    /// Returns None for the "type" pseudo-binding (tested separately).
    fn binding_event(keys: &str) -> Option<KeyEvent> {
        use KeyCode::*;
        let ctrl = KeyModifiers::CONTROL;
        let none = KeyModifiers::NONE;
        let ev = KeyEvent::new;
        Some(match keys {
            "↑/↓" => ev(Up, none),
            "PgUp/PgDn" => ev(PageDown, none),
            // Scroll-down rep: scroll-up at offset 0 is a no-op, so the pair is
            // represented by Ctrl+D, which always moves the preview.
            "Ctrl+U/Ctrl+D" => ev(Char('d'), ctrl),
            "Ctrl+N/Ctrl+B" => ev(Char('n'), ctrl),
            "Ctrl+P" => ev(Char('p'), ctrl),
            "Ctrl+←/Ctrl+→" => ev(Left, ctrl),
            "Ctrl+O" => ev(Char('o'), ctrl),
            "Ctrl+R" => ev(Char('r'), ctrl),
            "←/→" => ev(Left, none),
            "Home/End" => ev(Home, none),
            "Backspace" => ev(Backspace, none),
            "Delete" => ev(Delete, none),
            "Enter" => ev(Enter, none),
            "Tab" => ev(Tab, none),
            "?" => ev(Char('?'), none),
            "Esc" => ev(Esc, none),
            "Ctrl+C" => ev(Char('c'), ctrl),
            "type" => return None,
            other => panic!("binding key {other:?} has no representative event mapping"),
        })
    }

    /// Snapshot of all observable App state a binding could plausibly change.
    /// Includes `toolbar_focus` so simple-mode Tab (which focuses a control and
    /// returns no `Action`) still registers as "did something".
    #[allow(clippy::type_complexity)]
    fn state_snapshot(app: &App) -> (usize, String, usize, bool, u16, u16, bool, bool, Focus) {
        (
            app.selected(),
            app.query().to_string(),
            app.query_cursor(),
            app.preview_visible(),
            app.preview_width_pct(),
            app.preview_scroll(),
            app.help_open(),
            app.modal_open(),
            app.toolbar_focus(),
        )
    }

    #[test]
    fn every_binding_is_handled() {
        // The catalog is mode-aware (Tab in particular), so exercise each mode's
        // bindings in that mode; every row must map to an Action or a state change.
        for mode in [SearchMode::Simple, SearchMode::Raw] {
            for b in crate::tui::keymap::bindings(&crate::tui::keymap::Keymap::defaults(), mode) {
                let Some(ev) = binding_event(&b.keys) else {
                    continue; // "type" tested in `typing_updates_query_and_requests_search`
                };
                // Fresh app per binding; populated + yolo-supported so Enter has work.
                let mut app = app_with(3);
                app.init_search(mode, Scope::All, None, String::new());
                // Give some query + preview matches so editing/match-nav chords act.
                for c in "agent:cl".chars() {
                    app.handle_key(key(KeyCode::Char(c)));
                }
                app.set_preview_matches(vec![1, 5]);
                app.handle_key(key(KeyCode::Down)); // ensure Up/PageUp have room to move
                app.handle_key(key(KeyCode::Left)); // cursor off the end so Delete/End act
                let before = state_snapshot(&app);
                let action = app.handle_key(ev);
                let after = state_snapshot(&app);
                let did_something = action != Action::None || before != after;
                assert!(
                    did_something,
                    "binding {:?} ({:?}) in {mode:?} fell into the no-op arm: \
                     no Action and no state change",
                    b.keys, b.label
                );
            }
        }
    }

    /// H3 decision (documented): the yolo confirm modal owns its own inline
    /// legend ("Tab toggles yolo · Enter resumes · Esc cancels"). `?` is NOT
    /// routed to help from the modal — it intentionally does nothing. The
    /// global footer's "? help" applies to the main view only.
    #[test]
    fn question_is_noop_inside_yolo_modal() {
        let mut app = app_with(1);
        app.open_yolo_modal();
        assert!(app.modal_open());
        let action = app.handle_key(key(KeyCode::Char('?')));
        assert_eq!(action, Action::None);
        assert!(app.modal_open(), "? must not close or change the modal");
        assert!(!app.help_open(), "? must not open help from the modal");
    }
}
