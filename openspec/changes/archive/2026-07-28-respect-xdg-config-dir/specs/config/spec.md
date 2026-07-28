## ADDED Requirements

### Requirement: Config file location

The system SHALL resolve the config file to a single path, identical on every supported platform
(macOS and Linux). The resolved path SHALL be:

- `$HOP_CONFIG`, used verbatim as a path to a config *file*, when that variable is set and non-empty;
- otherwise `<xdg-config-home>/hop/config.toml`, where `<xdg-config-home>` is `$XDG_CONFIG_HOME` when
  that variable is set, non-empty, and absolute, and `~/.config` otherwise.

`$XDG_CONFIG_HOME` and `~/.config` SHALL be mutually exclusive rather than searched in order: when
`$XDG_CONFIG_HOME` is honored, `~/.config` SHALL NOT be consulted.

Resolution SHALL NOT use `directories::ProjectDirs`, and SHALL NOT consult
`~/Library/Application Support/dev.hop.hop/config.toml` on macOS. When `$HOP_CONFIG` is unset and the
home directory cannot be determined, resolution SHALL yield no path rather than a path relative to
the working directory.

#### Scenario: macOS resolves to the documented XDG path

- **GIVEN** neither `$HOP_CONFIG` nor `$XDG_CONFIG_HOME` is set
- **WHEN** the config path is resolved on macOS
- **THEN** it SHALL be `~/.config/hop/config.toml`
- **AND** it SHALL NOT be under `~/Library/Application Support`

#### Scenario: HOP_CONFIG overrides the XDG location

- **GIVEN** `$HOP_CONFIG` is `/somewhere/custom.toml`
- **AND** `$XDG_CONFIG_HOME` is also set
- **WHEN** the config path is resolved
- **THEN** it SHALL be `/somewhere/custom.toml`

#### Scenario: HOP_CONFIG is honored even when the file does not exist

- **GIVEN** `$HOP_CONFIG` is `/somewhere/custom.toml` and that file does not exist
- **WHEN** the config path is resolved
- **THEN** it SHALL still be `/somewhere/custom.toml`

#### Scenario: XDG_CONFIG_HOME replaces the home-relative location

- **GIVEN** `$HOP_CONFIG` is unset and `$XDG_CONFIG_HOME` is `/xdg`
- **WHEN** the config path is resolved
- **THEN** it SHALL be `/xdg/hop/config.toml`
- **AND** `~/.config/hop/config.toml` SHALL NOT be consulted even if a file exists there

#### Scenario: Empty or relative environment values are ignored

- **GIVEN** `$XDG_CONFIG_HOME` is set to `""` or to a relative path such as `config`
- **WHEN** the config path is resolved
- **THEN** the `$XDG_CONFIG_HOME` value SHALL be ignored
- **AND** the resolved path SHALL be `~/.config/hop/config.toml`

#### Scenario: No home directory and no override

- **GIVEN** `$HOP_CONFIG` and `$XDG_CONFIG_HOME` are unset
- **AND** the home directory cannot be determined
- **WHEN** the config path is resolved
- **THEN** resolution SHALL yield no path

### Requirement: Config path resolution is pure and injectable

Path resolution SHALL be a pure function of explicit inputs — the `$HOP_CONFIG` value, the
`$XDG_CONFIG_HOME` value, and the home directory — with a single separate entry point responsible for
reading those values from the process environment. The pure function SHALL NOT read or write process
environment variables, SHALL NOT touch the filesystem (including existence checks), and SHALL NOT
branch on the target platform.

#### Scenario: Resolution driven by injected inputs

- **GIVEN** inputs specifying a temporary home directory and no environment overrides
- **WHEN** the path is resolved from those inputs
- **THEN** the result SHALL be derived from the temporary home directory
- **AND** the process environment SHALL NOT be read or modified

#### Scenario: Same result on every supported platform

- **GIVEN** one fixed set of inputs
- **WHEN** the path is resolved on macOS and on Linux
- **THEN** both SHALL yield the same path

## MODIFIED Requirements

### Requirement: Config loading
`Config::load` SHALL read from the path given by the `Config file location` requirement. It SHALL NOT
derive the path from `directories::ProjectDirs::config_dir()`. A missing file — or an unresolvable
path — SHALL produce default values without error. `Config::load` SHALL be paired with an injectable
variant that accepts resolution inputs, so loading is testable without touching the real environment.

#### Scenario: Missing config file
- **WHEN** no `config.toml` exists at the resolved path
- **THEN** `Config::load` SHALL return a `Config` with all default values

#### Scenario: Config in the documented XDG location is loaded on macOS
- **GIVEN** a `config.toml` at `~/.config/hop/config.toml` setting `[launcher] command = "kv --ai {agent}"`
- **WHEN** `Config::load` runs on macOS
- **THEN** the returned `Config` SHALL have `launcher.command` set to `"kv --ai {agent}"`

#### Scenario: Legacy macOS config is not read
- **GIVEN** a `config.toml` exists only at `~/Library/Application Support/dev.hop.hop/config.toml`
- **WHEN** `Config::load` runs on macOS
- **THEN** it SHALL return a `Config` with all default values

#### Scenario: Unreadable or malformed config is an error, not a silent default
- **GIVEN** a config file exists at the resolved path but cannot be read or fails to parse
- **WHEN** `Config::load` runs
- **THEN** it SHALL return an error naming the offending path
