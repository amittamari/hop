## Context

The TUI run loop reads only key events (`src/main.rs:390` —
`if event::poll(..) && let Event::Key(key) = event::read()?`), so any
`Event::Mouse` crossterm returns is silently dropped. The terminal is set up with
`ratatui::init()` (`src/main.rs:293`) and torn down with `ratatui::restore()`
(`src/main.rs:428`); `ratatui::init()` enables raw mode and the alternate screen
but does **not** enable mouse capture, so the app never receives wheel events in
the first place.

The conversation preview already has a working vertical scroll model: the
`preview_scroll: u16` offset (`src/tui/mod.rs:90`) is applied to the transcript
`Paragraph` via `.scroll((preview_scroll, 0))` (`src/tui/view/mod.rs:259`) and is
driven today by the keyboard `Command::ScrollPreview(d)` handler
(`src/tui/input.rs:153`), which steps by `preview_scroll_step` (≈ one page,
computed in `set_viewport_metrics`, `src/tui/app_state.rs:186`). The sessions
list has no independent scroll offset — its visible window is derived from
`selected` at render time.

This change adds a mouse-scroll path that reuses the existing `preview_scroll`
offset but with a smaller, wheel-appropriate step.

## Goals / Non-Goals

**Goals:**
- Wheel/trackpad scroll moves the conversation preview, not the list (issue #71).
- Mouse capture is enabled on entry and cleanly released on every exit path,
  including exec-resume.
- Wheel scrolling feels natural (small per-notch line step).

**Non-Goals:**
- Pane hit-testing / click-to-focus. Scroll always targets the preview when it is
  visible; we do not route based on cursor column. This keeps `App` free of stored
  render rects.
- Scrolling the sessions list with the wheel, or scrolling the preview while it is
  hidden.
- Click, drag, selection, or any non-scroll mouse interaction.
- A configuration toggle for mouse capture (can be added later if the text-selection
  tradeoff proves annoying).

## Decisions

**Enable mouse capture explicitly around the ratatui lifecycle.**
`ratatui::init()` does not enable capture, so wrap the standard setup with an
explicit `execute!(stdout(), EnableMouseCapture)` on entry and
`execute!(stdout(), DisableMouseCapture)` before/around `ratatui::restore()`. The
disable MUST also run on the resume path that restores the terminal before `exec`
(`src/main.rs:247` region), so the resumed agent CLI inherits a normal terminal.
Alternative considered: switch to a custom terminal init that bundles capture —
rejected as more churn than a paired execute! around the existing init/restore.

**Route all scroll events to the preview; no hit-testing.**
The issue asks for "scroll the preview instead of the list." The simplest
implementation matching that intent is: on `MouseEventKind::ScrollUp/ScrollDown`,
adjust `preview_scroll` when `preview_visible`, else ignore. This avoids storing
`list_area`/`transcript_area` rects on `App` for column hit-testing. If per-pane
routing is wanted later, the render already computes those rects and they can be
stored then.

**Add a small mouse step, distinct from the keyboard page step.**
The keyboard `ScrollPreview` step is ~one page (`preview_scroll_step`), too large
for a wheel notch. Introduce a small fixed line step (e.g. 3 lines) for mouse
scroll. Reuse the same clamp-at-zero logic as the keyboard handler
(`next.max(0)`); no upper clamp is added (consistent with existing keyboard
behavior, where over-scroll shows blank lines and is bounded by the Paragraph).

**Handle mouse in the run loop, delegate to an `App` method.**
Extend the `event::read()?` match in `main.rs` to also handle `Event::Mouse(me)`,
calling a new `App` method (e.g. `handle_mouse_scroll(direction)` or reusing a
small preview-scroll helper). Keeping the state mutation in `App` matches the
existing `handle_key` boundary and keeps `main.rs` as orchestration.

## Risks / Trade-offs

- [Mouse capture disables native click-drag text selection while the TUI runs] →
  This is standard for full-screen TUIs and the app runs on the alternate screen,
  so scrollback/history is unaffected. Most terminals still allow selection via a
  modifier (e.g. Shift+drag). Documented as a known tradeoff; a config opt-out can
  follow if needed.
- [Capture not released on an unexpected exit path leaves the terminal in mouse
  mode] → Ensure disable is paired with every restore path (normal exit + resume).
  Verify the resume path specifically, since it restores then `exec`s.
- [Wheel step feels too fast/slow] → Small fixed step is easy to tune; can become
  configurable later if requested.
