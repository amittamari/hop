# TUI Responsiveness & Layout Safety Implementation Plan

> **For implementers:** Work through this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for progress tracking; check each off as you complete it.

**Goal:** Make the `hop` TUI main screen safe and usable at any terminal size by guarding tiny terminals, collapsing the preview on narrow widths, and keeping the footer status readable when space is tight.

**Architecture:** All edits land in `src/tui/view.rs` (the render path) plus its in-file `#[cfg(test)]` tests, mirroring the guard/centering patterns already present in `src/tui/help.rs`. The render path stays pure (no I/O); responsiveness is driven entirely by the `Rect` dimensions handed to `render()`. A new shared `center()` helper replaces hand-rolled centering math in both `view.rs` and `help.rs`.

**Tech Stack:** Rust, Ratatui 0.30, crossterm.

## Dependencies & Sequencing

- This plan makes heavy edits to `src/tui/view.rs` and will conflict with the theme-system plan (`docs/plans/2026-06-13-tui-theme-system.md`) and the results-table plan (`docs/plans/2026-06-13-tui-results-table.md`), both of which also rewrite `render()` internals. **Recommend landing this plan AFTER theme-system** so theme color/token churn settles first, then rebasing the results-table plan on top.
- The FOOTER work here (L3, Task 3) overlaps with the bindings-table plan (`docs/plans/2026-06-13-tui-bindings-table.md`, finding H2 adds preview hints to the footer). **This plan OWNS footer truncation/layout** (the `Flex::SpaceBetween` split and warning-survival ordering). The bindings plan should land AFTER this one and plug new hint CONTENT into the truncation-aware `footer_line` built here — it must not re-architect the footer layout.
- Tasks within this plan are sequenced 1 → 6 and each ends in a commit. They share `src/tui/view.rs`, so execute them in order; do not parallelize.

## Current-State Reference (verified against the code on 2026-06-13)

`src/tui/view.rs` imports (lines 5-11):
```rust
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, HighlightSpacing, List, ListItem, ListState, Padding, Paragraph, Wrap,
};
use ratatui::Frame;
```

Key facts the tasks depend on:
- `render(f: &mut Frame, app: &App, model: RenderModel<'_>)` starts at line 51. The first statement builds `chunks` via `Layout::default().direction(Direction::Vertical).constraints([Length(1), Min(1), Length(1)]).split(f.area())` (lines 52-59). `chunks[0]`=header, `chunks[1]`=body, `chunks[2]`=footer.
- The body horizontal split is lines 86-96.
- The yolo modal hand-rolled centering math is at lines 272-277 inside `render_yolo_modal`.
- `footer_line(status: &StatusLine) -> Line<'static>` is lines 214-254. `FOOTER_HINTS` const is line 212.
- `Block::default().borders(Borders::ALL)` appears at lines 329-331 (`render_yolo_modal`) and in `help.rs` at lines 69-71.
- `help.rs` guard pattern is lines 50-53 (`if area.width < 8 || area.height < 6 { return; }`); its centering math is lines 60-65.
- App accessors (all exist, no changes needed): `app.preview_visible() -> bool`, `app.preview_width_pct() -> u16`, `app.query() -> &str`, `app.results() -> &[SessionSummary]`, `app.selected() -> usize`.
- The established render-test harness builds a `RenderModel { now, columns, enrichers, resolved, preview_lines, status, modal_command }` and draws into a `TestBackend`, then collects `buf.content().iter().map(|c| c.symbol()).collect::<String>()`. See existing tests at lines 463-518 (`renders_columns_and_preview`) and 638-677 (`renders_single_mode_footer_hints`).

When a step says "add a test", append it inside the existing `mod tests` block in `src/tui/view.rs` (currently lines 446-739), AFTER the last test (`visible_range_keeps_selection_in_view`, ends line 738) and before the closing `}` on line 739.

---

## Task 1 — Tiny-terminal guard on the main screen (L1)

**Files:**
- Modify `src/tui/view.rs:51-59` (top of `render()`)
- Test `src/tui/view.rs` (`mod tests`)

