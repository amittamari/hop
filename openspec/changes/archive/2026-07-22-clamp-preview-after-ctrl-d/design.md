## Context

The TUI preview pane scrolls via `preview_scroll: u16` on the `App` struct.
Scroll-down (Ctrl+D, mouse wheel) adds to this value; scroll-up (Ctrl+U)
subtracts. Jump-to-match (Ctrl+N) sets it directly to a match line number.
Only the lower bound is enforced (`max(0)`); there is no upper bound.

Ratatui's `Paragraph::scroll((row, col))` silently renders nothing when `row`
exceeds the content height, so overshooting leaves the preview blank.

The `App` has no knowledge of how many lines the preview contains — the line
count lives in `PreviewState::lines` outside the `App` struct. Scroll mutations
happen inside `App` methods, so they cannot clamp today.

## Goals / Non-Goals

**Goals:**
- Prevent `preview_scroll` from exceeding the transcript length on any scroll
  source (keyboard, mouse, match-jump).
- Keep the fix minimal — one new field, one setter, clamping at mutation sites.

**Non-Goals:**
- Adding "scroll-to-bottom" snap behavior or elastic overscroll.
- Changing the scroll step size or paging semantics.
- Refactoring `PreviewState` ownership.

## Decisions

### Store `preview_line_count` on `App`

Add a `preview_line_count: u16` field to `App`, set by a new
`set_preview_line_count(n)` method called from `PreviewState::update()` after
lines are computed. This mirrors the existing pattern of `set_preview_matches`.

Alternative considered: pass a max-scroll parameter into each scroll method.
Rejected because there are four independent scroll mutation sites (keyboard
scroll, mouse wheel, jump-to-match, match-set); threading a parameter through
all of them is more invasive and error-prone than reading a field.

### Clamp formula

```
max_scroll = preview_line_count.saturating_sub(1)
preview_scroll = preview_scroll.min(max_scroll)
```

Using `line_count - 1` (not `line_count - visible_height`) so the user can
always scroll the last line to the top of the viewport. This is consistent with
how code editors work and avoids needing the viewport height in the clamp.

The clamp is applied:
1. In `ScrollPreview` (Ctrl+D / Ctrl+U)
2. In `handle_mouse` (wheel scroll)
3. In `jump_preview_match` (Ctrl+N / Ctrl+Shift+N)
4. In `set_preview_line_count` itself (re-clamp when content changes)

### Centralize clamping in a helper

Add a private `fn clamp_preview_scroll(&mut self)` that applies the formula.
Called from each mutation site after updating `preview_scroll`. Keeps the
invariant in one place.

## Risks / Trade-offs

- [Stale line count] If the preview content changes without a
  `set_preview_line_count` call, the clamp uses a stale ceiling. → Mitigated:
  `PreviewState::update()` always calls the setter when it recomputes lines,
  and content only changes on selection change or query change — both of which
  reset `preview_scroll` to 0 anyway.
