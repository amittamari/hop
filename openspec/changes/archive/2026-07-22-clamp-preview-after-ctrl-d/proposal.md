## Why

Pressing Ctrl+D (scroll preview down) after Ctrl+N (jump to match) in the demo
flow — and in normal use — can scroll `preview_scroll` past the end of the
transcript, leaving the preview pane completely blank. There is no upper-bound
clamping; only the lower bound (`max(0)`) is enforced. The same issue affects
mouse-wheel scrolling.

## What Changes

- Clamp `preview_scroll` so it can never exceed the last visible line of the
  transcript content. The clamp applies to all scroll sources: keyboard
  (Ctrl+D / Ctrl+U), mouse wheel, and jump-to-match (Ctrl+N / Ctrl+Shift+N).
- Track the preview line count in `App` state so the clamp has a ceiling to
  reference.

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `preview-rendering`: Add requirement that preview scroll position is clamped
  to prevent scrolling past the end of content.

## Impact

- `src/tui/mod.rs` — new field for preview line count
- `src/tui/input.rs` — clamp logic in `ScrollPreview`, `handle_mouse`, and
  `jump_preview_match`
- `src/tui/app_state.rs` — setter for preview line count
- `src/tui/preview.rs` — call the setter when lines are computed
- `demo/demo.tape` — the existing Ctrl+N → Ctrl+D sequence should now produce
  a visible (scrolled) preview instead of a blank pane
