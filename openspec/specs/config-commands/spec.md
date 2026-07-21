# Capability: Config Commands

## Purpose

CLI subcommands under `hop config` for managing the user's configuration file:
discovering its path, scaffolding a template, opening it in an editor, and
displaying the effective configuration.

## Requirements

### Requirement: Config path command
The system SHALL print the config file path to stdout when `hop config path` is invoked.

#### Scenario: Print config path
- **WHEN** user runs `hop config path`
- **THEN** the system prints the platform-resolved config file path to stdout

### Requirement: Config init command
The system SHALL scaffold a commented config template when `hop config init` is invoked.

#### Scenario: Init creates template when no config exists
- **WHEN** user runs `hop config init` and no config file exists
- **THEN** the system creates a config.toml at the platform config path containing every config section commented out with descriptions and default values, creates parent directories if needed, and prints the file path to stderr

#### Scenario: Init no-ops when config already exists
- **WHEN** user runs `hop config init` and a config file already exists
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
