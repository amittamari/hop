## Why

The TUI ignores mouse/trackpad input entirely, so the scroll wheel does nothing
useful inside the app. When the conversation preview is open, the natural gesture
for reading a long transcript is to scroll — but today that requires reaching for
Ctrl+U / Ctrl+D. Routing wheel scroll to the preview matches user expectation and
is better UX (issue #71).

## What Changes

- Enable mouse capture when the TUI starts and release it on teardown/resume so
  wheel events reach the app instead of the terminal emulator.
- Handle `Event::Mouse` in the run loop and route `ScrollUp`/`ScrollDown` to the
  conversation preview's vertical scroll offset (`preview_scroll`), not the
  sessions list.
- Use a small per-notch line step for wheel scrolling (distinct from the existing
  page-sized keyboard step) so trackpad/wheel scrolling feels natural.
- When the preview pane is not visible, wheel scroll is a no-op; keyboard list
  navigation is unchanged.

## Capabilities

### New Capabilities
- `mouse-scroll`: How the TUI captures mouse wheel/trackpad scroll events and
  routes them to the conversation preview pane, including scroll granularity and
  behavior when the preview is hidden.

### Modified Capabilities
<!-- No existing spec-level requirements change. Keyboard scrolling and list
     navigation behavior are preserved as-is. -->

## Impact

- `src/main.rs`: terminal init/restore (enable/disable mouse capture) and the
  event loop (`event::read` match) to also handle `Event::Mouse`.
- `src/tui/input.rs` and/or `src/tui/mod.rs`: a mouse-scroll path that adjusts
  `preview_scroll` by a small line step.
- Tradeoff: with mouse capture enabled, the terminal's native click-drag text
  selection is intercepted by the app while the TUI is running (standard for
  full-screen TUIs). Terminal scrollback is unaffected because the app runs on the
  alternate screen.
