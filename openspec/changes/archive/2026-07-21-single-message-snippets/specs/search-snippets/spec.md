## MODIFIED Requirements

### Requirement: Snippet field on SessionSummary
`SessionSummary` SHALL include a `snippet: Option<String>` field that carries the KWIC snippet text extracted from the indexed conversation content. The snippet SHALL be scoped to a single message — it SHALL NOT span text from multiple messages.

#### Scenario: Search with matching content
- **WHEN** a search query matches conversation content in the Tantivy index
- **THEN** the `snippet` field SHALL contain a text fragment from a single message showing the matched terms in context

#### Scenario: Empty query (LRU browsing)
- **WHEN** the search query is empty
- **THEN** the `snippet` field SHALL be `None`

#### Scenario: Query matches title only
- **WHEN** the query terms match the title but not the conversation content
- **THEN** the `snippet` field MAY be `None` or contain a title-derived fragment

#### Scenario: Match spans multiple messages
- **WHEN** the query term appears in multiple messages
- **THEN** the snippet SHALL be drawn from the single message with the most term occurrences, not from a window spanning multiple messages

### Requirement: Snippet generation uses custom single-message builder
The search path SHALL use a custom snippet builder that splits stored content by the message separator (`\x1E`), selects the best-matching message, and extracts a KWIC window within that message. The builder SHALL produce `<b>term</b>` HTML output compatible with the existing `snippet_line` renderer.

#### Scenario: Snippet builder selects best message
- **WHEN** a session's content contains messages `["the auth token expired", "I will implement the auth fix now", "please implement it"]` and the query is `"implement"`
- **THEN** the snippet SHALL be drawn from one of the messages containing "implement", not from a window crossing message boundaries

#### Scenario: Snippet builder wraps matched terms
- **WHEN** the best-matching message contains the term "auth"
- **THEN** the snippet output SHALL contain `<b>auth</b>` with surrounding context in plain text

#### Scenario: Snippet builder respects max length
- **WHEN** the best-matching message is longer than the snippet window size
- **THEN** the snippet SHALL be a substring centered on the first term occurrence, not the entire message

#### Scenario: Short message fits entirely
- **WHEN** the best-matching message is shorter than the snippet window size
- **THEN** the snippet SHALL contain the entire message text with matched terms wrapped in `<b>` tags

#### Scenario: Newlines collapsed to spaces
- **WHEN** the best-matching message contains newlines (from multiple blocks within a single message)
- **THEN** the snippet SHALL replace newlines with spaces so the snippet renders as a single line

## REMOVED Requirements

### Requirement: Snippet generation uses Tantivy SnippetGenerator
**Reason**: Replaced by custom single-message snippet builder that respects message boundaries.
**Migration**: No API change — the `snippet` field on `SessionSummary` retains the same `Option<String>` type and `<b>term</b>` HTML format. The `tantivy::snippet::SnippetGenerator` import and usage are removed from `index.rs`.
