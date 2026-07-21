# Design: splitting view.rs and mod.rs

Goal: reduce two god-files below the ~500-line soft limit by extracting cohesive
submodules, with **zero behavior change**. The existing test suite is the safety
net; each extracted file keeps its own `#[cfg(test)]` block so private-item tests
keep compiling.

## Constraint: inline tests see private items

Both files test private functions via colocated `#[cfg(test)]` modules. Tests
therefore cannot move to the integration `tests/` dir (which only sees the public
API). The split must keep each submodule's tests inside that submodule, or expose
seams as `pub(crate)`. Prefer keeping tests colocated.

## view.rs → `src/tui/view/`

Seams follow the existing top-level fns (verified in the current file):

```
view/mod.rs            render() orchestration, RenderModel, StatusLine,
                       rel_time, spinner_frame, empty_state_message,
                       visible_result_range  (+ re-exports so `tui::view::X`
                       paths stay valid)
view/footer.rs         footer_hints_line, footer_status_line, line_display_width
view/cards.rs          render_card, card_visible_range
view/preview_header.rs preview_header_lines
```

Each file carries the subset of the current `#[cfg(test)]` tests that exercise
its fns. `render_to_text` test helper lives with `view/mod.rs` tests (or a shared
`#[cfg(test)]` test-support fn).

Target: `view/mod.rs` ≈ 300–400 lines, others well under 300.

## mod.rs → tui module root + focused impls

Current `mod.rs` mixes three things: (1) the tui module root (submodule
declarations + shared types), (2) `App` state accessors/mutators, (3) key→`Action`
dispatch. Rust allows multiple `impl App` blocks across files, so:

```
tui/mod.rs        submodule declarations, re-exports, shared types
                  (SearchMode, Action, Mode), the `App` struct def + `new()`
tui/app_state.rs  impl App: getters/setters, init_search, effective_query,
                  preview/selection accessors
tui/input.rs      impl App: key handling → Action, navigation, modal transitions
```

Tests distribute to the file owning the code they test. Keep the module root thin
(type vocabulary + wiring), so `crate::tui::{App, Action, SearchMode}` imports are
unchanged.

Target: each file under ~450 lines.

## Verification

- `cargo test` and `cargo test --lib` green after each file split (split view
  first, then mod, committing between so a regression is bisectable).
- No new `pub` beyond what re-exports require; prefer `pub(crate)` if a seam must
  widen for tests.
- Re-check line counts: every resulting `.rs` file ≤ 500.
