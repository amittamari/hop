# TUI Keybindings Single Source of Truth Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** Collapse the three independent, drift-prone copies of the `hop` TUI keybindings — the key→command decoding (`keymap.rs`), the hand-typed help overlay text (`help.rs`), and the footer hint string (`view.rs` `FOOTER_HINTS`) — into ONE declarative source of truth: a static `&[Binding]` table. Drive the help overlay and the footer from that table, align the help columns programmatically (no hand-counted spaces), teach the footer the preview vocabulary when the preview pane is visible and the terminal is wide, and add tests that fail loudly when a binding is added to the table but not actually handled in `handle_key`.

**Architecture:** The `Binding` struct and `bindings()` accessor live in `src/tui/keymap.rs`. That module already owns key semantics (`Command`, `control_chord_action`) and carries the "single live-search interaction model, no modes" doc comment, so it is the natural home for the canonical key catalog. `help.rs` and `view.rs` import `bindings()` and render from it; neither hand-types key labels anymore. This is also a step toward the reserved `[keybindings]` config table noted as **P-001** in `docs/ARCHITECTURE.md` (config-driven rebinding is explicitly NOT in scope here, but a single declarative table is the prerequisite).

**Tech Stack:** Rust, Ratatui 0.30, crossterm (via `ratatui::crossterm`).

---

## Background: the current state (verified against code)

This is a deliberately **single-mode, search-first** TUI. Typing always edits the query; arrows always move the list selection; preview actions are Ctrl-chords. There are NO focusable panes and NO vim-style nav (`j`/`k` are query characters by design — see `keymap.rs:1-3` doc comment). **Do not add modes.** This plan only deduplicates the binding *catalog*; it changes no key semantics.

The three current sources, verified:

1. **`src/tui/keymap.rs:23-33`** — `control_chord_action` maps Ctrl-chords to `Command`:
   - `Ctrl+C` → `Quit`, `Ctrl+P` → `TogglePreview`, `Ctrl+U` → `ScrollPreview(-1)`, `Ctrl+D` → `ScrollPreview(1)`, `Ctrl+B` → `JumpPreviewMatch(-1)`, `Ctrl+N` → `JumpPreviewMatch(1)`, `Ctrl+Left` → `ResizePreview(-1)`, `Ctrl+Right` → `ResizePreview(1)`.
   - The doc comment (lines 16-18) notes line-editing chords `Ctrl+A`/`Ctrl+E`/`Ctrl+W` are handled directly in `App::handle_key`, not here.

2. **`src/tui/mod.rs:177-301`** — `App::handle_key(&mut self, key: KeyEvent) -> Action`. The `Action` enum (`mod.rs:11-24`) is `None | Quit | Search | Resume { index, yolo }`. The main `match key.code` (lines 214-300) handles `Esc`, `Down`, `Up`, `PageDown`, `PageUp`, `Enter`, `Tab`, `Left`, `Right`, `Home`, `End`, `Delete`, `Backspace`, `Ctrl+A`, `Ctrl+E`, `Ctrl+W`, `?`, and the catch-all `Char(c)` (types into query) plus `_ => Action::None`. The yolo-modal branch (lines 193-209) returns early. `Ctrl+C` short-circuits at line 182. Help-overlay branch is lines 186-191.

3. **`src/tui/help.rs:10-37`** — `lines()` hand-types every row with hand-counted leading-space padding (the H4 alignment smell). The substring guard test is at `help.rs:92-112`.

4. **`src/tui/view.rs:212`** — `const FOOTER_HINTS: &str = "type to search · ↑↓ move · Enter resume · ? help · Esc clear/quit";`, consumed by `footer_line` at `view.rs:214-254`.

### The canonical table this plan introduces

One entry per user-facing row in the help overlay. Fields:
- `keys`: display label for the key(s), e.g. `"Ctrl+P"`, `"↑/↓"`, `"Ctrl+←/→"`.
- `group`: the section heading, one of `"Navigation"`, `"Preview"`, `"Search Editing"`, `"Actions"`.
- `label`: the human description, e.g. `"toggle preview"`.
- `primary: bool`: true for the small subset shown in the main-view footer.

---

## Dependencies & Sequencing

