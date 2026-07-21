# Capability: TUI Keymap

## Purpose

Configurable Ctrl-chord key bindings for the single live-search interaction model. All chord actions require Ctrl so they never collide with query editing. Bindings are configurable via `config.toml`'s `[keybindings]` table.

## Requirements

### Requirement: Default bindings
The keymap SHALL define default Ctrl-chord bindings: Ctrl+C (quit), Ctrl+P (toggle preview), Ctrl+U/D (scroll preview up/down), Ctrl+B/N (jump match prev/next), Ctrl+Left/Right (resize preview), Ctrl+O (open PR), Ctrl+R (toggle search mode).

#### Scenario: Default toggle preview binding
- **WHEN** the keymap is built with no config overrides
- **THEN** Ctrl+P SHALL be bound to TogglePreview

### Requirement: Config overrides
`Keymap::from_config` SHALL overlay user-specified bindings from the `[keybindings]` table. Each binding SHALL be parsed as a `ctrl+<key>` string. Invalid or non-Ctrl bindings SHALL be skipped with the default preserved.

#### Scenario: User rebinds toggle_preview
- **GIVEN** `[keybindings] toggle_preview = "ctrl+t"`
- **WHEN** the keymap is built
- **THEN** Ctrl+T SHALL trigger toggle_preview and Ctrl+P SHALL be unbound

### Requirement: Ctrl-only invariant
Every chord in the keymap SHALL include the Ctrl modifier. This ensures no key ever does double duty between query editing and commands.

#### Scenario: Non-Ctrl binding rejected
- **GIVEN** `[keybindings] quit = "q"`
- **WHEN** the keymap is built
- **THEN** the binding SHALL be ignored and the default Ctrl+C SHALL remain

### Requirement: Chord matching
`chord_action` SHALL match an incoming `KeyEvent` against the resolved chord table and return the corresponding `Command`, or `None` if no match.

#### Scenario: Unbound key returns None
- **WHEN** `chord_action` is called with a key not in the chord table
- **THEN** it SHALL return `None`

#### Scenario: Bound key returns command
- **WHEN** `chord_action` is called with Ctrl+P (default keymap)
- **THEN** it SHALL return `Some(Command::TogglePreview)`

### Requirement: Binding catalog
`bindings` SHALL return a structured list of all active bindings grouped by category (Navigation, Preview, Search Editing, Actions), suitable for rendering in the help overlay and footer. The catalog SHALL reflect the active (possibly overridden) key labels.

#### Scenario: Catalog includes all groups
- **WHEN** `bindings` is called with the default keymap
- **THEN** the result SHALL include entries in groups Navigation, Preview, Search Editing, and Actions

### Requirement: Help-aware labels
`label_for` SHALL return the key label string for a given command, reflecting any user overrides, so the footer and help overlay always show the correct key.

#### Scenario: Label reflects override
- **GIVEN** toggle_preview is rebound to Ctrl+T
- **WHEN** `label_for(TogglePreview)` is called
- **THEN** the result SHALL contain `"Ctrl+T"`
