## 1. Add preview line count to App

- [x] 1.1 Add `preview_line_count: u16` field to the `App` struct in `src/tui/mod.rs`, initialized to 0
- [x] 1.2 Add `pub fn set_preview_line_count(&mut self, n: usize)` to `src/tui/app_state.rs` that stores `n.min(u16::MAX as usize) as u16` and calls `clamp_preview_scroll()`
- [x] 1.3 Add private `fn clamp_preview_scroll(&mut self)` to `src/tui/input.rs` that clamps `preview_scroll` to `preview_line_count.saturating_sub(1)`

## 2. Apply clamp at all scroll mutation sites

- [x] 2.1 Call `self.clamp_preview_scroll()` after updating `preview_scroll` in `ScrollPreview` handler (`src/tui/input.rs` ~line 179)
- [x] 2.2 Call `self.clamp_preview_scroll()` after updating `preview_scroll` in `handle_mouse` (`src/tui/input.rs` ~line 31)
- [x] 2.3 Call `self.clamp_preview_scroll()` after setting `preview_scroll` in `jump_preview_match` (`src/tui/input.rs` ~line 294)
- [x] 2.4 Call `self.clamp_preview_scroll()` at the end of `set_preview_matches` in `src/tui/app_state.rs` (re-clamp when first match jumps near end)

## 3. Feed line count from PreviewState

- [x] 3.1 Call `app.set_preview_line_count(self.lines.len())` in `PreviewState::update()` (`src/tui/preview.rs`) after lines are computed, next to the existing `set_preview_matches` call

## 4. Tests

- [x] 4.1 Add test: `ScrollPreview` past end clamps to last line (not blank)
- [x] 4.2 Add test: jump-to-match near end + `ScrollPreview` stays clamped
- [x] 4.3 Add test: mouse scroll past end stays clamped
- [x] 4.4 Add test: setting a smaller `preview_line_count` re-clamps an existing scroll position
