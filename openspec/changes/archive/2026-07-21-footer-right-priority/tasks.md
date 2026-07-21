## 1. Footer layout

- [x] 1.1 In `src/tui/view/mod.rs` footer block (~264-285), keep building `status_line` once and measure it with `line_display_width`; also measure the hints line width.
- [x] 1.2 Add a `const FOOTER_GAP: u16 = 1;` (or inline named constant) and compute whether `hints_w + FOOTER_GAP + status_w <= footer_area.width`.
- [x] 1.3 When both fit: render the current `SpaceBetween` layout (hints left `Min(0)`, status `Length(status_w)` right-aligned) unchanged.
- [x] 1.4 When they do not fit: render only the hints across the full `footer_area` and skip rendering the status widget entirely.

## 2. Docs / comments

- [x] 2.1 Update the priority/clipping comments in `src/tui/view/footer.rs` (lines ~10-12 and ~48-49) to state that the right-side status is the low-priority half, hidden when both halves don't fit.
- [x] 2.2 Update the footer comment in `src/tui/view/mod.rs` (~264-266) to match the new priority.

## 3. Tests

- [x] 3.1 In `src/tui/view/tests_footer.rs` (and/or `tests_layout.rs`), add a test: wide footer → both hints and status render.
- [x] 3.2 Add a test: narrow footer where both don't fit → status is absent and hints occupy the full row.
- [x] 3.3 Add/confirm a test: empty status → only hints render across the full row.
- [x] 3.4 Run `cargo test --lib` and confirm the footer suite passes.
