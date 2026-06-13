# TUI Results Table Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** Replace the hand-formatted results list (a `List<ListItem>` of pre-joined `Line`s plus a SEPARATELY rendered header `Paragraph` that is manually offset by the selection-marker width) with a single Ratatui `Table` widget. This fixes review findings **W1** (two fragile alignment code paths collapsed into one Table whose own layout solver owns per-column width/alignment/truncation) and **W5** (search-match highlighting is currently invisible in the list — thread the query's `free_terms()` into the TITLE cell and reuse `preview::highlight_terms`).

**Architecture:** There are TWO distinct layout responsibilities, and this is the crux of the change — keep them separate:

1. **Column SELECTION (stays in `src/columns.rs`)** — the priority-based *which columns are shown at this width* logic. `solve_layout` / `solve_layout_with_desired` drop columns by descending `priority` until the kept set fits the pane; TITLE (`flex: true`, `priority: u8::MAX`) and AGENT (`priority: u8::MAX`) never drop. We KEEP this. It decides the *set* of columns and also yields a per-column width we use as the desired/content-fit width.

2. **Per-column WIDTH/ALIGNMENT/TRUNCATION within the chosen set (moves to the Table)** — instead of `columns::fit` padding each cell to an exact width and `Span::raw(" ")` spacers between them, we hand the Table a `Vec<Constraint>` (one per kept column) and let Ratatui's own table layout solver own the rest. Fixed columns become `Constraint::Length(width)` (using the width the column solver computed, which is already display-width / content aware — preserving **I-010**); the single flex TITLE column becomes `Constraint::Min(min_width)` so it absorbs leftover space. `.column_spacing(1)` replaces the manual `GAP`/`Span::raw(" ")` spacers. `.highlight_symbol("❯ ")` reserves the left marker column automatically, which DELETES the manual `SELECTION_MARKER_WIDTH` header offset hack.

**Viewport-bounding (I-006) is preserved exactly.** We do NOT build `Row`s for all results. We continue to compute `visible_result_range(...)`, slice `visible_results = app.results().get(visible.clone())`, and build one `Row` per visible session only. The `TableState` selection is set to `app.selected().saturating_sub(visible.start)` — the same offset arithmetic the current `ListState` uses. The column solver is still fed only `visible_results` for content-aware desired widths.

**Match highlight (W5):** `view::render` already has `app.query()`. We parse it with `crate::query::parse(app.query()).free_terms()` once per frame and thread the resulting `&[String]` into the TITLE cell builder. The TITLE cell becomes a `Cell` built from a `Line` whose spans are produced by reusing `preview::highlight_terms` (multi-byte safe, applies `Modifier::REVERSED`). All other cells stay single-span styled `Cell`s.

**Tech Stack:** Rust, Ratatui 0.30 (crossterm backend).

---

## Dependencies & Sequencing

This is the **most invasive** change to results rendering. It touches the list area in `src/tui/view.rs` (lines ~98–148) and adds a public helper to `src/tui/results_list.rs`. It CONFLICTS with the responsiveness, screen-states, and scroll-affordances plans, which also edit the list area of `view.rs`.

**Recommended order (and why):**

1. **Land `2026-06-13-tui-theme-system.md` FIRST.** It introduces `App::theme()` returning a `&Theme` with semantic role fields (`muted`, `selection_fg`, `selection_bg`, `accent`, `match_fg`). Everything below references those roles. The theme plan also rewrites `view.rs` heavily, so landing it first avoids a double rebase.
2. **Land THIS plan SECOND (immediately after theme).** Reason: screen-states (empty-state branch) and scroll-affordances (list scrollbar gutter) build on the list area. If they land first, this Table migration must re-integrate their empty-state branch and scrollbar `Layout` split. It is strictly easier for those two plans to wrap an already-migrated `Table` render (they add an early-return empty branch and a scrollbar column beside the table) than for this plan to retrofit a Table under their additions. So: **theme → table → screen-states → scroll-affordances.**

**Theme role usage in this plan.** If the theme-system plan has landed, use `let theme = app.theme();` and:
- header row style: `Style::default().fg(theme.muted)`
- selection / `row_highlight_style`: `Style::default().fg(theme.selection_fg).bg(theme.selection_bg).add_modifier(Modifier::BOLD)`
- resolved PR cell: `theme.accent`
- match highlight emphasis in TITLE: `Modifier::REVERSED` (matches preview), which needs no color role.

If theme-system has NOT landed yet (you are building against current `master`), use the existing constants instead, with the SAME visual result:
- `theme::DIM` in place of `theme.muted`
- `theme::SELECTED_FG` / `theme::SELECTED_BG` in place of `theme.selection_fg` / `theme.selection_bg`
- `theme::ACCENT` in place of `theme.accent`
- `theme::agent_color(s.agent)` for the AGENT badge color.

**This plan is written against current `master` (the constants form).** Where a line uses `theme::DIM`/`theme::SELECTED_*`/`theme::ACCENT`/`theme::agent_color`, that is intentional and compiles today. If theme-system has already merged when you execute this, mechanically swap each to the `theme.<role>` equivalent above and pass `app.theme()` instead — the structure is identical.

---

## Required Reading (do this before Task 1)

Read these exact regions so every referenced symbol is grounded. Verify the line numbers — they may have shifted.

