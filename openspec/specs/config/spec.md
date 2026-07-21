# Capability: Configuration

## Purpose

Loads and represents the user's `config.toml` from the platform config directory. Covers all configuration sections: data directories, display settings, columns, keybindings, launcher, and search mode.

## Requirements

### Requirement: Config loading
`Config::load` SHALL read from the platform-specific config directory (via `directories::ProjectDirs`). A missing file SHALL produce default values without error.

#### Scenario: Missing config file
- **WHEN** no `config.toml` exists in the config directory
- **THEN** `Config::load` SHALL return a `Config` with all default values

### Requirement: Data directory overrides
The `[data_dirs]` table SHALL allow overriding the default data directory for each agent by slug. Unset agents SHALL fall back to their well-known defaults (`~/.claude/projects`, `~/.codex`, `~/.cursor/projects`).

#### Scenario: Override one agent
- **GIVEN** config contains `[data_dirs] claude = "/custom/claude"`
- **WHEN** `data_dir(AgentId::Claude)` is called
- **THEN** it SHALL return `/custom/claude`
- **AND** `data_dir(AgentId::Codex)` SHALL still return the default `~/.codex`

### Requirement: Display section
The `[display]` section SHALL support:
- `row_style`: `"card"` (default) or `"compact"`.
- `icons`: boolean, default `true` (opt-out for nerd-font icons).
- `visible`: boolean, default `false` (preview pane initial visibility).
- `width_pct`: u16, default 30 (preview pane width percentage, clamped 20-80).
- `metadata_header`: boolean, default `true` (compact-mode preview header).

#### Scenario: Default display values
- **WHEN** no `[display]` section exists
- **THEN** `row_style` SHALL be `"card"`, `icons` SHALL be `true`, `visible` SHALL be `false`

### Requirement: Columns section
The `[columns]` section SHALL support `disabled` (list of column ids to hide) and `order` (explicit column ordering).

#### Scenario: Disable columns
- **GIVEN** config contains `[columns] disabled = ["pr", "msgs"]`
- **WHEN** columns are configured
- **THEN** the `pr` and `msgs` columns SHALL be excluded from the layout

### Requirement: Keybindings section
The `[keybindings]` table SHALL map command names (e.g. `toggle_preview`) to Ctrl-chord strings (e.g. `"ctrl+t"`). The keymap module consumes these overrides.

#### Scenario: Custom keybinding
- **GIVEN** config contains `[keybindings] toggle_preview = "ctrl+t"`
- **WHEN** keybindings are loaded
- **THEN** the `toggle_preview` command SHALL be mapped to `ctrl+t`

### Requirement: Launcher section
The `[launcher]` section SHALL support a `command` template string with `{agent}` interpolation. The template SHALL replace `{agent}` with the agent slug, split by shell quoting rules, and prepend to the resume argv (dropping the original program name). Unknown template variables SHALL produce an error.

#### Scenario: Launcher rewrite
- **GIVEN** launcher command `"kv --ai {agent}"` and resume argv `["claude", "--resume", "id"]`
- **WHEN** `rewrite_argv` is called for Claude
- **THEN** the result SHALL be `["kv", "--ai", "claude", "--resume", "id"]`

### Requirement: Search mode
The `search_mode` field SHALL accept `"simple"` (default) or `"raw"`. Unknown values SHALL resolve to simple.

#### Scenario: Default search mode
- **WHEN** no `search_mode` is specified
- **THEN** the resolved mode SHALL be simple

#### Scenario: Unknown search mode value
- **GIVEN** config contains `search_mode = "foobar"`
- **WHEN** the mode is resolved
- **THEN** it SHALL fall back to simple
