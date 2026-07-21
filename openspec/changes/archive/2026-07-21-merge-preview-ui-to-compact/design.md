## Context

The preview panel has two transcript rendering paths gated on `RowStyle`:

- **Card mode** (`render_transcript_with_separators`): Thin horizontal rules
  between messages (`── user ──────` / `── claude ──────`). Clean, flat
  content with no per-line role prefixes.
- **Compact mode** (`render_transcript_with_terms`): Inline role markers —
  user messages prefixed with `› `, agent messages headed by `● BADGE` and
  indented by two spaces.

The branching lives in `PreviewState::update` (`preview.rs:459-469`), which
takes a `use_separators: bool` parameter. The caller in `main.rs:351` passes
`row_style == RowStyle::Card`.

The metadata header (`preview_header.rs`) is already compact-only and stays
unchanged.

## Goals / Non-Goals

**Goals:**

- Unify preview transcript rendering so both modes use the separator-based
  style.
- Remove dead code (the old rendering path and its helpers).
- Simplify the `PreviewState::update` API.

**Non-Goals:**

- Changing the metadata header behavior (already compact-only, stays as-is).
- Changing the list-side rendering (card rows vs table rows).
- Adding new preview features.

## Decisions

### Always use separator-based rendering

Remove the `use_separators` branch in `PreviewState::update`. Always call
`render_transcript_with_separators`. This means `preview_width` must always
be available — it already is, since the caller computes it regardless of mode.

**Alternative considered**: Keep both renderers and let the user pick via
config. Rejected — the separator style is strictly better (cleaner visual
hierarchy, no indentation that wastes horizontal space), and maintaining two
paths adds complexity for no user benefit.

### Remove dead code

After unifying, `render_transcript_with_terms`, `prefix_first`, and `indent`
become unreachable. Delete them and their tests. The public
`render_for_preview` wrapper that calls `render_transcript_with_terms` also
needs updating to route through the separator path (it takes no width
argument today, so it needs one added or the call site refactored).

### Keep the `preview_width` plumbing as-is

`render_transcript_with_separators` needs the preview width to draw the
horizontal rules. The caller already computes this in `main.rs:340-343`.
No plumbing changes needed.

## Risks / Trade-offs

- **Visual change for compact-mode users**: Users who configured
  `row_style = "compact"` will see a different preview format. The separator
  style is the default experience (card mode is the default row style), so
  this aligns compact with what most users already see. Low risk.
- **Test churn**: Tests asserting on the old `› ` / `● BADGE` output format
  need updating. Mechanical, no logic risk.