- `src/columns.rs` — FULL. Note the public API you will reuse:
  - `pub enum Align { Left, Right }` (lines 4–8)
  - `pub struct Column { pub id: &'static str, pub header: &'static str, pub align: Align, pub priority: u8, pub min_width: u16, pub flex: bool }` (lines 11–22)
  - `pub fn default_columns() -> Vec<Column>` (lines 25–84)
  - `pub fn solve_layout(columns: &[Column], total_width: u16) -> Vec<(usize, u16)>` (lines 121–123)
  - `pub fn solve_layout_with_desired(columns: &[Column], total_width: u16, desired_widths: &[u16]) -> Vec<(usize, u16)>` (lines 128–191) — returns `Vec<(column_index, resolved_width)>` for the KEPT columns only, in column order.
  - `pub fn fit(s: &str, width: u16, align: Align) -> String` (194) and `pub fn display_width(s: &str) -> usize` (216). The Table migration STOPS using `fit` for row cells (the Table truncates/pads), but the column solver still uses `display_width` internally — I-010 is preserved through the solver.
  - The unit tests at lines 322–418 (`title_always_survives_when_very_narrow`, `volatile_columns_drop_before_repo_and_branch_when_narrow`, `flex_column_absorbs_extra_width`, `desired_non_flex_widths_grow_before_title_takes_leftover`, `fit_*`, `configured_columns_orders_and_disables`) MUST keep passing unchanged — you are NOT modifying `columns.rs` logic, only consuming it differently.
- `src/tui/results_list.rs` — FULL. You will keep `cell` (50–74), `enrichment_cell` (76–96), `layout_for` (98–101), `layout_for_rows` (104–114), `desired_widths` (116–139). You will ADD a new `pub fn rows(...)` and `pub fn header_row(...)` and `pub fn widths(...)` (Task 1–3). You will likely DELETE `row_line` (15–33) and `header_line` (35–48) once nothing calls them (Task 4), plus update their tests (164–199). Note the `"⟳"` pending glyph (line 93) and the `"—"` em-dash fallbacks (87, 92) — keep these.
- `src/tui/view.rs` — FULL. Critical regions:
  - imports (1–14): `List, ListItem, ListState, HighlightSpacing` will be removed; `Table, Row, Cell, TableState` added.
  - `const SELECTION_MARKER` / `SELECTION_MARKER_WIDTH` (48–49) — `SELECTION_MARKER_WIDTH` is DELETED; `SELECTION_MARKER` stays as the highlight symbol value `"❯ "`.
  - the list build (98–148): `list_inner_w` (100), `split_list_area` (101), `visible_result_range`/`visible_results` (102–107), `layout_for_rows` (108–115), `header_line` + manual offset insert (116–120), the `items` Vec + `ListState` + `List` render (122–148).
  - `split_list_area` (201–210) — KEEP (still splits header row from rows area).
  - `visible_result_range` (433–444) — KEEP unchanged.
  - test `renders_columns_and_preview` (462–518) — MUST still pass (assertions on AGENT/REPO/CLAUDE/fix auth/feat/auth). The `/work/api` assertion at line 517 is satisfied by the PREVIEW header (`preview_header_lines` renders `s.directory`), NOT the table row (directory is not a column) — verify this still holds after migration; the preview is unchanged so it will.
  - test `selected_result_has_marker_and_focus_style` (590–636) — asserts the marker glyph at `buf[(0,2)]` and `SELECTED_*` colors. The Table's `highlight_symbol` renders the marker in the same leftmost cells, but exact cell coordinates may shift; UPDATE this test in Task 4 to assert the marker symbol + selection bg/fg are present on the selected row (scan for them rather than hardcoding `(0,2)` if needed).
- `src/tui/preview.rs` — `pub`-ness and signature of the highlight helper. NOTE: `highlight_terms` is currently `fn highlight_terms(line: &Line<'static>, terms: &[String]) -> Line<'static>` (line 265) and is **private** (`fn`, not `pub fn`). You must make it `pub fn highlight_terms(...)` in Task 5 so `results_list` can reuse it. `crate::query::parse(query).free_terms()` is the canonical term source (see `render_transcript` at 161–164).
- `src/tui/mod.rs` — `App::query()` (76), `App::selected()` (89), `App::results()` (86). `App` currently has NO `theme()` accessor on master (that arrives with the theme plan).
- `src/query.rs` — `pub fn parse(input: &str) -> ParsedQuery` (150) and `ParsedQuery::free_terms(&self) -> Vec<String>` (98).
- `src/tui/theme.rs` — `ACCENT` (Cyan), `DIM` (DarkGray), `SELECTED_BG`, `SELECTED_FG`, `agent_color(agent)`.

Run to confirm callers of the symbols you remove:

```sh
rg -n "row_line|header_line" src/
rg -n "highlight_terms" src/
```

`row_line`/`header_line` should only be referenced in `view.rs` and `results_list.rs` tests. If anything else references them, update those call sites too in Task 4.

---

## Ratatui 0.30 Table API (reference)

