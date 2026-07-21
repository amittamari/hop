## Why

TUI display settings are scattered and partly duplicated. Preview keys live in
their own `[preview]` config section while every other list-rendering knob
(`row_style`, `icons`) lives under `[display]`. Worse, `visible` and `width_pct`
also exist in a persisted `UiState` (`ui_state.toml`) that unconditionally
shadows the config — so after the first run, editing those config values does
nothing, a silent trap. Consolidating gives the TUI one coherent config home and
one clear rule: config seeds display state, runtime changes live in memory.

## What Changes

- **BREAKING**: Remove the `[preview]` config section. Its three keys
  (`visible`, `width_pct`, `metadata_header`) move into `[display]`. A config
  that still uses `[preview]` will no longer apply those values.
- **BREAKING**: Remove `UiState` persistence (`ui_state.toml`) entirely. Preview
  visibility and width are seeded from `[display]` at launch, mutated in memory
  during the session, and discarded on exit — they no longer persist across
  runs.
- Collapse `PreviewConfig` into `DisplayConfig` in `src/config.rs`; delete the
  `UiState` struct, its `load`/`save`, and `ui_state_path()` in `src/main.rs`.
- Update the `main.rs` startup path to seed preview state directly from
  `config.display` and drop the save-on-exit and `ui_path` threading.
- Document `metadata_header` as a compact-view-only preference (the render
  already gates the preview header to non-card row styles).
- Update `README.md` and the demo `config.toml` to the merged `[display]`.

## Capabilities

### New Capabilities
- `configuration`: Documents the `config.toml` schema contract for TUI display —
  which sections exist, where preview settings live, and that display state is
  config-seeded and in-memory only (not persisted).

### Modified Capabilities
<!-- No existing capability spec governs config schema or UI-state persistence. -->

## Impact

- **Code**: `src/config.rs` (`PreviewConfig` and `UiState` removed,
  `DisplayConfig` extended, `Serialize` import dropped), `src/main.rs`
  (`ui_state_path`, `UiState::load` seed, save-on-exit, and the `ui_path`
  parameter of `run_tui` all removed).
- **User-facing**: TOML key layout changes (breaking for a `[preview]` section);
  preview visibility/width no longer persist between sessions.
- **Docs**: `README.md` config reference, demo `config.toml`.
