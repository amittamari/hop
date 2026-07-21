## Context

The update checker already runs on a background thread during the TUI session.
Its result is currently consumed *after* `run_tui()` returns and printed to
stderr. The TUI event loop already consumes an `mpsc::Receiver<Update>` for sync
progress events and calls `LoopState::process_sync()` on each tick. The footer
status line already renders volatile info (sync state, PR-pending count,
warnings) via `StatusLine` fields.

## Goals / Non-Goals

**Goals:**
- Surface upgrade availability inside the TUI where the user is already looking.
- Minimal wiring: reuse the existing `Update` channel and `StatusLine` pattern.
- Compact, non-intrusive, accent-colored indicator.

**Non-Goals:**
- Interactive upgrade (running `brew upgrade` from the TUI).
- Dismissable / toast-style animation.
- Changing the `--version` output or the update-check logic itself.

## Decisions

### 1. Reuse `Update` enum over a separate channel

Add `Update::UpgradeAvailable { latest: String }` to the existing
`engine::Update` enum. The background thread sends this variant through a clone
of the same `Sender<Update>` used by sync. `LoopState::process_sync()` already
drains this channel every tick.

**Alternative considered:** A separate `oneshot` channel polled independently.
Rejected because it adds a new parameter to `run_tui`, a new poll site in the
loop, and a new dependency — all for a single message.

### 2. Store the version string in `LoopState`, expose via `StatusLine`

`LoopState` gains `update_available: Option<String>`. When
`Update::UpgradeAvailable` is received, set it once. `build_status()` passes it
through to `StatusLine { update: … }`.

### 3. Render with accent color, persistent, compact

The footer renderer appends `↑ v<version>` styled with `theme.accent` (cyan).
It uses the same separator glyph and priority rules as other status fields. No
dismissal mechanism — the field persists for the session.

### 4. Thread plumbing: clone the sender

`run_tui` already receives `updates: Receiver<Update>`. The `Sender` is held by
the sync thread. To give the update-check thread a sender:

- Create the channel in `run()` (the caller), which already does this.
- Clone the `Sender` before spawning the update thread, pass the clone in.
- The update thread sends `Update::UpgradeAvailable` and exits.
- Remove the `JoinHandle`-based post-exit path entirely.

### 5. Drop the stderr fallback

The post-exit `eprintln!` is removed. Edge case: if the user quits before the
check completes, they miss the notification — acceptable because they'll see it
next launch. The `--version` path is untouched.

## Risks / Trade-offs

- **[Narrow terminal]** The footer drops the right-side status entirely when
  width is tight. The upgrade notice would be hidden along with sync/warning
  status. → Acceptable; existing behavior for all status fields, and the notice
  reappears when the terminal is resized wider.

- **[Channel ordering]** `Update::UpgradeAvailable` arrives on the same channel
  as sync events. It could arrive mid-sync or after. → No risk; `process_sync`
  handles each variant independently, and the update field is write-once (first
  `UpgradeAvailable` wins, subsequent ones are no-ops since the check only sends
  one).