```rust
use ratatui::widgets::{Table, Row, Cell, TableState};
use ratatui::layout::Constraint;

// Cells accept Text/Line/Span, so per-span styling (match highlight) works:
let row = Row::new(vec![
    Cell::from(Span::styled("CLAUDE", Style::default().fg(color))),
    Cell::from(title_line), // title_line: Line<'static> with per-span REVERSED highlight
]).height(1);

let widths: Vec<Constraint> = vec![Constraint::Length(6), Constraint::Min(10)];

let table = Table::new(rows, widths)
    .header(Row::new(header_cells).style(Style::default().fg(theme::DIM)))
    .column_spacing(1)
    .row_highlight_style(selection_style)
    .highlight_symbol("❯ ");

let mut state = TableState::default();
state.select(Some(selected_idx)); // index into the rows passed to the table
frame.render_stateful_widget(table, area, &mut state);
```

`highlight_symbol("❯ ")` occupies a left column automatically — NO manual marker column / header offset. `TableState::select` has the same `select(Some(i))` API as `ListState`.

---

## Task 1 — Build the `Vec<Constraint>` widths from the column model

Add a pure helper that converts the kept-column layout (`Vec<(usize, u16)>` from the solver) into the Table's `Vec<Constraint>`: `Constraint::Length(w)` for non-flex columns, `Constraint::Min(min_width)` for the single flex (TITLE) column so the Table absorbs leftover space there.

**Files:**
- `src/tui/results_list.rs`

Steps:

- [ ] Add the import at the top of `src/tui/results_list.rs` (it currently imports from `ratatui::style` and `ratatui::text`):
  ```rust
  use ratatui::layout::Constraint;
  use ratatui::widgets::{Cell, Row};
  ```
  (Add `Cell, Row` now even though they are used in Tasks 2–3; they are harmless unused-import-wise only until then — if clippy/`-D warnings` complains mid-task, add them in the task that first uses them instead. Prefer adding `Constraint` here and `Cell, Row` in Task 2.)

- [ ] Write a FAILING unit test. Append to the `#[cfg(test)] mod tests` block in `results_list.rs`:
  ```rust
  #[test]
  fn widths_are_length_for_fixed_and_min_for_flex() {
      use ratatui::layout::Constraint;
      let cols = default_columns();
      let layout = layout_for(&cols, 120);
      let ws = widths(&layout, &cols);
      assert_eq!(ws.len(), layout.len());
      // the flex TITLE column must be a Min constraint; all others Length.
      let title_pos = layout.iter().position(|&(i, _)| cols[i].id == "title").unwrap();
      assert!(matches!(ws[title_pos], Constraint::Min(_)));
      for (n, &(ci, w)) in layout.iter().enumerate() {
          if cols[ci].flex {
              continue;
          }
          assert_eq!(ws[n], Constraint::Length(w));
      }
  }
  ```

- [ ] Run (expected FAIL — `widths` does not exist):
  ```sh
  cargo test --lib tui::results_list::tests::widths_are_length_for_fixed_and_min_for_flex -- --nocapture
  ```

- [ ] Minimal impl. Add to `results_list.rs` (non-test code):
  ```rust
  /// Map the solved layout (kept columns + resolved widths) into Table column
  /// constraints. Fixed columns get a Length equal to the solver's width; the
  /// single flex column (TITLE) gets a Min so the Table absorbs leftover space.
  pub fn widths(layout: &[(usize, u16)], columns: &[Column]) -> Vec<Constraint> {
      layout
          .iter()
          .map(|&(ci, w)| {
              if columns[ci].flex {
                  Constraint::Min(columns[ci].min_width)
              } else {
                  Constraint::Length(w)
              }
          })
          .collect()
  }
  ```

- [ ] Run (expected PASS):
  ```sh
  cargo test --lib tui::results_list::tests::widths_are_length_for_fixed_and_min_for_flex -- --nocapture
  ```

