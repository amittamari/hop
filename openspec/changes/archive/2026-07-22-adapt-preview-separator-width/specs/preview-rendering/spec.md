## ADDED Requirements

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
