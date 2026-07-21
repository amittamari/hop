## Why

The footer's right-side status (sync / PR-pending / warning) currently wins the
fight for space: it is sized to its exact width and the left-side key-hints clip
first on narrow terminals. That inverts the intended priority — the key-hints are
the always-useful navigation aid, while the right-side status is ancillary. On a
narrow terminal the two halves can also collide or the more useful hints vanish
first, which is the wrong trade-off.

## What Changes

- Reverse footer priority: the left-side key-hints become the high-priority half
  and the right-side status becomes the low-priority half.
- Hide the right-side status entirely when the footer row is too narrow to show
  both halves (with a minimum gap between them), rather than clipping the hints.
- When both fit, keep today's layout: hints on the left, status right-aligned on
  the right.
- Update the footer module/render comments that currently document the old
  priority (right survives, left clips first).

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `footer`: add a requirement governing right-side status priority and its
  visibility when the footer row lacks space for both halves.

<!-- Note: the `footer` capability spec and the migration of the pre-existing
     "Footer filters echo removed" requirement out of `card-rows` were applied
     directly to the main specs as a no-behavior-change doc cleanup, separate
     from this change. This change adds only the new priority requirement. -->

## Impact

- `src/tui/view/mod.rs`: footer layout block (`~264-285`) — decide whether the
  status region is allocated at all based on available width vs. both halves'
  measured widths.
- `src/tui/view/footer.rs`: doc comments describing priority/clipping order.
- `src/tui/view/tests_footer.rs` / `tests_layout.rs`: coverage for the
  hide-when-narrow behavior.
- No config, data, or persistence changes.