- **Tasks 1, 2, 4 (table + help overlay + reachability test) are independent** of any other plan. They touch only `keymap.rs` and `help.rs`.
- **Task 3 (footer preview hints, finding H2) overlaps with the responsiveness plan** `docs/plans/2026-06-13-tui-responsiveness.md` (finding L3, its Task 3). **The responsiveness plan OWNS the footer layout** — it splits `footer_line` into `footer_hints_line()` + `footer_status_line()` and renders them with `Flex::SpaceBetween` so the volatile status survives narrow-width clipping. This bindings plan must NOT re-architect that layout.
  - **Preferred path — responsiveness has landed:** verify `footer_hints_line()` exists in `view.rs`. Build the hints line from `bindings()` (primary subset) instead of the `FOOTER_HINTS` const, and append preview-group hints when the preview is visible and width allows. The width budget is already handled by `Flex::SpaceBetween` + truncation; this plan only contributes hint CONTENT.
  - **Fallback path — responsiveness has NOT landed:** `footer_line(status)` is still the single un-truncated function at `view.rs:214-254`. This plan must add minimal width-budgeting itself: pass the footer width into the hint builder and drop trailing preview hints that would not fit, so hints DEGRADE rather than clip. Task 3 describes both paths; pick the one matching the current tree and skip the other.
- **Recommended order:** land responsiveness first, then this plan. If executing this plan standalone, use the fallback path in Task 3.
- Verify which path applies before starting Task 3:

```sh
rg -n "footer_hints_line|footer_status_line|Flex::SpaceBetween" src/tui/view.rs
```

If those symbols exist → preferred path. If only `footer_line` / `FOOTER_HINTS` exist → fallback path.

---

## Task 1 — Introduce the canonical `Binding` table in `keymap.rs`

Define `Binding`, populate `bindings()` from the current keymap + help, and assert the table is well-formed. No consumers change yet (help and footer still hand-type; Tasks 2-3 migrate them). This task is pure addition.

**Files:**
- `src/tui/keymap.rs` (add `Binding`, `bindings()`, and a test)

Steps:

- [ ] **1.1 Write a failing test for the table shape.** Append to the `mod tests` block in `src/tui/keymap.rs` (the block starts at line 36; add before its closing `}` at line 85):

```rust
    #[test]
    fn bindings_table_is_well_formed() {
        let table = bindings();
        assert!(!table.is_empty(), "bindings table must not be empty");
        for b in table {
            assert!(!b.keys.is_empty(), "binding keys must be non-empty");
            assert!(!b.label.is_empty(), "binding label must be non-empty");
            assert!(!b.group.is_empty(), "binding group must be non-empty");
        }
        // At least one binding is flagged primary (footer subset).
        assert!(table.iter().any(|b| b.primary), "need at least one primary binding");
    }
```

- [ ] **1.2 Run it (expected FAIL — `Binding`/`bindings` undefined).**

```sh
cargo test --lib tui::keymap::tests::bindings_table_is_well_formed -- --nocapture
```

Expected: compile error, `cannot find function bindings` / `cannot find type Binding`.

- [ ] **1.3 Minimal implementation — add the struct and the static table.** Insert into `src/tui/keymap.rs` immediately after the `Command` enum (after line 14, before the `control_chord_action` doc comment at line 16):

