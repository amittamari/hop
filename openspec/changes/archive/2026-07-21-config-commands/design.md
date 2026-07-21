## Context

Hop's `Config` struct is loaded from a platform-specific TOML file
(`directories::ProjectDirs::from("dev", "hop", "hop")` → `config.toml`). It
covers display, keybindings, columns, launcher, search mode, and data dirs.
Users currently have no CLI surface to discover, inspect, or scaffold this file.

The CLI (`cli.rs`) already has a two-tier subcommand pattern:
`Command::Hooks { action: HooksAction }` and `Command::Meta { action: MetaAction }`.
Adding `Command::Config { action: ConfigAction }` follows the same shape.

## Goals / Non-Goals

**Goals:**
- Users can discover the config file path without reading source code.
- Users can scaffold a commented template without hand-authoring TOML.
- Users can inspect the effective config (defaults + overrides) for debugging.
- Users can open the config in their editor with one command.

**Non-Goals:**
- Interactive wizard or guided setup.
- Programmatic `set`/`get` of individual keys.
- Auto-generating the template from struct metadata.

## Decisions

### 1. Add `Serialize` to config structs via serde

`hop config show` needs to serialize the effective `Config` back to TOML.
Add `#[derive(Serialize)]` to `Config`, `DisplayConfig`, `ColumnsConfig`,
and `LauncherConfig`. The `toml` crate already supports serialization.

`RowStyle` is not deserialized directly (it's derived from a string field),
so it doesn't need `Serialize`.

**Alternative**: Print a hand-formatted string instead of serializing.
Rejected — it would drift from the actual struct and miss new fields.

### 2. Hand-maintained template string in `config.rs`

A `pub fn config_template() -> &'static str` function returns the scaffold.
Every section is commented out with `#` prefixes, includes a description,
and shows the default value. This is the single source for both `init` and
`edit` (auto-create).

A round-trip test (`#[test] fn template_parses`) strips `# ` comment prefixes
and parses the result through `Config::from_toml_str` to catch structural
drift.

**Alternative**: Generate the template at runtime from `Config::default()` +
annotations. Rejected — TOML serialization doesn't produce comments, and
proc-macro annotation is heavy for this scope.

### 3. Editor resolution chain

`$VISUAL` → `$EDITOR` → `vi`. Standard Unix convention. The command is
spawned with `std::process::Command::new(editor).arg(path).status()`,
inheriting stdin/stdout/stderr so the editor runs interactively.

### 4. `init` is idempotent, `edit` auto-creates

`init` writes the template only if the file doesn't exist, prints a message
if it already does. `edit` auto-creates from the template if missing, then
opens. This makes `hop config edit` the single entry point for new users.

### 5. Config dir creation

The parent directory may not exist (especially on first run). Both `init`
and `edit` (auto-create path) call `std::fs::create_dir_all` on the config
directory before writing.

### 6. All four commands live in a new `src/config/commands.rs`

The handlers are pure functions that take a `&Path` (config path) and return
`Result<()>`, keeping them testable without filesystem side effects in the
config loading path. `config.rs` becomes `config/mod.rs` to house both the
existing `Config` struct and the new `commands` submodule.

**Alternative**: Keep handlers inline in `main.rs`. Rejected — `main.rs` is
already orchestration-heavy and the handlers have enough logic (template
writing, editor spawning) to warrant their own module.

## Risks / Trade-offs

- **Template drift** → Mitigated by the round-trip parse test. If a new field
  is added to `Config` but not the template, the test still passes (defaults
  fill in). The risk is a missing field in the scaffold, not a broken parse.
  Acceptable for now; a lint could be added later.
- **Editor spawn failure** → If `$EDITOR` is unset and `vi` is missing, the
  process will fail. Print a clear error suggesting they set `$EDITOR`.
- **Platform path differences** → `directories::ProjectDirs` handles this.
  The `path` command makes the resolved path discoverable regardless of OS.
