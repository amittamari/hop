## Why

The default preview resize keybindings (Ctrl+Left / Ctrl+Right) do not work on macOS because the OS intercepts those key combinations for Mission Control space-switching before the terminal application ever receives them. This makes a documented feature silently broken for the majority of users on the primary development platform.

## What Changes

- Change the default preview resize keybindings from Ctrl+Left/Right to key chords that do not conflict with macOS system shortcuts.
- Enable the Kitty keyboard protocol where the terminal supports it, improving modifier+arrow key detection for users who have disabled the macOS shortcuts or use Linux.
- Update help overlay, footer hints, README, and config template to reflect the new defaults.

## Capabilities

### New Capabilities

_(none)_

### Modified Capabilities

- `tui-keymap`: Default bindings for `resize_preview_smaller` and `resize_preview_larger` change from Ctrl+Left/Right to non-conflicting Ctrl chords. Kitty keyboard protocol support is added to improve modified-key detection.

## Impact

- `src/tui/keymap.rs` — default chord definitions change.
- `src/main.rs` — terminal init adds Kitty keyboard protocol push/pop.
- `src/tui/app_tests.rs` — existing resize test updates to new default keys.
- `README.md` — keybinding table updated.
- `src/config.rs` — config template comment updated.
- Users who explicitly overrode these bindings in `config.toml` are unaffected; only the defaults change.
