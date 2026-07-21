## MODIFIED Requirements

### Requirement: KWIC snippets
When the query has free-text terms, a custom snippet builder SHALL produce HTML-formatted KWIC snippets scoped to a single message from the `content` field. The builder SHALL split the stored content by the message separator (`\x1E`), select the message with the most term occurrences, and extract a KWIC window within that message. Non-empty snippets SHALL be attached to the result's `snippet` field.

#### Scenario: Snippet generated for text query
- **GIVEN** a session containing "auth refresh token" in one message and "please fix it" in another
- **WHEN** the query is `"auth"`
- **THEN** the result's `snippet` SHALL be `Some` with HTML from the message containing "auth", not spanning both messages

#### Scenario: No snippet for empty query
- **WHEN** the query has no free-text terms
- **THEN** `snippet` SHALL be `None` for all results

### Requirement: Message separator in indexed content
The `content` field stored in the index SHALL use ASCII Record Separator (`\x1E`) as the delimiter between messages. Blocks within a single message SHALL be delimited by `\n`. This enables the snippet builder to split content back into per-message chunks at search time.

#### Scenario: Content field format
- **GIVEN** a session with two messages, the first containing blocks "hello" and "world", the second containing "goodbye"
- **WHEN** the session is indexed
- **THEN** the stored `content` field SHALL be `"hello\nworld\x1Egoodbye"`

#### Scenario: Schema version bump
- **WHEN** the application starts with an index built before this change
- **THEN** the schema version mismatch SHALL trigger an automatic index rebuild with the new separator format