- [ ] **1.1 Write the failing test.** Append this test inside `mod tests` in `src/tui/view.rs` (after `visible_range_keeps_selection_in_view`, before the module's closing brace):
```rust
    #[test]
    fn tiny_terminal_shows_too_small_message() {
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

        let backend = TestBackend::new(20, 4);
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
        assert!(text.contains("too small"), "expected too-small notice, got: {text:?}");
    }
```

- [ ] **1.2 Run it (expect FAIL).** Run:
```sh
cargo test --lib tui::view::tests::tiny_terminal_shows_too_small_message -- --nocapture
```
Expected: the assertion fails — the panic message is `expected too-small notice, got: "..."` because at 20x4 the body collapses and renders columns/marker, not the words "too small".

- [ ] **1.3 Minimal implementation.** In `src/tui/view.rs`, insert this guard as the FIRST statements inside `render()` (immediately after the `pub fn render(f: &mut Frame, app: &App, model: RenderModel<'_>) {` line at 51, before the `let chunks = ...` block):
```rust
    let area = f.area();
    if area.width < 30 || area.height < 6 {
        let msg = Paragraph::new("terminal too small")
            .alignment(Alignment::Center)
            .style(Style::default().fg(theme::DIM));
        f.render_widget(msg, area);
        return;
    }
```
(`Alignment` and `theme` are already imported; `Paragraph` and `Style` too. No new imports.)

- [ ] **1.4 Run it (expect PASS).** Run:
```sh
cargo test --lib tui::view::tests::tiny_terminal_shows_too_small_message -- --nocapture
```
Expected: `test result: ok. 1 passed`.

- [ ] **1.5 Confirm existing render tests still pass.** Run:
```sh
cargo test --lib tui::view::tests -- --nocapture
```
Expected: all `tui::view::tests` pass. Note: the existing `selected_result_has_marker_and_focus_style` test uses `TestBackend::new(80, 8)` and `renders_columns_and_preview` uses `(100, 12)` — both are above the 30x6 threshold, so the guard does not affect them.

- [ ] **1.6 Commit.** Run:
```sh
git add src/tui/view.rs && git commit -m "$(cat <<'EOF'
feat(tui): guard against tiny terminals

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

---

## Task 2 — Responsive preview collapse on narrow widths (L2)

The body split (lines 86-96) currently always splits by percentage when `preview_visible`. Add a width gate: below ~100 columns, drop the preview and give the whole body to the list. When the preview IS shown, floor the list side at `Min(48)` so the grid never starves.

**Files:**
- Modify `src/tui/view.rs:86-96` (body horizontal split)
- Test `src/tui/view.rs` (`mod tests`)

- [ ] **2.1 Write the failing test.** Append inside `mod tests`:
```rust
    #[test]
    fn narrow_width_drops_preview() {
        use crate::core::{Block, Message, Role};
        use crate::enrich::Enricher;
        use std::collections::HashMap;

        let mut app = App::new();
        app.set_preview(true, 50); // preview requested ON
        app.set_preview_header(false);
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
        let transcript = vec![Message {
            role: Role::User,
            blocks: vec![Block::Prose("PREVIEWBODYTOKEN".into())],
        }];
        let lines =
            crate::tui::preview::render_transcript(&transcript, app.query(), AgentId::Claude);

        let backend = TestBackend::new(40, 15);
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
        // At 40 cols the preview is dropped entirely.
        assert!(
            !text.contains("PREVIEWBODYTOKEN"),
            "preview should be hidden at narrow width, got: {text:?}"
        );
    }

    #[test]
    fn wide_width_keeps_preview_and_list_floor() {
        use crate::core::{Block, Message, Role};
        use crate::enrich::Enricher;
        use std::collections::HashMap;

        let mut app = App::new();
        app.set_preview(true, 80); // even maxed preview pct must not starve the list
        app.set_preview_header(false);
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
        let transcript = vec![Message {
            role: Role::User,
            blocks: vec![Block::Prose("PREVIEWBODYTOKEN".into())],
        }];
        let lines =
            crate::tui::preview::render_transcript(&transcript, app.query(), AgentId::Claude);

        let backend = TestBackend::new(140, 15);
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
        // Preview is present at wide width...
        assert!(
            text.contains("PREVIEWBODYTOKEN"),
            "preview should be shown at wide width, got: {text:?}"
        );
        // ...and the list still shows its content (grid not starved).
        assert!(text.contains("fix auth"), "list content missing: {text:?}");
        assert!(text.contains("feat/auth"), "list branch missing: {text:?}");
    }
```

- [ ] **2.2 Run them (expect FAIL).** Run:
```sh
cargo test --lib tui::view::tests::narrow_width_drops_preview -- --nocapture
```
Expected FAIL: `narrow_width_drops_preview` panics with `preview should be hidden at narrow width, got: "...PREVIEWBODYTOKEN..."` because the current code splits by percentage regardless of width, so the preview renders even at 40 cols.

- [ ] **2.3 Minimal implementation.** Replace the body-split block at `src/tui/view.rs:86-96`:
```rust
    // body: list (| preview)
    let (list_area, preview_area) = if app.preview_visible() {
        let pw = app.preview_width_pct();
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100 - pw), Constraint::Percentage(pw)])
            .split(chunks[1]);
        (body[0], Some(body[1]))
    } else {
        (chunks[1], None)
    };
