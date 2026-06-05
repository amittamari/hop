# TUI Keymap Review

**Date:** 2026-06-05
**Status:** Review artifact for later action-item extraction
**Scope:** Current TUI keymap behavior, help/footer language, and fit with common terminal UI expectations.

## Summary

The default `search` keymap is directionally sound for `hop`: it behaves like a search palette, where typing edits the query, arrow keys move the result selection, and Enter activates the selected row. That is a familiar pattern for terminal pickers and command palettes.

The current implementation has several rough edges where the bindings stop matching that model. The main issue is that printable keys (`?`, `[`, and `]`) are treated as global commands before query editing, so users cannot type those characters into an always-live search. Modal mode is also reasonable as an opt-in vim-style preset, but the rendered UI does not show whether the user is editing the query or navigating results.

Recommended direction: keep the overall keymap shape, but make global commands truly global only when they are non-printable chords, make printable commands context-sensitive, and make modal state visible.

## Sources Reviewed

- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/specs/2026-06-04-hop-design.md`
- `docs/specs/2026-06-04-hop-tui-v2-design.md`
- `docs/superpowers/plans/2026-06-04-hop-tui-v2.md`
- `docs/reviews/2026-06-05-architecture-rust-tui-review.md`
- `src/tui/keymap.rs`
- `src/tui/mod.rs`
- `src/tui/help.rs`
- `src/tui/view.rs`

## Verification

- `cargo test --lib tui`: passed, 41 tests.

The current tests encode some of the questionable behavior, especially printable commands working while the query is non-empty. Passing tests therefore confirm implementation consistency, not that the user experience is correct.

## Current Behavior

Default `search` preset:

- Typing appends characters to the live query.
- `Up` / `Down` move selection.
- `PgUp` / `PgDn` move selection by 10 rows.
- `Enter` resumes the selected session, opening the yolo confirmation modal when needed.
- `Tab` autocompletes keyword values.
- `Ctrl+Y` opens the yolo resume prompt for the selected row.
- `Ctrl+P` toggles the preview.
- `Ctrl+U` / `Ctrl+D` scroll the preview by 8 lines.
- `[` / `]` resize the preview.
- `?` opens help.
- `Esc` quits.
- `Ctrl+C` quits, except while help is open.

Modal `keymap = "modal"` preset:

- Starts in query-editing mode.
- `Esc` leaves query editing and enters navigate mode.
- In navigate mode, `j` / `k` move, `g` / `G` jump top/bottom, `p` toggles preview, `/` returns to search editing, and `Esc` quits.
- The footer does not expose the current modal sub-mode.

## Findings

### K-001: Printable command keys conflict with always-live search

**Severity:** High
**Files:** `src/tui/keymap.rs`, `src/tui/mod.rs`

`keymap::chord_action` maps plain `?`, `[`, and `]` before `App::handle_key` reaches query editing. This means those characters cannot be typed into the query in the default always-live search preset.

That is surprising for a search-palette UI. In this model, plain printable keys generally type unless the user is in a navigation/normal mode. Commands that must work everywhere are usually non-printable keys or modified chords.

This also contradicts the implementation plan note in `docs/superpowers/plans/2026-06-04-hop-tui-v2.md`, which says printable `?`, `[`, and `]` should act as controls only when the query is empty and type literally otherwise.

**Action candidate:** Keep `Ctrl+P`, `Ctrl+U`, `Ctrl+D`, and `Ctrl+Y` as global chords. Move plain `?`, `[`, and `]` handling into `App::handle_key` so they are commands only when the query is empty, or replace them with non-printable/modified alternatives.

### K-002: `Ctrl+C` is not truly global

**Severity:** Medium
**Files:** `src/tui/mod.rs`

The help overlay branch runs before the `Ctrl+C` quit check. While help is open, `Ctrl+C` is swallowed and returns `Action::None`.

Terminal users commonly expect `Ctrl+C` to exit from any non-shell prompt state. The help text also presents `Esc/Ctrl+C quit`, but that is not true while help is open.

**Action candidate:** Move the `Ctrl+C` quit check before the help overlay branch. Keep `Esc` as "close overlay" while help is open.

### K-003: Modal preset state is not visible

**Severity:** Medium
**Files:** `src/tui/mod.rs`, `src/tui/view.rs`, `src/tui/help.rs`

Modal mode has two states: query editing and navigate mode. The `App` model stores this as `navigate: bool`, but the renderer does not expose it. The footer always says `esc quit` in the main view, even though in modal query-editing mode `Esc` enters navigate mode first.

This is already tracked as `P-006 TUI Mode Visibility Gap` in `docs/ARCHITECTURE.md`.

**Action candidate:** Add a small mode indicator and mode-specific footer text:

- Search preset: `type search | Up/Down move | Enter resume | ? help | Esc quit`
- Modal editing: `SEARCH | Esc navigate | Tab complete | Enter resume | Ctrl+C quit`
- Modal navigate: `NAV | j/k move | / search | Enter resume | Esc quit`

### K-004: Query editing is too limited for a polished search box

**Severity:** Medium
**Files:** `src/tui/mod.rs`

The search input supports append-only typing plus Backspace. It does not support cursor movement, Delete, Home/End, or common terminal editing chords like `Ctrl+A`, `Ctrl+E`, `Ctrl+W`, and `Ctrl+U`.

For a short query field this is usable, but it falls below common expectations once filters and longer free-text queries are supported.

**Action candidate:** Introduce cursor position in `App` and implement a focused minimal editing set:

- Left / Right
- Home / End
- Delete
- Backspace before cursor
- `Ctrl+A` / `Ctrl+E`
- `Ctrl+W` delete previous token

Decide carefully whether `Ctrl+U` should keep scrolling preview or clear query. In many terminal contexts `Ctrl+U` clears input; in this app it currently scrolls preview. If preview scroll remains on `Ctrl+U`, help should make that tradeoff explicit.

### K-005: Page and preview scroll distances are fixed rather than viewport-aware

**Severity:** Low
**Files:** `src/tui/mod.rs`, `src/tui/view.rs`

`PgUp` / `PgDn` move by 10 rows, and preview `Ctrl+U` / `Ctrl+D` scroll by 8 lines. The bindings are standard, but the fixed distances are less standard than page-size or half-page movement derived from the visible viewport.

**Action candidate:** Track visible list height and preview height in model/effect state, or pass a scroll quantum from the event loop, so page movement follows the rendered viewport.

## Fit With Common TUI Conventions

What currently makes sense:

- Search-first default keymap.
- Arrows for selection movement.
- `Enter` for primary activation.
- `Esc` to quit in the default non-modal flow.
- `Ctrl+C` as terminal escape hatch.
- `?` for help, when not conflicting with typing.
- Optional vim-style modal navigation.
- `j` / `k`, `g` / `G`, and `/` in modal navigate mode.

What is questionable:

- Plain printable commands in an always-live text input.
- Static footer text that does not reflect modal sub-state.
- `Ctrl+C` not taking priority over help.
- Reusing `Ctrl+U` for preview scroll in a query editor without supporting other query-editing chords.
- Fixed page movement independent of viewport height.

## Recommended Action Order

1. Make `Ctrl+C` global before overlay/modal handling.
2. Make printable command keys context-sensitive, especially `?`, `[`, and `]`.
3. Add modal-mode visibility and mode-specific footer/help text.
4. Add query cursor state and minimal editing keys.
5. Make list paging and preview scrolling viewport-aware.

## Suggested Tests

- `ctrl_c_quits_with_help_open`
- `question_types_when_query_nonempty`
- `brackets_type_when_query_nonempty`
- `question_opens_help_when_query_empty`
- `modal_footer_mentions_search_mode_before_esc`
- `modal_footer_mentions_nav_mode_after_esc`
- `left_right_move_query_cursor`
- `delete_removes_char_at_cursor`