```rust
/// One user-facing keybinding row. This is the single source of truth that
/// drives both the help overlay and the main-view footer hints, so adding a
/// binding here surfaces it everywhere without hand-editing strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    /// Display label for the key(s), e.g. "Ctrl+P", "↑/↓", "Ctrl+←/→".
    pub keys: &'static str,
    /// Section heading the row belongs to.
    pub group: &'static str,
    /// Human-readable description of what the key does.
    pub label: &'static str,
    /// Whether this row appears in the compact main-view footer.
    pub primary: bool,
}

/// The canonical keybinding catalog. Ordered for display: rows are grouped by
/// `group` in the order they first appear. Keep this in sync with
/// `control_chord_action` above and `App::handle_key`; the reachability test in
/// `tui::view` / `tui::mod` guards against drift.
pub fn bindings() -> &'static [Binding] {
    const TABLE: &[Binding] = &[
        // Navigation
        Binding { keys: "↑/↓",       group: "Navigation",     label: "move selection",       primary: true },
        Binding { keys: "PgUp/PgDn", group: "Navigation",     label: "page list",            primary: false },
        Binding { keys: "Ctrl+U/D",  group: "Navigation",     label: "scroll preview",       primary: false },
        Binding { keys: "Ctrl+N/B",  group: "Navigation",     label: "preview matches",      primary: false },
        // Preview
        Binding { keys: "Ctrl+P",    group: "Preview",        label: "toggle preview",       primary: false },
        Binding { keys: "Ctrl+←/→",  group: "Preview",        label: "resize preview",       primary: false },
        // Search Editing
        Binding { keys: "←/→",       group: "Search Editing", label: "move cursor",          primary: false },
        Binding { keys: "Home/End",  group: "Search Editing", label: "jump cursor",          primary: false },
        Binding { keys: "Backspace", group: "Search Editing", label: "delete left",          primary: false },
        Binding { keys: "Delete",    group: "Search Editing", label: "delete at cursor",     primary: false },
        Binding { keys: "Ctrl+A/E",  group: "Search Editing", label: "start / end",          primary: false },
        Binding { keys: "Ctrl+W",    group: "Search Editing", label: "delete word",          primary: false },
        // Actions
        Binding { keys: "type",      group: "Actions",        label: "search",               primary: true },
        Binding { keys: "Enter",     group: "Actions",        label: "resume",               primary: true },
        Binding { keys: "Tab",       group: "Actions",        label: "autocomplete keyword", primary: false },
        Binding { keys: "?",         group: "Actions",        label: "toggle help",          primary: true },
        Binding { keys: "Esc",       group: "Actions",        label: "clear query / quit",   primary: true },
        Binding { keys: "Ctrl+C",    group: "Actions",        label: "quit",                 primary: false },
    ];
    TABLE
}
```

> Note: the `keys: "type"` row models the footer's `type to search` hint and the implicit "any printable char types into the query" behavior. It is intentionally NOT a single key; the reachability test (Task 4) special-cases it.

- [ ] **1.4 Run it (expected PASS).**

```sh
cargo test --lib tui::keymap::tests::bindings_table_is_well_formed -- --nocapture
```

- [ ] **1.5 Commit.**