```
with:
```rust
    // body: list (| preview). The preview only appears when both requested AND
    // there is room for it without starving the list grid. Below the width
    // threshold the list takes the whole body. When shown, the list side is
    // floored at Min(48) so its columns never collapse.
    const PREVIEW_MIN_WIDTH: u16 = 100;
    const LIST_MIN_WIDTH: u16 = 48;
    let (list_area, preview_area) = if app.preview_visible() && chunks[1].width >= PREVIEW_MIN_WIDTH {
        let pw = app.preview_width_pct();
        let [list, preview] = Layout::horizontal([
            Constraint::Min(LIST_MIN_WIDTH),
            Constraint::Percentage(pw),
        ])
        .areas(chunks[1]);
        (list, Some(preview))
    } else {
        (chunks[1], None)
    };
```
Notes: `Layout::horizontal(...).areas(rect)` returns `[Rect; 2]` here (Ratatui 0.30). The `Min(48)` list constraint wins over the `Percentage(pw)` preview when space is tight, so the list never drops below 48 cols. `chunks[1]` is the body Rect from the still-legacy vertical split (rewritten to array-destructuring in Task 5). `100 - pw` is no longer used because the list now uses `Min` instead of a complementary percentage.

- [ ] **2.4 Run them (expect PASS).** Run:
```sh
cargo test --lib tui::view::tests::narrow_width_drops_preview tui::view::tests::wide_width_keeps_preview_and_list_floor -- --nocapture
```
Expected: both pass.

- [ ] **2.5 Confirm the wrap test still passes.** The existing `wraps_long_preview_prose` test uses `TestBackend::new(80, 8)` with the preview requested ON; 80 < 100 so the preview is now DROPPED, which would break its `wrap-start`/`wrap-end` assertions. Update that test to a wide backend so the preview is present. In `src/tui/view.rs`, inside `wraps_long_preview_prose`, change:
```rust
        let backend = TestBackend::new(80, 8);
```
to:
```rust
        let backend = TestBackend::new(140, 8);
```
Then run:
```sh
cargo test --lib tui::view::tests::wraps_long_preview_prose -- --nocapture
```
Expected: pass (the long prose still wraps inside the preview pane, which is now wide enough to appear).

- [ ] **2.6 Run the whole view test module.** Run:
```sh
cargo test --lib tui::view::tests -- --nocapture
```
Expected: all pass.

- [ ] **2.7 Commit.** Run:
```sh
git add src/tui/view.rs && git commit -m "$(cat <<'EOF'
feat(tui): collapse preview on narrow terminals and floor the list width

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

---

## Task 3 — Truncation-aware footer (L3)

The footer is a single un-truncated `Line`; on narrow terminals the appended status/warning spans (rendered in the ACCENT color — the most important info) fall off the right edge. Split the footer into a left region (static hints, dropped first when tight) and a right region (volatile status: sync/pr/filters/warning) using `Flex::SpaceBetween`, so the status survives clipping.

**Files:**
- Modify `src/tui/view.rs:187-188` (footer render call) and `src/tui/view.rs:214-254` (`footer_line`)
- Test `src/tui/view.rs` (`mod tests`)

