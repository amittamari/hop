## 1. Mouse capture lifecycle

- [x] 1.1 Enable mouse capture on TUI entry: after `ratatui::init()` in `src/main.rs`, `execute!(std::io::stdout(), EnableMouseCapture)` (import from `ratatui::crossterm::{execute, event::EnableMouseCapture}`).
- [x] 1.2 Disable mouse capture on normal teardown: `execute!(std::io::stdout(), DisableMouseCapture)` before/around `ratatui::restore()` at `src/main.rs:428`.
- [x] 1.3 Disable mouse capture on the resume path that restores the terminal before `exec`. `run_tui` restores the terminal (line ~428) before returning; the caller exec-resumes only after that, so the single `DisableMouseCapture` before `ratatui::restore()` covers both quit and resume paths.
- [x] 1.4 Confirmed by code inspection: both exit paths (quit `Ok(None)` and resume `Ok(Some(..))`) flow through the closure return to the shared `DisableMouseCapture` + `ratatui::restore()`.

## 2. Mouse scroll routing

- [x] 2.1 In `src/main.rs`, extend the `event::read()?` handling (now a `match` on the read event) to also handle `Event::Mouse(me)`.
- [x] 2.2 Map `MouseEventKind::ScrollUp` / `MouseEventKind::ScrollDown` to a scroll direction and ignore other mouse event kinds (handled inside `App::handle_mouse`).
- [x] 2.3 Added `App::handle_mouse(me)` in `src/tui/input.rs` that adjusts `preview_scroll` by a small fixed line step (`MOUSE_SCROLL_LINES = 3`) only when `preview_visible`; clamps at zero via `.max(0)`; no-op when the preview is hidden.
- [x] 2.4 Run loop calls `app.handle_mouse(me)` for mouse events; the method never touches `selected`.

## 3. Tests & docs

- [x] 3.1 Added unit tests in `src/tui/app_tests.rs`: `mouse_scroll_moves_preview_not_selection`, `mouse_scroll_up_clamps_at_top`, `mouse_scroll_ignored_when_preview_hidden` — covering scroll step, zero clamp, `selected` unchanged, and hidden-preview no-op.
- [x] 3.2 Updated `README.md`: added a mouse/trackpad scroll row to the keybindings table and a "Mouse capture" note on the text-selection tradeoff. (Help overlay left as-is: it is generated from the chord-based `bindings()` catalog and mouse scroll is not a chord.)
- [x] 3.3 Ran `cargo fmt` and `cargo test` (300 passed, build clean).