```sh
git add -A && git commit -m "$(cat <<'EOF'
refactor(tui): add canonical Binding table in keymap

Introduce a single declarative source of truth for keybindings. No consumers
migrated yet; help overlay and footer still hand-type (Tasks 2-3).

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

---

## Task 2 — Drive the help overlay from `bindings()` (fixes H1 + H4)

Replace the hand-typed `help::lines()` with a renderer that groups `bindings()`, pads the key column to the max key width programmatically (fixing the H4 hand-counted alignment), and emits each row as two `Span`s. Replace the brittle substring guard test with one that asserts every binding's label is present (so it can no longer drift).

**Files:**
- `src/tui/help.rs` (rewrite `lines()`, update tests)

Steps:

- [ ] **2.1 Write a failing test asserting help renders every binding from the table.** Replace the existing `help_lists_core_bindings` test body in `src/tui/help.rs` (lines 92-112) with:

```rust
    fn rendered_text() -> String {
        lines()
            .iter()
            .map(|x| {
                x.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn help_lists_every_binding_from_table() {
        let text = rendered_text();
        for b in crate::tui::keymap::bindings() {
            assert!(
                text.contains(b.label),
                "help overlay missing binding label {:?}",
                b.label
            );
            // The "type" pseudo-key has no literal key column in help.
            if b.keys != "type" {
                assert!(
                    text.contains(b.keys),
                    "help overlay missing binding keys {:?}",
                    b.keys
                );
            }
        }
        // Group headings still render.
        assert!(text.contains("Navigation"));
        assert!(text.contains("Preview"));
        assert!(text.contains("Search Editing"));
        assert!(text.contains("Actions"));
        // The removed modal keymap and its toggles stay gone.
        assert!(!text.to_lowercase().contains("modal"));
        assert!(!text.contains("Ctrl+Y"));
    }

    #[test]
    fn help_key_column_is_aligned() {
        // Every non-heading, non-blank row pads the key column to a constant
        // width, so the label column starts at the same offset on every line.
        let key_w = crate::tui::keymap::bindings()
            .iter()
            .filter(|b| b.keys != "type")
            .map(|b| b.keys.chars().count())
            .max()
            .unwrap();
        let body = lines();
        let mut checked = 0usize;
        for line in &body {
            // Rows rendered by the table have exactly two spans: key + label.
            if line.spans.len() == 2 {
                let key_span = line.spans[0].content.as_ref();
                assert_eq!(
                    key_span.chars().count(),
                    // leading "  " indent + padded key column + trailing "  "
                    2 + key_w + 2,
                    "key column not padded to constant width: {key_span:?}"
                );
                checked += 1;
            }
        }
        assert!(checked > 0, "expected at least one table row");
    }
```

- [ ] **2.2 Run it (expected FAIL).**

```sh
cargo test --lib tui::help::tests -- --nocapture
```

Expected: `help_lists_every_binding_from_table` fails on `keys: "type"` not having a label match (the old hand-typed text has no `search` row), and `help_key_column_is_aligned` fails because old rows are single-span hand-padded strings.

- [ ] **2.3 Minimal implementation — rewrite `lines()` to render from the table.** Replace the entire `lines()` function in `src/tui/help.rs` (lines 10-37) with:

```rust
pub fn lines() -> Vec<Line<'static>> {
    let table = crate::tui::keymap::bindings();
    // Pad the key column to the widest key label (skipping the "type"
    // pseudo-key, which is shown as prose, not a key chord). This replaces the
    // old hand-counted leading spaces.
    let key_w = table
        .iter()
        .filter(|b| b.keys != "type")
        .map(|b| b.keys.chars().count())
        .max()
        .unwrap_or(0);

    // Distinct groups, in first-seen order.
    let mut groups: Vec<&'static str> = Vec::new();
    for b in table {
        if !groups.contains(&b.group) {
            groups.push(b.group);
        }
    }

    let mut out: Vec<Line<'static>> = Vec::new();
    for (gi, group) in groups.iter().enumerate() {
        if gi > 0 {
            out.push(Line::from(""));
        }
        out.push(section(group));
        for b in table.iter().filter(|b| &b.group == group) {
            if b.keys == "type" {
                // Prose row: "type to <label>", no key chord column.
                out.push(Line::from(format!("  type to {}", b.label)));
                continue;
            }
            let key_col = format!("  {:<width$}  ", b.keys, width = key_w);
            out.push(Line::from(vec![
                Span::styled(key_col, Style::default().fg(theme::ACCENT)),
                Span::raw(b.label.to_string()),
            ]));
        }
    }
    out
}
```

> Note: `format!("{:<width$}", b.keys, width = key_w)` pads by `char` count via the fill spec; the multibyte arrows (`↑`, `←`) each count as one `char`, matching the test's `chars().count()`. The `key_w` row width assertion in 2.1 (`2 + key_w + 2`) matches the `"  "` + padded + `"  "` layout here. The `keys: "type"` prose row produces a single span, so it is excluded from the alignment assertion (which only checks two-span rows).

- [ ] **2.4 Run it (expected PASS).**

```sh
cargo test --lib tui::help::tests -- --nocapture
```

- [ ] **2.5 Verify the rendered overlay still draws (TestBackend smoke check).** Add this test to the `mod tests` block in `src/tui/help.rs`:

```rust
    #[test]
    fn overlay_renders_labels_into_buffer() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let backend = TestBackend::new(64, 24);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| render(f)).unwrap();
        let text: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("toggle preview"));
        assert!(text.contains("resume"));
        assert!(text.contains("help"));
    }
```

- [ ] **2.6 Run it (expected PASS).**

```sh
cargo test --lib tui::help::tests::overlay_renders_labels_into_buffer -- --nocapture
```

- [ ] **2.7 Commit.**

```sh
git add -A && git commit -m "$(cat <<'EOF'
refactor(tui): render help overlay from bindings table

Help rows are now generated from keymap::bindings() with the key column padded
programmatically, removing hand-typed text (H1) and hand-counted alignment (H4).

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

---

## Task 3 — Drive footer hints from `bindings()` and teach preview vocabulary (fixes H1 footer + H2)

The footer must (a) build its compact hints from the `primary` subset of `bindings()` instead of the `FOOTER_HINTS` const, and (b) when the preview is visible AND the terminal is wide enough, append the preview-group hints (`Ctrl+P toggle`, scroll, match nav, resize) so users discover the preview vocabulary that the on-by-default preview otherwise hides.

**Determine which path applies first** (see Dependencies & Sequencing):

