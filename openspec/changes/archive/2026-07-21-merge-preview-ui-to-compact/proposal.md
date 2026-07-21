## Why

The card-mode preview uses a cleaner transcript rendering
(`render_transcript_with_separators`) with thin horizontal rules between
messages (`── user ──────`), while compact mode still uses the older
prefix-based rendering (`render_transcript_with_terms`) with inline role
markers (`›` / `● CLAUDE`). Both modes should use the same separator-based
rendering so the preview experience is consistent regardless of row style.

## What Changes

- Use `render_transcript_with_separators` for both card and compact modes,
  removing the `use_separators` conditional branching in `PreviewState::update`.
- Remove the now-unused `render_transcript_with_terms` function and its helpers
  (`prefix_first`, `indent`).
- Keep the 3-line metadata header for compact mode only (unchanged behavior —
  compact table rows show less metadata than cards, so the header remains
  useful).
- Simplify the `PreviewState::update` signature by dropping the
  `use_separators` parameter.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

(none)

## Impact

- `src/tui/preview.rs`: Remove `render_transcript_with_terms`,
  `prefix_first`, `indent`; remove `use_separators` branch in
  `PreviewState::update`; always call `render_transcript_with_separators`.
- `src/main.rs`: Drop the `row_style == RowStyle::Card` argument from the
  `preview.update(…)` call.
- Preview-related tests that assert on the old prefix-based output need
  updating to match the separator-based format.
