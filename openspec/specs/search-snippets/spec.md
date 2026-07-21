# Capability: Search Snippets

## Purpose

Defines how keyword-in-context (KWIC) snippets are generated from Tantivy search results and carried through SessionSummary for display in the TUI card layout.

## Requirements

### Requirement: Snippet field on SessionSummary
`SessionSummary` SHALL include a `snippet: Option<String>` field that carries the KWIC snippet text extracted from the indexed conversation content.

#### Scenario: Search with matching content
- **WHEN** a search query matches conversation content in the Tantivy index
- **THEN** the `snippet` field SHALL contain a text fragment showing the matched terms in context

#### Scenario: Empty query (LRU browsing)
- **WHEN** the search query is empty
- **THEN** the `snippet` field SHALL be `None`

#### Scenario: Query matches title only
- **WHEN** the query terms match the title but not the conversation content
- **THEN** the `snippet` field MAY be `None` or contain a title-derived fragment

### Requirement: Snippet generation uses Tantivy SnippetGenerator
The search path SHALL use `tantivy::SnippetGenerator` to extract snippets from the stored `content` field. The generator SHALL be created once per search invocation, not per document.

#### Scenario: Snippet generator lifecycle
- **WHEN** `SearchIndex::search()` is called with a non-empty query
- **THEN** a `SnippetGenerator` SHALL be created once targeting the `content` field
- **THEN** `snippet_from_doc()` SHALL be called for each result document

#### Scenario: Performance on large conversations
- **WHEN** a session has a large stored content field (100KB+)
- **THEN** snippet generation SHALL complete without blocking the TUI render loop (sub-millisecond per doc)

### Requirement: Snippet highlight rendering
When a snippet is displayed on a card row, matched terms within the snippet text SHALL be rendered in bold with the theme's accent color.

#### Scenario: Single term match
- **WHEN** a snippet contains one matched term
- **THEN** the matched term SHALL render in bold + accent color and the surrounding context SHALL render in muted style

#### Scenario: Multiple term matches
- **WHEN** a snippet contains multiple matched terms
- **THEN** each matched term SHALL render in bold + accent color independently

#### Scenario: No snippet displayed when absent
- **WHEN** a session's snippet field is `None`
- **THEN** no snippet line SHALL be rendered on the card (row height reduces to 2 lines)

### Requirement: Snippet ellipsis indicators

When a KWIC snippet is rendered on a card row, the display SHALL include leading and trailing `...` (three ASCII dots) to indicate the snippet is a fragment of longer content. Tantivy's inter-fragment `...` separators pass through unchanged.

#### Scenario: Leading ellipsis on snippet
- **WHEN** a snippet is rendered on a card row
- **THEN** the displayed text SHALL begin with `...` followed by the snippet content

#### Scenario: Trailing ellipsis on snippet
- **WHEN** a snippet is rendered on a card row
- **THEN** the displayed text SHALL end with `...` after the snippet content

#### Scenario: Ellipsis style
- **WHEN** ellipsis indicators are rendered (leading or trailing)
- **THEN** they SHALL use the muted text style, consistent with non-highlighted snippet text

#### Scenario: Ellipsis within width budget
- **WHEN** a snippet is rendered on a narrow terminal
- **THEN** the leading and trailing ellipsis SHALL be included within the available width budget, reducing the visible context text rather than causing overflow
