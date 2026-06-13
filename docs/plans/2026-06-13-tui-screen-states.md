# TUI Screen States Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** Give the hop TUI three missing screen states: (1) an **empty state** that explains what to do when the result list is blank (branching on whether the query is empty), (2) a **loading/indexing state** with an animated braille throbber so a cold start or large corpus shows that work is happening, and (3) fix the **pending enricher glyph** that currently renders a static `⟳` that reads as a spinner but never moves.

**Architecture:** All rendering changes are confined to `src/tui/view.rs` (the body/list area and the search line) and `src/tui/results_list.rs` (the pending-enricher cell). New state lives as fields on `App` in `src/tui/mod.rs` with plain accessors/setters. The spinner is hand-rolled (no new crate): a `frame: u64` counter on `App` indexes a fixed braille frame table; `src/main.rs` advances it once per redraw (the run loop already polls every 50ms, so it redraws continuously) and sets the `indexing` count from sync status. No timer subsystem is introduced.

**Tech Stack:** Rust, Ratatui 0.30.

---

## Dependencies & Sequencing

- **Muted color role.** This plan uses the muted text color. If the theme-system plan (`docs/plans/2026-06-13-tui-theme-system.md`) has landed and `App` exposes `app.theme().muted`, use that. As of writing it has **not** landed, so this plan uses the current constant **`theme::DIM`** (defined in `src/tui/theme.rs` as `Color::DarkGray`). All code below uses `theme::DIM`; if the theme plan lands first, swap `theme::DIM` → `app.theme().muted` at each call site noted with `// MUTED`.
- **List/body area conflict.** This plan touches the list/body branch in `src/tui/view.rs` (`render()`, roughly lines 86–148). The results-table plan (`docs/plans/2026-06-13-tui-results-table.md`) rewrites that same region. **Coordinate:** land *this* plan first and have the table plan preserve the empty-state branch added here (Task 1); or land the table plan first and re-apply Task 1's branch on top. The empty-state check is a single early `if visible_results.is_empty()` guard, so re-applying it is cheap.
- **App state in `mod.rs`.** Adds three fields (`frame: u64`, `indexing: Option<usize>`) plus accessors. Low conflict — additive only.
- **`src/main.rs` wiring** (Task 5) reads `App` accessors added in Tasks 2–3; do Tasks 2–3 before Task 5.

---

## Task 1: Empty-results state in the body area

When `app.results()` is empty, the list area renders a blank box (only `0/0` shows in the search line). Render a centered muted message instead, branching on whether the query is empty.

**Files:**
- `src/tui/view.rs` (modify `render`, add helper `empty_state_message`, add test)

Steps:

- [ ] Add a failing test in the `#[cfg(test)] mod tests` block of `src/tui/view.rs` for the empty + empty-query case. Append this test:
  ```rust
  #[test]
  fn empty_results_empty_query_shows_prompt() {
      use crate::enrich::Enricher;
      use std::collections::HashMap;

      let app = App::new(); // empty results, empty query
      let enr: Vec<Box<dyn Enricher>> = vec![];
      let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
      let cols = crate::columns::default_columns();
      let backend = TestBackend::new(100, 12);
      let mut term = Terminal::new(backend).unwrap();
      term.draw(|f| {
          render(
              f,
              &app,
              RenderModel {
                  now: 100,
                  columns: &cols,
                  enrichers: &enr,
                  resolved: &resolved,
                  preview_lines: &[],
                  status: &StatusLine::default(),
                  modal_command: None,
              },
          )
      })
      .unwrap();
      let buf = term.backend().buffer().clone();
      let text: String = buf.content().iter().map(|c| c.symbol()).collect();
      assert!(text.contains("Type to search"));
  }
  ```
- [ ] Run `cargo test --lib tui::view::tests::empty_results_empty_query_shows_prompt -- --nocapture` — expect FAIL (no empty-state text rendered; the list area is blank).
- [ ] Add the empty-state helper above the `#[cfg(test)]` block in `src/tui/view.rs` (e.g. just after `footer_line`):
  ```rust
  /// Message shown in the body area when there are no results.
  fn empty_state_message(query_is_empty: bool) -> &'static str {
      if query_is_empty {
          "Type to search your Claude Code / Codex sessions."
      } else {
          "No sessions match. Press Esc to clear the query."
      }
  }
  ```
