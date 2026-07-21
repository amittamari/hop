# Capability: TUI Toolbar

## Purpose

Renders the simple-mode search toolbar: guided Scope and Sort controls shown under the query input, providing a structured surface over the search DSL for common filtering and sorting.

## Requirements

### Requirement: Scope control
`Scope` SHALL provide two values: `ThisRepo` (injects the launch repo's `repo:` slug) and `All` (no repo filter). `toggled()` SHALL switch between them.

#### Scenario: Scope toggles
- **GIVEN** scope is `ThisRepo`
- **WHEN** `toggled()` is called
- **THEN** the result SHALL be `All`

### Requirement: Sort control
The toolbar SHALL display the current `SortOrder` label. Left/Right SHALL cycle through Relevance/Recent/Oldest.

#### Scenario: Sort label displayed
- **GIVEN** the current sort is `Recent`
- **WHEN** the toolbar is rendered
- **THEN** the Sort control SHALL display `"Recent"`

### Requirement: Focus model
`Focus` SHALL cycle through Query, Scope, and Sort via Tab (forward) and Shift+Tab (reverse). When the launch directory has no resolvable repo (`has_repo == false`), the Scope control SHALL be skipped in the cycle.

#### Scenario: Focus cycles with repo
- **GIVEN** `has_repo` is true and focus is on Query
- **WHEN** `next` is called
- **THEN** focus SHALL move to Scope

#### Scenario: Focus skips scope without repo
- **GIVEN** `has_repo` is false and focus is on Query
- **WHEN** `next` is called
- **THEN** focus SHALL move to Sort

### Requirement: Visual feedback
The focused control SHALL render its value in bracket notation (`[Value]`) with bold accent styling. Unfocused controls SHALL render the value in accent color without brackets.

#### Scenario: Focused control shows brackets
- **GIVEN** focus is on the Sort control with value `"Relevance"`
- **WHEN** the toolbar is rendered
- **THEN** the Sort value SHALL appear as `"[Relevance]"` in bold accent style

### Requirement: Scope visibility
When `has_repo` is false, the Scope control SHALL be hidden entirely from the toolbar line.

#### Scenario: Toolbar without repo
- **GIVEN** `has_repo` is false
- **WHEN** the toolbar line is rendered
- **THEN** the output SHALL not contain the word `"Scope"`
- **AND** the Sort control SHALL still be visible

### Requirement: Rendering
`toolbar::line` SHALL produce a `ratatui::Line` with the controls, suitable for rendering beneath the query input area.

#### Scenario: Line contains controls
- **WHEN** `toolbar::line` is called with a repo and default settings
- **THEN** the returned Line SHALL contain spans for Scope and Sort labels and values
