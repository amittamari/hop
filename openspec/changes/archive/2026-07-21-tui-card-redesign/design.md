## Context

The TUI renders search results as a single-row-per-session table via a column solver (`columns.rs`) that adapts to terminal width by dropping low-priority columns. The result list, preview pane, toolbar, and footer are composed in `view.rs::render()`. The data contract between the engine and TUI is `SessionSummary` — a flat struct populated from Tantivy docs in `SearchIndex::search()` and pushed into `App` via `sync_results_into_app()`.

Tantivy 0.26.1 ships `SnippetGenerator` which can extract KWIC fragments from stored fields. The `content` field is `TEXT | STORED` and the doc is already loaded for `to_summary()`, so snippet generation is additive CPU work with no extra I/O.

The TUI has a clean view-model separation: `App` is a state container, `RenderModel` is an ephemeral frame bag, and `view::render` is stateless. This makes branching on a config flag straightforward.

## Goals / Non-Goals

**Goals:**
- Replace dense 1-line rows with scannable multi-line card rows as the default layout
- Surface Tantivy KWIC snippets inline on search result cards so users see *why* a result matched
- Improve preview pane readability with explicit message separators
- Fix toolbar chip contrast with bracket notation
- Clean up footer noise
- Preserve the existing compact layout as a config option

**Non-Goals:**
- Theming or color customization beyond the existing `Theme` struct
- Keyboard shortcut changes (card selection still uses Up/Down)
- Changes to the search engine, query DSL, or indexing pipeline (only the result presentation layer)
- Mouse interaction or click targets on cards

## Decisions

### D1: Card row rendering bypasses the column solver

**Decision**: Card mode uses a fixed 3-line layout (agent+title+time / metadata / snippet) rendered as a multi-line ratatui `Row`. It does not use the `columns.rs` solver.

**Rationale**: The column solver is designed for single-line tabular rows with dynamic column widths. Cards have a fundamentally different layout — line 1 is a flex title with bookend values, line 2 is variable-length dot-separated metadata, line 3 is a snippet string. Forcing this through the column solver would require it to understand multi-line cells, which adds complexity for no benefit. Compact mode continues to use the solver unchanged.

**Alternatives considered**: (a) Extend the column solver to emit multi-line rows — rejected, high complexity for a layout that doesn't vary by terminal width in the same way. (b) Render each card as a nested widget — rejected, ratatui's `Table` already supports multi-line `Row` heights.

### D2: Snippet generation happens in the search path, not lazily

**Decision**: `SnippetGenerator` is created once per `SearchIndex::search()` call. Snippets are generated per-doc inside the existing result collection loop and stored in `SessionSummary.snippet`.

**Rationale**: The doc is already loaded for `to_summary()`. Snippet generation is CPU-only string scanning — microseconds per doc even for 100KB conversations. Generating eagerly for all results (typically ≤50 in a page) keeps the architecture simple: the TUI never calls back into the engine for display data. Lazy generation would require the TUI to hold a reference to the searcher, breaking the one-directional data flow.

**Alternatives considered**: (a) Generate only for visible rows — rejected, requires coupling the TUI viewport to the engine. (b) A separate snippet cache with async fill — rejected, over-engineering for a sub-millisecond operation.

### D3: Selection is a full box border, not a row highlight

**Decision**: The selected card gets a thin box border (`┌─┐│└─┘`) in the accent color on all four sides. Unselected cards have no border — just content with blank-line gaps.

**Rationale**: With multi-line rows, a background highlight across 2-3 lines is visually heavy and obscures the content styling (bold title, muted metadata, accent-colored snippet highlights). A box border clearly delineates the selected card without overpainting content styles.

**Implementation**: Ratatui's `Row` doesn't natively support per-row borders. The approach is to render the selected card as a `Paragraph` inside a `Block::bordered()` widget, while other cards render as plain `Paragraph` widgets in a manual vertical layout (not a `Table`). This means card mode abandons `Table` entirely in favor of manual `Rect` slicing — which is actually simpler since we don't need column alignment.

**Alternatives considered**: (a) Left-only accent bar (`▎`) — rejected by user, insufficient visual weight. (b) Background highlight on all card lines — rejected, conflicts with inline snippet highlighting and metadata styling.

### D4: Preview pane drops the metadata header in card mode

**Decision**: When `row_style = "card"`, the preview pane renders only the transcript with thin-rule message separators. The 3-line metadata header (title, agent/dir/branch/time, rule line) is removed.

**Rationale**: All metadata that was in the preview header now lives on the card row itself. Showing it twice wastes vertical space. In compact mode, the preview header is preserved since compact rows don't carry full metadata.

### D5: Config structure uses `[display]` section

**Decision**: A new `[display]` top-level section with `row_style = "card" | "compact"`. Default is `"card"`.

**Rationale**: `row_style` is orthogonal to `[columns]` (which only applies in compact mode) and `[preview]` (which controls a separate pane). A dedicated section keeps concerns separated and leaves room for future display settings.

### D6: Toolbar chips use bracket notation

**Decision**: Selected/focused values render as `[Value]` with bold styling. Unfocused values render as plain `Value` in accent color. No background color blocks.

**Rationale**: The current white-on-cyan has poor contrast on many terminal color schemes. Brackets are structural (work in monochrome), universally readable, and lighter weight visually.

## Risks / Trade-offs

**[Reduced session density]** → Card rows show ~12 sessions per 40-row terminal vs ~38 with compact rows. Mitigated by preserving compact mode as a config option and by the observation that most workflows (search or LRU) don't need 38 visible rows.

**[SnippetGenerator quality]** → Tantivy's built-in snippet generator uses a simple term-position algorithm, not a sophisticated summarizer. Snippets may occasionally be low-signal (e.g., matching a common word in a boilerplate section). → Acceptable for v1; the snippet is supplementary to the title, not the primary identifier. Can be refined later with custom snippet scoring.

**[Card mode abandons Table widget]** → Moving from `Table` to manual `Rect` layout for card mode means reimplementing scroll/selection mechanics that `Table` + `TableState` provided. → The existing `visible_result_range()` already computes the viewport window; translating that to manual layout is straightforward. The compact path still uses `Table` unchanged.

**[Snippet field inflates SessionSummary]** → Each snippet is a short string (~100-200 chars). For 50 results, this adds negligible memory. The field is `Option<String>` so it's zero-cost when empty (LRU browsing).
