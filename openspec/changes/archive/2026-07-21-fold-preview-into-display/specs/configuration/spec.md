## ADDED Requirements

### Requirement: TUI display settings live under `[display]`

The `config.toml` `[display]` section SHALL be the single home for how the TUI
renders the session list and preview. It SHALL accept `row_style`, `icons`,
`visible`, `width_pct`, and `metadata_header`. No standalone `[preview]` section
SHALL be recognized.

#### Scenario: preview keys are read from `[display]`

- **WHEN** a config file sets `visible`, `width_pct`, and `metadata_header` under
  `[display]`
- **THEN** those values seed preview visibility, width, and the compact-view
  metadata header respectively

#### Scenario: unset preview keys keep their defaults

- **WHEN** a config file omits the preview keys (or has no `[display]` section)
- **THEN** preview `visible` defaults to `false`, `width_pct` to `50`, and
  `metadata_header` to `true`

#### Scenario: a legacy `[preview]` section is ignored

- **WHEN** a config file still declares a `[preview]` section
- **THEN** the keys in it do not change TUI behavior, and defaults apply unless
  the same keys are set under `[display]`

### Requirement: preview display state is config-seeded and in-memory only

Preview visibility and width SHALL be initialized from `[display]` at launch and
held in memory for the session. The system SHALL NOT persist preview state to
disk, and SHALL NOT read any prior-session UI-state file.

#### Scenario: runtime toggles do not persist across sessions

- **WHEN** the user toggles preview or resizes it during a session and then exits
- **THEN** the next launch starts from the `[display]` config values, not the
  last-session geometry

#### Scenario: a stale ui-state file is ignored

- **WHEN** a `ui_state.toml` from a previous version exists on disk
- **THEN** it is never read and has no effect on preview state

### Requirement: the metadata header applies only to compact rows

The `metadata_header` setting SHALL control the preview metadata header only when
`row_style` is compact. In card row style the preview metadata header SHALL NOT
render regardless of `metadata_header`.

#### Scenario: metadata header is suppressed in card mode

- **WHEN** `row_style` is `card` and `metadata_header` is `true`
- **THEN** the preview pane does not render the metadata header

#### Scenario: metadata header honors the setting in compact mode

- **WHEN** `row_style` is `compact`
- **THEN** the preview metadata header renders when `metadata_header` is `true`
  and is omitted when it is `false`