- [ ] **3.1 Write the failing test.** Append inside `mod tests`:
```rust
    #[test]
    fn footer_warning_survives_narrow_width() {
        use crate::enrich::Enricher;
        use std::collections::HashMap;

        let app = App::new();
        let enr: Vec<Box<dyn Enricher>> = vec![];
        let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
        let cols = crate::columns::default_columns();
        let status = StatusLine {
            sync: None,
            pr_pending: 0,
            warning: Some("WARNTOKEN".to_string()),
            filters: None,
        };

        // 50 cols is too narrow for the full static hint + warning on one line;
        // the warning must still be present.
        let backend = TestBackend::new(50, 8);
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
                    status: &status,
                    modal_command: None,
                },
            )
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        let text: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            text.contains("WARNTOKEN"),
            "warning must survive narrow footer, got: {text:?}"
        );
    }
```

- [ ] **3.2 Run it (expect FAIL).** Run:
```sh
cargo test --lib tui::view::tests::footer_warning_survives_narrow_width -- --nocapture
```
Expected FAIL: `warning must survive narrow footer, got: "..."` — at 50 cols the static hints (`type to search · ↑↓ move · Enter resume · ? help · Esc clear/quit`, ~58 display cols) consume the whole line and the warning is clipped off the right edge.

- [ ] **3.3 Minimal implementation — split `footer_line` into two halves.** Replace the entire `footer_line` function (`src/tui/view.rs:214-254`) with two functions that return the static-hint line and the volatile-status line separately:
```rust
/// Static, low-priority hints shown on the left of the footer. Dropped first
/// when the terminal is too narrow for both halves.
fn footer_hints_line() -> Line<'static> {
    let mut spans = Vec::new();
    let (label, rest) = FOOTER_HINTS.split_once(" · ").unwrap_or((FOOTER_HINTS, ""));
    spans.push(Span::styled(
        label.to_string(),
        Style::default()
            .fg(theme::ACCENT)
            .add_modifier(Modifier::BOLD),
    ));
    if !rest.is_empty() {
        spans.push(Span::styled(
            format!(" · {rest}"),
            Style::default().fg(theme::DIM),
        ));
    }
    Line::from(spans)
}

/// Volatile, high-priority status shown on the right of the footer. Rendered
/// right-aligned so it survives clipping ahead of the static hints.
fn footer_status_line(status: &StatusLine) -> Line<'static> {
    let mut spans = Vec::new();
    let mut push_sep = |spans: &mut Vec<Span<'static>>| {
        if !spans.is_empty() {
            spans.push(Span::styled(" · ".to_string(), Style::default().fg(theme::DIM)));
        }
    };
    if let Some(sync) = status.sync.as_deref().filter(|s| !s.is_empty()) {
        push_sep(&mut spans);
        spans.push(Span::styled(sync.to_string(), Style::default().fg(theme::DIM)));
    }
    if status.pr_pending > 0 {
        push_sep(&mut spans);
        spans.push(Span::styled(
            format!("pr {} pending", status.pr_pending),
            Style::default().fg(theme::DIM),
        ));
    }
    if let Some(filters) = status.filters.as_deref().filter(|s| !s.is_empty()) {
        push_sep(&mut spans);
        spans.push(Span::styled(
            format!("filters {filters}"),
            Style::default().fg(theme::DIM),
        ));
    }
    if let Some(warning) = status.warning.as_deref().filter(|s| !s.is_empty()) {
        push_sep(&mut spans);
        spans.push(Span::styled(warning.to_string(), Style::default().fg(theme::ACCENT)));
    }
    Line::from(spans)
}
```
Notes: the leading `" · "` separators from the old single-line version are dropped; each half now formats its own internal separators. The status text content (`sync complete; parse errors 2`, `pr 1 pending`, `filters agent:claude`, `source unavailable`) is unchanged so the existing `renders_yolo_dialog_and_status_footer` assertions still match substrings.