```sh
rg -n "footer_hints_line|footer_status_line|Flex::SpaceBetween" src/tui/view.rs
```

### Path A — responsiveness landed (`footer_hints_line` exists)

**Files:**
- `src/tui/view.rs` (replace the hints-line body to source from `bindings()`, add preview hints; the `Flex::SpaceBetween` layout already truncates)

Steps:

- [ ] **3A.1 Write a failing test.** Add to `mod tests` in `src/tui/view.rs`. This uses the existing render harness (see `renders_single_mode_footer_hints` at lines 638-677). Render once wide with preview ON, assert a preview hint appears; render once narrow, assert it is dropped (degrades, not clips):

```rust
    fn footer_text(width: u16, preview: bool) -> String {
        use crate::enrich::Enricher;
        use std::collections::HashMap;

        let mut app = App::new();
        app.set_preview(preview, 50);
        let enr: Vec<Box<dyn Enricher>> = vec![];
        let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
        let cols = crate::columns::default_columns();
        let backend = TestBackend::new(width, 8);
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
        term.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn footer_teaches_preview_vocabulary_when_wide() {
        let wide = footer_text(160, true);
        assert!(wide.contains("type to search"), "primary hints still shown");
        assert!(
            wide.contains("Ctrl+P") || wide.contains("toggle preview"),
            "wide footer with preview on must teach preview vocabulary: {wide:?}"
        );
    }

    #[test]
    fn footer_drops_preview_hints_when_narrow() {
        let narrow = footer_text(60, true);
        assert!(
            !narrow.contains("Ctrl+P"),
            "narrow footer must degrade by dropping preview hints, not clip: {narrow:?}"
        );
    }
```

- [ ] **3A.2 Run it (expected FAIL).** `cargo test --lib tui::view::tests::footer_teaches_preview_vocabulary_when_wide -- --nocapture` — fails because `footer_hints_line` still uses the static `FOOTER_HINTS` const with no preview hints.

- [ ] **3A.3 Minimal implementation — source hints from `bindings()` and append preview hints.** Replace the body of `footer_hints_line()` (built by the responsiveness plan). It currently takes no preview/width info; change its signature to `footer_hints_line(preview_visible: bool, width: u16)` and update the single caller in `render`. New body:

```rust
/// Static, low-priority hints shown on the left of the footer, built from the
/// canonical bindings table. When the preview is visible and the terminal is
/// wide enough, preview-vocabulary hints are appended; they degrade (drop
/// trailing) instead of clipping.
fn footer_hints_line(preview_visible: bool, width: u16) -> Line<'static> {
    use crate::tui::keymap;

    // Primary hints: "type to search · ↑/↓ move · Enter resume · ? help · Esc clear / quit".
    let primary: Vec<String> = keymap::bindings()
        .iter()
        .filter(|b| b.primary)
        .map(|b| {
            if b.keys == "type" {
                format!("type to {}", b.label)
            } else {
                format!("{} {}", b.keys, b.label)
            }
        })
        .collect();

    // Preview vocabulary, only when the pane is visible and width permits.
    let preview: Vec<String> = if preview_visible {
        keymap::bindings()
            .iter()
            .filter(|b| b.group == "Preview" || b.label == "scroll preview" || b.label == "preview matches")
            .map(|b| format!("{} {}", b.keys, b.label))
            .collect()
    } else {
        Vec::new()
    };

    let mut spans = Vec::new();
    let mut used = 0usize;
    let cap = width as usize;

    // Primary hints always render (they are the compact baseline).
    push_hint_group(&mut spans, &mut used, &primary, theme::ACCENT, true);

    // Append preview hints while they fit, dropping the rest (degrade, not clip).
    for hint in &preview {
        let cost = 3 + hint.chars().count(); // " · " + hint
        if used + cost > cap {
            break;
        }
        used += cost;
        spans.push(Span::styled(format!(" · {hint}"), Style::default().fg(theme::DIM)));
    }

    Line::from(spans)
}

/// Render a `·`-joined group of hints. The first span uses `head_style`; the
/// rest are DIM. Tracks display width into `used`.
fn push_hint_group(
    spans: &mut Vec<Span<'static>>,
    used: &mut usize,
    hints: &[String],
    head_color: ratatui::style::Color,
    head_bold: bool,
) {
    for (i, hint) in hints.iter().enumerate() {
        if i == 0 {
            *used += hint.chars().count();
            let mut style = Style::default().fg(head_color);
            if head_bold {
                style = style.add_modifier(Modifier::BOLD);
            }
            spans.push(Span::styled(hint.clone(), style));
        } else {
            *used += 3 + hint.chars().count();
            spans.push(Span::styled(
                format!(" · {hint}"),
                Style::default().fg(theme::DIM),
            ));
        }
    }
}
```

