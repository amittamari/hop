# Capability: TUI App State

## Purpose

Defines the `App` model: query state, search mode, selection, preview visibility/scroll/matches, toolbar focus, viewport metrics, modal state, and the effective-query composition that bridges simple and raw search modes.

## Requirements

### Requirement: Query state
The app SHALL hold a query string and a byte-level cursor position. `set_query` SHALL update both and place the cursor at the end.

#### Scenario: Setting a query
- **WHEN** `set_query("auth bug")` is called
- **THEN** the query SHALL be `"auth bug"`
- **AND** the cursor SHALL be at byte offset 8

### Requirement: Search mode
The app SHALL support two search modes: `Simple` (guided toolbar) and `Raw` (direct DSL input). The mode determines how the effective query is composed and whether the toolbar is shown.

#### Scenario: Initial search mode
- **WHEN** the app is initialized with Simple mode
- **THEN** `search_mode()` SHALL return `Simple`

### Requirement: Effective query
In Raw mode, the effective query SHALL be the query string verbatim. In Simple mode, it SHALL be composed via `compose_simple` with the active repo scope prepended when `Scope::ThisRepo` is active.

#### Scenario: Raw mode passes through
- **GIVEN** the app is in Raw mode with query `"agent:claude auth"`
- **WHEN** `effective_query()` is called
- **THEN** the result SHALL be `"agent:claude auth"`

#### Scenario: Simple mode prepends repo scope
- **GIVEN** the app is in Simple mode with query `"auth"` and repo slug `"me/hop"` and scope ThisRepo
- **WHEN** `effective_query()` is called
- **THEN** the result SHALL be `"repo:me/hop auth"`

### Requirement: Selection management
`set_results` SHALL replace the result set and clamp the selection index to the valid range. `set_results_with_yolo` SHALL additionally track per-row yolo support flags.

#### Scenario: Selection clamped on shorter results
- **GIVEN** the selection is at index 5
- **WHEN** `set_results` is called with 3 results
- **THEN** the selection SHALL be clamped to index 2

### Requirement: Preview state
The app SHALL track preview visibility, width percentage (clamped 20-80), scroll position, header visibility, and match positions for jump-to-match navigation.

#### Scenario: Width percentage clamped
- **WHEN** `set_preview(true, 95)` is called
- **THEN** `preview_width_pct()` SHALL return 80

### Requirement: Viewport metrics
`set_viewport_metrics` SHALL record the list page size and preview scroll step from the rendered layout, so page-up/down and Ctrl+U/D scroll by the correct amount.

#### Scenario: Metrics from rendered layout
- **WHEN** `set_viewport_metrics(20, 30)` is called
- **THEN** list page size SHALL be 19 (height minus 1)
- **AND** preview scroll step SHALL be 29 (height minus 1)

### Requirement: Modal state
The app SHALL track the current mode: `Main` or `YoloModal { index, yolo }`. The modal opens for yolo-capable agents and archived sessions.

#### Scenario: Opening yolo modal
- **WHEN** `open_yolo_modal()` is called with selection at index 2
- **THEN** `yolo_modal()` SHALL return `Some((2, false))`

### Requirement: Frame counter
`tick` SHALL advance a frame counter used to animate spinners and throbbers without a dedicated timer.

#### Scenario: Frame advances on tick
- **GIVEN** the frame counter is at 0
- **WHEN** `tick()` is called twice
- **THEN** `frame()` SHALL return 2
