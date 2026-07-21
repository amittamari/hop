## Context

`src/config.rs` splits TUI display settings across two structs — `DisplayConfig`
(`row_style`, `icons`) and `PreviewConfig` (`visible`, `width_pct`,
`metadata_header`) — deserialized from `[display]` and `[preview]`. Separately, a
persisted `UiState` (`ui_state.toml`) holds `preview_visible`/`preview_width_pct`.

The two overlap. At startup (`main.rs:226-228`) `UiState` is loaded and
*unconditionally* shadows the config; on exit (`main.rs:440-444`) the live
geometry is written back. So the config `visible`/`width_pct` only matter on the
very first run before `ui_state.toml` exists — after that, editing them is a
no-op. `metadata_header` is config-only and never persisted; the render at
`view/mod.rs:232` already draws the preview header only when
`row_style != Card`, so it is inherently a compact-view preference.

## Goals / Non-Goals

**Goals:**
- One config section (`[display]`) for all TUI look-and-feel settings.
- One clear ownership rule: config seeds display state; runtime changes live in
  memory only.
- Remove `PreviewConfig` and `UiState`, eliminating the duplication and the
  dead-config trap.

**Non-Goals:**
- No key renames — `visible`/`width_pct`/`metadata_header` keep their names, only
  their section moves.
- No new persistence mechanism to replace `UiState`; cross-session memory of
  preview geometry is intentionally dropped.
- No backward-compat shim for the old `[preview]` section or `ui_state.toml`.

## Decisions

### Decision: display state is config-seeded, in-memory only

Remove `UiState`, its `load`/`save`, and `ui_state_path()`. `main.rs` seeds the
initial preview state directly: `app.set_preview(config.display.visible,
config.display.width_pct)`. Runtime toggles (Ctrl+P) and resizes mutate `App`
state and are discarded on exit. The `ui_path` parameter is removed from
`run_tui`.

Rationale: preview geometry is cheap to re-derive from config and low-value to
persist. Keeping it in memory removes an entire file I/O path and the confusing
config/persistence precedence in one move.

Alternative considered: keep `UiState` but make config the true default and only
persist *diffs*. Rejected — there is no way to distinguish a remembered toggle
from a seeded default in a flat file, and the added machinery is not worth it for
a single-user local tool.

### Decision: merge fields into `DisplayConfig`, drop `PreviewConfig`

Add `visible`, `width_pct`, `metadata_header` directly onto `DisplayConfig` with
their existing serde defaults, and delete `PreviewConfig` and `Config::preview`.
`metadata_header` is documented as compact-only (it has no effect in card mode).

### Decision: no compatibility shim

serde ignores unknown sections, so a stale `[preview]` parses without error but
has no effect; a leftover `ui_state.toml` is simply never read. Accepted breaking
change for a single local user, documented in README.

## Risks / Trade-offs

- [Preview visibility/width no longer persist across sessions] → Intended. Set
  the desired defaults once in `[display]`; every launch starts from them.
- [A user with a `[preview]` section silently loses those settings] → Documented
  as **BREAKING**; keys keep their names, so migration is moving three lines
  under `[display]`.
- [Tests referencing `cfg.preview.*` or `UiState` break] → Updated/removed in the
  same change (the `ui_state_roundtrips` test is deleted).

## Migration Plan

1. Land the code + doc changes together.
2. Users move any `[preview]` keys under `[display]`; a stale `ui_state.toml` can
   be deleted but is harmless if left (never read).
