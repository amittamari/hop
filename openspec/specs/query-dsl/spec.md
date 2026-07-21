# Capability: Query DSL

## Purpose

Parses the hop query language into a structured `ParsedQuery`, supporting free-text search, agent/directory/repo filters with include/exclude, date filters with named periods and duration comparisons, sort order selection, tab autocomplete, and simple-mode query composition.

## Requirements

### Requirement: Free-text extraction
Tokens that are not recognized keyword filters SHALL be collected as `free_text`, preserving order and whitespace joining.

#### Scenario: Mixed tokens
- **GIVEN** query `"auth agent:claude refresh"`
- **WHEN** parsed
- **THEN** `free_text` SHALL be `"auth refresh"`

### Requirement: Agent filter
`agent:<slug>` SHALL add the agent to the include list. Comma-separated slugs (`agent:claude,codex`) SHALL be supported. A `!` prefix on individual slugs or `-`/`!` on the entire token SHALL negate (exclude).

#### Scenario: Mixed include/exclude
- **GIVEN** query `"agent:claude,!codex login"`
- **WHEN** parsed
- **THEN** include SHALL be `[Claude]`, exclude SHALL be `[Codex]`, free_text SHALL be `"login"`

### Requirement: Directory filter
`dir:<value>` SHALL add a directory substring include; `-dir:<value>` SHALL exclude.

#### Scenario: Directory include and exclude
- **GIVEN** query `"dir:api -dir:vendor bug"`
- **WHEN** parsed
- **THEN** dirs.include SHALL be `["api"]`, dirs.exclude SHALL be `["vendor"]`, free_text SHALL be `"bug"`

### Requirement: Repo filter
`repo:<value>` SHALL add a repo URL substring include; `-repo:<value>` SHALL exclude. This matches across all worktrees of a repository.

#### Scenario: Repo include
- **GIVEN** query `"repo:hop"`
- **WHEN** parsed
- **THEN** repos.include SHALL be `["hop"]`

### Requirement: Date filter
`date:` SHALL support named periods (`today`, `yesterday`, `week`, `month`), within-duration (`<Nh`, `<Nd`, `<Nw`), and older-than (`>Nh`, `>Nd`, `>Nw`). `today` and `yesterday` SHALL use local calendar-day boundaries via the system timezone.

#### Scenario: Duration parsing
- **GIVEN** `date:<2d`
- **WHEN** parsed
- **THEN** the filter SHALL be `Within(2 * 86400)`

### Requirement: Sort order
`SortOrder` SHALL provide three modes: `Relevance` (default, blends text score with recency), `Recent` (newest-first), and `Oldest` (oldest-first). Each SHALL have a stable label and support forward/backward cycling.

#### Scenario: Sort cycles forward
- **GIVEN** sort is `Relevance`
- **WHEN** `next()` is called
- **THEN** sort SHALL be `Recent`

#### Scenario: Sort cycles backward
- **GIVEN** sort is `Relevance`
- **WHEN** `prev()` is called
- **THEN** sort SHALL be `Oldest`

### Requirement: Tab autocomplete
`autocomplete` SHALL complete the last token's value when unambiguous: `agent:cl` -> `agent:claude`, `date:to` -> `date:today`. Already-complete or ambiguous tokens SHALL return `None`.

#### Scenario: Unambiguous completion
- **GIVEN** input `"agent:cl"`
- **WHEN** `autocomplete` is called
- **THEN** it SHALL return `Some("agent:claude")`

#### Scenario: Already complete
- **GIVEN** input `"agent:claude"`
- **WHEN** `autocomplete` is called
- **THEN** it SHALL return `None`

### Requirement: Simple mode composition
`compose_simple` SHALL prepend a `repo:` scope token (when present) to the user's free text, producing a query string the standard parser can consume.

#### Scenario: Repo scope with free text
- **GIVEN** free text `"auth"` and repo scope `"me/hop"`
- **WHEN** `compose_simple` is called
- **THEN** the result SHALL be `"repo:me/hop auth"`

### Requirement: Free term extraction
`free_terms` SHALL return deduplicated, lowercased free-text terms for highlighting in the TUI.

#### Scenario: Deduplication and lowercasing
- **GIVEN** query `"Auth agent:codex Auth dir:api"`
- **WHEN** `free_terms` is called on the parsed query
- **THEN** it SHALL return `["auth"]`
