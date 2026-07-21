## Context

The footer is one terminal row rendered in `src/tui/view/mod.rs` (~264-285). It
splits into two `Line`s built in `src/tui/view/footer.rs`:

- Left — `footer_hints_line`: static, keymap-derived primary key-hints.
- Right — `footer_status_line`: volatile sync / PR-pending / warning segments.

Today the layout is:

```rust
let [hints_area, status_area] = Layout::horizontal([
    Constraint::Min(0),
    Constraint::Length(line_display_width(&status_line)),
])
.flex(Flex::SpaceBetween)
.areas(footer_area);
```

The status region is pinned to its exact measured width and the hints get the
slack (`Min(0)`), so on a narrow row the hints are the half that clips. Comments
in both files document this as the intended priority. The request reverses it:
the hints are the useful navigation aid and must win; the status is ancillary
and should vanish when both cannot fit.

A whole-footer guard already exists upstream: below `30x6` the TUI shows
"terminal too small" instead, so the footer only renders at width ≥ 30.

## Goals / Non-Goals

**Goals:**
- Left hints take priority; right status is dropped when both do not fit.
- Right status is hidden as a unit (all-or-nothing), never partially clipped.
- Preserve the existing both-fit layout exactly (hints left, status right-aligned).
- Keep measurement in the render path cheap (no second build of the status line).

**Non-Goals:**
- No change to what the hints or status *contain*.
- No abbreviation / truncation of the status when it doesn't fit — it is hidden,
  not shrunk.
- No new config option to toggle the behavior.

## Decisions

**Decision: Decide status visibility by comparing measured widths, not by relying
on layout clipping.** Measure the hints line and the status line with the existing
`line_display_width` helper. Reserve a minimum gap `G` (1 column) between them.
Allocate the status region only when `hints_w + G + status_w <= footer_area.width`;
otherwise give the whole row to the hints and skip rendering the status widget.

- Rationale: `Flex::SpaceBetween` with a fixed-length status region cannot express
  "drop the low-priority side" — it always reserves the status width and clips the
  flexible (hints) side. An explicit width comparison makes the priority explicit
  and testable.
- Alternative considered: swap the constraints so hints get `Length` and status
  gets `Min(0)`. Rejected — that would clip the status from the right and leave a
  partial, misleading status rather than hiding it, and could still starve the
  hints if hints are long.

**Decision: The status line is built once and measured, as today.** Keep building
`status_line` before the layout and reuse it for both the width check and the
render, so we don't pay to build it twice.

**Decision: When both fit, keep the current `SpaceBetween` layout verbatim.** Only
the "doesn't fit" branch changes behavior, minimizing regression surface.

**Decision: Minimum gap `G = 1`.** One blank column keeps the two halves visually
separated at the boundary width. It is a small named constant, not configurable.

## Risks / Trade-offs

- [Hints themselves exceed the row width] → Hints are higher priority, so they may
  clip naturally when alone; this matches the desired priority and the pre-existing
  behavior for an over-long single line. No special handling.
- [Flicker as status appears/disappears while resizing] → Acceptable and expected;
  the toggle is deterministic on width and only at the boundary.
- [Comments/docs drift] → Update the stale priority comments in `footer.rs` and
  `mod.rs` as part of the change so the documented order matches the code.
