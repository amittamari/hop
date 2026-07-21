pub mod columns;
pub mod glyphs;
pub mod help;
pub mod keymap;
pub mod modal;
pub mod preview;
pub mod results_list;
pub mod theme;
pub mod toolbar;
pub mod view;

mod app_state;
mod input;

use crate::core::SessionSummary;
use crate::query::SortOrder;
use crate::tui::toolbar::{Focus, Scope};

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
    glyphs: crate::tui::glyphs::Glyphs,
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
            // Safe default; production selects the config-driven variant via
            // `set_glyphs`. ascii keeps `App::new()`-based tests tofu-free.
            glyphs: crate::tui::glyphs::Glyphs::default(),
            keymap: keymap::Keymap::defaults(),
            frame: 0,
            indexing: None,
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod app_tests;
