## Context

Tantivy's `SnippetGenerator` extracts keyword-in-context fragments from indexed conversation content. `Snippet::to_html()` returns HTML with `<b>` tags around matched terms and joins non-contiguous fragments with `...`. However, it adds no leading or trailing indicators — the snippet starts and ends abruptly, giving no visual cue that it's a window into longer content.

The rendering function `snippet_line()` in `results_list.rs` parses the HTML into styled `Span`s. Ellipsis handling belongs here since it's purely a display concern.

## Goals / Non-Goals

**Goals:**
- Add `...` prefix/suffix to rendered snippets to indicate truncation boundaries.
- Keep ellipsis within the existing width budget so they don't cause overflow.

**Non-Goals:**
- Changing snippet extraction or Tantivy configuration.
- Detecting whether the snippet truly starts/ends at a content boundary (conversations are long enough that snippets are always fragments in practice).
- Normalizing Tantivy's inter-fragment `...` — it already uses ASCII dots, consistent with the leading/trailing indicators.

## Decisions

### Always show leading and trailing ellipsis

Indexed conversations are typically thousands of tokens. A KWIC snippet is virtually never the full content. Rather than threading fragment-range metadata from the index layer to detect boundary positions, always render `...` at both ends.

**Alternative considered:** Check `Snippet::fragment_ranges()` against content length to conditionally show ellipsis. Rejected — adds complexity across layers for an edge case that doesn't occur in practice with full-conversation indexing.

### Use ASCII dots, not Unicode ellipsis

ASCII `...` is the native TUI idiom for truncation and matches what Tantivy already emits between non-contiguous fragments. Unicode `…` looks cramped in monospace terminals.

### Render ellipsis in muted style

Ellipsis indicators are structural chrome, not content. Render them in the same muted style as non-highlighted snippet text.

### Handle in the rendering layer only

All changes stay in `snippet_line()`. The index layer (`src/index.rs`) and `SessionSummary.snippet` field remain unchanged — the stored value is still raw Tantivy HTML.

## Risks / Trade-offs

- **Six characters of width consumed** — The leading and trailing `...` take 3 columns each from the snippet display budget. On very narrow terminals this means less context shown. Acceptable since the visual clarity gained outweighs the space cost.