- [ ] Commit:
  ```sh
  git add -A && git commit -m "$(cat <<'EOF'
  feat(tui): derive Table column constraints from layout solver

  Co-Authored-By: Claude <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 2 — Build a `Row` from a `SessionSummary`

Add a helper that builds one Table `Row` for a session, one `Cell` per kept column, reusing the existing `cell(...)` for text+style. TITLE takes a `terms: &[String]` argument so Task 5 can add match highlight; for now TITLE is a plain styled cell. Per-cell color (agent badge, ACCENT resolved PR) is preserved because `cell()` already returns the right `Style`.

**Files:**
- `src/tui/results_list.rs`

Steps:

- [ ] Ensure `use ratatui::widgets::{Cell, Row};` is present at the top (add if not already added in Task 1).

- [ ] Write a FAILING test. Append to `mod tests`:
  ```rust
  #[test]
  fn session_row_has_one_cell_per_kept_column_with_values() {
      let cols = default_columns();
      let enr: Vec<Box<dyn Enricher>> = vec![Box::new(BranchEnricher), Box::new(RepoEnricher)];
      let resolved = HashMap::new();
      let row_data = sess();
      let layout = layout_for_rows(&cols, 120, std::slice::from_ref(&row_data), &enr, &resolved, 3600);
      let row = session_row(&row_data, &layout, &cols, &enr, &resolved, 3600, &[]);
      // A Row exposes its cells via iteration is not public in 0.30; instead assert
      // by reconstructing through the same cell() path the row uses. We check the
      // helper produced a row by rendering it in Task 4. Here, assert cell() values
      // that the row is built from are correct:
      let (agent_text, _) = cell(&row_data, cols.iter().find(|c| c.id == "agent").unwrap(), &enr, &resolved, 3600);
      assert_eq!(agent_text, "CLAUDE");
      // and the row was constructed without panicking for every kept column:
      let _ = row; // constructed successfully
  }
  ```
  > NOTE: Ratatui 0.30 `Row` does not expose its cells for assertion. This test only guards that `session_row` constructs (no panic) and that the underlying `cell()` data is right. The real end-to-end assertion (rendered buffer contains the values) lives in Task 4's render test, which is the authoritative W1 check.

- [ ] Run (expected FAIL — `session_row` does not exist):
  ```sh
  cargo test --lib tui::results_list::tests::session_row_has_one_cell_per_kept_column_with_values -- --nocapture
  ```

- [ ] Minimal impl. Add to `results_list.rs`:
  ```rust
  /// Build one Table row for a session across the kept (visible) columns.
  /// `terms` are the query's free terms used to highlight matches in the TITLE
  /// cell (empty slice = no highlight). The Table itself pads/truncates each
  /// cell to its column width, so we do NOT call `fit` here.
  pub fn session_row(
      s: &SessionSummary,
      layout: &[(usize, u16)],
      columns: &[Column],
      enrichers: &[Box<dyn Enricher>],
      resolved: &HashMap<(String, &'static str), Option<String>>,
      now: i64,
      terms: &[String],
  ) -> Row<'static> {
      let cells: Vec<Cell<'static>> = layout
          .iter()
          .map(|&(ci, _)| {
              let col = &columns[ci];
              if col.id == "title" {
                  title_cell(&s.title, terms)
              } else {
                  let (text, style) = cell(s, col, enrichers, resolved, now);
                  Cell::from(Span::styled(text, style))
              }
          })
          .collect();
      Row::new(cells).height(1)
  }

  /// TITLE cell. In Task 5 this reuses preview::highlight_terms; for now it is a
  /// single plain span.
  fn title_cell(title: &str, _terms: &[String]) -> Cell<'static> {
      Cell::from(Span::raw(title.to_string()))
  }
  ```

- [ ] Run (expected PASS):
  ```sh
  cargo test --lib tui::results_list::tests::session_row_has_one_cell_per_kept_column_with_values -- --nocapture
  ```

- [ ] Commit:
  ```sh
  git add -A && git commit -m "$(cat <<'EOF'
  feat(tui): build Table rows from session summaries

  Co-Authored-By: Claude <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 3 — Build the header `Row`

Add a helper producing the muted header `Row` for the kept columns. Replaces `header_line` (which produced a `Line` for a `Paragraph`).

**Files:**
- `src/tui/results_list.rs`

Steps:

- [ ] Write a FAILING test. Append to `mod tests`:
  ```rust
  #[test]
  fn header_row_constructs_for_visible_columns() {
      let cols = default_columns();
      let layout = layout_for(&cols, 120);
      // header_row must build without panic and produce a Row.
      let _row = header_row(&layout, &cols);
      // assert the column count matches kept columns (indirect: layout len)
      assert_eq!(layout.len(), 7); // wide pane keeps all 7 default columns
  }
  ```

- [ ] Run (expected FAIL — `header_row` does not exist):
  ```sh
  cargo test --lib tui::results_list::tests::header_row_constructs_for_visible_columns -- --nocapture
  ```

- [ ] Minimal impl. Add to `results_list.rs`:
  ```rust
  /// Build the muted header row for the kept columns. Styled at the Row level so
  /// every header cell shares the muted color.
  pub fn header_row(layout: &[(usize, u16)], columns: &[Column]) -> Row<'static> {
      let cells: Vec<Cell<'static>> = layout
          .iter()
          .map(|&(ci, _)| Cell::from(columns[ci].header))
          .collect();
      Row::new(cells).style(Style::default().fg(theme::DIM))
  }
  ```
  (`Style` and `theme` are already imported in `results_list.rs`.)

- [ ] Run (expected PASS):
  ```sh
  cargo test --lib tui::results_list::tests::header_row_constructs_for_visible_columns -- --nocapture
  ```

