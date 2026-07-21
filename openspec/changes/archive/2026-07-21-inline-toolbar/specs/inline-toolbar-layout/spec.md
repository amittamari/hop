## ADDED Requirements

### Requirement: Toolbar controls render on the header row
The TUI SHALL render the Scope and Sort toolbar controls on the same row as the search input, right-aligned, when in simple search mode.

#### Scenario: Simple mode shows toolbar inline with search
- **WHEN** the search mode is Simple
- **THEN** the header row SHALL display the search input on the left and the Scope/Sort controls on the right, on a single row

#### Scenario: Raw mode shows no toolbar
- **WHEN** the search mode is Raw
- **THEN** the header row SHALL display only the search input, with no toolbar controls

#### Scenario: Toolbar focus styling preserved inline
- **WHEN** Tab is pressed to focus a toolbar control (Scope or Sort)
- **THEN** the focused control SHALL render with reversed/bold highlight styling on the header row, identical to current toolbar focus appearance

### Requirement: Vertical layout uses three bands
The TUI vertical layout SHALL use exactly three bands: header (1 row), body (flexible), footer (1 row). The toolbar SHALL NOT occupy a separate vertical band.

#### Scenario: Simple mode layout has no separate toolbar row
- **WHEN** the search mode is Simple and the terminal has N rows
- **THEN** the body area SHALL start at row 1 (immediately after the header), gaining 1 additional row compared to the previous 4-band layout

#### Scenario: Raw mode layout unchanged
- **WHEN** the search mode is Raw
- **THEN** the layout SHALL be identical to the previous raw-mode layout (header, body, footer — no toolbar row existed before either)

### Requirement: Header row uses horizontal split for query and toolbar
The header row SHALL be split horizontally with the query area taking flexible width (Min(0)) and the toolbar area taking fixed width (Length of toolbar display width).

#### Scenario: Query area shrinks to accommodate toolbar
- **WHEN** the terminal is narrow and the combined query + toolbar width exceeds the header width
- **THEN** the query area SHALL shrink while the toolbar area retains its fixed width

#### Scenario: Empty toolbar gives full width to query
- **WHEN** the toolbar Line has zero display width (raw mode)
- **THEN** the query area SHALL take the full header width
