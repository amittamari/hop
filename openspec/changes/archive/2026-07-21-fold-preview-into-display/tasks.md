## 1. Config schema

- [x] 1.1 In `src/config.rs`, add `visible: bool`, `width_pct: u16`, and
  `metadata_header: bool` to `DisplayConfig` with the existing serde defaults
  (`default`, `default_width_pct`, `default_true`); doc-comment
  `metadata_header` as compact-view-only
- [x] 1.2 Extend the `Default` impl for `DisplayConfig` with `visible: false`,
  `width_pct: 50`, `metadata_header: true`
- [x] 1.3 Remove the `PreviewConfig` struct, its `Default` impl, and the
  `preview: PreviewConfig` field from `Config`

## 2. Remove UiState

- [x] 2.1 Delete the `UiState` struct and its `load`/`save` impl from
  `src/config.rs`; drop the now-unused `Serialize` import
- [x] 2.2 Delete `ui_state_path()` from `src/main.rs`
- [x] 2.3 In `src/main.rs`, replace the `UiState::load(...).unwrap_or(...)` seed
  (~line 226) with `(config.display.visible, config.display.width_pct)`
- [x] 2.4 Remove the save-on-exit `UiState { .. }.save(&ui_path)` block
  (~line 440) in `run_tui`
- [x] 2.5 Remove the `ui_path` parameter from `run_tui` and its call site

## 3. Preview consumers

- [x] 3.1 In `src/main.rs`, change the metadata-header read (~line 310) to
  `app.set_preview_header(config.display.metadata_header)`
- [x] 3.2 Confirm `app.set_preview(init_preview.0, init_preview.1)` still wires
  the seeded config values (no behavior change beyond the source)

## 4. Tests

- [x] 4.1 Update `preview_defaults` and `preview_from_toml` in `src/config.rs` to
  assert against `cfg.display.*` using a `[display]` TOML section
- [x] 4.2 Delete the `ui_state_roundtrips` test
- [x] 4.3 Run `cargo test --lib` and `cargo test` to confirm green

## 5. Docs and demo

- [x] 5.1 Update `README.md`: merge the `[preview]` keys into the documented
  `[display]` section, remove the standalone `[preview]` block, and note preview
  visibility/width are not persisted across sessions
- [x] 5.2 Update `demo/.demo-home/Library/Application Support/dev.hop.hop/config.toml`
  to move `visible`/`width_pct` under `[display]`
