//! Behavioral tests for `App`: key dispatch, modal/help transitions, toolbar
//! focus, search-mode toggling, query editing, and the binding-coverage check.

use super::*;
use crate::core::{AgentId, SessionSummary};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn scroll(kind: MouseEventKind) -> MouseEvent {
    MouseEvent { kind, column: 0, row: 0, modifiers: KeyModifiers::NONE }
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
fn mouse_scroll_moves_preview_not_selection() {
    let mut app = app_with(3);
    app.set_preview(true, 50);
    assert_eq!(app.preview_scroll(), 0);
    assert_eq!(app.selected(), 0);

    app.handle_mouse(scroll(MouseEventKind::ScrollDown));
    assert!(app.preview_scroll() > 0, "scroll down should advance the preview");
    let after_down = app.preview_scroll();
    // The sessions-list selection is untouched by wheel scroll.
    assert_eq!(app.selected(), 0);

    // Scrolling up returns toward the top.
    app.handle_mouse(scroll(MouseEventKind::ScrollUp));
    assert!(app.preview_scroll() < after_down);
}

#[test]
fn mouse_scroll_up_clamps_at_top() {
    let mut app = app_with(3);
    app.set_preview(true, 50);
    // Already at the top; scrolling up must not underflow.
    app.handle_mouse(scroll(MouseEventKind::ScrollUp));
    assert_eq!(app.preview_scroll(), 0);
}

#[test]
fn mouse_scroll_ignored_when_preview_hidden() {
    let mut app = app_with(3);
    assert!(!app.preview_visible());
    app.handle_mouse(scroll(MouseEventKind::ScrollDown));
    assert_eq!(app.preview_scroll(), 0);
    assert_eq!(app.selected(), 0);
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
    app.init_search(SearchMode::Simple, Scope::ThisRepo, Some("me/web".to_string()), String::new());
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
    app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
    assert!(app.preview_width_pct() > before);
    app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
    app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
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
        "Ctrl+K/Ctrl+L" => ev(Char('l'), ctrl),
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

#[test]
fn scroll_preview_clamps_at_content_end() {
    let mut app = app_with(1);
    app.set_viewport_metrics(6, 10); // scroll_step = 9
    app.set_preview_line_count(20);
    // Scroll twice: 0 + 9 = 9, 9 + 9 = 18; both within 19 (max).
    app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert_eq!(app.preview_scroll(), 9);
    app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert_eq!(app.preview_scroll(), 18);
    // Third scroll would hit 27, but clamped to 19.
    app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert_eq!(app.preview_scroll(), 19);
}

#[test]
fn jump_match_then_scroll_clamps() {
    let mut app = app_with(1);
    app.set_viewport_metrics(6, 10); // scroll_step = 9
    app.set_preview_line_count(15);
    app.set_preview_matches(vec![12]);
    // Match jumps to line 12; Ctrl+D adds 9 → 21, clamped to 14.
    app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert_eq!(app.preview_scroll(), 14);
}

#[test]
fn mouse_scroll_clamps_at_content_end() {
    let mut app = app_with(1);
    app.set_preview(true, 50);
    app.set_preview_line_count(5);
    for _ in 0..10 {
        app.handle_mouse(scroll(MouseEventKind::ScrollDown));
    }
    assert_eq!(app.preview_scroll(), 4);
}

#[test]
fn smaller_line_count_reclamps_scroll() {
    let mut app = app_with(1);
    app.set_preview_line_count(100);
    app.set_viewport_metrics(6, 50);
    // Scroll deep.
    app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert_eq!(app.preview_scroll(), 49);
    // Shrink content — scroll must re-clamp.
    app.set_preview_line_count(10);
    assert_eq!(app.preview_scroll(), 9);
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