Update the caller inside `render` (the footer render call built by the responsiveness plan, near `view.rs:187`) to pass `app.preview_visible()` and the footer chunk width, e.g.:

```rust
    let hints = footer_hints_line(app.preview_visible(), chunks[2].width);
```

Remove the now-unused `const FOOTER_HINTS` if Path A's `footer_hints_line` was its only consumer.

> Note: `push_hint_group` budgets primary hints unconditionally (they are the baseline contract). The width cap only gates the OPTIONAL preview hints, satisfying "degrade trailing, not clip." If the responsiveness plan's `footer_hints_line` already truncates the whole left region via `Flex`, the cap here is belt-and-suspenders and harmless — the `Flex` split still owns the layout.

- [ ] **3A.4 Run it (expected PASS).** `cargo test --lib tui::view::tests::footer -- --nocapture`

- [ ] **3A.5 Confirm existing footer/status tests still pass** (`renders_single_mode_footer_hints`, `renders_yolo_dialog_and_status_footer`):

```sh
cargo test --lib tui::view::tests -- --nocapture
```

If `renders_single_mode_footer_hints` (lines 638-677) asserts `text.contains("type to search")` and `text.contains("Esc clear/quit")` — note the table renders `Esc clear / quit` (with spaces, matching the `label: "clear query / quit"` → footer `Esc clear / quit`). Update that assertion to `assert!(text.contains("Esc"))` and `assert!(text.contains("clear"))` if it breaks, OR set the Esc binding's footer rendering to match. Prefer adjusting the test's substring to `"clear"` to avoid coupling to exact spacing.

- [ ] **3A.6 Commit.**

```sh
git add -A && git commit -m "$(cat <<'EOF'
feat(tui): footer hints from bindings table; teach preview vocabulary

Footer primary hints now derive from keymap::bindings() (H1). When the preview
pane is visible and the terminal is wide, preview-vocabulary hints are appended
and degrade gracefully on narrow widths (H2).

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

### Path B — responsiveness NOT landed (`footer_line` is the single function at `view.rs:214-254`)

**Files:**
- `src/tui/view.rs` (rewrite `footer_line` to take preview/width, source from `bindings()`, self-budget)

Steps:

- [ ] **3B.1 Write the same failing tests as 3A.1** (`footer_text` helper, `footer_teaches_preview_vocabulary_when_wide`, `footer_drops_preview_hints_when_narrow`). Add to `mod tests` in `view.rs`.

- [ ] **3B.2 Run them (expected FAIL).** `cargo test --lib tui::view::tests::footer_teaches_preview_vocabulary_when_wide -- --nocapture`

- [ ] **3B.3 Minimal implementation — change `footer_line`'s signature and source from the table.** Replace `footer_line(status: &StatusLine)` (lines 214-254). New signature: `footer_line(status: &StatusLine, preview_visible: bool, width: u16)`. Build the leading hint spans from `bindings()` primary subset + preview hints (reuse the `push_hint_group` helper and the preview-hint budgeting loop from Path A 3A.3), THEN append the existing `status.sync` / `pr_pending` / `filters` / `warning` spans exactly as they are today (lines 229-252, unchanged). Delete the `FOOTER_HINTS` const (line 212) and its `split_once` derivation (lines 216-228).

Update the caller at `view.rs:188`:

```rust
    f.render_widget(
        Paragraph::new(footer_line(model.status, app.preview_visible(), chunks[2].width)),
        chunks[2],
    );
