## 1. Layout restructure

- [x] 1.1 In `view.rs` `render()`, change the vertical layout from 4 bands (`header`, `toolbar`, `body`, `footer`) to 3 bands (`header`, `body`, `footer`), removing the `Constraint::Length(app.toolbar_rows())` band
- [x] 1.2 In `view.rs` `render()`, build the toolbar `Line` via `toolbar::line()` before rendering the header, measure its width with `line_display_width()`, and split `header_area` horizontally into `[query_area: Min(0), toolbar_area: Length(toolbar_w)]`
- [x] 1.3 Render the search input `Paragraph` into `query_area` and the toolbar `Paragraph` into `toolbar_area` (right-aligned), replacing the old separate render sites

## 2. Cleanup

- [x] 2.1 Remove or simplify `App::toolbar_rows()` in `mod.rs` since it no longer drives the vertical layout
- [x] 2.2 Update cursor position calculation in `render()` to use `query_area` instead of `header_area` if needed (verify x-offset is correct)

## 3. Tests

- [x] 3.1 Update `simple_mode_renders_scope_and_sort_toolbar` test to verify toolbar text appears on the header row (same row as the search prompt)
- [x] 3.2 Update `raw_mode_hides_toolbar` test to verify no toolbar text on the header row in raw mode
- [x] 3.3 Run `cargo test` and fix any other render tests broken by the row-count change (e.g., tests that depend on body starting at row 2 in simple mode)
