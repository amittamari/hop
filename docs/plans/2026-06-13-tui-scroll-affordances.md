# TUI Scroll Affordances Implementation Plan

> **For implementers:** Work through this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for progress tracking; check each off as you complete it.

**Goal:** Add scroll-position affordances to the `hop` TUI so users can sense their location in long result sets and long transcripts: a vertical scrollbar on the results list, a vertical scrollbar on the preview transcript, a bottom clamp on preview scrolling (so you can't scroll past the end into blank space), and ellipsis truncation of the preview header's "title · directory" line so a long path no longer wraps or clips the 2-row header.

**Architecture:** This is a Ratatui 0.30 / crossterm TUI. The render path lives in `src/tui/view.rs` (`render` fn, pure function of `&App` + a `RenderModel`). App state and the key/command handlers live in `src/tui/mod.rs`. Display-width-aware truncation lives in `src/columns.rs` (`fit` / `display_width`). The preview state (transcript lines) lives in `src/tui/preview.rs` (`PreviewState.lines`). The run loop in `src/main.rs` wires viewport metrics and preview lines into the `App` and `RenderModel` each frame.

**Tech Stack:** Rust, Ratatui 0.30.

---

## Dependencies & Sequencing

- **Border color role.** Both scrollbars and the preview divider use a "border/divider" color. As of this plan the code uses the module constant `theme::DIVIDER` (see `src/tui/theme.rs:14`, used at `view.rs:154`). If the theme-system plan (`docs/plans/2026-06-13-tui-theme-system.md`) has landed and introduced an `app.theme()` accessor with a `.border` role, use `app.theme().border` for the scrollbar style instead; otherwise use `theme::DIVIDER`. This plan is written against `theme::DIVIDER` — substitute mechanically if the theme plan landed first.
- **Results-table plan conflict.** The list scrollbar attaches to the list area produced around `view.rs:87-101` and split by `split_list_area` (`view.rs:201-210`). The results-table plan (`docs/plans/2026-06-13-tui-results-table.md`) rewrites that list region. **Recommendation: land this plan AFTER the results-table plan**, or coordinate so the reserved 1-column scrollbar gutter survives the table rewrite. If both are in flight, the gutter-reservation step (Task 1) is the integration point to re-check.
- **Responsiveness plan.** Light edit overlap with `view.rs`/`mod.rs` (the responsiveness plan also touches `set_viewport_metrics` in `mod.rs:164` and the render path). Expect trivial merge conflicts only; the field added in Task 3 (`preview_viewport_height`) is additive.
- No new crates. `Scrollbar`, `ScrollbarOrientation`, `ScrollbarState` are already in `ratatui::widgets` (ratatui = "0.30", see `Cargo.toml:21`).

### Ratatui 0.30 Scrollbar API (reference)

```rust
use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};

let mut sb_state = ScrollbarState::new(content_len).position(pos);
frame.render_stateful_widget(
    Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓")),
    area,
    &mut sb_state,
);
```

- `content_len` is the **total** item/line count (e.g. `app.results().len()`, or preview line count). `pos` is the current top/selected index (the list selected index, or `preview_scroll`).
- The scrollbar renders **inside the right column of `area`**. Reserve a 1-col gutter (split it off the list/preview area) so it does not overlap content. The thumb glyph is `█` (or `▐` in some terminals); tests assert one of these glyphs is present in the rendered buffer.

---

## Task 1: Reserve a scrollbar gutter and render the results-list scrollbar

**Files:**
- `src/tui/view.rs` (imports at lines 8-10; list region `view.rs:87-148`; tests module starting `view.rs:446`)

### Steps

- [ ] **Failing test (real code).** Add this test inside the `#[cfg(test)] mod tests` block at the end of `src/tui/view.rs` (after the existing `visible_range_keeps_selection_in_view` test, before the closing `}` of the module). It builds 50 rows into an 8-row-tall terminal so the list overflows, then asserts a scrollbar thumb glyph appears:

```rust
    #[test]
    fn results_list_shows_scrollbar_when_overflowing() {
        use crate::enrich::Enricher;
        use std::collections::HashMap;

        let mut app = App::new();
        // Hide the preview so the whole body width is the list (keeps the
        // scrollbar on the far right of the frame, simplest to assert).
        app.set_preview(false, 50);
        app.set_results(
            (0..50)
                .map(|i| SessionSummary {
                    id: format!("s{i}"),
                    agent: AgentId::Claude,
                    title: format!("session {i}"),
                    directory: "/work/api".into(),
                    timestamp: 0,
                    message_count: 1,
                    yolo: false,
                    branch: None,
                    repo_url: None,
                    source_path: None,
                })
                .collect(),
        );

        let enr: Vec<Box<dyn Enricher>> = vec![];
        let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
        let cols = crate::columns::default_columns();
        let backend = TestBackend::new(100, 8);
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
        assert!(
            text.contains('█') || text.contains('▐'),
            "expected a scrollbar thumb glyph in the rendered list"
        );
    }
```

- [ ] **Run (expect FAIL).** `cargo test --lib results_list_shows_scrollbar_when_overflowing -- --nocapture` — should fail (no thumb glyph yet).

- [ ] **Minimal impl: imports.** In `src/tui/view.rs`, extend the widgets `use` (currently lines 8-10) to add the scrollbar types. Replace:

```rust
use ratatui::widgets::{
    Block, Borders, Clear, HighlightSpacing, List, ListItem, ListState, Padding, Paragraph, Wrap,
};
```

with:

```rust
use ratatui::widgets::{
    Block, Borders, Clear, HighlightSpacing, List, ListItem, ListState, Padding, Paragraph,
    Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};
```

- [ ] **Minimal impl: reserve gutter + render scrollbar.** In `render`, locate the list/column block. Currently (`view.rs:98-101`):

```rust
    // column grid
    let cols = model.columns;
    let list_inner_w = list_area.width.saturating_sub(SELECTION_MARKER_WIDTH);
    let (list_header_area, list_rows_area) = split_list_area(list_area);
```

Split a 1-col scrollbar gutter off the **right** of `list_area` before computing inner width and splitting header/rows. Replace those four lines with:

```rust
    // column grid. Reserve a 1-col gutter on the right for the list scrollbar
    // so the bar never overlaps row content.
    let cols = model.columns;
    let (list_area, list_scrollbar_area) = split_scrollbar_gutter(list_area);
    let list_inner_w = list_area.width.saturating_sub(SELECTION_MARKER_WIDTH);
    let (list_header_area, list_rows_area) = split_list_area(list_area);
```

Then, immediately after the existing `f.render_stateful_widget(list, list_rows_area, &mut state);` line (`view.rs:148`), render the scrollbar. Insert:

```rust

    // Vertical scrollbar reflecting selection position within all results.
    if let Some(sb_area) = list_scrollbar_area {
        let total = app.results().len();
        if total > list_rows_area.height as usize {
            let mut sb_state = ScrollbarState::new(total).position(app.selected());
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(Some("↑"))
                    .end_symbol(Some("↓"))
                    .style(Style::default().fg(theme::DIVIDER)),
                sb_area,
                &mut sb_state,
            );
        }
    }
```

- [ ] **Minimal impl: gutter helper.** Add this helper next to `split_list_area` (after `split_list_area`'s closing `}` at `view.rs:210`):

```rust
/// Carve a 1-column scrollbar gutter off the right of `area`. Returns the
/// content area and the gutter (or `None` if the area is too narrow to spare a
/// column). Vertical scrollbars render inside the right column of their area.
fn split_scrollbar_gutter(area: Rect) -> (Rect, Option<Rect>) {
    if area.width <= 1 {
        return (area, None);
    }
    let content = Rect { width: area.width - 1, ..area };
    let gutter = Rect { x: area.right() - 1, width: 1, ..area };
    (content, Some(gutter))
}
```

- [ ] **Run (expect PASS).** `cargo test --lib results_list_shows_scrollbar_when_overflowing -- --nocapture`.

- [ ] **Regression check.** `cargo test --lib --` — the existing `selected_result_has_marker_and_focus_style` test asserts buffer cells at columns 0 and 2; reserving a gutter on the far right does not move those. The `renders_columns_and_preview` test uses a 100-wide terminal; confirm it still passes. If any width-sensitive assertion regresses, the gutter is 1 col on the right only — investigate before proceeding.

- [ ] **Commit.**

```
feat(tui): add results list scrollbar

Reserve a 1-col gutter on the right of the list area and render a
vertical Scrollbar driven by total result count and the selected index,
so long result sets show a position indicator.

Co-Authored-By: Claude <noreply@anthropic.com>
```

---

## Task 2: Render the preview transcript scrollbar

**Files:**
- `src/tui/view.rs` (preview block `view.rs:151-185`; tests module)

### Steps

- [ ] **Failing test (real code).** Add this test to the `#[cfg(test)] mod tests` block in `src/tui/view.rs`. It enables the preview, feeds many preview lines so the transcript overflows, and asserts a thumb glyph:

```rust
    #[test]
    fn preview_shows_scrollbar_for_long_transcript() {
        use crate::enrich::Enricher;
        use std::collections::HashMap;

        let mut app = App::new();
        app.set_preview(true, 50);
        app.set_preview_header(false);
        app.set_results(vec![SessionSummary {
            id: "a".into(),
            agent: AgentId::Claude,
            title: "fix auth".into(),
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
        let lines: Vec<Line<'static>> =
            (0..40).map(|i| Line::from(format!("line {i}"))).collect();

        let backend = TestBackend::new(100, 8);
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
                    preview_lines: &lines,
                    status: &StatusLine::default(),
                    modal_command: None,
                },
            )
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        let text: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            text.contains('█') || text.contains('▐'),
            "expected a scrollbar thumb glyph in the rendered preview"
        );
    }
```

- [ ] **Run (expect FAIL).** `cargo test --lib preview_shows_scrollbar_for_long_transcript -- --nocapture`.

- [ ] **Minimal impl.** In `render`, the preview transcript is drawn at `view.rs:178-184`:

```rust
        f.render_widget(
            Paragraph::new(model.preview_lines.to_vec())
                .style(Style::default().fg(theme::PREVIEW_TEXT))
                .wrap(Wrap { trim: false })
                .scroll((app.preview_scroll(), 0)),
            transcript_area,
        );
```

Reserve a gutter off `transcript_area` for the preview scrollbar, render the transcript into the narrowed area, then render the scrollbar. Replace the block above with:

```rust
        let (transcript_area, preview_scrollbar_area) = split_scrollbar_gutter(transcript_area);
        f.render_widget(
            Paragraph::new(model.preview_lines.to_vec())
                .style(Style::default().fg(theme::PREVIEW_TEXT))
                .wrap(Wrap { trim: false })
                .scroll((app.preview_scroll(), 0)),
            transcript_area,
        );
        // Vertical scrollbar reflecting scroll position within the transcript.
        if let Some(sb_area) = preview_scrollbar_area {
            let total = model.preview_lines.len();
            if total > transcript_area.height as usize {
                let mut sb_state =
                    ScrollbarState::new(total).position(app.preview_scroll() as usize);
                f.render_stateful_widget(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .begin_symbol(Some("↑"))
                        .end_symbol(Some("↓"))
                        .style(Style::default().fg(theme::DIVIDER)),
                    sb_area,
                    &mut sb_state,
                );
            }
        }
```

Note: `total` here is the raw line count (pre-wrap). Wrapping can make the true rendered length longer, but line count is the cheap, available signal and matches how `preview_scroll` is measured (in source lines), so the scrollbar stays consistent with the scroll offset.

- [ ] **Run (expect PASS).** `cargo test --lib preview_shows_scrollbar_for_long_transcript -- --nocapture`.

- [ ] **Regression check.** `cargo test --lib --` — confirm `wraps_long_preview_prose` and `renders_columns_and_preview` still pass (the preview now has a 1-col gutter; both tests assert text content, not exact columns near the right edge).

- [ ] **Commit.**

```
feat(tui): add preview transcript scrollbar

Reserve a gutter off the transcript area and render a vertical Scrollbar
driven by the total preview line count and the current preview_scroll
offset.

Co-Authored-By: Claude <noreply@anthropic.com>
```

---

## Task 3: Clamp preview scroll at the bottom (W7)

The `ScrollPreview` handler (`mod.rs:315-320`) clamps `preview_scroll` only at the top (`.max(0)`), so the user can scroll past the content into blank space. Clamp the upper bound to `line_count.saturating_sub(viewport_height)`.

`App` already knows the preview viewport height implicitly: `set_viewport_metrics` (`mod.rs:164-167`) receives `preview_height` and stores `preview_scroll_step = preview_height - 1`. We add a stored `preview_viewport_height` and a stored `preview_line_count`, then clamp in the handler.

**Files:**
- `src/tui/mod.rs` (struct fields `mod.rs:36-53`; `App::new` `mod.rs:56-74`; `set_viewport_metrics` `mod.rs:164-167`; `ScrollPreview` handler `mod.rs:315-320`; tests module)
- `src/main.rs` (run loop, after `preview_state.update`, around `main.rs:152`)

### Steps

- [ ] **Failing test (real code).** Add this unit test to the `#[cfg(test)] mod tests` block in `src/tui/mod.rs` (alongside `viewport_metrics_drive_paging_and_preview_scroll`). It sets a small viewport and a known line count, then scrolls down hard and asserts `preview_scroll` never exceeds `line_count - viewport_height`:

```rust
    #[test]
    fn preview_scroll_clamps_at_bottom() {
        let mut app = app_with(1);
        // viewport: list_rows_height irrelevant here; preview_height = 5 rows.
        app.set_viewport_metrics(6, 5);
        app.set_preview_line_count(30);
        // Scroll down far more than the content allows.
        for _ in 0..20 {
            app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
        }
        // Max top line = 30 - 5 = 25; never past it into blank space.
        assert_eq!(app.preview_scroll(), 25);
    }
```

- [ ] **Run (expect FAIL).** `cargo test --lib preview_scroll_clamps_at_bottom -- --nocapture` — fails to compile (`set_preview_line_count` does not exist yet).

- [ ] **Minimal impl: add fields.** In `src/tui/mod.rs`, add two fields to `struct App` (after `preview_scroll_step: u16,` at `mod.rs:50`):

```rust
    preview_viewport_height: u16,
    preview_line_count: usize,
```

- [ ] **Minimal impl: init fields.** In `App::new` (`mod.rs:56-74`), after `preview_scroll_step: 8,` add:

```rust
            preview_viewport_height: 1,
            preview_line_count: 0,
```

- [ ] **Minimal impl: store viewport height + add setter.** In `set_viewport_metrics` (`mod.rs:164-167`), record the viewport height. Replace:

```rust
    pub fn set_viewport_metrics(&mut self, list_rows_height: u16, preview_height: u16) {
        self.list_page_size = usize::from(list_rows_height.saturating_sub(1).max(1));
        self.preview_scroll_step = preview_height.saturating_sub(1).max(1);
    }
```

with:

```rust
    pub fn set_viewport_metrics(&mut self, list_rows_height: u16, preview_height: u16) {
        self.list_page_size = usize::from(list_rows_height.saturating_sub(1).max(1));
        self.preview_scroll_step = preview_height.saturating_sub(1).max(1);
        self.preview_viewport_height = preview_height.max(1);
    }

    /// Number of source lines in the current preview transcript; used to clamp
    /// `preview_scroll` so it can't run past the end into blank space.
    pub fn set_preview_line_count(&mut self, count: usize) {
        self.preview_line_count = count;
    }
```

- [ ] **Minimal impl: clamp the handler.** In `apply_command`, the `ScrollPreview` arm (`mod.rs:315-320`) currently is:

```rust
            keymap::Command::ScrollPreview(d) => {
                let delta = d as i32 * i32::from(self.preview_scroll_step);
                let next = self.preview_scroll as i32 + delta;
                self.preview_scroll = next.max(0) as u16;
                Action::None
            }
```

Replace with a both-ends clamp:

```rust
            keymap::Command::ScrollPreview(d) => {
                let delta = d as i32 * i32::from(self.preview_scroll_step);
                let next = (self.preview_scroll as i32 + delta).max(0) as usize;
                let max_scroll = self
                    .preview_line_count
                    .saturating_sub(self.preview_viewport_height as usize);
                self.preview_scroll = next.min(max_scroll).min(u16::MAX as usize) as u16;
                Action::None
            }
```

- [ ] **Run (expect PASS).** `cargo test --lib preview_scroll_clamps_at_bottom -- --nocapture`.

- [ ] **Run (regression).** `cargo test --lib viewport_metrics_drive_paging_and_preview_scroll -- --nocapture` — this test sets `set_viewport_metrics(6, 4)` and does a single Ctrl+D, expecting `preview_scroll() == 3`. With the new clamp and `preview_line_count` defaulting to `0`, `max_scroll = 0.saturating_sub(4) = 0`, which would clamp the result to `0` and break this test. **Fix the test** so it has line count to scroll into: add `app.set_preview_line_count(100);` right after `app.set_viewport_metrics(6, 4);` in that test. Then `max_scroll = 100 - 4 = 96`, and the single step lands at `3` as before. Re-run to confirm PASS.

- [ ] **Minimal impl: wire line count in the run loop.** In `src/main.rs`, immediately after the `preview_state.update(...)` call (it ends at `main.rs:152` with `);`), feed the line count into the app:

```rust
            app.set_preview_line_count(preview_state.lines.len());
```

- [ ] **Run (expect PASS).** `cargo test --lib --` (full lib suite). Also `cargo build` to confirm `main.rs` compiles with the new call.

- [ ] **Commit.**

```
fix(tui): clamp preview scroll at the bottom

Clamp preview_scroll to line_count - viewport_height so the user can no
longer scroll past the end of the transcript into blank space. Wire the
current preview line count from the run loop.

Co-Authored-By: Claude <noreply@anthropic.com>
```

---

## Task 4: Ellipsize the preview header's title · directory line (W8)

`preview_header_lines` (`view.rs:364-416`) builds a second header line as `title · directory` with no truncation. A long directory path wraps/clips the fixed 2-row header. The yolo modal already truncates via `fit_for_modal` (`view.rs:354-362`, which wraps `columns::fit`). Apply the same precedent: fit the whole `title · directory` string to the preview inner width.

`preview_header_lines` does not currently know the available width. Add a `width` parameter and pass the header area's width from the call site.

**Files:**
- `src/tui/view.rs` (`preview_header_lines` signature `view.rs:364-368` and body `view.rs:406-415`; call site `view.rs:171-177`; tests module)

### Steps

- [ ] **Failing test (real code).** Add this test to the `#[cfg(test)] mod tests` block in `src/tui/view.rs`. It renders a result with a very long directory in a narrow preview, keeps the header visible, and asserts (a) an ellipsis appears and (b) the header stays 2 rows (the directory text does not spill into the transcript region):

```rust
    #[test]
    fn preview_header_ellipsizes_long_directory() {
        use crate::enrich::Enricher;
        use std::collections::HashMap;

        let long_dir =
            "/Users/someone/workspaces/very/deeply/nested/project/path/that/keeps/going/api";
        let mut app = App::new();
        app.set_preview(true, 50);
        app.set_preview_header(true);
        app.set_results(vec![SessionSummary {
            id: "a".into(),
            agent: AgentId::Claude,
            title: "fix auth".into(),
            directory: long_dir.into(),
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
        // The header line is the title + directory; check it is fit to width.
        let header_lines = preview_header_lines(&app.results()[0], 100, &resolved, 30);
        assert_eq!(header_lines.len(), 2, "header is always 2 lines");
        let second: String = header_lines[1]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(
            second.contains('…'),
            "long title·directory line should be ellipsized, got {second:?}"
        );
        assert!(
            crate::columns::display_width(&second) <= 30,
            "fit line must not exceed the given width"
        );

        // And the rendered 2-row header must not push the long path into the
        // transcript rows: render and confirm the full untruncated path is gone.
        let backend = TestBackend::new(100, 8);
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
        assert!(
            !text.contains(long_dir),
            "the full untruncated directory must not render"
        );
    }
```

- [ ] **Run (expect FAIL).** `cargo test --lib preview_header_ellipsizes_long_directory -- --nocapture` — fails to compile (`preview_header_lines` currently takes 3 args, not 4).

- [ ] **Minimal impl: add a width param.** Change the signature of `preview_header_lines` (`view.rs:364-368`) from:

```rust
fn preview_header_lines(
    s: &SessionSummary,
    now: i64,
    resolved: &HashMap<(String, &'static str), Option<String>>,
) -> Vec<Line<'static>> {
```

to:

```rust
fn preview_header_lines(
    s: &SessionSummary,
    now: i64,
    resolved: &HashMap<(String, &'static str), Option<String>>,
    width: u16,
) -> Vec<Line<'static>> {
```

- [ ] **Minimal impl: fit the second line.** The function ends (`view.rs:406-415`) by returning two lines; the second is the un-truncated `title · directory`:

```rust
    vec![
        Line::from(first),
        Line::from(vec![
            Span::raw(s.title.clone()),
            Span::styled(
                format!(" · {}", s.directory),
                Style::default().fg(theme::DIM),
            ),
        ]),
    ]
}
```

Replace that `vec![ ... ]` return with a version that fits the combined string to `width`. The title stays bright and the directory stays dim, so fit each piece against its share of the width (title takes whatever it needs up to leaving room for `" · "` + directory; the directory absorbs the remainder and gets ellipsized):

```rust
    // Fit "title · directory" to the available width so a long path can't wrap
    // or clip the fixed 2-row header. Mirror fit_for_modal's use of columns::fit.
    let title = s.title.clone();
    let title_w = crate::columns::display_width(&title) as u16;
    let sep = " · ";
    let sep_w = crate::columns::display_width(sep) as u16;
    let second = if title_w + sep_w >= width {
        // No room for the directory; ellipsize the title alone.
        Line::from(Span::raw(fit_for_modal(&title, width as usize)))
    } else {
        let dir_w = width - title_w - sep_w;
        let dir = fit_for_modal(&s.directory, dir_w as usize);
        Line::from(vec![
            Span::raw(title),
            Span::styled(format!("{sep}{dir}"), Style::default().fg(theme::DIM)),
        ])
    };

    vec![Line::from(first), second]
}
```

Note: `fit_for_modal` (`view.rs:354-362`) trims trailing padding, so the fit output won't add spurious spaces; the ellipsis `…` is what proves truncation occurred.

- [ ] **Minimal impl: pass width at the call site.** In `render`, the header is drawn at `view.rs:171-177`:

```rust
        if let (Some(header_area), Some(session)) = (header_area, selected) {
            f.render_widget(
                Paragraph::new(preview_header_lines(session, model.now, model.resolved))
                    .style(Style::default().fg(theme::PREVIEW_TEXT)),
                header_area,
            );
        }
```

Pass `header_area.width`:

```rust
        if let (Some(header_area), Some(session)) = (header_area, selected) {
            f.render_widget(
                Paragraph::new(preview_header_lines(
                    session,
                    model.now,
                    model.resolved,
                    header_area.width,
                ))
                .style(Style::default().fg(theme::PREVIEW_TEXT)),
                header_area,
            );
        }
```

- [ ] **Run (expect PASS).** `cargo test --lib preview_header_ellipsizes_long_directory -- --nocapture`.

- [ ] **Run (regression).** `cargo test --lib --` — `renders_columns_and_preview` renders a short directory `/work/api` in a 100-wide terminal; the preview header is wide enough that fitting is a no-op and the text still appears. Confirm it passes.

- [ ] **Commit.**

```
fix(tui): ellipsize long preview header directory

Run the preview header's "title · directory" line through columns::fit
against the header inner width (mirroring the yolo modal), so a long
directory path no longer wraps or clips the fixed 2-row header.

Co-Authored-By: Claude <noreply@anthropic.com>
```

---

## Task 5: Final verification

**Files:** none (verification only)

### Steps

- [ ] **Full lib tests.** `cargo test --lib` — all green, including the four new tests:
  - `results_list_shows_scrollbar_when_overflowing`
  - `preview_shows_scrollbar_for_long_transcript`
  - `preview_scroll_clamps_at_bottom`
  - `preview_header_ellipsizes_long_directory`
- [ ] **Full build (binary too).** `cargo build` — confirms `src/main.rs` compiles with the `set_preview_line_count` call.
- [ ] **Lint.** `cargo clippy --all-targets -- -D warnings` — fix any warnings introduced (e.g. unused imports if a step was skipped). Expect clean.
- [ ] **Smoke (optional, manual).** `cargo run -- --rebuild` then run `hop`, page through a long result list (scrollbar tracks selection), open a long transcript and Ctrl+D past the end (scroll stops at the bottom, no blank scroll), select a session with a deep directory (header stays 2 rows with an ellipsis).
- [ ] **Doc check.** This change is a UI affordance, not an architectural boundary change; no `docs/ARCHITECTURE.md` rule edits are required. If the design review `docs/reviews/2026-06-13-tui-design-review.md` tracks resolution status, mark L5, W4, W7, W8 as addressed.