- [ ] Commit:
  ```sh
  git add -A && git commit -m "$(cat <<'EOF'
  feat(tui): build muted Table header row from column model

  Co-Authored-By: Claude <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 4 — Swap the `List` render for a `Table` + `TableState`; delete the manual header Paragraph/offset

This is the core W1 fix. In `view::render`, replace the header `Paragraph` (with its `SELECTION_MARKER_WIDTH` offset insert) and the `List<ListItem>`/`ListState` render with a single `Table` whose `.header(...)` carries the column labels and whose `.highlight_symbol(SELECTION_MARKER)` reserves the marker column. Build `Row`s ONLY for `visible_results` (I-006). Set `TableState` selection to `app.selected().saturating_sub(visible.start)`.

**Files:**
- `src/tui/view.rs`

Steps:

- [ ] Update the imports (lines 8–10). Replace the `ratatui::widgets` import line so it no longer pulls `List, ListItem, ListState, HighlightSpacing` and instead pulls the Table types. The new line:
  ```rust
  use ratatui::widgets::{
      Block, Borders, Cell, Clear, Padding, Paragraph, Row, Table, TableState, Wrap,
  };
  ```
  (Note: `HighlightSpacing` was only used for the list; the Table reserves the highlight symbol column whenever a header/highlight symbol is set, so we drop it. `Cell` and `Row` are imported in case you build header cells inline, but you will mostly call the `results_list` helpers — keep `Cell, Row` only if used; remove to satisfy `-D warnings` if not. Prefer building header/rows entirely via `results_list::header_row` / `results_list::session_row`, in which case you can OMIT `Cell, Row` from this import and keep just `Table, TableState`.)

- [ ] Delete the now-unused `SELECTION_MARKER_WIDTH` constant (line 49) and any use of it. KEEP `const SELECTION_MARKER: &str = "❯ ";` (line 48).

- [ ] Update the failing render test FIRST so it reflects the Table path. Modify `renders_columns_and_preview` (lines 462–518) — it already asserts the right things; it should pass unchanged after migration since the Table renders the same header labels and row values. But ADD a guard that the manual header offset is gone is unnecessary; instead, ADD a narrow-terminal column-drop assertion and a match-highlight assertion in separate tests (Tasks 5–6). For THIS task, just keep `renders_columns_and_preview` as-is and make it pass against the Table.

- [ ] Replace the list-build block. Find lines 98–148 (from `// column grid` through `f.render_stateful_widget(list, list_rows_area, &mut state);`). Replace the entire region with:
  ```rust
  // results table
  let cols = model.columns;
  let (list_header_area, list_rows_area) = split_list_area(list_area);
  let visible = visible_result_range(
      app.results().len(),
      app.selected(),
      list_rows_area.height as usize,
  );
  let visible_results = app.results().get(visible.clone()).unwrap_or_default();

  // The Table reserves a left column for the highlight symbol, so the column
  // solver gets the area width minus that marker width.
  let marker_w = crate::columns::display_width(SELECTION_MARKER) as u16;
  let list_inner_w = list_area.width.saturating_sub(marker_w);
  let layout = results_list::layout_for_rows(
      cols,
      list_inner_w,
      visible_results,
      model.enrichers,
      model.resolved,
      model.now,
  );

  // Query terms drive TITLE match highlighting (W5).
  let terms = crate::query::parse(app.query()).free_terms();

  let header_row = results_list::header_row(&layout, cols);
  let rows: Vec<Row> = visible_results
      .iter()
      .map(|s| {
          results_list::session_row(
              s,
              &layout,
              cols,
              model.enrichers,
              model.resolved,
              model.now,
              &terms,
          )
      })
      .collect();
  let widths = results_list::widths(&layout, cols);

  let mut state = TableState::default();
  if !visible_results.is_empty() {
      state.select(Some(app.selected().saturating_sub(visible.start)));
  }

  let table = Table::new(rows, widths)
      .header(header_row)
      .column_spacing(1)
      .row_highlight_style(
          Style::default()
              .fg(theme::SELECTED_FG)
              .bg(theme::SELECTED_BG)
              .add_modifier(Modifier::BOLD),
      )
      .highlight_symbol(SELECTION_MARKER);
  f.render_stateful_widget(table, list_rows_area, &mut state);
  let _ = list_header_area; // header now lives inside the Table; see note below.
  ```

  **Header placement decision (IMPORTANT — resolve this, do not leave both):** The Table renders its own `.header(...)` as the first row of `list_rows_area`. The old code reserved a *separate* one-line `list_header_area` via `split_list_area` for a header `Paragraph`. With the Table owning the header, you have two clean options:
  - **(A) Let the Table own the header (recommended).** Render the Table into the FULL `list_area` (not `list_rows_area`), and do NOT split off a header area. Delete the `split_list_area` call here and pass `list_area` to `render_stateful_widget`. The Table draws the header on its first line and rows below. This is simplest and removes `split_list_area` from the render path (keep the fn if other plans use it; otherwise it becomes dead — see cleanup below).
  - **(B) Keep the split** and render the Table into `list_rows_area` with NO `.header(...)`, drawing the header separately. This re-introduces the two-path problem W1 is eliminating — DO NOT do this.

  **Adopt option (A).** So the final replacement uses `list_area` directly:
  ```rust
  // results table (option A: Table owns its header; no separate header pane)
  let cols = model.columns;
  let marker_w = crate::columns::display_width(SELECTION_MARKER) as u16;
  let list_inner_w = list_area.width.saturating_sub(marker_w);
  let visible = visible_result_range(
      app.results().len(),
      app.selected(),
      list_area.height.saturating_sub(1) as usize, // minus 1 for the header row
  );
  let visible_results = app.results().get(visible.clone()).unwrap_or_default();
  let layout = results_list::layout_for_rows(
      cols, list_inner_w, visible_results, model.enrichers, model.resolved, model.now,
  );
  let terms = crate::query::parse(app.query()).free_terms();
  let rows: Vec<Row> = visible_results
      .iter()
      .map(|s| results_list::session_row(s, &layout, cols, model.enrichers, model.resolved, model.now, &terms))
      .collect();
  let mut state = TableState::default();
  if !visible_results.is_empty() {
      state.select(Some(app.selected().saturating_sub(visible.start)));
  }
  let table = Table::new(rows, results_list::widths(&layout, cols))
      .header(results_list::header_row(&layout, cols))
      .column_spacing(1)
      .row_highlight_style(
          Style::default()
              .fg(theme::SELECTED_FG)
              .bg(theme::SELECTED_BG)
              .add_modifier(Modifier::BOLD),
      )
      .highlight_symbol(SELECTION_MARKER);
  f.render_stateful_widget(table, list_area, &mut state);
  ```
  > Viewport-bounding (I-006) note: `visible_result_range` is given `list_area.height - 1` (the header row consumes one line of `list_area`). This keeps the visible row count correct so we still build `Row`s only for what fits.

