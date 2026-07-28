## Context

`src/config/mod.rs:150` resolves the config file with a single expression:

```rust
directories::ProjectDirs::from("dev", "hop", "hop")
    .map(|dirs| dirs.config_dir().join("config.toml"))
```

`ProjectDirs::config_dir()` is platform-idiomatic, not XDG-uniform:

| Platform | `config_dir()` |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/hop` → `~/.config/hop` |
| macOS | `~/Library/Application Support/dev.hop.hop` |

`README.md:243` says the config lives at `~/.config/hop/config.toml`. On Linux that is true; on
macOS it is wrong, and the failure is silent — `Config::load` sees a non-existent path, returns
`Config::default()`, and `hop` runs as if the user never wrote a config. The reported symptom was
`[launcher]` being ignored, but the whole file is dropped.

Constraints:

- `hop` is Unix-only: `src/resume.rs` uses `std::os::unix::process::CommandExt`, and resume
  `exec`-replaces the process. So the platform matrix is macOS + Linux, and the *only* thing
  `ProjectDirs` contributes to config resolution is the macOS `Application Support` convention that
  causes this bug. There is no Windows case to preserve.
- `config_path()` reads the process environment and the real home directory, so today's tests in
  `src/config/commands.rs` only exercise `write_template` against a `tempdir` — resolution itself is
  untested. New precedence logic must be testable without `std::env::set_var`, which is `unsafe` in
  Rust 2024 and racy under `cargo test`'s thread-per-test.
- Cache paths (`src/main.rs:25-41`) also go through `ProjectDirs`, landing in
  `~/Library/Caches/dev.hop.hop` on macOS. Machine-managed, never hand-edited, not documented as XDG
  paths — out of scope. The resulting asymmetry needs writing down or a future reader will "fix" it.

The owner's call is that the legacy macOS location has no users worth a read-only fallback. That
decision is what makes the rest of this design small.

## Goals / Non-Goals

**Goals:**

- Honor `~/.config/hop/config.toml` on macOS, matching what the README documents.
- Resolve the config to exactly one path, with no platform-conditional branch.
- Keep `$HOP_CONFIG` as an explicit escape hatch, which doubles as the seam for hermetic tests.
- Make resolution a pure, unit-testable function.
- Tell the one population this breaks, instead of silently dropping their settings.

**Non-Goals:**

- Reading the legacy `Application Support` path. Dropped per the owner's call.
- Auto-migrating or moving any file. The `init` notice is text only; `hop` never touches the old file.
- Changing cache/data directory resolution (`index`, `enrich`, `update_check`).
- Merging or layering multiple config files. There is one config file.
- Windows support, which the crate does not have.

## Decisions

### D1: One resolved path, not a candidate chain

```
$HOP_CONFIG                        if set and non-empty  (a file path, verbatim)
<xdg-config-home>/hop/config.toml  otherwise
    where <xdg-config-home> = $XDG_CONFIG_HOME  if set, non-empty, and absolute
                              ~/.config         otherwise
