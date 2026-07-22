## 1. Extract preview width helper

- [x] 1.1 Promote `PREVIEW_MIN_WIDTH` and `LIST_MIN_WIDTH` from function-local `const` in `tui/view/mod.rs::render()` to module-level `pub(crate) const`
- [x] 1.2 Add `pub(crate) fn preview_inner_width(body_width: u16, preview_pct: u16) -> u16` in `tui/view/mod.rs` that runs `Layout::horizontal([Min(LIST_MIN_WIDTH), Percentage(pw)])` on a synthetic `Rect` and subtracts 2 (border + padding)
- [x] 1.3 Update `render()` to use the new constants (no logic change, just reference the module-level names)

## 2. Fix width computation in main loop

- [x] 2.1 Replace the `preview_w` estimate in `main.rs` (`area.width * pct / 100 - 3`) with a call to `preview_inner_width(area.width, pct)`
- [x] 2.2 Gate the call behind the same `preview_visible && width >= PREVIEW_MIN_WIDTH` guard as the view, falling back to `area.width` otherwise

## 3. Add width to preview cache key

- [x] 3.1 Change `PreviewState.key` type from `Option<(String, String)>` to `Option<(String, String, u16)>` in `tui/preview.rs`
- [x] 3.2 Include the `preview_width` parameter in the cache key construction (`preview_key` tuple)
- [x] 3.3 Verify that Ctrl+K/L resize and terminal resize both trigger re-render by inspecting that `preview_w` changes between ticks

## 4. Update tests

- [x] 4.1 Update `render_transcript` call sites in `tui/view/tests_preview.rs` if the signature or width semantics changed
- [x] 4.2 Update any preview cache tests in `tui/preview.rs` that assert on the key type
- [x] 4.3 Add a unit test for `preview_inner_width` covering: normal 50% split, narrow terminal where `LIST_MIN_WIDTH` dominates, and the border/padding deduction
