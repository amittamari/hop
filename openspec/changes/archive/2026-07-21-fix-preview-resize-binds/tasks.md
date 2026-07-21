## 1. Change default resize bindings

- [x] 1.1 In `src/tui/keymap.rs` `chord_specs()`, change `resize_preview_smaller` default from `(ctrl, KeyCode::Left)` to `(ctrl, KeyCode::Char('k'))` and `resize_preview_larger` default from `(ctrl, KeyCode::Right)` to `(ctrl, KeyCode::Char('l'))`
- [x] 1.2 Update the test `ctrl_arrows_resize_preview` in `src/tui/app_tests.rs` to use the new default keys (Ctrl+K / Ctrl+L)

## 2. Enable Kitty keyboard protocol

- [x] 2.1 In `src/main.rs` terminal init, after `EnableMouseCapture`, check `crossterm::event::supports_keyboard_enhancement()` and if supported push `KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES`
- [x] 2.2 In `src/main.rs` terminal shutdown, pop keyboard enhancement (before `DisableMouseCapture`) if it was pushed
- [x] 2.3 Extend the panic hook (or add a drop guard) to pop keyboard enhancement on panic

## 3. Update documentation and config template

- [x] 3.1 Update the keybinding table in `README.md` to show Ctrl+K / Ctrl+L for resize preview
- [x] 3.2 Update the config template comment in `src/config.rs` (if it mentions Ctrl+Left/Right) to reflect the new defaults (N/A — template only shows example overrides, no arrow-key references)
- [x] 3.3 Run `cargo test` and verify all keymap tests pass
