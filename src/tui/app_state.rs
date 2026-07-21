//! `App` state: query/preview/selection accessors and mutators, search-mode
//! initialization, and the effective-query composition. Input handling lives in
//! `input`; the type itself and its constructor live in the module root.

use super::{App, Mode, SearchMode};
use crate::core::SessionSummary;
use crate::query::SortOrder;
use crate::tui::keymap;
use crate::tui::toolbar::{Focus, Scope};

impl App {
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
    pub fn glyphs(&self) -> &crate::tui::glyphs::Glyphs {
        &self.glyphs
    }
    /// Replace the glyph set (e.g. with the config-driven variant resolved at
    /// startup, with per-agent glyphs injected from the adapters).
    pub fn set_glyphs(&mut self, glyphs: crate::tui::glyphs::Glyphs) {
        self.glyphs = glyphs;
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
}
