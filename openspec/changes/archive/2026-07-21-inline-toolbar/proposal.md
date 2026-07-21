## Why

The simple-mode toolbar (Scope + Sort controls) occupies a dedicated row below the search input, consuming vertical space that could display an extra result row. On typical terminal heights (24-40 rows), every row matters for scan-ability. The toolbar controls are compact enough to share the header row with the search input.

## What Changes

- Merge the toolbar controls into the search input header row, right-aligned
- Remove the dedicated toolbar vertical band from the layout (4 bands → 3)
- Split the header row horizontally: query input left (flexible), toolbar right (fixed width)
- Remove or simplify `toolbar_rows()` since the toolbar no longer occupies its own row

## Capabilities

### New Capabilities

- `inline-toolbar-layout`: Render the Scope and Sort toolbar controls on the same row as the search input, right-aligned, using a horizontal split that mirrors the existing footer layout pattern

### Modified Capabilities

## Impact

- `src/tui/view.rs` — vertical layout bands reduced from 4 to 3; header area split horizontally; toolbar render site moves into header
- `src/tui/mod.rs` — `toolbar_rows()` method simplified or removed
- Render tests in `view.rs` that assert on toolbar position need updates
- No changes to `src/tui/toolbar.rs` (line builder stays identical)
- No changes to app logic, keybindings, or focus cycling behavior
