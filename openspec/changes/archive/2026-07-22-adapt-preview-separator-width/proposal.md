## Why

The preview pane's thin-rule separators (`── user ──────`) are rendered using a
width estimate computed in the main loop (`area.width * pct / 100 - 3`), but the
actual pane width is determined later by the ratatui `Layout` engine — which also
accounts for the `LIST_MIN_WIDTH` floor and the left-border + padding. When the
two disagree the separator either falls short or overflows, producing a visible
glitch. The separator width should derive from the real pane geometry, not a
pre-render guess.

## What Changes

- Move separator-width derivation from the pre-render estimate in `main.rs` into
  the render path, where the actual `transcript_area.width` is known.
- Remove the `preview_width` parameter from `PreviewState::update` and
  `render_transcript`; instead pass the width at render time or re-render the
  separator lines to the actual width.
- Keep `thin_rule` signature unchanged — it already takes a `width: u16`.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `preview-rendering`: separator width must track the actual pane width, not a
  pre-computed estimate.

## Impact

- `src/main.rs` — remove `preview_w` computation and the width arg to
  `PreviewState::update`.
- `src/tui/preview.rs` — `PreviewState::update` and `render_transcript` lose the
  width parameter; separator generation is deferred or re-computed at render time.
- `src/tui/view/mod.rs` — pass the resolved `transcript_area.width` into the
  render path so separators are sized correctly.
- Tests in `src/tui/view/tests_preview.rs` that call `render_transcript` with
  a width argument will need updating.
