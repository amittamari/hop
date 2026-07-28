## 1. Path resolution core

- [x] 1.1 Add `src/config/paths.rs` and declare `pub mod paths;` from `src/config/mod.rs`. Keeping resolution in its own module keeps `mod.rs` (~400 lines with colocated tests) from growing further.
- [x] 1.2 Define `ConfigPathInputs { hop_config: Option<PathBuf>, xdg_config_home: Option<PathBuf>, home: Option<PathBuf> }`. No platform field — resolution is platform-independent.
- [x] 1.3 Implement `ConfigPathInputs::from_env()` as the only code that reads `std::env::var` and `directories::BaseDirs`. Treat empty env values as unset.
- [x] 1.4 Implement the pure `resolve(&ConfigPathInputs) -> Option<PathBuf>`: return `hop_config` verbatim when set; else `<xdg>/hop/config.toml` where `<xdg>` is `xdg_config_home` when non-empty and absolute, else `home/.config`; else `None` when `home` is `None`. No filesystem access, no `cfg!(target_os = ...)`.

## 2. Unit tests for resolution

- [x] 2.1 Add colocated tests in `src/config/paths.rs` driving `ConfigPathInputs` built against a `tempfile::tempdir()` home — no `std::env::set_var` anywhere.
- [x] 2.2 Cover `$HOP_CONFIG` winning over a set `$XDG_CONFIG_HOME`, and being returned even when the file does not exist.
- [x] 2.3 Cover `$XDG_CONFIG_HOME` *replacing* `~/.config` rather than ranking above it: with `$XDG_CONFIG_HOME=/xdg`, the result is `/xdg/hop/config.toml` even when a file exists at `<home>/.config/hop/config.toml`.
- [x] 2.4 Cover the guards: empty and relative `$XDG_CONFIG_HOME` fall back to `<home>/.config`; empty `$HOP_CONFIG` is treated as unset; `home: None` with no overrides yields `None`.
- [x] 2.5 Assert the resolved path never contains `Application Support`, so a future reintroduction of `ProjectDirs` fails loudly.

## 3. Wire `Config::load` through the resolver

- [x] 3.1 Rewrite `config_path()` in `src/config/mod.rs` as `paths::resolve(&paths::ConfigPathInputs::from_env())`, keeping the `-> Option<PathBuf>` signature so `src/config/commands.rs` callers are untouched. Delete the `ProjectDirs::from("dev","hop","hop")` call from this function.
- [x] 3.2 Rewrite `Config::load` to use the resolved path: `None` or non-existent ⇒ `Ok(Config::default())`; existing ⇒ read and parse, keeping the existing `with_context(|| format!("reading {}", path.display()))` so a malformed or unreadable config errors with its path rather than silently defaulting.
- [x] 3.3 Add `Config::load_from_inputs(&ConfigPathInputs) -> Result<Config>` holding that logic, with `load()` as the `from_env()` wrapper, so load-level behavior is testable.
- [x] 3.4 Add tests: a `[launcher] command` in `<tmp-home>/.config/hop/config.toml` is loaded; a file at the legacy `Library/Application Support/dev.hop.hop/` path under the tmp home is *not* loaded (returns defaults); a malformed config at the resolved path returns an error naming the path.
- [x] 3.5 Confirm nothing else in the tree still resolves a config path independently — `grep -rn "ProjectDirs" src/` should only match `src/main.rs`'s cache paths.

## 4. Config commands

- [x] 4.1 Verify `cmd_path`, `cmd_edit`, and `cmd_show` need no changes: each goes through `config_path()` / `Config::load` and inherits the corrected path. Do not add a `--verbose` flag — with one resolved path there is no candidate selection to explain.
- [x] 4.2 Add a `legacy_macos_config(home: &Path) -> Option<PathBuf>` helper in `src/config/paths.rs` returning the pre-`0.4.0` `Library/Application Support/dev.hop.hop/config.toml` path when it exists. Take `home` as a parameter so it is testable; gate the `cfg!(target_os = "macos")` check at the call site, not inside.
- [x] 4.3 In `cmd_init`, when about to scaffold and the helper reports a legacy file, print a notice to stderr naming that path, stating its settings are no longer read and suggesting the file be moved. Do not read, move, or modify the file; do not block scaffolding.
- [x] 4.4 Keep the notice out of `Config::load`, `cmd_show`, `cmd_edit`, and the TUI launch path so normal startup stays silent.
- [x] 4.5 Add tests: `legacy_macos_config` returns `Some` only when the file exists under the given home; the init notice text names the legacy path; init still writes the template when the notice fires.

## 5. Documentation

- [x] 5.1 Replace `README.md:243`'s "your platform's config directory (e.g., `~/.config/hop/config.toml`)" with the concrete resolution rule: `$HOP_CONFIG` if set, else `$XDG_CONFIG_HOME/hop/config.toml`, else `~/.config/hop/config.toml` — the same on macOS and Linux. The vague phrasing is what let this bug survive.
- [x] 5.2 Document `$HOP_CONFIG` in the README as the explicit override, and mention `hop config path` as the way to confirm which file is in effect.
- [x] 5.3 Add a CHANGELOG entry for the macOS breaking change, naming the old path and giving the one-line fix: `mv "$HOME/Library/Application Support/dev.hop.hop/config.toml" "$HOME/.config/hop/config.toml"`.
- [x] 5.4 Add a `Known Pressure Points` entry to `docs/ARCHITECTURE.md` recording that config resolution is XDG-only on both platforms while cache/data dirs still use `ProjectDirs`, so the asymmetry reads as a decision rather than an oversight.
- [x] 5.5 Update `config_template()`'s header comment in `src/config/mod.rs` if it references the platform config directory, so a scaffolded file does not restate the old assumption.

## 6. Verification

- [x] 6.1 Run `cargo test` and `cargo clippy --all-targets` clean; confirm no `unsafe` env mutation was introduced.
- [x] 6.2 Manually verify on macOS: `hop config path` prints `~/.config/hop/config.toml`; `hop config init` creates it there, not in `Application Support`; a `[launcher]` command placed there takes effect on resume — the originally reported symptom.
- [x] 6.3 Verify `HOP_CONFIG=/tmp/alt.toml hop config path` prints `/tmp/alt.toml` and `HOP_CONFIG=/tmp/alt.toml hop config show` reflects that file.
- [x] 6.4 Verify `XDG_CONFIG_HOME=/tmp/xdg hop config path` prints `/tmp/xdg/hop/config.toml`, and that a relative `XDG_CONFIG_HOME=rel` falls back to `~/.config/hop/config.toml`.
- [x] 6.5 Verify the legacy notice: place a file at `~/Library/Application Support/dev.hop.hop/config.toml` with no XDG config present, run `hop config init`, and confirm the notice appears and the old file is untouched.