- [ ] Remove the now-dead `split_list_area` call from `render` (option A no longer splits). Check whether `split_list_area` (201–210) is still referenced anywhere:
  ```sh
  rg -n "split_list_area" src/
  ```
  If it has no remaining callers, delete the `split_list_area` fn (201–210) too, to satisfy `-D warnings`.

- [ ] Update `selected_result_has_marker_and_focus_style` (590–636). The Table renders the header on row 0 of `list_area`, so the FIRST data row (the only session) is now on a different y than before, and the marker x may differ. Rewrite the assertions to SCAN the buffer for the marker glyph and selection style rather than hardcoding `(0,2)`:
  ```rust
  let buf = term.backend().buffer();
  // the selection marker glyph appears somewhere in the buffer
  let marker = SELECTION_MARKER.trim();
  let has_marker = buf.content().iter().any(|c| c.symbol() == marker);
  assert!(has_marker, "selection marker should be rendered");
  // some cell carries the selection background (the highlighted row)
  let has_sel_bg = buf.content().iter().any(|c| c.bg == theme::SELECTED_BG);
  assert!(has_sel_bg, "selected row should carry the selection background");
  // and the selected foreground is used somewhere
  let has_sel_fg = buf.content().iter().any(|c| c.fg == theme::SELECTED_FG && c.bg == theme::SELECTED_BG);
  assert!(has_sel_fg, "selected row text should use the selection fg over selection bg");
  ```

- [ ] DELETE the now-unused `row_line` (15–33) and `header_line` (35–48) from `results_list.rs`, plus their tests `row_renders_repo_branch_title` (164–184), `header_renders_visible_column_labels` (187–199), `pending_pr_shows_glyph` (224–232), `pr_cell_reads_resolved_with_full_enricher_list` (235–250) IF they call `row_line`/`header_line`. Replace those deleted tests with equivalents that exercise `session_row` / the rendered buffer where the assertion still matters:
  - `pending_pr_shows_glyph` and `pr_cell_reads_resolved_*` assert on the `"⟳"` glyph and `"#42"` PR text. Those values now flow through `cell()` into `session_row`. Re-assert them via `cell()` directly (it is still in scope in the test module):
    ```rust
    #[test]
    fn pending_pr_cell_shows_glyph() {
        let cols = default_columns();
        let enr: Vec<Box<dyn Enricher>> = vec![Box::new(crate::enrich::gh_pr::GhPrEnricher)];
        let resolved = HashMap::new();
        let pr_col = cols.iter().find(|c| c.id == "pr").unwrap();
        let (text, _) = cell(&sess(), pr_col, &enr, &resolved, 0);
        assert_eq!(text, "⟳");
    }

    #[test]
    fn resolved_pr_cell_reads_resolved() {
        let cols = default_columns();
        let enr: Vec<Box<dyn Enricher>> = vec![
            Box::new(RepoEnricher),
            Box::new(BranchEnricher),
            Box::new(crate::enrich::gh_pr::GhPrEnricher),
        ];
        let mut resolved = HashMap::new();
        resolved.insert(("claude:a".to_string(), "pr"), Some("#42".to_string()));
        let pr_col = cols.iter().find(|c| c.id == "pr").unwrap();
        let (text, style) = cell(&sess(), pr_col, &enr, &resolved, 0);
        assert_eq!(text, "#42");
        assert_eq!(style.fg, Some(theme::ACCENT));
    }
    ```
  - Keep `visible_row_content_sizes_repo_and_branch_before_title_flexes` (201–221) — it only uses `layout_for_rows`, which is unchanged.

  > If you prefer fewer deletions, you MAY keep `row_line`/`header_line` and their tests temporarily, but they will be dead code and trip `-D warnings`. Cleaner to delete now.

- [ ] Run the affected tests (expected PASS):
  ```sh
  cargo test --lib tui:: -- --nocapture
  ```
  Fix any compile error (most likely a leftover reference to `row_line`, `header_line`, `SELECTION_MARKER_WIDTH`, `HighlightSpacing`, `List`, `ListItem`, or `ListState`).

