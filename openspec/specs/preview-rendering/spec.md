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

### Requirement: Preview scroll position is clamped to content bounds

The preview scroll offset SHALL never exceed the last line of the transcript
content. All scroll mutation paths — keyboard page-scroll, mouse wheel, and
jump-to-match — SHALL enforce this upper bound so that the preview pane always
shows content when content exists.

#### Scenario: Keyboard scroll down does not overshoot

- **WHEN** the preview pane is visible with a transcript of N lines
- **AND** the user presses Ctrl+D (scroll down) enough times that
  `preview_scroll + scroll_step` would exceed N − 1
- **THEN** `preview_scroll` SHALL be clamped to N − 1
- **AND** the preview pane SHALL display the tail of the transcript

#### Scenario: Mouse wheel scroll down does not overshoot

- **WHEN** the preview pane is visible with a transcript of N lines
- **AND** the user scrolls the mouse wheel down enough that
  `preview_scroll + wheel_delta` would exceed N − 1
- **THEN** `preview_scroll` SHALL be clamped to N − 1

#### Scenario: Scroll down after jump-to-match does not overshoot

- **WHEN** the user jumps to a match near the end of the transcript via Ctrl+N
- **AND** then presses Ctrl+D (scroll down)
- **THEN** `preview_scroll` SHALL be clamped to N − 1
- **AND** the preview pane SHALL NOT be blank

#### Scenario: Content change re-clamps scroll position

- **WHEN** the user has scrolled deep into a long transcript
- **AND** the selection changes to a session with a shorter transcript
- **THEN** `preview_scroll` SHALL be re-clamped to the new content's bounds
  (note: selection change already resets scroll to 0; this covers edge cases
  where the line count is set after a match-jump)

#### Scenario: Scroll up still works normally

- **WHEN** the user presses Ctrl+U (scroll up)
- **THEN** `preview_scroll` SHALL decrease by `scroll_step`, clamped at 0
- **AND** existing lower-bound behavior SHALL be unchanged
