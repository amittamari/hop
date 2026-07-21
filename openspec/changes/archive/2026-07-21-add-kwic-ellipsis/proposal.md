## Why

KWIC snippets shown on search result cards start and end abruptly, with no visual indication that the displayed text is a fragment extracted from a larger conversation. Adding leading/trailing ellipsis (`…`) makes it immediately clear the snippet is a window into longer content.

## What Changes

- Add a `…` prefix when the snippet does not begin at the start of the indexed content.
- Add a `…` suffix when the snippet does not end at the end of the indexed content.
- Render ellipsis characters in the muted style, consistent with non-highlighted snippet text.

## Capabilities

### New Capabilities

_(none)_

### Modified Capabilities

- `search-snippets`: Snippets SHALL include leading/trailing ellipsis to indicate truncation boundaries.

## Impact

- `src/tui/results_list.rs` — `snippet_line()` rendering logic gains ellipsis handling.
- `src/index.rs` — snippet extraction may need to attach boundary metadata, or the renderer can infer boundaries from the snippet text itself.
- No dependency or API changes.
