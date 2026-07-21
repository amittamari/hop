# Capability: Card Rows

## Purpose

Defines the multi-line card layout for session results in the TUI, including selection styling, compact mode fallback, display configuration, preview pane behavior, and toolbar notation.

## Requirements

### Requirement: Card row layout
The results list SHALL render each session as a multi-line card when `row_style` is `"card"`. Each card SHALL have 2 or 3 content lines plus a blank separator line after it.

#### Scenario: Browsing without a query
- **WHEN** the query input is empty (LRU browsing)
- **THEN** each card SHALL render 2 content lines:
  - Line 1: agent badge (colored) + session title (bold, flex width) + relative time (right-aligned, muted)
  - Line 2: dot-separated metadata (repo, branch, PR, model, message count) in muted style, showing only non-empty values

#### Scenario: Searching with a query
- **WHEN** the query input is non-empty and the session has a snippet
- **THEN** each card SHALL render 3 content lines:
  - Lines 1-2: same as browsing layout
  - Line 3: KWIC snippet text with matched terms in bold + accent color

#### Scenario: Searching but no snippet available
- **WHEN** the query input is non-empty but the session has no snippet (snippet is None)
- **THEN** the card SHALL render 2 content lines (same as browsing layout)

### Requirement: Card selection border
The selected card SHALL be visually distinguished by a thin box border on all four sides rendered in the theme's accent color. Unselected cards SHALL have no border.

#### Scenario: Navigating the list
- **WHEN** the user presses Up/Down to change selection
- **THEN** the box border SHALL move to the newly selected card and be removed from the previously selected card

#### Scenario: Terminal width variation
- **WHEN** the terminal is narrower than the card content
- **THEN** the card title and metadata lines SHALL truncate with ellipsis rather than wrapping, and the box border SHALL fit the available width

### Requirement: Compact mode preservation
When `row_style` is `"compact"`, the results list SHALL render using the existing single-line table layout with the column solver. All existing compact-mode behavior (column priorities, header row, highlight style) SHALL be preserved unchanged.

#### Scenario: Compact mode renders legacy layout
- **WHEN** config contains `[display] row_style = "compact"`
- **THEN** the results list SHALL render identical to the pre-change single-line table layout

### Requirement: Display configuration
The `[display]` config section SHALL accept a `row_style` field with values `"card"` (default) or `"compact"`.

#### Scenario: Default config
- **WHEN** no `[display]` section exists in config
- **THEN** `row_style` SHALL default to `"card"`

#### Scenario: Explicit compact config
- **WHEN** config contains `[display] row_style = "compact"`
- **THEN** the compact single-line layout SHALL be used

### Requirement: Preview pane defaults to off
The preview pane SHALL default to not visible. Users MAY enable it via Ctrl+P toggle or by setting `[preview] visible = true` in config.

#### Scenario: Fresh install preview state
- **WHEN** no config or UI state file exists
- **THEN** the preview pane SHALL not be visible

#### Scenario: Toggling preview on
- **WHEN** the user presses Ctrl+P
- **THEN** the preview pane SHALL become visible and its state SHALL be persisted

### Requirement: Preview message separators
When the preview pane is visible, messages SHALL be separated by thin horizontal rules with a bold role name label.

#### Scenario: User message separator
- **WHEN** a user message is rendered in the preview
- **THEN** it SHALL be preceded by a thin rule line formatted as `── user ────` with the role name in bold

#### Scenario: Agent message separator
- **WHEN** an agent message is rendered in the preview
- **THEN** it SHALL be preceded by a thin rule line formatted as `── <agent> ────` where `<agent>` is the agent's badge name (e.g., `claude`, `codex`) in bold

### Requirement: Preview pane has no metadata header in card mode
When `row_style` is `"card"`, the preview pane SHALL NOT render the metadata header (title, agent/directory/branch/time line, rule). The preview SHALL begin directly with the transcript content.

#### Scenario: Card mode preview
- **WHEN** `row_style` is `"card"` and the preview pane is visible
- **THEN** the preview SHALL show only the transcript with message separators, no metadata header

#### Scenario: Compact mode preview
- **WHEN** `row_style` is `"compact"` and the preview pane is visible
- **THEN** the preview SHALL show the metadata header followed by the transcript (existing behavior)

### Requirement: Toolbar bracket notation
Toolbar filter chips SHALL use bracket notation for the selected value instead of background color blocks. The format SHALL be `Label: [Value]`.

#### Scenario: Unfocused chip
- **WHEN** a toolbar chip is not focused
- **THEN** the value SHALL render in accent color without brackets

#### Scenario: Focused chip
- **WHEN** a toolbar chip has focus (via Tab)
- **THEN** the value SHALL render as `[Value]` with bold styling
