# Capability: TUI Columns

## Purpose

Pluggable result-list column definitions and a responsive layout solver. Produces resolved widths from column definitions and available terminal width. Used by the compact row style.

## Requirements

### Requirement: Column definition
Each column SHALL have: `id` (stable config key), `header` (display label), `align` (Left or Right), `priority` (lower drops last), `min_width`, and `flex` (whether it absorbs leftover space).

#### Scenario: Title column is flex
- **WHEN** the default column set is inspected
- **THEN** the `"title"` column SHALL have `flex: true`
- **AND** all other columns SHALL have `flex: false`

### Requirement: Default column set
The default columns SHALL be: agent, repo, branch, title (flex), msgs, model, pr, time. Title SHALL be the only flex column.

#### Scenario: Default column ids
- **WHEN** `default_columns()` is called
- **THEN** the ids SHALL be `["agent", "repo", "branch", "title", "msgs", "model", "pr", "time"]`

### Requirement: Config-driven customization
`configured_columns` SHALL apply user preferences: columns listed in `disabled` SHALL be removed; columns listed in `order` SHALL appear first in that order, with remaining enabled columns preserving default order.

#### Scenario: Disabling a column
- **GIVEN** disabled list `["pr", "msgs"]`
- **WHEN** `configured_columns` is called
- **THEN** the result SHALL not contain columns with ids `"pr"` or `"msgs"`

### Requirement: Layout solver
`solve` SHALL distribute available width across visible columns:
1. All columns receive their `min_width`.
2. Non-flex columns grow to fit their content (up to `max_widths`), starting from the lowest-priority columns.
3. Any remaining width goes to flex columns.
4. When total `min_width` exceeds available space, columns are dropped in priority order (highest priority number first) until they fit. Columns with `priority == u8::MAX` are never dropped.

#### Scenario: Narrow terminal drops low-priority columns
- **GIVEN** a terminal width too narrow for all columns
- **WHEN** `solve` is called
- **THEN** columns with the highest priority number SHALL be dropped first
- **AND** columns with `priority == u8::MAX` SHALL never be dropped

### Requirement: Content fitting
`fit` SHALL truncate a cell value to the column width with an ellipsis when it overflows, respecting alignment (left or right).

#### Scenario: Long value truncated
- **GIVEN** a value `"very long title text"` and width 10
- **WHEN** `fit` is called with Left alignment
- **THEN** the result SHALL be 10 characters wide ending with an ellipsis

### Requirement: Cell rendering
`render_cell` SHALL produce a styled `Span` with correct alignment, width padding, and optional highlighting of matched query terms.

#### Scenario: Cell with matched term
- **GIVEN** a cell value containing a matched query term
- **WHEN** `render_cell` is called with highlight terms
- **THEN** the matched portion SHALL be styled with the match color