```

`config_path()` keeps its current `-> Option<PathBuf>` signature; `None` only when `$HOP_CONFIG` is
unset *and* the home directory cannot be determined. Callers are unchanged.

The key consequence of dropping the legacy path: because every remaining candidate is one `hop` may
freely create, there is no distinction between "the file we read" and "the file we write." An earlier
draft of this design needed an *effective path* (first existing candidate) and a *primary path*
(first writable candidate) precisely to express "read the legacy file, but create new files in the
XDG location." With the legacy entry gone, both collapse into one path, and with them go the
candidate vector, the writability predicate, the resolution trace, and the `hop config path
--verbose` flag that existed to explain which candidate won. Nothing selects, so nothing needs
explaining.

Note that `$XDG_CONFIG_HOME` and `~/.config` are *mutually exclusive*, not ranked. The XDG base
directory spec says `$XDG_CONFIG_HOME` replaces `~/.config` when set; it is not a higher-priority
sibling to search first. Treating them as two ordered candidates would mean a user with a custom
`$XDG_CONFIG_HOME` could still be read from `~/.config`, which is neither XDG-correct nor what
`ProjectDirs` does today.

*Alternatives considered.* **Read-only legacy fallback** — no silent breakage for existing macOS
users, but it forces the effective/primary split through the whole module and command layer, and
keeps a permanent macOS-gated branch. Rejected by the owner on the grounds that the affected
population is empty. **Auto-migrating the legacy file** — makes `hop` mutate user files as a side
effect of launching a TUI, and a half-completed move leaves the user worse off than either
alternative. Rejected.

### D2: `ProjectDirs` is dropped for config, kept for cache

The new resolution reproduces `ProjectDirs`' Linux behavior exactly, so Linux users observe no
change. On macOS it deliberately diverges. `BaseDirs::new().home_dir()` still supplies the home
directory, and `src/main.rs`'s `hop_dirs()` and three cache paths stay untouched.

This leaves config on XDG and cache on platform-native conventions. That asymmetry is intentional —
config is hand-edited and documented, cache is neither — and gets a `Known Pressure Points` entry in
`docs/ARCHITECTURE.md` so it is not "corrected" later.

### D3: Resolution is a pure function over injected inputs

```rust
pub struct ConfigPathInputs {
    pub hop_config: Option<PathBuf>,      // $HOP_CONFIG
    pub xdg_config_home: Option<PathBuf>, // $XDG_CONFIG_HOME
    pub home: Option<PathBuf>,
}
```

`ConfigPathInputs::from_env()` is the only code that touches `std::env` and `BaseDirs`;
`resolve(&ConfigPathInputs) -> Option<PathBuf>` is pure and touches no filesystem — not even for
existence, which is the caller's separate concern.

This is what makes the spec's scenarios testable: a test builds inputs pointing at a `tempdir` and
asserts the resolved path, with no `unsafe { std::env::set_var }` and no dependency on the
developer's real `~/.config`. Without this split, precedence stays as untested as it is today.

Guards, all in the pure function: `$XDG_CONFIG_HOME` is honored only when non-empty and absolute (the
XDG spec requires ignoring relative values); an empty `$HOP_CONFIG` is treated as unset; when `home`
is `None` and no `$HOP_CONFIG` is set, the result is `None` rather than a path joined onto a relative
root.

There is no `cfg!(target_os = ...)` anywhere in the resolver, so every scenario runs on both
platforms in CI — a property the read-only-fallback design could not offer.

`$HOP_CONFIG` names a *file*, not a directory: it is an override for "load exactly this config,"
which is the right shape for scripts, profile switching, and tests. Pointed at a directory, the read
fails loudly with the path in the message, which is correct for an explicit override.

### D4: The one place the legacy path is still named is `hop config init`

Dropping the read means a macOS user with a config under `Application Support` silently loses every
setting. `hop config init` is where that is cheapest to catch: when it is about to scaffold a fresh
config on macOS and the old file exists, it prints a note naming the old path and saying its settings
are no longer read.

Deliberately scoped:

- Command layer only. `Config::load` never mentions the legacy path, so the load path — and the TUI
  launch path — stays clean and quiet.
- `init` only, not `edit`/`show`/`path`. `init` is the command that already knows it is creating a
  config from scratch, which is exactly the moment the notice is actionable.
- A printed note. No move, no copy, no write to the old location.

The cost is one path constant and one existence check behind `cfg!(target_os = "macos")`. If the
affected population really is empty, this is dead code that costs one task to delete; if it is not,
it is the difference between a clear message and silent data loss. The check takes `home` as a
parameter so it stays testable.

## Risks / Trade-offs

- **A macOS user with a legacy config silently loses their settings** → The `init` notice covers
  anyone who reruns `init`; a CHANGELOG entry covers the rest. This is the accepted cost of the
  owner's decision that the population is empty, and it is why the notice was kept rather than
  dropped along with the fallback.
- **Users who never rerun `init` get no in-product signal** → `hop config path` prints the new path,
  which makes the discrepancy visible the moment anyone investigates. A launch-time warning was
  rejected: the TUI must stay quiet, and warning on every launch for a population believed empty is
  the wrong trade.
- **`$HOP_CONFIG` is a new public surface that must keep working** → Documented in the README and
  pinned by spec scenarios. It is also load-bearing for tests, so a regression breaks CI rather than
  escaping quietly.
- **Config/cache asymmetry looks like an inconsistency** → Recorded in `docs/ARCHITECTURE.md` with
  the rationale, so the next reader sees a decision rather than an oversight.
- **Typo'd config keys remain silently ignored** → `Config` does not use
  `#[serde(deny_unknown_fields)]`, so a misspelled key is dropped without complaint. This is a
  distinct silent-config failure mode with the same user-facing shape as the bug being fixed
  ("I set it and nothing happened"), but it is out of scope here and worth its own change.

## Migration Plan

- Linux users: no observable change; the resolver reproduces `ProjectDirs` on Linux.
- macOS users following the README: their config starts working. This is the point of the change.
- macOS users with a legacy `Application Support` config: must move it to `~/.config/hop/config.toml`.
  Surfaced by the `hop config init` notice and a CHANGELOG entry; `hop config path` shows the new
  location. One-line fix for the user:
  `mv "$HOME/Library/Application Support/dev.hop.hop/config.toml" "$HOME/.config/hop/config.toml"`
- Rollback is a straight revert. This change writes and moves nothing on its own; only `init`/`edit`
  write, at explicit user request, as they already did.

## Open Questions

- Should the CHANGELOG entry be flagged as breaking for `0.4.0`, or is a note under fixes enough
  given the belief that no one is on the legacy path? Leaning breaking-note: it costs a line and the
  claim about the population is an assumption, not a measurement.
- Should `$HOP_CONFIG` also accept a directory (joining `config.toml`)? Deferred as unnecessary
  ambiguity; file-only is the simpler contract and can be widened compatibly later.