```

Budget rule for the self-truncating fallback: count display width of primary hints + each appended preview hint; stop appending preview hints once `used + cost > width`. Do not budget the status spans — they are high priority and the responsiveness plan will later own their survival; here they simply append after hints.

- [ ] **3B.4 Run it (expected PASS).** `cargo test --lib tui::view::tests::footer -- --nocapture`

- [ ] **3B.5 Fix `renders_single_mode_footer_hints` substring** if needed (same note as 3A.5: change `"Esc clear/quit"` assertion to `"clear"`). Confirm `renders_yolo_dialog_and_status_footer` still passes (status spans unchanged).

```sh
cargo test --lib tui::view::tests -- --nocapture
```

- [ ] **3B.6 Commit** (same message as 3A.6).

---

## Task 4 — Reachability test: every binding is actually handled

Add a test that maps each `Binding` to a representative `KeyEvent`, feeds it through `App::handle_key`, and asserts it does NOT fall into the no-op catch-all — i.e. it either returns a non-`None` `Action` or mutates observable state (selection, query, cursor, preview, help, modal). This is the guard that catches "added to the table but never wired up" drift. Also encode the H3 decision: `?` inside the yolo modal is an intentional no-op.

**Files:**
- `src/tui/mod.rs` (add tests to the existing `mod tests` block, lines 447-699)

Steps:

- [ ] **4.1 Write the failing reachability test.** Add to `mod tests` in `src/tui/mod.rs`:

```rust
    /// Map a Binding's display key label to a representative KeyEvent.
    /// Returns None for the "type" pseudo-binding (tested separately).
    fn binding_event(keys: &str) -> Option<KeyEvent> {
        use KeyCode::*;
        let ctrl = KeyModifiers::CONTROL;
        let none = KeyModifiers::NONE;
        let ev = |code, m| KeyEvent::new(code, m);
        Some(match keys {
            "↑/↓" => ev(Up, none),
            "PgUp/PgDn" => ev(PageDown, none),
            "Ctrl+U/D" => ev(Char('u'), ctrl),
            "Ctrl+N/B" => ev(Char('n'), ctrl),
            "Ctrl+P" => ev(Char('p'), ctrl),
            "Ctrl+←/→" => ev(Left, ctrl),
            "←/→" => ev(Left, none),
            "Home/End" => ev(Home, none),
            "Backspace" => ev(Backspace, none),
            "Delete" => ev(Delete, none),
            "Ctrl+A/E" => ev(Char('a'), ctrl),
            "Ctrl+W" => ev(Char('w'), ctrl),
            "Enter" => ev(Enter, none),
            "Tab" => ev(Tab, none),
            "?" => ev(Char('?'), none),
            "Esc" => ev(Esc, none),
            "Ctrl+C" => ev(Char('c'), ctrl),
            "type" => return None,
            other => panic!("binding key {other:?} has no representative event mapping"),
        })
    }

    /// Snapshot of all observable App state a binding could plausibly change.
    fn state_snapshot(app: &App) -> (usize, String, usize, bool, u16, u16, bool, bool) {
        (
            app.selected(),
            app.query().to_string(),
            app.query_cursor(),
            app.preview_visible(),
            app.preview_width_pct(),
            app.preview_scroll(),
            app.help_open(),
            app.modal_open(),
        )
    }

    #[test]
    fn every_binding_is_handled() {
        for b in crate::tui::keymap::bindings() {
            let Some(ev) = binding_event(b.keys) else {
                continue; // "type" tested in `typing_updates_query_and_requests_search`
            };
            // Fresh app per binding; populated + yolo-supported so Enter has work.
            let mut app = app_with(3);
            // Give some query + preview matches so editing/match-nav chords act.
            for c in "agent:cl".chars() {
                app.handle_key(key(KeyCode::Char(c)));
            }
            app.set_preview_matches(vec![1, 5]);
            let before = state_snapshot(&app);
            let action = app.handle_key(ev);
            let after = state_snapshot(&app);
            let did_something = action != Action::None || before != after;
            assert!(
                did_something,
                "binding {:?} ({:?}) fell into the no-op arm: no Action and no state change",
                b.keys, b.label
            );
        }
    }
