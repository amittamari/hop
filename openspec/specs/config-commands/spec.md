# Capability: Config Commands

## Purpose

CLI subcommands under `hop config` for managing the user's configuration file:
discovering its path, scaffolding a template, opening it in an editor, and
displaying the effective configuration.

## Requirements

### Requirement: Config path command
The system SHALL print the resolved config file path to stdout when `hop config path` is invoked. The
path SHALL be the one defined by the `config` capability's `Config file location` requirement — the
file `hop` would load — printed whether or not that file exists, so the output doubles as "where do I
put it?". Stdout SHALL remain a single path so the command stays usable in shell substitution.

#### Scenario: Print config path
- **WHEN** user runs `hop config path`
- **THEN** the system prints the resolved config file path to stdout as a single line

#### Scenario: Path reflects the XDG location on macOS
- **GIVEN** neither `$HOP_CONFIG` nor `$XDG_CONFIG_HOME` is set
- **WHEN** user runs `hop config path` on macOS
- **THEN** the printed path SHALL be `~/.config/hop/config.toml`
- **AND** it SHALL NOT be under `~/Library/Application Support`

#### Scenario: Path honors HOP_CONFIG
- **GIVEN** `$HOP_CONFIG` is set to `/somewhere/custom.toml`
- **WHEN** user runs `hop config path`
- **THEN** the printed path SHALL be `/somewhere/custom.toml`

### Requirement: Config init command
The system SHALL scaffold a commented config template at the resolved config path when
`hop config init` is invoked.

#### Scenario: Init creates template when no config exists
- **WHEN** user runs `hop config init` and no config file exists at the resolved path
- **THEN** the system creates a config.toml at the resolved config path containing every config section commented out with descriptions and default values, creates parent directories if needed, and prints the file path to stderr

#### Scenario: Init no-ops when config already exists
- **WHEN** user runs `hop config init` and a config file already exists at the resolved path
- **THEN** the system prints a message indicating the file already exists and does not overwrite it

### Requirement: Config edit command
The system SHALL open the config file in the user's editor when `hop config edit` is invoked.

#### Scenario: Edit opens existing config
- **WHEN** user runs `hop config edit` and a config file exists
- **THEN** the system opens the file in the resolved editor (`$VISUAL` > `$EDITOR` > `vi`)

#### Scenario: Edit auto-creates config when missing
- **WHEN** user runs `hop config edit` and no config file exists
- **THEN** the system creates the config file from the same template used by `init`, creates parent directories if needed, then opens it in the editor

#### Scenario: Editor resolution
- **WHEN** the system resolves the editor command
- **THEN** it SHALL check `$VISUAL` first, then `$EDITOR`, then fall back to `vi`

### Requirement: Config show command
The system SHALL print the effective configuration as valid TOML when `hop config show` is invoked.

#### Scenario: Show prints effective config
- **WHEN** user runs `hop config show`
- **THEN** the system loads the config (applying defaults for missing fields), serializes it to TOML, and prints it to stdout

#### Scenario: Show with no config file
- **WHEN** user runs `hop config show` and no config file exists
- **THEN** the system prints the full default configuration as TOML

### Requirement: Config template integrity
The config template string SHALL remain structurally valid against the Config struct.

#### Scenario: Template round-trip parse
- **WHEN** the template's comment prefixes are stripped
- **THEN** the result SHALL parse successfully through `Config::from_toml_str`

### Requirement: Stale legacy macOS config notice

The system SHALL print a notice when `hop config init` scaffolds a new config on macOS while a file exists at the pre-`0.4.0` location `~/Library/Application Support/dev.hop.hop/config.toml`.

The notice SHALL go to stderr, SHALL name that path, and SHALL state that its settings are no longer
read. The notice SHALL NOT block scaffolding, and the system SHALL NOT read, move, copy, modify, or
delete the old file.

The notice SHALL be confined to `hop config init`. `Config::load` and the TUI launch path SHALL NOT
reference the legacy location, so normal startup stays silent.

#### Scenario: Init warns about a stale legacy config

- **GIVEN** the platform is macOS
- **AND** `~/Library/Application Support/dev.hop.hop/config.toml` exists
- **AND** no config exists at the resolved config path
- **WHEN** user runs `hop config init`
- **THEN** the system SHALL print a notice to stderr naming the legacy path
- **AND** it SHALL still create the template at the resolved config path
- **AND** the legacy file SHALL remain unmodified

#### Scenario: No notice when no legacy config exists

- **GIVEN** no file exists at the legacy location
- **WHEN** user runs `hop config init`
- **THEN** no legacy notice SHALL be printed