- [ ] In `render`, gate the list build/render on non-empty results. Replace the block that builds `items`, `state`, and the `List` and calls `f.render_stateful_widget(list, list_rows_area, &mut state);` (currently lines ~122–148) with:
  ```rust
  if visible_results.is_empty() {
      let msg = empty_state_message(app.query().is_empty());
      let para = Paragraph::new(msg)
          .style(Style::default().fg(theme::DIM)) // MUTED
          .alignment(Alignment::Center);
      // Vertically center the single line within the rows area.
      let y = list_rows_area
          .y
          .saturating_add(list_rows_area.height / 2);
      let centered = Rect {
          x: list_rows_area.x,
          y,
          width: list_rows_area.width,
          height: 1.min(list_rows_area.height),
      };
      f.render_widget(para, centered);
  } else {
      let items: Vec<ListItem> = visible_results
          .iter()
          .map(|s| {
              ListItem::new(results_list::row_line(
                  s,
                  &layout,
                  cols,
                  model.enrichers,
                  model.resolved,
                  model.now,
              ))
          })
          .collect();
      let mut state = ListState::default();
      if !items.is_empty() {
          state.select(Some(app.selected().saturating_sub(visible.start)));
      }
      let list = List::new(items)
          .highlight_symbol(SELECTION_MARKER)
          .highlight_spacing(HighlightSpacing::Always)
          .highlight_style(
              Style::default()
                  .fg(theme::SELECTED_FG)
                  .bg(theme::SELECTED_BG)
                  .add_modifier(Modifier::BOLD),
          );
      f.render_stateful_widget(list, list_rows_area, &mut state);
  }
  ```
  Note: `Alignment` and `Rect` are already imported at the top of `view.rs` (`ratatui::layout::{Alignment, ..., Rect}`), so no new `use` is needed.
- [ ] Run `cargo test --lib tui::view::tests::empty_results_empty_query_shows_prompt -- --nocapture` — expect PASS.
- [ ] Add a second failing test for the non-empty-query case. Append:
  ```rust
  #[test]
  fn empty_results_with_query_shows_no_match() {
      use crate::enrich::Enricher;
      use std::collections::HashMap;

      let mut app = App::new();
      app.set_query("nope".to_string()); // results stay empty
      let enr: Vec<Box<dyn Enricher>> = vec![];
      let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
      let cols = crate::columns::default_columns();
      let backend = TestBackend::new(100, 12);
      let mut term = Terminal::new(backend).unwrap();
      term.draw(|f| {
          render(
              f,
              &app,
              RenderModel {
                  now: 100,
                  columns: &cols,
                  enrichers: &enr,
                  resolved: &resolved,
                  preview_lines: &[],
                  status: &StatusLine::default(),
                  modal_command: None,
              },
          )
      })
      .unwrap();
      let buf = term.backend().buffer().clone();
      let text: String = buf.content().iter().map(|c| c.symbol()).collect();
      assert!(text.contains("No sessions match"));
      assert!(!text.contains("Type to search"));
  }
  ```
- [ ] Run `cargo test --lib tui::view::tests::empty_results_with_query_shows_no_match -- --nocapture` — expect PASS (the branch already handles it; this test locks the behavior). If it FAILS, fix `empty_state_message` until it passes.
- [ ] Add a test asserting rows still render when results are present (regression guard for the new `else` branch). Append:
  ```rust
  #[test]
  fn non_empty_results_render_rows_not_empty_message() {
      use crate::enrich::Enricher;
      use std::collections::HashMap;

      let mut app = App::new();
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
      let backend = TestBackend::new(100, 12);
      let mut term = Terminal::new(backend).unwrap();
      term.draw(|f| {
          render(
              f,
              &app,
              RenderModel {
                  now: 100,
                  columns: &cols,
                  enrichers: &enr,
                  resolved: &resolved,
                  preview_lines: &[],
                  status: &StatusLine::default(),
                  modal_command: None,
              },
          )
      })
      .unwrap();
      let text: String = term
          .backend()
          .buffer()
          .content()
          .iter()
          .map(|c| c.symbol())
          .collect();
      assert!(text.contains("fix auth"));
      assert!(!text.contains("Type to search"));
      assert!(!text.contains("No sessions match"));
  }
  ```