- [ ] **3.4 Minimal implementation — render the two halves with `Flex::SpaceBetween`.** Replace the footer render call at `src/tui/view.rs:187-188`:
```rust
    // footer
    f.render_widget(Paragraph::new(footer_line(model.status)), chunks[2]);
```
with:
```rust
    // footer: static hints on the left, volatile status on the right. The two
    // halves share the footer row via SpaceBetween so right-aligned status
    // (sync/pr/filters/warning) survives clipping ahead of the static hints.
    let [hints_area, status_area] = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(footer_status_width(model.status)),
    ])
    .flex(ratatui::layout::Flex::SpaceBetween)
    .areas(chunks[2]);
    f.render_widget(Paragraph::new(footer_hints_line()), hints_area);
    f.render_widget(
        Paragraph::new(footer_status_line(model.status)).alignment(Alignment::Right),
        status_area,
    );
```
Then add this width helper directly below `footer_status_line` (so the status region is sized to fit its content and the hints take the rest):
```rust
/// Display width of the rendered status line, used to size the right footer
/// region so the status is never clipped.
fn footer_status_width(status: &StatusLine) -> u16 {
    let text: String = footer_status_line(status)
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect();
    crate::columns::display_width(&text).min(u16::MAX as usize) as u16
}
```
Notes: `crate::columns::display_width(&str) -> usize` is already used in `render()` (lines 80-81) for the cursor math, so it is in scope at the crate path. `Flex` is referenced via its full path `ratatui::layout::Flex::SpaceBetween` to avoid touching the import list in this task (the import is added in Task 5 when the layouts are modernized). `Alignment` is already imported.

- [ ] **3.5 Run the new test (expect PASS).** Run:
```sh
cargo test --lib tui::view::tests::footer_warning_survives_narrow_width -- --nocapture
```
Expected: pass. The status region is sized to exactly fit `WARNTOKEN` and right-aligned, so it renders even when the hints overflow.

- [ ] **3.6 Confirm footer-content tests still pass.** Run:
```sh
cargo test --lib tui::view::tests::renders_single_mode_footer_hints tui::view::tests::renders_yolo_dialog_and_status_footer -- --nocapture
```
Expected: both pass. `renders_single_mode_footer_hints` uses `(100, 8)` and asserts `type to search` + `Esc clear/quit` are present and `NAV` is absent — all still hold. `renders_yolo_dialog_and_status_footer` uses `(180, 16)` and asserts `parse errors 2`, `pr 1 pending`, `filters agent:claude`, `source unavailable` — all still rendered (width 180 fits everything).

- [ ] **3.7 Commit.** Run:
```sh
git add src/tui/view.rs && git commit -m "$(cat <<'EOF'
feat(tui): keep footer status readable on narrow terminals

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

---

## Task 4 — Body slack constraint `Min(1)` → `Min(0)` (L4)

The vertical body constraint is currently `Constraint::Min(1)`. With the Task 1 guard handling the degenerate case, the body should be the single slack absorber `Min(0)`.

**Files:**
- Modify `src/tui/view.rs:52-59` (vertical layout constraints)
- Test `src/tui/view.rs` (`mod tests`)

- [ ] **4.1 Write the failing test.** Append inside `mod tests`. This asserts that at the minimum allowed height (6 rows), header (1) + footer (1) + body (4) all coexist and the list content renders:
```rust
    #[test]
    fn min_height_keeps_header_body_footer() {
        use crate::enrich::Enricher;
        use std::collections::HashMap;

        let mut app = App::new();
        app.set_preview(false, 50);
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

        let backend = TestBackend::new(80, 6); // exactly the guard floor
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
        // Header query position marker, list row, and footer hint all present.
        assert!(text.contains("/1"), "header count missing: {text:?}");
        assert!(text.contains("fix auth"), "list row missing: {text:?}");
        assert!(text.contains("type to search"), "footer missing: {text:?}");
    }
```

- [ ] **4.2 Run it (expect PASS or FAIL).** Run:
```sh
cargo test --lib tui::view::tests::min_height_keeps_header_body_footer -- --nocapture
```
Expected: this test likely already PASSES with `Min(1)` at height 6, because `Min(1)` and `Min(0)` both absorb the same slack here. It is a regression guard for the constraint change in 4.3 — if it FAILS, the guard threshold (Task 1) or layout is wrong; stop and recheck Task 1 before continuing.

- [ ] **4.3 Minimal implementation.** In `src/tui/view.rs`, in the vertical layout constraints (lines 54-58), change the middle constraint from `Constraint::Min(1)` to `Constraint::Min(0)`:
```rust
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
```
(This block is rewritten to array-destructuring in Task 5; keep the change minimal here.)

- [ ] **4.4 Run it (expect PASS).** Run:
```sh
cargo test --lib tui::view::tests::min_height_keeps_header_body_footer -- --nocapture
```
Expected: pass.

- [ ] **4.5 Commit.** Run:
```sh
git add src/tui/view.rs && git commit -m "$(cat <<'EOF'
refactor(tui): make body the single Min(0) slack absorber

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

