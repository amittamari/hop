## Why

The upgrade notification currently prints to stderr after the TUI exits, where it
is easily missed or lost in terminal noise. Moving it into the TUI footer makes
it visible while the user is actively looking at hop, without interrupting their
workflow.

## What Changes

- The background update-check thread sends its result into the TUI event loop
  (via the existing `Update` channel) instead of returning a `JoinHandle` consumed
  after exit.
- The footer's right-side status area gains an `update` field rendered with the
  theme's accent color, showing a compact `↑ v<latest>` label that persists for
  the session.
- The post-exit `eprintln!` upgrade message is removed. The `--version` path
  keeps its verbose stderr output (version numbers + install command) unchanged.

## Capabilities

### New Capabilities

_(none — this is a wiring change across existing capabilities)_

### Modified Capabilities

- `update-checker`: The check result is delivered to the TUI loop as an `Update`
  variant instead of being printed to stderr after exit. The `--version` path is
  unchanged.
- `footer`: The right-side status gains an `update` field styled with
  `theme.accent`, rendered as `↑ v<version>`, persistent for the session.

## Impact

- `src/main.rs`: Replace `JoinHandle` plumbing with a channel send; remove
  post-exit `eprintln!`. Pass a `Sender<Update>` clone to the update thread.
- `src/engine.rs`: Add `Update::UpdateAvailable` variant carrying the latest
  version string.
- `src/tui/view/mod.rs`: Add `update: Option<String>` to `StatusLine`.
- `src/tui/view/footer.rs`: Render the new field with accent color.
- `docs/ARCHITECTURE.md`: Update the line about stderr upgrade notice.