- [ ] Run `cargo test --lib tui::view::tests -- --nocapture` — expect all PASS.
- [ ] Commit:
  ```
  feat(tui): add empty-results screen state

  Render a centered muted message in the body area when there are no
  results, branching on whether the query is empty.

  Co-Authored-By: Claude <noreply@anthropic.com>
  ```

---

## Task 2: Add `frame` counter to `App` (spinner clock)

The run loop polls every 50ms and redraws each iteration, so a per-redraw `frame` counter drives a smooth throbber with no new timer. Add the field and accessors.

**Files:**
- `src/tui/mod.rs` (add field, accessors, test)

Steps:

- [ ] Add a failing test in the `#[cfg(test)] mod tests` block of `src/tui/mod.rs`. Append:
  ```rust
  #[test]
  fn frame_starts_at_zero_and_advances() {
      let mut app = App::new();
      assert_eq!(app.frame(), 0);
      app.tick();
      app.tick();
      assert_eq!(app.frame(), 2);
  }
  ```
- [ ] Run `cargo test --lib tui::tests::frame_starts_at_zero_and_advances -- --nocapture` — expect FAIL (no `frame`/`tick`).
- [ ] In `src/tui/mod.rs`, add the field to the `App` struct (after `preview_match_index: usize,`):
  ```rust
      frame: u64,
      indexing: Option<usize>,
  ```
  (`indexing` is added here too so the struct is touched once; it is used in Task 3.)
- [ ] In `App::new`, initialize the new fields (after `preview_match_index: 0,`):
  ```rust
          frame: 0,
          indexing: None,
  ```
- [ ] Add accessors. Place them near the other simple accessors (e.g. after the `selected` accessor):
  ```rust
      pub fn frame(&self) -> u64 {
          self.frame
      }
      /// Advance the spinner clock by one redraw. The run loop calls this once
      /// per iteration; the loop polls every 50ms, so the throbber animates
      /// without a dedicated timer.
      pub fn tick(&mut self) {
          self.frame = self.frame.wrapping_add(1);
      }
  ```
- [ ] Run `cargo test --lib tui::tests::frame_starts_at_zero_and_advances -- --nocapture` — expect PASS.
- [ ] Commit:
  ```
  feat(tui): add per-redraw frame counter to App

  Add a wrapping `frame` clock advanced once per redraw, plus an
  `indexing` field, to drive a hand-rolled spinner without a timer.

  Co-Authored-By: Claude <noreply@anthropic.com>
  ```

---

## Task 3: Indexing state + braille throbber in the search line

Surface an `is_indexing` state via an `indexing: Option<usize>` count (field added in Task 2) and render an animated braille throbber plus an `indexing N…` label in the search line.

**Files:**
- `src/tui/mod.rs` (add `indexing` accessor + setter, test)
- `src/tui/view.rs` (spinner frame table + helper, render the label, test)

Steps:

- [ ] Add a failing test in `src/tui/mod.rs` tests for the indexing accessor/setter. Append:
  ```rust
  #[test]
  fn indexing_state_round_trips() {
      let mut app = App::new();
      assert_eq!(app.indexing(), None);
      app.set_indexing(Some(42));
      assert_eq!(app.indexing(), Some(42));
      app.set_indexing(None);
      assert_eq!(app.indexing(), None);
  }
  ```
- [ ] Run `cargo test --lib tui::tests::indexing_state_round_trips -- --nocapture` — expect FAIL.
- [ ] In `src/tui/mod.rs`, add the accessor + setter (near `frame`/`tick` from Task 2):
  ```rust
      /// Number of sessions still being indexed, or `None` when idle.
      pub fn indexing(&self) -> Option<usize> {
          self.indexing
      }
      pub fn set_indexing(&mut self, count: Option<usize>) {
          self.indexing = count;
      }
  ```