---

## Task 5 — Modernize layout idioms to Ratatui 0.30 (L6)

Convert the legacy `Layout::default().direction(..).constraints([..]).split(..)` calls (returning `Rc<[Rect]>` indexed as `chunks[0]`) to 0.30 array-destructuring `let [a, b, c] = Layout::vertical([..]).areas(area);`. Two splits in `render()`: the top vertical split and the inner preview vertical split. Also tidy imports.

**Files:**
- Modify `src/tui/view.rs:5-11` (imports), `52-59` (vertical split), `161-167` (preview inner split), and the `chunks[..]` usages at lines 75, 78-83, 188 (now footer render), 192-198
- Test `src/tui/view.rs` (existing tests are the regression guard)

- [ ] **5.1 Modernize the top vertical split.** Replace the block at `src/tui/view.rs:52-59`:
```rust
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.area());
```
with:
```rust
    let [header_area, body_area, footer_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
```
(`area` is the local bound at the top of `render()` from Task 1: `let area = f.area();`.)

- [ ] **5.2 Replace `chunks[..]` references in `render()`.** Update every remaining `chunks[N]` that refers to this top split:
  - Line 75: `f.render_widget(Paragraph::new(header), chunks[0]);` → `chunks[0]` becomes `header_area`.
  - Lines 78-83 (cursor math): all three `chunks[0]` occurrences → `header_area`.
  - Line 87 (body split, Task 2 code): `chunks[1]` → `body_area` (both the `.width >= PREVIEW_MIN_WIDTH` check and the `.areas(chunks[1])` arg).
  - Lines 94-95 fallback (Task 2 code): `(chunks[1], None)` → `(body_area, None)`.
  - Footer block (Task 3 code): `.areas(chunks[2])` → `.areas(footer_area)`.
  Note: the `header` Line variable (built at lines 67-74) shadows nothing here since we renamed the Rect to `header_area`. The inner-preview block reuses the name `chunks` locally (next step) — that is a separate, nested binding and is fine, but we rename it in 5.3 for clarity.

- [ ] **5.3 Modernize the inner preview vertical split.** Replace the block at `src/tui/view.rs:161-167` (inside the `if let Some(area) = preview_area` arm):
```rust
        let (header_area, transcript_area) =
            if app.preview_header_visible() && selected.is_some() && area.height >= 3 {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(2), Constraint::Min(1)])
                    .split(area);
                (Some(chunks[0]), chunks[1])
            } else {
                (None, area)
            };
```
with:
```rust
        let (preview_header_area, transcript_area) =
            if app.preview_header_visible() && selected.is_some() && area.height >= 3 {
                let [head, body] =
                    Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).areas(area);
                (Some(head), body)
            } else {
                (None, area)
            };
```
Then update the consumer two lines below (currently `if let (Some(header_area), Some(session)) = (header_area, selected) {`) to use the renamed binding:
```rust
        if let (Some(preview_header_area), Some(session)) = (preview_header_area, selected) {
            f.render_widget(
                Paragraph::new(preview_header_lines(session, model.now, model.resolved))
                    .style(Style::default().fg(theme::PREVIEW_TEXT)),
                preview_header_area,
            );
        }
```
(Renamed to `preview_header_area` to avoid colliding with the top-level `header_area` from 5.1. Also bumped the inner `Min(1)` to `Min(0)` for the single-slack idiom — the `area.height >= 3` guard already covers the degenerate case.)

