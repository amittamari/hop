# Capability: TUI Icons

## Purpose

Defines the centralized glyph set for the TUI chrome, its nerd-font and ascii
variants, the opt-out `[display] icons` config, and where icons may and may not
appear. Per-agent identity glyphs are supplied through the `Adapter` trait so
generic layers stay agent-agnostic.

## Requirements

### Requirement: Glyph set with nerd and ascii variants
The TUI SHALL own a single centralized glyph set that resolves every icon and
decorative glyph used in the chrome. The set SHALL have two variants: a `nerd`
variant using Private Use Area (PUA) nerd-font icons and an `ascii` variant that
reproduces the pre-change appearance. Exactly one variant SHALL be selected once
at startup and threaded through the render path; no render site SHALL hardcode an
icon literal.

#### Scenario: Nerd variant selected
- **WHEN** the glyph set resolves to the `nerd` variant
- **THEN** icon accessors SHALL return the PUA nerd-font glyph for each surface

#### Scenario: Ascii variant is visually equivalent to pre-change
- **WHEN** the glyph set resolves to the `ascii` variant
- **THEN** every chrome surface SHALL render with the same text and layout it had
  before this change, containing no PUA (nerd-font) code point
- **AND** field icons (branch, repo, PR, time, message count, archived) SHALL
  contribute no leading glyph, leaving the existing text unchanged

### Requirement: Icons config defaults on (opt-out)
The `[display]` config section SHALL accept an `icons` boolean field that selects
the glyph variant. It SHALL default to `true` (the `nerd` variant) so icons are
opt-out. A value of `false` SHALL select the `ascii` variant.

#### Scenario: Default config enables icons
- **WHEN** no `icons` field is present in `[display]`
- **THEN** the glyph set SHALL resolve to the `nerd` variant

#### Scenario: Explicit opt-out
- **WHEN** config contains `[display] icons = false`
- **THEN** the glyph set SHALL resolve to the `ascii` variant

### Requirement: Per-agent glyph via adapter boundary
Each agent's identity glyph SHALL be provided through the `Adapter` trait as an
agent-agnostic method with a safe default, overridden per adapter. No generic
layer (`engine`, `core`, `index`, `tui`) SHALL contain an agent-specific glyph
literal or match on agent identity to choose a glyph.

#### Scenario: Adapter supplies its own glyph
- **WHEN** an agent's adapter overrides the agent-glyph method
- **THEN** the TUI SHALL render that glyph for the agent's mark

#### Scenario: Default glyph for an adapter that does not override
- **WHEN** an adapter does not override the agent-glyph method
- **THEN** the TUI SHALL render the trait's safe default glyph without a
  compile-time or render-time error

### Requirement: Agent mark renders as glyph plus text in brand color
When icons are enabled, the agent mark SHALL render as the agent's glyph followed
by the agent's short text label, both styled in the agent's existing brand color.
When icons are disabled, the agent mark SHALL render as the text label alone in
the brand color, exactly as before this change.

#### Scenario: Agent mark with icons enabled
- **WHEN** icons are enabled and a session's agent mark is rendered
- **THEN** the mark SHALL show the agent glyph, a separating space, and the agent
  text label, all in the agent's brand color

#### Scenario: Agent mark with icons disabled
- **WHEN** icons are disabled and a session's agent mark is rendered
- **THEN** the mark SHALL show only the agent text label in the brand color

### Requirement: Metadata field icons in chrome
When icons are enabled, chrome metadata fields SHALL be prefixed with a
field-appropriate nerd-font glyph: branch, repo, PR, time, and message count in
the card metadata line and the preview header, and the archived marker on the
title. These glyphs SHALL be gated by the glyph set so that disabling icons
removes them without altering the surrounding text.

#### Scenario: Card metadata line with icons enabled
- **WHEN** icons are enabled and a card's metadata line renders non-empty fields
- **THEN** each present field (repo, branch, PR, message count) SHALL be prefixed
  with its glyph, keeping the existing separator between fields

#### Scenario: Archived marker with icons enabled
- **WHEN** icons are enabled and a session is archived
- **THEN** the archived indicator SHALL render as an archive glyph in place of the
  `arch ` text prefix

#### Scenario: Field icons removed when disabled
- **WHEN** icons are disabled
- **THEN** metadata fields and the archived marker SHALL render with their
  pre-change text and no leading glyph

### Requirement: Status glyphs use themed status colors
When icons are enabled, warning, success, and error status indicators SHALL be
rendered with dedicated nerd-font glyphs colored by the corresponding
`theme.warning`, `theme.success`, and `theme.error` roles. When icons are
disabled, the status text SHALL render with its themed color and no glyph.

#### Scenario: Warning status with icons enabled
- **WHEN** icons are enabled and a warning status is shown
- **THEN** the warning SHALL be prefixed with a warning glyph styled in
  `theme.warning`

#### Scenario: Status without icons
- **WHEN** icons are disabled and a status is shown
- **THEN** the status text SHALL render in its themed color with no leading glyph

### Requirement: Icons confined to chrome
Icons SHALL be applied only to chrome surfaces (agent mark, metadata fields,
status indicators, and optionally modal field labels and help section headings).
The footer key-hint bindings, the card snippet text, and the transcript prose
SHALL NOT gain icons.

#### Scenario: Footer hints unchanged
- **WHEN** the footer key-hints render
- **THEN** they SHALL contain no field or action icons beyond the pre-existing
  key-label glyphs (arrows and separators)

#### Scenario: Snippet and transcript prose unchanged
- **WHEN** the card snippet or transcript prose renders
- **THEN** its content SHALL contain no added chrome icons