- [ ] Run `cargo test --lib tui::tests::indexing_state_round_trips -- --nocapture` — expect PASS.
- [ ] Add a failing render test in `src/tui/view.rs` tests for the throbber + label. Append:
  ```rust
  #[test]
  fn indexing_state_shows_spinner_and_label() {
      use crate::enrich::Enricher;
      use std::collections::HashMap;

      let mut app = App::new();
      app.set_indexing(Some(7));
      let enr: Vec<Box<dyn Enricher>> = vec![];
      let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
      let cols = crate::columns::default_columns();
      let backend = TestBackend::new(100, 12);
      let mut term = Terminal::new(backend).unwrap();
      term.draw(|f| {
          render(
              f,
              &app,
              RenderModel {
                  now: 100,
                  columns: &cols,
                  enrichers: &enr,
                  resolved: &resolved,
                  preview_lines: &[],
                  status: &StatusLine::default(),
                  modal_command: None,
              },
          )
      })
      .unwrap();
      let buf = term.backend().buffer().clone();
      let text: String = buf.content().iter().map(|c| c.symbol()).collect();
      // The braille frame at frame=0 is the first table entry.
      assert!(text.contains(SPINNER_FRAMES[0]));
      assert!(text.contains("indexing 7"));
  }
  ```
- [ ] Run `cargo test --lib tui::view::tests::indexing_state_shows_spinner_and_label -- --nocapture` — expect FAIL (no `SPINNER_FRAMES`, no label).
- [ ] In `src/tui/view.rs`, add the spinner frame table and a helper near the top-level consts (e.g. just after `const SELECTION_MARKER_WIDTH`):
  ```rust
  /// Braille throbber frames, indexed by the per-redraw frame counter. Hand-rolled
  /// to avoid a spinner crate; advances one frame per redraw (the run loop polls
  /// every 50ms, so it animates smoothly).
  pub(crate) const SPINNER_FRAMES: [&str; 10] = [
      "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏",
  ];

  /// The current throbber glyph for a given frame counter.
  fn spinner_frame(frame: u64) -> &'static str {
      SPINNER_FRAMES[(frame as usize) % SPINNER_FRAMES.len()]
  }
  ```
- [ ] In `render`, append the indexing throbber to the search-line `header` before it is rendered. The header is built as `let header = Line::from(vec![...]);` (currently lines ~67–74) followed by `f.render_widget(Paragraph::new(header), chunks[0]);`. Change `let header` to `let mut header` and insert before the render call:
  ```rust
  let mut header = Line::from(vec![
      Span::styled(" ❯ ", Style::default().fg(theme::ACCENT)),
      Span::styled(
          app.query().to_string(),
          Style::default().fg(theme::SELECTED_FG),
      ),
      Span::raw(format!("   {}/{}", pos, total)).fg(theme::DIM),
  ]);
  if let Some(count) = app.indexing() {
      header.spans.push(Span::styled(
          format!("   {} indexing {count}…", spinner_frame(app.frame())),
          Style::default().fg(theme::DIM), // MUTED
      ));
  }
  f.render_widget(Paragraph::new(header), chunks[0]);
  ```
  (Delete the original `let header = ...` and its `f.render_widget(Paragraph::new(header), chunks[0]);` so this single block replaces both.)
- [ ] Run `cargo test --lib tui::view::tests::indexing_state_shows_spinner_and_label -- --nocapture` — expect PASS.
- [ ] Commit:
  ```
  feat(tui): add indexing state with animated braille throbber

  Surface an indexing count on App and render a hand-rolled braille
  spinner plus an "indexing N…" label in the search line. The spinner
  advances one frame per redraw; no new crate or timer is introduced.

  Co-Authored-By: Claude <noreply@anthropic.com>
  ```

---

## Task 4: Animate the pending-enricher glyph

`results_list.rs` renders a static `⟳` for an unresolved slow enricher; it reads as a spinner but never moves. Reuse the W3 frame counter so it animates consistently with the search-line throbber. The cell builders take no frame, so plumb a `frame: u64` parameter through the cell path.

**Files:**
- `src/tui/results_list.rs` (thread `frame` into `cell`/`enrichment_cell`/`row_line`/`layout_for_rows`/`desired_widths`, render the throbber, test)
- `src/tui/view.rs` (pass `app.frame()` to `row_line` and `layout_for_rows`)

Steps:

- [ ] Update the existing `pending_pr_shows_glyph` test in `src/tui/results_list.rs` to assert the **animated frame** instead of `⟳`, and add a frame argument. Replace the body of `pending_pr_shows_glyph` with:
  ```rust
  #[test]
  fn pending_pr_shows_animated_spinner_glyph() {
      let cols = default_columns();
      let layout = layout_for(&cols, 120);
      let enr: Vec<Box<dyn Enricher>> = vec![Box::new(crate::enrich::gh_pr::GhPrEnricher)];
      let resolved = HashMap::new();
      // frame=0 -> first braille frame; frame=3 -> fourth.
      let line0 = row_line(&sess(), &layout, &cols, &enr, &resolved, 0, 0);
      let t0: String = line0.spans.iter().map(|s| s.content.as_ref()).collect();
      assert!(t0.contains(crate::tui::view::SPINNER_FRAMES[0]));
      let line3 = row_line(&sess(), &layout, &cols, &enr, &resolved, 0, 3);
      let t3: String = line3.spans.iter().map(|s| s.content.as_ref()).collect();
      assert!(t3.contains(crate::tui::view::SPINNER_FRAMES[3]));
  }
  ```
  Also delete the old `pending_pr_shows_glyph` test (this replaces it).
- [ ] Run `cargo test --lib tui::results_list -- --nocapture` — expect FAIL (signatures don't take `frame`; old `⟳`).
- [ ] In `src/tui/results_list.rs`, add a frame parameter and thread it through. Change `enrichment_cell` to accept `frame: u64` and render the spinner for the `None` arm:
  ```rust
  fn enrichment_cell(
      id: &str,
      s: &SessionSummary,
      enrichers: &[Box<dyn Enricher>],
      resolved: &HashMap<(String, &'static str), Option<String>>,
      frame: u64,
  ) -> (String, Style) {
      let Some(enr) = enrichers.iter().find(|e| e.id() == id) else {
          return (String::new(), Style::default());
      };
      match enr.kind() {
          EnrichKind::Fast => {
              let text = enr.resolve(s).map(|v| v.text).unwrap_or_else(|| "—".into());
              (text, Style::default().fg(theme::DIM))
          }
          EnrichKind::Slow => match resolved.get(&(s.document_key(), enr.id())) {
              Some(Some(text)) => (text.clone(), Style::default().fg(theme::ACCENT)),
              Some(None) => ("—".into(), Style::default().fg(theme::DIM)),
              None => (
                  crate::tui::view::spinner_glyph(frame).to_string(),
                  Style::default().fg(theme::DIM),
              ),
          },
      }
  }
  ```
- [ ] In `src/tui/view.rs`, expose a `pub(crate)` accessor for the glyph (the existing `spinner_frame` is private). Add next to `spinner_frame`:
  ```rust
  /// Public throbber glyph for callers outside this module (e.g. the pending
  /// enricher cell), reusing the same frame table.
  pub(crate) fn spinner_glyph(frame: u64) -> &'static str {
      spinner_frame(frame)
  }
  ```
- [ ] In `src/tui/results_list.rs`, update `cell` to take and forward `frame`:
  ```rust
  fn cell(
      s: &SessionSummary,
      col: &Column,
      enrichers: &[Box<dyn Enricher>],
      resolved: &HashMap<(String, &'static str), Option<String>>,
      now: i64,
      frame: u64,
  ) -> (String, Style) {
  ```
  and change the fallthrough arm `other => enrichment_cell(other, s, enrichers, resolved),` to `other => enrichment_cell(other, s, enrichers, resolved, frame),`.
- [ ] Update `row_line` to take `frame: u64` (append as the last parameter) and forward it to `cell`. Change the signature's final params to `... now: i64, frame: u64,` and the call inside the loop from `cell(s, col, enrichers, resolved, now)` to `cell(s, col, enrichers, resolved, now, frame)`.
- [ ] Update `desired_widths` and `layout_for_rows` to take `frame: u64` and forward it to `cell`. In `desired_widths`, add `frame: u64,` as the last param and change `let (text, _) = cell(row, col, enrichers, resolved, now);` to `let (text, _) = cell(row, col, enrichers, resolved, now, frame);`. In `layout_for_rows`, add `frame: u64,` as the last param and change `let desired = desired_widths(columns, rows, enrichers, resolved, now);` to `let desired = desired_widths(columns, rows, enrichers, resolved, now, frame);`.
- [ ] Update the remaining tests in `src/tui/results_list.rs` that call `row_line` / `layout_for_rows` to pass a frame. Add a trailing `0` (frame) argument to each call:
  - `row_renders_repo_branch_title`: `layout_for_rows(&cols, 120, std::slice::from_ref(&row), &enr, &resolved, 3600, 0)` and `row_line(&row, &layout, &cols, &enr, &resolved, 3600, 0)`.
  - `visible_row_content_sizes_repo_and_branch_before_title_flexes`: `layout_for_rows(&cols, 120, &[row], &enr, &resolved, 0, 0)`.
  - `pr_cell_reads_resolved_with_full_enricher_list`: `layout_for_rows(&cols, 120, std::slice::from_ref(&row), &enr, &resolved, 0, 0)` and `row_line(&row, &layout, &cols, &enr, &resolved, 0, 0)`.
- [ ] In `src/tui/view.rs` `render`, pass `app.frame()` to the two call sites. Change `results_list::layout_for_rows(cols, list_inner_w, visible_results, model.enrichers, model.resolved, model.now,)` to add a trailing `app.frame(),`, and change the `results_list::row_line(s, &layout, cols, model.enrichers, model.resolved, model.now,)` inside the `else` branch (from Task 1) to add a trailing `app.frame(),`.
- [ ] Run `cargo test --lib tui::results_list -- --nocapture` — expect PASS.
- [ ] Run `cargo test --lib tui::view -- --nocapture` — expect PASS (signature wiring compiles; existing view tests unaffected).
- [ ] Commit:
  ```
  feat(tui): animate pending enricher glyph via frame counter

  Thread the per-redraw frame counter into the result cell builders and
  render the same braille throbber for unresolved slow enrichers, so the
  pending glyph animates instead of reading as a frozen spinner.

  Co-Authored-By: Claude <noreply@anthropic.com>
  ```

---

## Task 5: Wire indexing state and the frame tick in `src/main.rs`

`App::frame` and `App::indexing` exist but nothing advances/sets them. Wire the run loop: tick every iteration and set the indexing count from sync status.

**Files:**
- `src/main.rs` (modify `run_tui` loop)

Context: in `run_tui` (`src/main.rs`), `sync_status: Option<String>` starts as `Some("syncing".to_string())` and becomes `Some(report.status_line())` on `Update::Done`. While syncing (before `Done`), the corpus is still indexing. Use the current result count as the "indexing N" number, and clear it once sync is done.

Steps:

- [ ] In `run_tui`, immediately after `app.set_viewport_metrics(list_rows_height, preview_height);` (currently ~line 142), add the per-redraw tick:
  ```rust
  app.tick();
  ```
- [ ] Track whether sync has finished. Add a flag near `let mut sync_status = Some("syncing".to_string());` (currently ~line 126):
  ```rust
  let mut sync_done = false;
  ```
- [ ] In the `Update::Done { report }` arm of the `updates.try_recv()` match (currently ~line 206), set the flag:
  ```rust
  Update::Done { report } => {
      sync_status = Some(report.status_line());
      sync_done = true;
  }
  ```
- [ ] Set the indexing count just before `terminal.draw(...)` (after the `status` and `modal_command` are built, ~line 172). Show the count only while indexing is in progress:
  ```rust
  app.set_indexing(if sync_done {
      None
  } else {
      Some(app.results().len())
  });
  ```
- [ ] Run `cargo build` — expect success (no test for main; verified by compile + Task 3/4 unit tests).
- [ ] Run `cargo test --lib` — expect all PASS.
- [ ] Commit:
  ```
  feat(tui): drive spinner tick and indexing count from run loop

  Advance the frame counter once per redraw and set the indexing count
  from sync status, clearing it when background sync reports Done.

  Co-Authored-By: Claude <noreply@anthropic.com>
  ```

---

## Task 6: Final verification

**Files:** none (verification only)

Steps:

- [ ] Run `cargo test --lib` — expect all PASS.
- [ ] Run `cargo clippy --all-targets -- -D warnings` — expect no warnings. Fix any introduced (e.g. unused imports, needless `mut`).
- [ ] Run `cargo fmt` and confirm no churn beyond the touched files (`git diff --stat`).
- [ ] Manual smoke (optional, no auto-test): `cargo run -- --rebuild` on a real corpus and confirm: empty query shows the "Type to search" prompt; a no-match query shows "No sessions match"; during cold-start sync the search line shows a moving braille throbber + "indexing N…"; an unresolved PR cell shows a moving glyph (not a frozen `⟳`).
