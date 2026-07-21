# Capability: Search Index

## Purpose

Manages the Tantivy full-text search index: schema definition, document upsert/delete, incremental diff, multi-mode search with KWIC snippets, and scoring with recency boost.

## Requirements

### Requirement: Schema versioning
The index SHALL maintain a `.schema_version` marker file. When the on-disk version mismatches the current `SCHEMA_VERSION`, the entire index directory SHALL be wiped and recreated empty.

#### Scenario: Schema version mismatch
- **GIVEN** an existing index with schema version 4 and the current version is 5
- **WHEN** `open_or_create` is called
- **THEN** the index directory SHALL be wiped and a new empty index SHALL be created

### Requirement: Index fields
The schema SHALL index: `doc_key` (STRING), `id`, `agent`, `title` (TEXT), `content` (TEXT), `directory`, `timestamp` (u64, FAST), `mtime`, `message_count`, `yolo`, `branch`, `repo_url`, `source_path`, `archived`, `worktree`, `permission_mode`, `model`, `commit`, `sidecar_stamp`.

#### Scenario: Session round-trip through index
- **GIVEN** a session with all metadata fields populated
- **WHEN** upserted and then loaded back via `load_session`
- **THEN** all stored fields SHALL be preserved in the returned `Session`

### Requirement: Writer lifecycle
The index SHALL lazily create a single `IndexWriter` on first write and reuse it for the handle's lifetime. Read-only handles SHALL never acquire the writer lock. On drop, merge threads SHALL be waited on to release the directory lock deterministically.

#### Scenario: Read-only handle does not acquire writer
- **GIVEN** an index opened for searching only
- **WHEN** only `search` and `reload` are called
- **THEN** the writer lock SHALL never be acquired

### Requirement: Upsert semantics
`upsert` SHALL delete any existing document with the same `doc_key` term, then add the new document. This ensures idempotent re-indexing.

#### Scenario: Re-upsert replaces document
- **GIVEN** a session with doc_key `"claude:abc"` already indexed
- **WHEN** `upsert` is called with updated content for the same doc_key
- **THEN** search SHALL return only one result for that doc_key with the updated content

### Requirement: Search modes
`search` SHALL support three sort orders:
- **Relevance**: text score boosted by exact match (5x) and fuzzy term matches, combined with a recency boost (exponential decay, 1-week half-life). Scores are bucketed, with ties broken by timestamp.
- **Recent**: strict newest-first by timestamp.
- **Oldest**: strict oldest-first by timestamp.

#### Scenario: Recent mode orders by timestamp descending
- **GIVEN** sessions with timestamps 100, 200, 300
- **WHEN** search is called with `SortOrder::Recent`
- **THEN** results SHALL be ordered 300, 200, 100

### Requirement: Query construction
The search SHALL build a boolean query combining: free-text search (exact + fuzzy across title and content), agent include/exclude filters, and date range filters. Special characters SHALL be sanitized before passing to Tantivy's QueryParser.

#### Scenario: Special characters sanitized
- **GIVEN** a query containing `"auth:fix (bug)"`
- **WHEN** the query is sanitized
- **THEN** colons and parentheses SHALL be replaced with spaces

### Requirement: Post-filters
`dir:` and `repo:` filters SHALL be applied as post-filters (case-insensitive substring match) after Tantivy returns candidates, since these fields use STRING indexing without tokenization.

#### Scenario: Repo filter matches case-insensitively
- **GIVEN** a session with `repo_url: "git@github.com:ofirg/hop.git"`
- **WHEN** the query includes `repo:HOP`
- **THEN** the session SHALL be included in results

### Requirement: KWIC snippets
When the query has free-text terms, a `SnippetGenerator` SHALL produce HTML-formatted KWIC snippets from the `content` field. Non-empty snippets SHALL be attached to the result's `snippet` field.

#### Scenario: Snippet generated for text query
- **GIVEN** a session containing "auth refresh token" in its content
- **WHEN** the query is `"auth"`
- **THEN** the result's `snippet` SHALL be `Some` with HTML containing the matched term

#### Scenario: No snippet for empty query
- **WHEN** the query has no free-text terms
- **THEN** `snippet` SHALL be `None` for all results

### Requirement: Incremental diff
`diff` and `diff_authoritative` SHALL compare known mtimes against scanned entries to identify changed and deleted sessions. Deletions SHALL only apply to agents whose scan completed successfully (authoritative).

#### Scenario: Non-authoritative agent preserves rows
- **GIVEN** a Claude session is indexed but the Claude adapter scan fails
- **WHEN** `diff_authoritative` is called
- **THEN** the Claude session SHALL NOT appear in the deleted list

### Requirement: Pagination
Search SHALL fetch results in pages of 1000, applying post-filters progressively, until the requested limit is reached or all hits are exhausted.

#### Scenario: Post-filter reduces page below limit
- **GIVEN** 500 indexed sessions, 100 matching the text query, and 10 matching the `dir:` post-filter
- **WHEN** search is called with limit 50
- **THEN** the search SHALL return exactly 10 results after exhausting all pages
