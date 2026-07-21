# Capability: Preview Role Accents

## Purpose

Defines role-aware styling for transcript preview separators while preserving
the existing rendering of message bodies.

## Requirements

### Requirement: Agent separator uses brand color and glyph
The transcript preview thin-rule separator for agent messages SHALL render in
the agent's brand color (`theme.agent_color(agent)`). When icons are enabled,
the separator label SHALL include the agent's glyph before the text badge. When
icons are disabled, the separator label SHALL show the text badge alone, still
in the agent's brand color.

#### Scenario: Agent separator with icons enabled
- **WHEN** an agent message separator renders in the transcript preview
- **AND** icons are enabled
- **THEN** the separator line (prefix dashes, label, and fill dashes) SHALL be
  styled in `theme.agent_color(agent)`
- **AND** the label SHALL consist of the agent glyph, a space, and the
  lowercased agent badge (e.g. `✱ claude`)

#### Scenario: Agent separator with icons disabled
- **WHEN** an agent message separator renders in the transcript preview
- **AND** icons are disabled
- **THEN** the separator line SHALL be styled in `theme.agent_color(agent)`
- **AND** the label SHALL consist of only the lowercased agent badge (e.g.
  `claude`), with no glyph

#### Scenario: User separator unchanged
- **WHEN** a user message separator renders in the transcript preview
- **THEN** the separator line SHALL render with `theme.border` for rule
  characters and `theme.preview_text` with bold for the label
- **AND** the label SHALL be the literal text `user` with no glyph
- **AND** the styling SHALL be identical to the pre-change behavior

### Requirement: Message bodies remain unchanged
Role differentiation SHALL be confined to the thin-rule separator. Agent and
user prose and code body lines SHALL retain their existing content, indentation,
and styling without role-specific prefixes.

#### Scenario: Agent body rendering
- **WHEN** a prose or code block line renders inside an agent message
- **THEN** the line SHALL NOT receive an accent bar or other role decoration
- **AND** its existing markdown or syntect styling SHALL be preserved