- [ ] **5.4 Tidy imports.** In `src/tui/view.rs:5`, the `Direction` import is now unused (both vertical splits use `Layout::vertical`). Add `Flex` (used by the Task 3 footer). Replace line 5:
```rust
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Position, Rect};
```
with:
```rust
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Position, Rect};
```
Then, in the Task 3 footer code, simplify `.flex(ratatui::layout::Flex::SpaceBetween)` to `.flex(Flex::SpaceBetween)`.
Note: `split_list_area` (lines 201-210) still uses `Layout::default().direction(Direction::Vertical)`. Convert it too so `Direction` can be removed:
```rust
fn split_list_area(area: Rect) -> (Rect, Rect) {
    if area.height == 0 {
        return (area, area);
    }
    let [header, rows] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
    (header, rows)
}
```

- [ ] **5.5 Run the full view module + build (expect PASS).** Run:
```sh
cargo test --lib tui::view::tests -- --nocapture
```
Expected: all pass (this is a pure refactor; behavior is identical). If the compiler reports `unused import: Direction`, confirm step 5.4 removed every `Direction::` usage (the two `render()` splits, `split_list_area`, and check there are no others via `rg -n "Direction" src/tui/view.rs` — expect zero matches).

- [ ] **5.6 Commit.** Run:
```sh
git add src/tui/view.rs && git commit -m "$(cat <<'EOF'
refactor(tui): adopt Ratatui 0.30 array-destructuring layouts

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

---

## Task 6 — Shared `center()` helper + `Block::bordered()` (L7, L9)

Extract a shared `fn center(area: Rect, w: u16, h: u16) -> Rect` using `Flex::Center` on both axes, replacing the hand-rolled centering math in `view.rs` (`render_yolo_modal`, lines 272-277) and `help.rs` (lines 60-65). Also convert `Block::default().borders(Borders::ALL)` to `Block::bordered()` at `view.rs:329-331` and `help.rs:69-71`, dropping the now-unused `Borders` import where applicable.

**Files:**
- Modify `src/tui/view.rs` (add `center`, use it in `render_yolo_modal`, `Block::bordered()`)
- Modify `src/tui/help.rs` (use `view::center`, `Block::bordered()`)
- Test `src/tui/view.rs` (`mod tests`)

- [ ] **6.1 Write the failing test for `center`.** Append inside `mod tests`:
```rust
    #[test]
    fn center_centers_on_both_axes() {
        let area = Rect::new(0, 0, 100, 40);
        let rect = center(area, 20, 10);
        assert_eq!(rect.width, 20);
        assert_eq!(rect.height, 10);
        assert_eq!(rect.x, 40); // (100 - 20) / 2
        assert_eq!(rect.y, 15); // (40 - 10) / 2
    }
```

- [ ] **6.2 Run it (expect FAIL — does not compile).** Run:
```sh
cargo test --lib tui::view::tests::center_centers_on_both_axes -- --nocapture
```
Expected FAIL: compile error `cannot find function center in this scope` because `center` does not exist yet.

- [ ] **6.3 Implement `center`.** Add this `pub` function to `src/tui/view.rs`, directly above `fn render_yolo_modal` (currently line 256):
```rust
/// A `w` x `h` rect centered within `area` on both axes (clamped to `area`).
pub fn center(area: Rect, w: u16, h: u16) -> Rect {
    let [_, mid, _] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(w.min(area.width)),
        Constraint::Fill(1),
    ])
    .flex(Flex::Center)
    .areas(area);
    let [_, rect, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(h.min(area.height)),
        Constraint::Fill(1),
    ])
    .flex(Flex::Center)
    .areas(mid);
    rect
}
```
(`Flex`, `Layout`, `Constraint`, `Rect` are all imported as of Task 5.)

- [ ] **6.4 Run the center test (expect PASS).** Run:
```sh
cargo test --lib tui::view::tests::center_centers_on_both_axes -- --nocapture
```
Expected: pass.

- [ ] **6.5 Use `center` in `render_yolo_modal`.** In `src/tui/view.rs`, replace the hand-rolled rect at lines 272-277:
```rust
    let rect = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
```
with:
```rust
    let rect = center(area, w, h);
```

- [ ] **6.6 Convert the yolo modal block to `Block::bordered()`.** In `render_yolo_modal`, replace the block at lines 329-331:
```rust
                Block::default()
                    .borders(Borders::ALL)
                    .title(" confirm resume "),
