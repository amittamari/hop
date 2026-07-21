# Proposal: Add `hop config` CLI commands

## Problem

Hop's configuration lives in a TOML file at a platform-specific path
(`~/Library/Application Support/dev.hop.hop/config.toml` on macOS). Users have
no way to discover this path, see what options exist, or inspect their effective
settings without reading source code. The config file has grown to cover display,
keybindings, columns, launcher, search mode, and data dirs — all invisible to
new users.

## Solution

Add a `hop config` subcommand group with four actions:

| Command             | Purpose                                           |
|----------------------|---------------------------------------------------|
| `hop config init`   | Scaffold a commented config template              |
| `hop config edit`   | Open config in `$VISUAL`/`$EDITOR`/`vi`           |
| `hop config show`   | Print the effective config (defaults + overrides)  |
| `hop config path`   | Print the config file path                         |

### `hop config init`

Writes a config.toml with every section commented out, annotated with
descriptions and default values. Prints the file path on success. No-ops with a
message if the file already exists (does not overwrite).

### `hop config edit`

Opens the config file in the user's editor. If the file does not exist,
auto-creates it from the same template `init` uses before opening. Editor
resolution: `$VISUAL` > `$EDITOR` > `vi`.

### `hop config show`

Prints the effective configuration as valid TOML — defaults merged with any
user overrides. No annotations, just the resolved values. Useful for debugging
("why is my preview pane hidden?").

### `hop config path`

Prints the config file path to stdout. Enables scripting: `cat $(hop config path)`,
`rm $(hop config path)`.

## Scope

- Four new subcommands under `hop config`.
- `Serialize` derive added to all config structs.
- A hand-maintained template string in `config.rs`.
- No interactive wizard, no `set`/`get` for individual keys.

## Non-goals

- Interactive configuration wizard (possible future enhancement).
- Programmatic `set`/`get` of individual keys.
- Automatic template generation from struct definitions.

## Risks

- **Template drift**: the template string must stay in sync with the `Config`
  struct. Mitigated by colocating it in `config.rs` and adding a test that
  parses the template through `Config::from_toml_str` (uncommented) to catch
  structural mismatches.
