## 1. Config & Data Model

- [x] 1.1 Add `DisplayConfig` struct to `config.rs` with `row_style: String` field (default `"card"`), add `[display]` TOML deserialization, and expose `RowStyle` enum (`Card` / `Compact`)
- [x] 1.2 Add `snippet: Option<String>` field to `SessionSummary` in `core.rs`, derive Default as `None`
- [x] 1.3 Change `[preview] visible` default from `true` to `false` in config defaults and `App::new()`

## 2. Snippet Extraction

- [x] 2.1 In `SearchIndex::search()`, create a `tantivy::SnippetGenerator` for the `content` field when the query has free terms, call `snippet_from_doc()` per result, and populate `SessionSummary.snippet`
- [x] 2.2 Add unit test: search with a query produces non-None snippets containing the query terms
- [x] 2.3 Add unit test: search with empty query produces None snippets

## 3. Card Row Rendering

- [x] 3.1 Add `card_row()` function to `results_list.rs` that builds a multi-line card: line 1 (agent + bold title + right-aligned time), line 2 (muted dot-separated metadata), optional line 3 (snippet with bold+accent matched terms)
- [x] 3.2 Add `card_selection_border()` rendering: draw a `Block::bordered()` in accent color around the selected card, no border for unselected cards
- [x] 3.3 In `view.rs`, add the card-mode rendering path in `render()`: replace the `Table` widget with manual vertical `Rect` layout that computes card heights (2 or 3 lines + separator), slices areas, and renders each card. Branch on `RowStyle` so compact mode still uses the existing `Table` path unchanged
- [x] 3.4 Update `visible_result_range()` to account for variable card heights (2-3 content lines + 1 separator per card) when computing the visible window
- [x] 3.5 Wire `RowStyle` from config through the render path: `main.rs` reads `DisplayConfig`, passes `row_style` to `RenderModel` or `App`
- [x] 3.6 Add unit tests for card rendering: card with snippet shows 3 content lines, card without snippet shows 2, selected card has border, compact mode renders legacy layout

## 4. Preview Pane Redesign

- [x] 4.1 Add `render_transcript_with_separators()` to `preview.rs`: thin-rule message separators (`── user ────` / `── claude ────`) with bold role name, replacing `› ` and `● BADGE` prefixes
- [x] 4.2 In `view.rs`, skip the preview metadata header when `row_style` is `Card`; preserve header for `Compact` mode
- [x] 4.3 Add unit tests for the new separator rendering: user messages get `── user ──` prefix, agent messages get `── <badge> ──` prefix

## 5. Toolbar & Footer Cleanup

- [x] 5.1 Modify `push_control()` in `toolbar.rs`: unfocused chips render as plain `Value` in accent color; focused chips render as `[Value]` in bold
- [x] 5.2 Remove the `filters` span from `footer_status_line()` in `view.rs`
- [x] 5.3 Update toolbar and footer unit tests to match new rendering

## 6. Integration & Polish

- [x] 6.1 Run full test suite (`cargo test`), fix any regressions from SessionSummary field addition or default changes
- [x] 6.2 Manual TUI testing: verify card layout, selection border, snippet display, preview separators, toolbar chips, footer cleanup across normal and narrow terminal widths (requires interactive terminal)
- [x] 6.3 Test compact mode: verify `row_style = "compact"` renders identically to pre-change behavior (requires interactive terminal)
- [x] 6.4 Update `docs/ARCHITECTURE.md` with card-mode rendering path and `[display]` config section
