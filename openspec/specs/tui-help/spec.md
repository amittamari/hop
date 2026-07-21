# Capability: TUI Help Overlay

## Purpose

Renders a centered help overlay listing all active key bindings (reflecting any user overrides) and a static query keyword reference.

## Requirements

### Requirement: Binding catalog
The help overlay SHALL list every binding from the keymap's `bindings()` catalog, grouped by category (Navigation, Preview, Search Editing, Actions). Group headings SHALL use accent color with bold styling.

#### Scenario: All groups present
- **WHEN** the help overlay is rendered with the default keymap
- **THEN** the output SHALL contain group headings for Navigation, Preview, Search Editing, and Actions

### Requirement: Key column alignment
All key labels SHALL be padded to the width of the widest key label, so the description column starts at a consistent offset on every row. The "type" pseudo-key SHALL be shown as prose ("type to ..."), not in the key column.

#### Scenario: Consistent column offset
- **WHEN** the help overlay is rendered
- **THEN** every binding row (except the "type" pseudo-key) SHALL have a key column of equal character width

### Requirement: Query keyword reference
A "Query Keywords (raw mode)" section SHALL list the DSL filter keywords: `agent:claude`, `dir:api`, `repo:hop`, `date:today`, `date:<2d`, with brief descriptions. These are static help text, not key bindings.

#### Scenario: Keywords section present
- **WHEN** the help overlay is rendered
- **THEN** the output SHALL contain `"Query Keywords"` and `"agent:claude"` and `"date:today"`

### Requirement: Override awareness
The overlay SHALL reflect the active keymap bindings, including any user overrides from `config.toml`. Key labels SHALL match what the user has configured, not just the defaults.

#### Scenario: Overridden key appears in help
- **GIVEN** toggle_preview is rebound to Ctrl+T
- **WHEN** the help overlay is rendered
- **THEN** the toggle preview row SHALL show `"Ctrl+T"` not `"Ctrl+P"`

### Requirement: Overlay rendering
The overlay SHALL be centered in the terminal, dimming the background with overlay colors. It SHALL use a bordered block titled " help " with inner padding. Minimum terminal size for rendering SHALL be 8x6.

#### Scenario: Small terminal skips rendering
- **GIVEN** a terminal smaller than 8x6
- **WHEN** the help overlay render is called
- **THEN** nothing SHALL be rendered

### Requirement: Dismissal
The help overlay SHALL be closed by pressing Esc or `?`.

#### Scenario: Esc closes overlay
- **GIVEN** the help overlay is open
- **WHEN** the user presses Esc
- **THEN** the help overlay SHALL close

#### Scenario: Question mark closes overlay
- **GIVEN** the help overlay is open
- **WHEN** the user presses `?`
- **THEN** the help overlay SHALL close
