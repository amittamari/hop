## Context

The TUI layout in `view.rs` uses a 4-band vertical layout: header (search input), toolbar (Scope + Sort controls), body (results + preview), footer (hints + status). The toolbar band is `Length(1)` in simple mode and `Length(0)` in raw mode, controlled by `App::toolbar_rows()`. The toolbar `Line` is built by `toolbar::line()` which returns a styled `Line<'static>` with the controls.

The footer already uses a horizontal split pattern (`Layout::horizontal([Min(0), Length(status_width)])` with `Flex::SpaceBetween`) to place hints left and status right on a single row. The existing `line_display_width()` helper measures a `Line`'s display width for sizing.

## Goals / Non-Goals

**Goals:**
- Reclaim 1 vertical row in simple mode by rendering the toolbar inline with the search header
- Maintain identical interaction model (Tab focus cycling, Left/Right adjustment)
- Keep raw mode behavior unchanged (no toolbar, query takes full width)

**Non-Goals:**
- Changing toolbar controls, adding new controls, or modifying their visual style
- Changing the focus/keybinding model
- Responsive hiding of toolbar controls on narrow terminals (natural clipping is acceptable)

## Decisions

### D1: Horizontal split of the header row (over appending spans)

Split the header area into `[query_area: Min(0), toolbar_area: Length(toolbar_w)]` rather than appending toolbar spans to the header `Line`.

**Rationale**: A layout split keeps query and toolbar as independent render regions, preventing visual bleeding between them. It mirrors the proven footer pattern at `view.rs:245-250`. The cursor position calculation stays clean since `query_area` starts at the same x-offset as the old `header_area`.

**Alternative considered**: Appending toolbar spans directly to the header `Line` — simpler code but no visual separation, and toolbar text could overlap with long queries or indexing indicators.

### D2: Remove toolbar_rows() and the dedicated vertical band

Eliminate the `toolbar_rows()` method and the `toolbar_area` from the vertical `Layout`. The vertical layout becomes 3 bands: header, body, footer.

**Rationale**: The method existed solely to size the dedicated toolbar row. With the toolbar inside the header, it serves no purpose. Removing it simplifies the layout and eliminates a mode-dependent branch.

### D3: Build toolbar Line first, then size the split

Compute the toolbar `Line` before splitting the header area, measure its width with `line_display_width()`, and use that measurement as the `Length` constraint.

**Rationale**: This avoids hardcoding toolbar width or computing it separately from the actual rendered content. When the toolbar is empty (raw mode), `Length(0)` naturally gives the query the full row.

## Risks / Trade-offs

- [Narrow terminals] The toolbar takes fixed width from the header, potentially squeezing the query display area on terminals under ~60 cols. → Acceptable: the query text clips naturally via `Min(0)`, and the user can still type (cursor tracks correctly). This matches current footer behavior where hints clip before status.
- [Test churn] Several render tests assert toolbar text appears on the screen without checking its vertical position. These should pass without changes. Tests that explicitly check toolbar_rows or row counts will need minor updates.
