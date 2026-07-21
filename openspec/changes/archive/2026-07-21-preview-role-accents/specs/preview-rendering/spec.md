## MODIFIED Requirements

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
