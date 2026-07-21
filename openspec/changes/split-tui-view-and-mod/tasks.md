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
- [ ] 2.1 Keep `tui/mod.rs` as module root: submodule decls, re-exports, shared
      types (`SearchMode`, `Action`, `Mode`), `App` struct + `new()`.
- [ ] 2.2 Move state accessors/mutators (`init_search`, `effective_query`, query
      and preview/selection getters/setters) into `tui/app_state.rs` as an
      `impl App` block (+ their tests).
- [ ] 2.3 Move key→`Action` dispatch, navigation, and modal transitions into
      `tui/input.rs` as an `impl App` block (+ their tests).
- [ ] 2.4 `cargo test` green; `crate::tui::{App, Action, SearchMode}` imports
      unchanged; every resulting `.rs` file ≤ 500.

## 3. Verify
- [ ] 3.1 `cargo test` and `cargo test --lib` pass.
- [ ] 3.2 Confirm no file in `src/tui/` exceeds 500 lines (`wc -l`).
- [ ] 3.3 Commit view split and mod split separately so a regression is bisectable.
