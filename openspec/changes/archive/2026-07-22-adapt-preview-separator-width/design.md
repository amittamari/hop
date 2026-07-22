## Context

The preview pane's message separators (`── user ──────`) are rendered by
`thin_rule()` in `preview.rs`, which fills trailing `─` characters to a given
`width`. That width is computed in `main.rs` as
`area.width * pct / 100 - 3` — a rough estimate that does not account for:

1. The `LIST_MIN_WIDTH` (48) floor constraint that can steal columns from the
   preview percentage.
2. The actual ratatui `Layout` solver behavior, which may round differently.
3. The left border (1 col) + left padding (1 col) = 2 columns deducted by the
   preview `Block`, not 3.

Additionally, `PreviewState`'s cache key is `(document_key, query)` — it does
not include width. So resizing the terminal or adjusting the preview pane width
with Ctrl+K/L leaves stale separator widths until the user changes selection.

## Goals / Non-Goals

**Goals:**

- Separator rules fill exactly to the preview pane's inner width.
- Separators update when pane width changes (terminal resize or Ctrl+K/L).

**Non-Goals:**

- Changing separator visual style, colors, or label formatting.
- Refactoring the preview caching architecture beyond what's needed for width
  tracking.

## Decisions

### D-1: Compute actual preview inner width via Layout, share as a helper

Rather than duplicating constants or guessing the pane width, extract a
`pub fn preview_inner_width(body_width: u16, preview_pct: u16) -> u16`
helper in `tui/view/mod.rs` that runs the same
`Layout::horizontal([Min(LIST_MIN_WIDTH), Percentage(pw)])` split, then
deducts the Block's left border + padding (2 cols). Both `main.rs`
(for `PreviewState::update`) and the view renderer can call it.

**Alternative considered:** move transcript rendering into the draw closure.
Rejected because the draw closure borrows `terminal` mutably, making it
awkward to access `PreviewState` and `Engine` simultaneously, and it would
re-render every frame instead of caching.

**Alternative considered:** post-process separator lines in the view to patch
their width. Rejected as fragile — requires identifying separator lines by
convention after the fact.

### D-2: Include width in the preview cache key

Change `PreviewState.key` from `(String, String)` to `(String, String, u16)`
(doc_key, query, width). This causes re-rendering when the pane resizes,
while keeping the cache for the common case (scrolling, same selection).

### D-3: Invalidate on width change

Since width is now part of the cache key, `PreviewState::update` naturally
re-renders when width changes. No explicit invalidation call needed from the
resize or Ctrl+K/L handlers. The `preview_w` passed to `update()` will differ
on the next tick, triggering a cache miss.

## Risks / Trade-offs

- [Risk] Layout constants (`PREVIEW_MIN_WIDTH`, `LIST_MIN_WIDTH`) are currently
  `const` inside `render()`. Extracting them to module level or the helper
  function slightly changes code organization.
  → Mitigation: keep them as module-level `const` in `tui/view/mod.rs`, visible
  to both the render function and the exported helper.

- [Risk] `Layout::horizontal` behavior could change across ratatui versions,
  making the pre-computed width drift from the render-time width.
  → Mitigation: both call sites use the same helper, so they always agree.