- [ ] Commit:
  ```sh
  git add -A && git commit -m "$(cat <<'EOF'
  refactor(tui): render results with Table widget

  Replaces the hand-formatted List + separate header Paragraph (offset by the
  selection-marker width) with a single Ratatui Table. The Table owns per-column
  width/alignment/truncation within the column set chosen by the priority-based
  solver, while column dropping and content-aware widths still come from
  columns::solve_layout_with_desired. Viewport-bounding (I-006) is preserved:
  rows are built only for the visible slice and TableState selection is offset by
  the visible-range start.

  Co-Authored-By: Claude <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 5 — Match-highlight the TITLE cell (W5)

Make `preview::highlight_terms` public and reuse it in `title_cell` so query-term matches inside the TITLE are emphasized with `Modifier::REVERSED`, exactly like the preview pane.

**Files:**
- `src/tui/preview.rs`
- `src/tui/results_list.rs`

Steps:

- [ ] In `src/tui/preview.rs`, change the visibility of `highlight_terms` (line 265) from `fn highlight_terms` to `pub fn highlight_terms`. Add a one-line doc note that it is reused by the results table. Do not change its body.

- [ ] Write a FAILING test in `results_list.rs` `mod tests`:
  ```rust
  #[test]
  fn title_cell_highlights_query_terms() {
      use ratatui::style::Modifier;
      let terms = vec!["auth".to_string()];
      let cell = title_cell("fix auth bug", &terms);
      // A Cell wraps a Text; in 0.30 we can inspect via Text::from(cell)? Not public.
      // Instead test the underlying line builder directly:
      let line = title_line("fix auth bug", &terms);
      let highlighted = line.spans.iter().any(|s| {
          s.content.contains("auth") && s.style.add_modifier.contains(Modifier::REVERSED)
      });
      assert!(highlighted, "matched term in title must be reverse-highlighted");
      let _ = cell; // also constructs
  }
  ```
  > This requires a small refactor: extract a `title_line(title, terms) -> Line<'static>` that `title_cell` wraps, so the test can inspect spans (a `Cell` does not expose its content in 0.30).

- [ ] Run (expected FAIL — `title_line` does not exist):
  ```sh
  cargo test --lib tui::results_list::tests::title_cell_highlights_query_terms -- --nocapture
  ```

- [ ] Minimal impl. In `results_list.rs`, replace the placeholder `title_cell` from Task 2 with:
  ```rust
  /// Build the TITLE line, reverse-highlighting any query-term matches by
  /// reusing the preview's multi-byte-safe highlighter.
  fn title_line(title: &str, terms: &[String]) -> Line<'static> {
      let base = Line::from(Span::raw(title.to_string()));
      if terms.is_empty() {
          base
      } else {
          crate::tui::preview::highlight_terms(&base, terms)
      }
  }

  fn title_cell(title: &str, terms: &[String]) -> Cell<'static> {
      Cell::from(title_line(title, terms))
  }
  ```
  `Line` and `Span` are already imported in `results_list.rs` (line 9: `use ratatui::text::{Line, Span};`).

- [ ] Run (expected PASS):
  ```sh
  cargo test --lib tui::results_list::tests::title_cell_highlights_query_terms -- --nocapture
  ```

- [ ] Run the preview tests too (you changed visibility — `highlight_terms` callers inside preview are unaffected, but verify):
  ```sh
  cargo test --lib tui::preview:: -- --nocapture
  ```

