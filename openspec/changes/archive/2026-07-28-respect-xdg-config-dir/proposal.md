## Why

`Config::load` resolves the config file through `directories::ProjectDirs::config_dir()`, which on
macOS is `~/Library/Application Support/dev.hop.hop/config.toml`. `README.md` tells users the file
lives at `~/.config/hop/config.toml`, so a macOS user who follows the documentation gets a config
file that `hop` never reads — silently. The first user-visible symptom is that `[launcher]` appears
to do nothing (reported as "the launcher doesn't work"), but every section is equally ignored:
`[data_dirs]`, `[display]`, `[keybindings]`, `[columns]`, and `search_mode`.

`hop` is Unix-only (`src/resume.rs` uses `std::os::unix::process::CommandExt`), so the only thing
`ProjectDirs` buys here is the macOS `Application Support` convention — the exact convention causing
the bug. Dropping it makes config resolution one path on both supported platforms.

## What Changes

- Resolve the config file to a single path instead of a platform-dependent one:
  1. `$HOP_CONFIG`, when set — an explicit path to a config *file*, used verbatim
  2. otherwise `<xdg-config-home>/hop/config.toml`, where `<xdg-config-home>` is `$XDG_CONFIG_HOME`
     when set to an absolute path, else `~/.config`
- Stop reading `~/Library/Application Support/dev.hop.hop/config.toml` entirely. **BREAKING on
  macOS** for anyone whose config lives there (see Impact).
- Remove `directories::ProjectDirs` from config resolution. `BaseDirs` is still used for the home
  directory, and `ProjectDirs` still backs the cache paths in `src/main.rs`.
- Make resolution a pure function over injected inputs (`$HOP_CONFIG`, `$XDG_CONFIG_HOME`, home) so
  precedence is unit-testable without mutating process environment variables — it is untested today.
- `hop config path`, `init`, `edit`, and `show` need no logic changes; they inherit the corrected
  path through `config_path()`. On macOS they now target `~/.config/hop/config.toml`.
- Add a one-time notice to `hop config init` when an old `Application Support` config is present, so
  the one population this breaks gets told instead of silently losing their settings.
- On Linux, behavior is unchanged: `$XDG_CONFIG_HOME` then `~/.config` is exactly what `ProjectDirs`
  already resolved to.
- Cache and data directories (`index`, `enrich`, `update_check`) keep using `ProjectDirs::cache_dir()`
  and are out of scope; they are machine-managed, never hand-edited, and not documented as XDG paths.

## Capabilities

### New Capabilities

None. Config path resolution is an existing concern of the `config` capability, not a new one.

### Modified Capabilities

- `config`: adds a requirement fixing the config file location and its resolution order; modifies
  `Config loading` to read that path instead of deriving one from `directories::ProjectDirs`.
- `config-commands`: `path` and `init` now target the resolved XDG path rather than the platform
  config dir, and `init` reports a stale `Application Support` config when it finds one.

## Impact

- `src/config/mod.rs` — `config_path()` keeps its `Option<PathBuf>` signature but resolves through
  the new pure function; `Config::load` gains an injectable variant for tests.
- `src/config/paths.rs` — new module holding `ConfigPathInputs` and resolution.
- `src/config/commands.rs` — no resolution changes; `cmd_init` gains the stale-config notice.
- `src/cli.rs`, `src/main.rs` — unchanged.
- `README.md` — replace "your platform's config directory (e.g., `~/.config/hop/config.toml`)" with
  the actual resolution order; this vague phrasing is what let the mismatch survive.
- `docs/ARCHITECTURE.md` — record that config resolution is XDG-only while cache dirs still use
  `ProjectDirs`, so the asymmetry reads as a decision.
- **Breaking, macOS only**: a config at `~/Library/Application Support/dev.hop.hop/config.toml` stops
  being read. Anyone affected ran `hop config init` on macOS before this change, since that is the
  only way a file lands there. Mitigation is the `hop config init` notice plus a CHANGELOG entry
  telling them to move the file to `~/.config/hop/config.toml`.
- No new dependencies; no dependency removed (`directories` still used for home and cache dirs).
