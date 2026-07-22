# Capability: Preview Rendering

## Purpose

Defines consistent transcript preview rendering across configured row styles.

## Requirements

### Requirement: Unified transcript rendering across row styles
The preview panel SHALL use the separator-based transcript rendering
(`── role ──────` thin rules between messages) for all row styles, including
compact mode. The rendering style SHALL NOT vary based on `RowStyle`. Agent
separators SHALL be styled in the agent's brand color; user separators SHALL
retain the neutral border/preview-text styling.

#### Scenario: Compact mode preview uses separator-based rendering
- **WHEN** the user has `row_style = "compact"` configured
- **AND** the preview panel is visible with a session selected
- **THEN** the transcript SHALL render with thin horizontal rule separators
  between messages (e.g. `── user ──────`, `── ✱ claude ──────`)
- **AND** agent separators SHALL use `theme.agent_color(agent)` for all spans
- **AND** user separators SHALL use `theme.border` / `theme.preview_text`
- **AND** message body rendering SHALL remain unchanged

#### Scenario: Card mode preview unchanged
- **WHEN** the user has `row_style = "card"` configured (or default)
- **AND** the preview panel is visible with a session selected
- **THEN** the transcript SHALL render with the same separator-based style
  and role-aware coloring as compact mode

#### Scenario: Metadata header remains compact-only
- **WHEN** the row style is compact and `metadata_header` is enabled
- **THEN** the 3-line metadata header (title, meta row, rule) SHALL appear
  above the transcript in the preview panel
- **WHEN** the row style is card
- **THEN** the metadata header SHALL NOT appear

### Requirement: Separator width tracks actual pane geometry

The thin-rule message separators (`── role ──────`) SHALL fill to the actual
inner width of the preview pane as resolved by the layout engine, not a
pre-computed estimate. The width SHALL account for the list-side minimum-width
constraint and the preview block's border and padding.

#### Scenario: Separator fills to pane inner width

- **WHEN** the preview pane is visible with a session selected
- **AND** the layout engine resolves the preview pane to W columns
- **AND** the preview block has a left border (1 col) and left padding (1 col)
- **THEN** each thin-rule separator SHALL have a display width of exactly W − 2

#### Scenario: Separator adapts on pane resize via Ctrl+K/L

- **WHEN** the user presses Ctrl+K or Ctrl+L to grow or shrink the preview pane
- **THEN** the separator lines SHALL re-render to match the new inner width
- **AND** the update SHALL happen on the next frame without requiring a
  selection change

#### Scenario: Separator adapts on terminal resize

- **WHEN** the terminal window is resized
- **THEN** the separator lines SHALL re-render to match the new inner width
- **AND** the update SHALL happen on the next frame without requiring a
  selection change

#### Scenario: Narrow terminal where list minimum dominates

- **WHEN** the terminal is narrow enough that the list-side minimum-width
  constraint (48 columns) reduces the preview pane below its percentage
  allocation
- **THEN** the separators SHALL still fill to the actual (reduced) inner width
  rather than the percentage-based estimate
