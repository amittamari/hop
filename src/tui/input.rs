//! `App` input handling: the `handle_key` dispatch, keymap command
//! application, toolbar/search-mode transitions, query-line editing, and the
//! `activate` (resume vs. confirm-modal) decision.

use super::{Action, App, Mode, SearchMode};
use crate::tui::keymap;
use crate::tui::toolbar::{Focus, Scope};
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
};

/// Lines the preview scrolls per mouse-wheel/trackpad scroll event. Deliberately
/// small and distinct from `preview_scroll_step` (≈ one page, used by the
/// keyboard scroll commands) so wheel scrolling reads smoothly.
const MOUSE_SCROLL_LINES: u16 = 3;

impl App {
    /// Route a mouse event. Only wheel scroll is handled: when the preview pane
    /// is visible it scrolls the transcript (never the sessions list); otherwise
    /// it is ignored. Non-scroll mouse events are dropped.
    pub fn handle_mouse(&mut self, me: MouseEvent) {
        if !self.preview_visible {
            return;
        }
        let delta = match me.kind {
            MouseEventKind::ScrollUp => -i32::from(MOUSE_SCROLL_LINES),
            MouseEventKind::ScrollDown => i32::from(MOUSE_SCROLL_LINES),
            _ => return,
        };
        let next = self.preview_scroll as i32 + delta;
        self.preview_scroll = next.max(0) as u16;
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
