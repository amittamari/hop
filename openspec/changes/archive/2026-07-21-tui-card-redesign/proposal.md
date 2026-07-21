## Why

The TUI result list packs 8 columns into a single-line row. This maximizes session density but makes individual rows hard to scan — metadata is cramped, visually undifferentiated, and partially duplicated in the preview header. Most users either search (and need to see *why* a result matched) or browse recent sessions (and need to orient quickly on *which* session this is). Neither workflow benefits from raw density. The preview pane also lacks visual separation between user and agent messages, and toolbar filter chips have a contrast issue (white text on cyan background).

## What Changes

- Replace the 1-line-per-result table rows with multi-line "card" rows: line 1 (agent + title + time), line 2 (metadata), optional line 3 (KWIC search snippet). Selected card gets a full box border in accent color.
- Add Tantivy `SnippetGenerator` integration to extract keyword-in-context snippets from indexed conversation content, surfaced inline on each search result card.
- Redesign the preview pane: off by default, no metadata header (metadata moves to the card row), thin-rule separators with bold role names between user/agent messages.
- Restyle toolbar chips from background-colored blocks to bracket notation (`[value]`) for better contrast.
- Clean up footer: remove the `filters` echo that redundantly shows the resolved query.
- Preserve the existing compact (1-line) layout behind a `[display] row_style = "compact"` config flag.

## Capabilities

### New Capabilities
- `card-rows`: Multi-line card layout for the results list with dynamic row height (2 lines when browsing, 3 lines when searching with snippet).
- `search-snippets`: Tantivy SnippetGenerator integration to extract and display KWIC snippets from indexed conversation content on search result cards.

### Modified Capabilities
<!-- No existing specs to modify — this is the first spec-driven change in this repo. -->

## Impact

- **Core types**: `SessionSummary` gains `snippet: Option<String>` field.
- **Index**: `SearchIndex::search()` creates `SnippetGenerator` per query and populates snippets per result.
- **TUI rendering**: `results_list.rs` gains `card_row()` alongside existing `session_row()`. `view.rs` branches on `row_style` config. `preview.rs` gains thin-rule separator rendering. `toolbar.rs` switches chip style.
- **Config**: `config.rs` adds `DisplayConfig` with `row_style` field. `[preview] visible` default changes to `false`.
- **Dependencies**: No new crate dependencies — `tantivy::SnippetGenerator` is already available in tantivy 0.26.1.