```

> Why this design: a binding that is in the table but unhandled hits `_ => Action::None` in `handle_key` with no state mutation, so `did_something` is false and the test names the offender. The `app_with(3)` + non-empty query + preview matches setup ensures every legitimate binding has observable work (e.g. `Esc` clears the query → `Search` + query change; `Up` moves selection or is a no-op only when already at top — note `app_with(3)` starts at `selected==0`, so use `Down`-direction representatives where a top-of-list key would no-op). The chosen reps (`Up`, `Left`, `Home`, `Backspace`, `Delete`, `Ctrl+A`) all act given the seeded query/selection: `Up` from selected 0 is the one risk — see 4.2.

- [ ] **4.2 Run it (expected: likely FAIL on `↑/↓` only).**

```sh
cargo test --lib tui::tests::every_binding_is_handled -- --nocapture
```

If it fails on `↑/↓`: `Up` at `selected==0` saturates to 0 and only zeroes `preview_scroll` (which is already 0 after the last query edit), so no state change. Fix by moving selection down first in the test setup so `Up` has somewhere to go. Add before the snapshot, inside the loop, right after `set_preview_matches`:

```rust
            app.handle_key(key(KeyCode::Down)); // ensure Up/PageUp have room to move
```

(`Down` advances `selected` to 1; then `Up`/`PageUp` reps move it back, `preview_scroll` resets — observable.) Re-run.

- [ ] **4.3 Confirm PASS.**

```sh
cargo test --lib tui::tests::every_binding_is_handled -- --nocapture
```

If any binding OTHER than the expected ones fails, that is a real drift signal: the table lists a key that `handle_key` does not act on. Fix the table or the handler, not the test.

- [ ] **4.4 Write the H3 test: `?` is an intentional no-op inside the yolo modal.** Add to `mod tests`:

```rust
    /// H3 decision (documented): the yolo confirm modal owns its own inline
    /// legend ("Tab toggles yolo · Enter resumes · Esc cancels"). `?` is NOT
    /// routed to help from the modal — it intentionally does nothing. The
    /// global footer's "? help" applies to the main view only.
    #[test]
    fn question_is_noop_inside_yolo_modal() {
        let mut app = app_with(1);
        app.open_yolo_modal();
        assert!(app.modal_open());
        let action = app.handle_key(key(KeyCode::Char('?')));
        assert_eq!(action, Action::None);
        assert!(app.modal_open(), "? must not close or change the modal");
        assert!(!app.help_open(), "? must not open help from the modal");
    }
```

- [ ] **4.5 Run it (expected PASS — this documents current behavior; the modal branch at `mod.rs:193-209` already returns `Action::None` for `?`).**

```sh
cargo test --lib tui::tests::question_is_noop_inside_yolo_modal -- --nocapture
```

- [ ] **4.6 Commit.**

```sh
git add -A && git commit -m "$(cat <<'EOF'
test(tui): bindings reachability guard; document ? no-op in modal

Every Binding in the table is mapped through handle_key and asserted to either
return an Action or mutate state, catching table/handler drift. Also documents
the H3 decision: ? is an intentional no-op inside the yolo modal.

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

---

## Task 5 — Final verification

**Files:** none (verification only)

Steps:

- [ ] **5.1 Run the full lib test suite (expected: all PASS).**

```sh
cargo test --lib
```

- [ ] **5.2 Run integration tests if present (expected: all PASS).**

```sh
cargo test
```

- [ ] **5.3 Run clippy (expected: no warnings).**

```sh
cargo clippy --all-targets -- -D warnings
```

If clippy flags the new `bindings()` const slice or the `push_hint_group` arg count, address minimally (e.g. allow `clippy::too_many_arguments` on `push_hint_group` only if it genuinely trips; prefer simplifying first).

- [ ] **5.4 Manual smoke (optional but recommended).** Build and eyeball the help overlay alignment and the footer's preview hints:

```sh
cargo run -- --rebuild
```

Press `?` to confirm columns align and every group renders; with the preview visible on a wide terminal, confirm the footer shows preview hints; shrink the terminal and confirm they drop rather than clip.

- [ ] **5.5 Final commit if any verification fixes were made** (otherwise skip). Conventional message, e.g. `fix(tui): satisfy clippy on bindings footer helpers`, with the `Co-Authored-By: Claude <noreply@anthropic.com>` trailer.

---

## Done criteria

- One `bindings()` table in `keymap.rs` is the only place keybinding labels are authored.
- Help overlay renders entirely from the table with programmatic column alignment (H1, H4).
- Footer compact hints derive from the table's `primary` subset, and preview vocabulary appears when the preview is visible and width allows, degrading on narrow widths (H1 footer, H2).
- A reachability test fails if a binding is added to the table but not handled in `handle_key`.
- `?`-in-modal is documented as an intentional no-op (H3).
- `cargo test --lib` and `cargo clippy` pass clean.