- [ ] Commit:
  ```sh
  git add -A && git commit -m "$(cat <<'EOF'
  feat(tui): highlight query-term matches in results TITLE cell

  Reuses preview::highlight_terms (multi-byte safe) so full-text matches are
  visible in the results table, not only the preview pane.

  Co-Authored-By: Claude <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 6 — End-to-end render tests: column-dropping + viewport-bounding + match highlight in the rendered buffer

Add render-level tests in `view.rs` that assert (a) on a narrow terminal a low-priority column drops while TITLE survives, and (b) a query term is reverse-highlighted in the rendered TITLE cell. These are the authoritative W1/W5/I-006 guards through the actual `render` path.

**Files:**
- `src/tui/view.rs`

Steps:

- [ ] Write a FAILING test for column dropping at narrow width. Append to `view.rs` `mod tests`:
  ```rust
  #[test]
  fn narrow_terminal_drops_low_priority_columns_but_keeps_title() {
      use crate::enrich::Enricher;
      use std::collections::HashMap;

      let mut app = App::new();
      app.set_preview(false, 50); // no preview, so the list gets the full width
      app.set_results(vec![SessionSummary {
          id: "a".into(),
          agent: AgentId::Claude,
          title: "fix auth".into(),
          directory: "/work/api".into(),
          timestamp: 0,
          message_count: 3,
          yolo: false,
          branch: Some("feat/auth".into()),
          repo_url: None,
          source_path: None,
      }]);
      let enr: Vec<Box<dyn Enricher>> = vec![];
      let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
      let cols = crate::columns::default_columns();
      // 30 cols wide forces the priority solver to drop PR/MSGS/TIME.
      let backend = TestBackend::new(30, 8);
      let mut term = Terminal::new(backend).unwrap();
      term.draw(|f| {
          render(f, &app, RenderModel {
              now: 100, columns: &cols, enrichers: &enr, resolved: &resolved,
              preview_lines: &[], status: &StatusLine::default(), modal_command: None,
          })
      }).unwrap();
      let text: String = term.backend().buffer().content().iter().map(|c| c.symbol()).collect();
      assert!(text.contains("TITLE"), "TITLE header must survive narrow width");
      assert!(text.contains("fix auth"), "title value must survive");
      assert!(!text.contains("PR"), "lowest-priority PR column should be dropped");
  }
  ```
  > `"PR"` is a substring risk only if some other rendered text contains `PR`; with this fixture nothing else does. If it proves flaky, assert on the header label exact-match by scanning header row cells instead.

- [ ] Run (expected: it should PASS already if the Table + solver wiring is correct — this is a guard/regression test, not driving new code). If it FAILS because a column you expect dropped is still present, recompute the width: the solver receives `list_inner_w = width - marker_w`. Adjust the backend width down (e.g. 24) until PR/MSGS/TIME drop while TITLE/AGENT survive, matching `columns.rs`'s own `solve_layout` tests (which drop PR+MSGS at width 38). Use a width that the `columns.rs` unit tests already prove drops the low-priority columns.

- [ ] Write a FAILING test for match highlight through the render path. Append:
  ```rust
  #[test]
  fn query_match_is_highlighted_in_rendered_title() {
      use crate::enrich::Enricher;
      use ratatui::style::Modifier;
      use std::collections::HashMap;

      let mut app = App::new();
      app.set_preview(false, 50);
      app.set_query("auth".to_string());
      app.set_results(vec![SessionSummary {
          id: "a".into(),
          agent: AgentId::Claude,
          title: "fix auth bug".into(),
          directory: "/work/api".into(),
          timestamp: 0,
          message_count: 3,
          yolo: false,
          branch: None,
          repo_url: None,
          source_path: None,
      }]);
      let enr: Vec<Box<dyn Enricher>> = vec![];
      let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
      let cols = crate::columns::default_columns();
      let backend = TestBackend::new(100, 8);
      let mut term = Terminal::new(backend).unwrap();
      term.draw(|f| {
          render(f, &app, RenderModel {
              now: 100, columns: &cols, enrichers: &enr, resolved: &resolved,
              preview_lines: &[], status: &StatusLine::default(), modal_command: None,
          })
      }).unwrap();
      let buf = term.backend().buffer();
      // the letters of "auth" in the title should carry REVERSED.
      let any_reversed = buf.content().iter().any(|c| {
          c.modifier.contains(Modifier::REVERSED)
      });
      assert!(any_reversed, "matched query term in title should render reversed");
  }
  ```
  > `Cell::modifier` is `ratatui::style::Modifier` on a buffer cell in 0.30. If the field/access differs, inspect a cell with `buf[(x, y)]` and read its `modifier` field; the assertion is "some rendered cell has REVERSED".

- [ ] Run both new tests (expected PASS):
  ```sh
  cargo test --lib tui::view:: -- --nocapture
  ```

- [ ] Commit:
  ```sh
  git add -A && git commit -m "$(cat <<'EOF'
  test(tui): guard column-dropping, viewport-bounding, and title match highlight

  Co-Authored-By: Claude <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 7 — Verify `set_query` exists; add if missing

Task 6 calls `app.set_query("auth".to_string())`. Confirm `App::set_query` exists (it does — `src/tui/mod.rs` line 82). No change needed unless the test fails to compile.

**Files:**
- (verification only)

Steps:

- [ ] Confirm:
  ```sh
  rg -n "pub fn set_query" src/tui/mod.rs
  ```
  Expected: one hit at ~line 82. If absent, the test in Task 6 will not compile — in that case add a `set_query` accessor; but per current source it exists, so this is a no-op check.

---

## Task 8 — Final verification

**Files:**
- (none — verification only)

Steps:

- [ ] Run the full library test suite:
  ```sh
  cargo test --lib
  ```
  Expected: all pass, including the unchanged `columns.rs` solver tests (`title_always_survives_when_very_narrow`, `volatile_columns_drop_before_repo_and_branch_when_narrow`, `flex_column_absorbs_extra_width`, `desired_non_flex_widths_grow_before_title_takes_leftover`) and the migrated `results_list` / `view` tests.

- [ ] Run the full suite (this change is adjacent to columns/index code paths through the TUI):
  ```sh
  cargo test
  ```
  Expected: all pass.

- [ ] Run clippy with warnings as errors:
  ```sh
  cargo clippy --all-targets -- -D warnings
  ```
  Expected: clean. Common issues to fix: leftover unused imports (`List`, `ListItem`, `ListState`, `HighlightSpacing`, `Cell`/`Row` if you built header/rows entirely via helpers), an unused `SELECTION_MARKER_WIDTH`, or a dead `split_list_area` / `row_line` / `header_line`.

- [ ] If everything passes, the W1 + W5 findings are resolved and I-006/I-010 are preserved. Final review of the diff:
  ```sh
  git diff master --stat
  ```

---

## Done criteria

- The results list renders via a single `ratatui::widgets::Table` (no separate header `Paragraph`, no manual `SELECTION_MARKER_WIDTH` offset).
- Column SELECTION (priority dropping + content-aware widths) still comes from `columns::solve_layout_with_desired`; the Table owns width/alignment/truncation within the chosen set via `Vec<Constraint>` (`Length` for fixed, `Min` for flex TITLE) and `.column_spacing(1)`.
- Only visible rows build `Row`s (I-006); `TableState` selection is offset by `visible.start`.
- Display-width fitting (I-010) is preserved through the solver.
- Query-term matches are reverse-highlighted in the TITLE cell (W5) by reusing `preview::highlight_terms`.
- `cargo test`, `cargo test --lib`, and `cargo clippy --all-targets -- -D warnings` all pass.