```
with:
```rust
                Block::bordered().title(" confirm resume "),
```

- [ ] **6.7 Run the yolo render test (expect PASS).** Run:
```sh
cargo test --lib tui::view::tests::renders_yolo_dialog_and_status_footer -- --nocapture
```
Expected: pass — `confirm resume` title and bordered modal still render identically.

- [ ] **6.8 Use `center` and `Block::bordered()` in `help.rs`.** In `src/tui/help.rs`, replace the hand-rolled rect at lines 60-65:
```rust
    let rect = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
```
with:
```rust
    let rect = crate::tui::view::center(area, w, h);
```
Then replace the block at lines 69-71:
```rust
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::ACCENT))
```
with:
```rust
    let block = Block::bordered()
        .border_style(Style::default().fg(theme::ACCENT))
```

- [ ] **6.9 Tidy `help.rs` imports.** `Borders` is now unused in `help.rs`, and `Rect` is no longer constructed directly (but `center` takes/returns `Rect` and `area: Rect` is still annotated — `Rect` is still referenced via the `f.area()` return only implicitly; keep it). Update `src/tui/help.rs:7`:
```rust
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph};
```
to:
```rust
use ratatui::widgets::{Block, Clear, Padding, Paragraph};
```
Note: `Rect` on line 4 (`use ratatui::layout::{Alignment, Rect};`) is no longer used after removing the struct literal — verify with `rg -n "Rect" src/tui/help.rs`. If zero matches remain, change line 4 to `use ratatui::layout::Alignment;`. If `Rect` still appears, leave the import.

- [ ] **6.10 Check `Borders` in `view.rs`.** After 6.6, `Borders` may still be used in `view.rs` for the preview block (`Block::default().borders(Borders::LEFT)` at line 153) — that is a single-side border and stays as-is (`Block::bordered()` is all-sides only). So keep `Borders` in the `view.rs` import. Verify with `rg -n "Borders" src/tui/view.rs` — expect the `Borders::LEFT` usage to remain.

- [ ] **6.11 Run both modules + help tests (expect PASS).** Run:
```sh
cargo test --lib tui::view::tests tui::help::tests -- --nocapture
```
Expected: all pass.

- [ ] **6.12 Commit.** Run:
```sh
git add src/tui/view.rs src/tui/help.rs && git commit -m "$(cat <<'EOF'
refactor(tui): share Flex::Center helper and use Block::bordered

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

---

## Task 7 — Final verification

**Files:** none (verification only)

- [ ] **7.1 Run the full library test suite.** Run:
```sh
cargo test --lib
```
Expected: all tests pass, including every `tui::view::tests`, `tui::help::tests`, and `tui::tests` case.

- [ ] **7.2 Run the full test suite (integration tests included).** Run:
```sh
cargo test
```
Expected: all pass. (Integration tests under `tests/` do not touch the render path but confirm nothing else regressed.)

- [ ] **7.3 Run clippy with warnings as errors.** Run:
```sh
cargo clippy --all-targets -- -D warnings
```
Expected: no warnings, no errors. Common things to fix if it complains: leftover unused imports (`Direction`, `Borders` in `help.rs`, `Rect` in `help.rs`), or `needless_borrow` on `display_width(&text)`.

- [ ] **7.4 Final commit if clippy required fixes.** Only if 7.3 produced edits, run:
```sh
git add -A && git commit -m "$(cat <<'EOF'
chore(tui): satisfy clippy after responsiveness refactor

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```
Otherwise skip.

---

## Summary of changes

| Finding | Fix | Task |
|---------|-----|------|
| L1 | Tiny-terminal guard (`width < 30 \|\| height < 6`) at top of `render()` | 1 |
| L2 | Drop preview below 100 cols; floor list at `Min(48)` | 2 |
| L3 | Split footer with `Flex::SpaceBetween`; right-align volatile status so it survives clipping | 3 |
| L4 | Body constraint `Min(1)` → `Min(0)` | 4 |
| L6 | Array-destructuring `Layout::vertical/horizontal(..).areas(..)` | 5 |
| L7 | `Block::bordered()` in `view.rs` (yolo) and `help.rs` | 6 |
| L9 | Shared `center(area, w, h)` via `Flex::Center` | 6 |
