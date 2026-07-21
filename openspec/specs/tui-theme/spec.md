# Capability: TUI Theme

## Purpose

Defines semantic color roles for the TUI as a single `Theme` struct. Provides a hardcoded default palette and an agent-color lookup method.

## Requirements

### Requirement: Semantic roles
The theme SHALL define color roles for: `bg`, `fg`, `muted`, `accent`, `code`, `border`, `overlay_fg`, `overlay_bg`, `selection_fg`, `selection_bg`, `match_fg`, `warning`, `error`, `success`, `preview_text`, and per-agent brand colors (`agent_claude`, `agent_codex`, `agent_cursor`).

#### Scenario: All roles present
- **WHEN** the default theme is constructed
- **THEN** all semantic role fields SHALL be initialized with non-panic values

### Requirement: Default palette
The default theme SHALL use: DarkGray for muted, Cyan for accent, Yellow for code, distinct RGB values for border/overlay/selection/preview, and brand colors for each agent (amber for Claude, purple for Codex, green for Cursor).

#### Scenario: Default accent is Cyan
- **WHEN** the default theme is constructed
- **THEN** `accent` SHALL be `Color::Cyan`

#### Scenario: Default muted is DarkGray
- **WHEN** the default theme is constructed
- **THEN** `muted` SHALL be `Color::DarkGray`

### Requirement: Agent color lookup
`agent_color` SHALL map an `AgentId` to its brand color from the theme.

#### Scenario: Claude brand color
- **WHEN** `agent_color(AgentId::Claude)` is called on the default theme
- **THEN** the result SHALL be the amber RGB value `(245, 158, 11)`

### Requirement: Distinct signal colors
Warning, error, accent, and success SHALL all be distinct colors to avoid ambiguity in status indicators.

#### Scenario: Signal colors are distinct
- **WHEN** the default theme is constructed
- **THEN** warning, error, accent, and success SHALL each be a different color

### Requirement: Copy semantics
The theme SHALL be `Copy` so it can be cheaply lifted into locals when a `&mut App` borrow is in scope.

#### Scenario: Theme can be copied
- **GIVEN** a `Theme` value
- **WHEN** it is assigned to another variable by value
- **THEN** the original SHALL remain valid (no move)
