# Tasks

## 1. Split view.rs
- [x] 1.1 Create `src/tui/view/` module; move `render`, `RenderModel`, `StatusLine`,
      `rel_time`, `spinner_frame`, `empty_state_message`, `visible_result_range`
      into `view/mod.rs` with re-exports preserving `tui::view::*` paths.
- [x] 1.2 Extract `footer_hints_line`, `footer_status_line` into `view/footer.rs`
      (`line_display_width` stays in `mod.rs`; it also measures the toolbar).
- [x] 1.3 Extract `render_card`, `card_visible_range` into `view/cards.rs`.
- [x] 1.4 Extract `preview_header_lines` into `view/preview_header.rs`.
- [x] 1.5 Split the full-frame render tests into themed `#[cfg(test)]` submodules
      (`tests_{layout,list,preview,footer,modal}` + `test_support`); every
      `view/*.rs` ≤ 500. `cargo test` green, clippy `-D warnings` clean.

## 2. Split mod.rs
- [x] 2.1 Keep `tui/mod.rs` as module root: submodule decls, shared types
      (`SearchMode`, `Action`, `Mode`), `App` struct + `new()` + `Default`.
- [x] 2.2 Move state accessors/mutators (`init_search`, `effective_query`, query
      and preview/selection getters/setters) into `tui/app_state.rs` as an
      `impl App` block. (Child modules reach `App`'s private fields, so no
      visibility widening was needed.)
- [x] 2.3 Move key→`Action` dispatch, navigation, modal transitions, and the
      `prev/next_boundary` helpers into `tui/input.rs` as an `impl App` block.
- [x] 2.4 App behavior tests moved to `tui/app_tests.rs`. `cargo test` green;
      `crate::tui::{App, Action, SearchMode}` imports unchanged; every resulting
      `.rs` file ≤ 500 (mod 144, app_state 194, input 301, app_tests 491).

## 3. Verify
- [x] 3.1 `cargo test` (296) and `cargo test --lib` (244) pass.
- [x] 3.2 Both split targets (`view.rs`, `mod.rs`) are gone/≤500. The `src/tui/`
      files still over 500 (`preview.rs`, `results_list.rs`) are the breadcrumbed
      test-dominated exceptions, out of scope for this change.
- [x] 3.3 View split and mod split committed separately (bisectable).
