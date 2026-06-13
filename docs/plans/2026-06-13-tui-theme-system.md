# TUI Theme System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** Replace the flat `const`-bag in `src/tui/theme.rs` with a `struct Theme` of semantic roles (internal only, hardcoded default — NO config wiring), thread it through the TUI rendering paths, and fix two real safety/visual defects (T1, T2) plus two smaller cleanups (T3, T6) that the current flat palette causes.

**Architecture:** Threading approach (lowest-churn, applied consistently):
- Store `theme: Theme` on `App` (default-initialized in `App::new()`), exposed via an accessor `pub fn theme(&self) -> &Theme`. `Theme` derives `Clone, Copy` (all fields are `ratatui::style::Color`, which is `Copy`), so it can be cheaply copied into locals where a `&mut App` borrow would otherwise conflict.
- The frame-render entry point `view::render(f, &app, model)` already has `&App`, so its private helpers (`footer_line`, `render_yolo_modal`, `preview_header_lines`) and the per-row helpers in `results_list` will take a `&Theme` parameter, sourced from `app.theme()`. `help::render` will take a `&Theme` parameter too (passed from `view::render`).
- The **preview-line builders** (`preview::render_prose`, `render_transcript*`, `render_indexed_fallback*`, `highlight_terms`, `highlight_code`) run **outside** the `render(f, app, ...)` call — they are built in `PreviewState::update`, which already receives `app: &mut App`. We copy `*app.theme()` into a local `Theme` at the top of `update` (before any `&mut` use) and thread that `&Theme` down through the builders. Standalone unit tests for these builders pass `&Theme::default()`.
- The free function `theme::agent_color(agent)` becomes the method `Theme::agent_color(&self, agent)`.

This keeps `main.rs` orchestration-only (it never touches `Theme` directly; everything flows through `App` and the existing render/update calls).

**Tech Stack:** Rust, Ratatui 0.30 (crossterm backend).

---

## Dependencies & Sequencing

- **This plan lands FIRST.** It is the foundation. Downstream plans (`screen-states`, `scroll-affordances`, `results-table`) reference the `Theme` role field names defined here by name — do not rename any field.
- **Merge conflicts:** This plan edits `src/tui/view.rs` heavily; it will conflict with the responsiveness/scroll-affordances plan and the results-table plan, both of which also touch `view.rs`. Land this one first, then rebase the others.
- **Relation to `docs/ARCHITECTURE.md` pressure point P-001** (theming): this plan addresses the *internal* structure half of P-001 by introducing semantic roles, but **deliberately does NOT wire user-configurable theming** (no config file, no env var, no CLI flag). Config theming is explicitly out of scope and left for a later plan.

---

## Required Reading (do this before Task 1)

Read these exact regions so every referenced symbol is grounded:

- `src/tui/theme.rs` — full file (18 lines): current `agent_color` free fn + 7 `const`s.
- `src/tui/mod.rs` lines 36–74 — `struct App` fields and `App::new()`.
- `src/tui/view.rs` lines 1–4 (imports), 51 (`pub fn render`), 188 (footer render), 192 (`render_yolo_modal` call), 197 (`help::render` call), 214–254 (`footer_line`), 256–336 (`render_yolo_modal`), 364–416 (`preview_header_lines`), 449–462 (test module imports/helpers), 521–588 (`renders_yolo_dialog_and_status_footer`), 591–636 (`selected_result_has_marker_and_focus_style`).
- `src/tui/help.rs` — full file: `section()` uses `theme::ACCENT`; `render(f)` at line 49 uses `theme::OVERLAY_DIM` (line 67) and `theme::ACCENT`.
- `src/tui/results_list.rs` lines 4–7 (imports), 15–34 (`row_line`), 35–49 (`header_line`), 50–98 (`cell`/`enrichment_cell`), 143–250 (tests).
- `src/tui/preview.rs` lines 5–6 (imports), 34–62 (`highlight_code`), 64–160 (`render_prose`), 120–125 (the `Event::Code` arm — `Color::Yellow`), 161–246 (`render_transcript*`/`render_indexed_fallback*`), 248–264 (`prefix_first`/`indent`), 265–302 (`highlight_terms`, the `Modifier::REVERSED` at line 284), 333–395 (`PreviewState` + `update`), 399–600 (tests).
- `src/core.rs` — confirm `AgentId` has variants `Claude`, `Codex`, `Cursor` (used by `agent_color`).

Run this to confirm every call site is covered:

```sh
rg -n "theme::" src/tui/
```

Expected hits (verify before editing): `view.rs` lines 68,71,73,144,145,154,174,180,220,226,232,238,244,250,297,301,305,313,316,324,384,388,390,396,401,403,412,633,634,635; `results_list.rs` 44,60,69,71,88,91,92,93; `help.rs` 43,67,71,75; `preview.rs` 183,194,198,234. (`preview.rs:123` is `Color::Yellow`, not `theme::`, so it is NOT in this list — that is T3.)

---

## Task 1 — Introduce `struct Theme` with semantic roles

Define the new type alongside the existing constants (keep both temporarily so the build stays green; constants are removed in Task 7).

**Files:**
- `src/tui/theme.rs`

Field names (EXACT — downstream plans reference these; do not rename):
`bg, fg, muted, accent, code, border, overlay_fg, overlay_bg, selection_fg, selection_bg, match_fg, warning, error, success, preview_text, agent_claude, agent_codex, agent_cursor`.

- [ ] Add a failing unit test at the bottom of `src/tui/theme.rs`. Append a test module:

  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::core::AgentId;

      #[test]
      fn default_theme_distinguishes_warning_error_accent() {
          let t = Theme::default();
          assert_ne!(t.warning, t.accent);
          assert_ne!(t.error, t.accent);
          assert_ne!(t.warning, t.error);
          assert_ne!(t.warning, t.success);
      }

      #[test]
      fn default_theme_maps_legacy_constants() {
          let t = Theme::default();
          assert_eq!(t.muted, Color::DarkGray);
          assert_eq!(t.accent, Color::Cyan);
          assert_eq!(t.selection_fg, Color::White);
          assert_eq!(t.warning, Color::Yellow);
          assert_eq!(t.error, Color::Red);
          assert_eq!(t.success, Color::Green);
      }

      #[test]
      fn agent_color_method_matches_brand_colors() {
          let t = Theme::default();
          assert_eq!(t.agent_color(AgentId::Claude), Color::Rgb(245, 158, 11));
          assert_eq!(t.agent_color(AgentId::Codex), Color::Rgb(139, 92, 246));
          assert_eq!(t.agent_color(AgentId::Cursor), Color::Rgb(34, 197, 94));
      }
  }
  ```

- [ ] Run: `cargo test --lib tui::theme:: -- --nocapture` — expect FAIL (no `Theme` type / no `agent_color` method yet; compile error is an acceptable "fail" here).

- [ ] Implement the struct + Default + method. At the TOP of `src/tui/theme.rs` (after the existing `use` lines, before the existing `pub fn agent_color`), add:

  ```rust
  /// Semantic color roles for the TUI. Internal only: a single hardcoded
  /// default for now (no config wiring). `Copy` so it can be cheaply lifted
  /// into locals when a `&mut App` borrow is in scope.
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct Theme {
      pub bg: Color,
      pub fg: Color,
      pub muted: Color,
      pub accent: Color,
      pub code: Color,
      pub border: Color,
      pub overlay_fg: Color,
      pub overlay_bg: Color,
      pub selection_fg: Color,
      pub selection_bg: Color,
      pub match_fg: Color,
      pub warning: Color,
      pub error: Color,
      pub success: Color,
      pub preview_text: Color,
      pub agent_claude: Color,
      pub agent_codex: Color,
      pub agent_cursor: Color,
  }

  impl Default for Theme {
      fn default() -> Self {
          Self {
              bg: Color::Reset,
              fg: Color::Reset,
              muted: Color::DarkGray,                  // was DIM
              accent: Color::Cyan,                     // was ACCENT
              code: Color::Yellow,                     // was inline Color::Yellow (T3)
              border: Color::Rgb(55, 65, 81),          // was DIVIDER
              overlay_fg: Color::Rgb(64, 64, 64),      // was OVERLAY_DIM
              overlay_bg: Color::Rgb(12, 12, 12),      // NEW: real scrim bg (T2)
              selection_fg: Color::White,              // was SELECTED_FG
              selection_bg: Color::Rgb(20, 83, 91),    // was SELECTED_BG
              match_fg: Color::Cyan,                   // NEW: reserved for future unification (T6)
              warning: Color::Yellow,                  // NEW (T1)
              error: Color::Red,                       // NEW
              success: Color::Green,                   // NEW
              preview_text: Color::Rgb(205, 213, 219), // was PREVIEW_TEXT
              agent_claude: Color::Rgb(245, 158, 11),
              agent_codex: Color::Rgb(139, 92, 246),
              agent_cursor: Color::Rgb(34, 197, 94),
          }
      }
  }

  impl Theme {
      pub fn agent_color(&self, agent: AgentId) -> Color {
          match agent {
              AgentId::Claude => self.agent_claude,
              AgentId::Codex => self.agent_codex,
              AgentId::Cursor => self.agent_cursor,
          }
      }
  }
  ```

- [ ] Run: `cargo test --lib tui::theme:: -- --nocapture` — expect PASS.
- [ ] Commit:

  ```
  refactor(tui): introduce Theme struct with semantic roles

  Co-Authored-By: Claude <noreply@anthropic.com>
  ```

---

## Task 2 — Store `Theme` on `App` and expose `app.theme()`

**Files:**
- `src/tui/mod.rs`

- [ ] Add a failing unit test. In `src/tui/mod.rs`, find the existing `#[cfg(test)] mod tests` (or add one at end of file if none) and add:

  ```rust
  #[cfg(test)]
  mod theme_accessor_tests {
      use super::*;
      #[test]
      fn app_exposes_default_theme() {
          let app = App::new();
          assert_eq!(*app.theme(), crate::tui::theme::Theme::default());
      }
  }
  ```

- [ ] Run: `cargo test --lib tui::theme_accessor_tests -- --nocapture` — expect FAIL (no `theme` field / accessor).

- [ ] Implement. In `struct App` (line ~36), add a field after `preview_match_index: usize,`:

  ```rust
      theme: crate::tui::theme::Theme,
  ```

  In `App::new()` (line ~56) add to the struct literal after `preview_match_index: 0,`:

  ```rust
      theme: crate::tui::theme::Theme::default(),
  ```

  Add the accessor near the other accessors (e.g. after `pub fn help_open`):

  ```rust
  pub fn theme(&self) -> &crate::tui::theme::Theme {
      &self.theme
  }
  ```

- [ ] Run: `cargo test --lib tui::theme_accessor_tests -- --nocapture` — expect PASS.
- [ ] Commit:

  ```
  feat(tui): store Theme on App with accessor

  Co-Authored-By: Claude <noreply@anthropic.com>
  ```

---

## Task 3 — T1: warning text uses `theme.warning` (YOLO banner + footer warning)

The YOLO danger banner (`view.rs:309–318`) and the footer warning (`view.rs:247–251`) are painted with `theme::ACCENT` (cyan) — identical to ordinary chrome. Repaint them with the warning role. Thread `&Theme` into `footer_line` and `render_yolo_modal`.

**Files:**
- `src/tui/view.rs`

- [ ] Add a failing buffer-cell test in `view.rs`'s `#[cfg(test)] mod tests`. It asserts the "YOLO on" banner cells use `Theme::default().warning`, NOT `accent`:

  ```rust
  #[test]
  fn yolo_banner_uses_warning_color_not_accent() {
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
          branch: None,
          repo_url: None,
          source_path: None,
      }]);
      app.open_yolo_modal_with(true);

      let enr: Vec<Box<dyn Enricher>> = vec![];
      let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
      let cols = crate::columns::default_columns();
      let backend = TestBackend::new(120, 16);
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
      let warning = crate::tui::theme::Theme::default().warning;
      let accent = crate::tui::theme::Theme::default().accent;
      // Locate the 'Y' of "YOLO on" and check its foreground.
      let (w, h) = (buf.area.width, buf.area.height);
      let mut found = false;
      for y in 0..h {
          for x in 0..w {
              let cell = &buf[(x, y)];
              if cell.symbol() == "Y" {
                  // Only assert on a cell whose row text starts the YOLO banner.
                  if cell.fg == warning {
                      found = true;
                  }
                  assert_ne!(cell.fg, accent, "YOLO banner must not use accent");
              }
          }
      }
      assert!(found, "expected a 'Y' cell painted with the warning color");
  }
  ```

- [ ] Run: `cargo test --lib tui::view::tests::yolo_banner_uses_warning_color_not_accent -- --nocapture` — expect FAIL (banner currently cyan/accent).

- [ ] Implement. Change `render_yolo_modal` signature (line 256) to accept `&Theme`:

  ```rust
  fn render_yolo_modal(
      f: &mut Frame,
      session: Option<&SessionSummary>,
      yolo: bool,
      modal_command: Option<&[String]>,
      theme: &crate::tui::theme::Theme,
  ) {
  ```

  At its call site (line ~192 inside `render`), pass `app.theme()`:

  ```rust
  render_yolo_modal(f, session, yolo, model.modal_command, app.theme());
  ```

  Inside `render_yolo_modal`, replace the four `theme::DIM` labels (`Session`/`Directory`/`Command` lines 297,301,305) with `theme.muted`, and the danger banner (lines 311–316) with:

  ```rust
          Line::from(Span::styled(
              danger,
              if yolo {
                  Style::default()
                      .fg(theme.warning)
                      .add_modifier(Modifier::BOLD)
              } else {
                  Style::default().fg(theme.muted)
              },
          )),
  ```

  (Leave the backdrop `set_style` at line 324 for Task 4.)

  Change `footer_line` signature (line 214) to accept `&Theme`:

  ```rust
  fn footer_line(status: &StatusLine, theme: &crate::tui::theme::Theme) -> Line<'static> {
  ```

  At its call site (line ~188): `f.render_widget(Paragraph::new(footer_line(model.status, app.theme())), chunks[2]);`

  Inside `footer_line`, replace `theme::ACCENT` (line 220, the leading hint label) with `theme.accent`, every `theme::DIM` (lines 226,232,238,244) with `theme.muted`, and the WARNING span (line 250) with `theme.warning`:

  ```rust
      if let Some(warning) = status.warning.as_deref().filter(|s| !s.is_empty()) {
          spans.push(Span::styled(
              format!(" · {warning}"),
              Style::default().fg(theme.warning),
          ));
      }
  ```

- [ ] Run: `cargo test --lib tui::view::tests::yolo_banner_uses_warning_color_not_accent -- --nocapture` — expect PASS.
- [ ] Run `cargo test --lib tui::view -- --nocapture` to confirm no regression in the other view tests yet (the `selected_result_has_marker_and_focus_style` test at lines 633–635 still references `theme::SELECTED_BG`/`SELECTED_FG` consts — those still exist until Task 7, so it stays green).
- [ ] Commit:

  ```
  fix(tui): paint YOLO and footer warnings with warning role (T1)

  Co-Authored-By: Claude <noreply@anthropic.com>
  ```

---

## Task 4 — T2: modal backdrop dims via background (real scrim)

`view.rs:323–324` and `help.rs:66–67` apply `OVERLAY_DIM` as **fg-only** (`set_style(area, Style::default().fg(...))`), so the backdrop never actually dims cell backgrounds. Set BOTH `fg = overlay_fg` and `bg = overlay_bg`.

**Files:**
- `src/tui/view.rs`
- `src/tui/help.rs`

- [ ] Add a failing buffer-cell test in `view.rs` tests. It asserts a backdrop cell (outside the modal rect) has its `bg` set to `Theme::default().overlay_bg`:

  ```rust
  #[test]
  fn yolo_backdrop_dims_background() {
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
          branch: None,
          repo_url: None,
          source_path: None,
      }]);
      app.open_yolo_modal_with(true);

      let enr: Vec<Box<dyn Enricher>> = vec![];
      let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
      let cols = crate::columns::default_columns();
      let backend = TestBackend::new(120, 16);
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
      let overlay_bg = crate::tui::theme::Theme::default().overlay_bg;
      // Top-left corner (0,0) is outside the centered modal rect -> backdrop.
      assert_eq!(buf[(0, 0)].bg, overlay_bg, "backdrop must set bg, not fg-only");
  }
  ```

- [ ] Run: `cargo test --lib tui::view::tests::yolo_backdrop_dims_background -- --nocapture` — expect FAIL (bg currently unset/`Reset`).

- [ ] Implement in `view.rs`. Replace lines 323–324:

  ```rust
      f.buffer_mut().set_style(
          area,
          Style::default().fg(theme.overlay_fg).bg(theme.overlay_bg),
      );
  ```

  (`theme` is the param added in Task 3.)

- [ ] Implement in `help.rs`. Change `render` signature (line 49) to take `&Theme`:

  ```rust
  pub fn render(f: &mut Frame, theme: &crate::tui::theme::Theme) {
  ```

  Update its call site in `view.rs` (line ~197): `help::render(f, app.theme());`

  Replace the backdrop (lines 66–67):

  ```rust
      f.buffer_mut().set_style(
          area,
          Style::default().fg(theme.overlay_fg).bg(theme.overlay_bg),
      );
  ```

  Replace the border/title `theme::ACCENT` (lines 71,75) with `theme.accent`. Leave `section()` (line 43, `theme::ACCENT`) for Task 6 (or change it now to `theme.accent` — but `section()` has no `Theme` in scope; defer it to Task 6 which threads `&Theme` into `lines()`). For now `section()` keeps `theme::ACCENT` (const still exists until Task 7).

  Note: `help.rs`'s `lines()` (called inside `render`) does not yet take `&Theme`; the `section()` headings stay on the const `theme::ACCENT` until Task 6.

- [ ] Run: `cargo test --lib tui::view::tests::yolo_backdrop_dims_background -- --nocapture` — expect PASS.
- [ ] Run `cargo test --lib tui::help -- --nocapture` — expect PASS (the existing `help_lists_core_bindings` test only checks text, unaffected).
- [ ] Commit:

  ```
  fix(tui): dim modal backdrop via background scrim (T2)

  Co-Authored-By: Claude <noreply@anthropic.com>
  ```

---

## Task 5 — T3 + T6: thread `&Theme` through preview builders; inline code uses `theme.code`

Two changes in the preview-line pipeline:
- **T3:** `preview.rs:123` inline-code uses literal `Color::Yellow` → `theme.code`.
- **T6:** document the intentional REVERSED-vs-bg-swap difference at `preview.rs:284`, and route the term highlight through `theme` (still using `Modifier::REVERSED`, but reference the theme so `match_fg` is wired for future unification).

Thread `&Theme` from `PreviewState::update` (which already has `app: &mut App`) down through `render_transcript_with_terms` → `render_prose` / `highlight_terms` / `highlight_code` / `render_indexed_fallback_with_terms`. Also add the one-line "intentional RGB island" comment at the syntect block (`preview.rs:37`).

**Files:**
- `src/tui/preview.rs`

- [ ] Add a failing unit test in `preview.rs`'s `#[cfg(test)] mod tests`. It asserts inline code spans use `Theme::default().code`:

  ```rust
  #[test]
  fn inline_code_uses_theme_code_role() {
      let theme = crate::tui::theme::Theme::default();
      let lines = render_prose("use the `cargo test` command", &theme);
      let found = lines.iter().any(|l| {
          l.spans.iter().any(|s| {
              s.content.contains("cargo test") && s.style.fg == Some(theme.code)
          })
      });
      assert!(found, "inline code span should use theme.code");
  }
  ```

- [ ] Run: `cargo test --lib tui::preview::tests::inline_code_uses_theme_code_role -- --nocapture` — expect FAIL (compile error: `render_prose` takes one arg / uses `Color::Yellow`).

- [ ] Implement signature threading. Add `use crate::tui::theme::Theme;` to the imports (line ~6, alongside `use crate::tui::theme;`). Update these signatures to take a trailing `theme: &Theme` param:

  - `pub fn render_prose(text: &str, theme: &Theme) -> Vec<Line<'static>>` (line 64)
  - `pub fn highlight_code(code: &str, lang: Option<&str>, theme: &Theme) -> Vec<Line<'static>>` (line 34) — note: the syntect colors stay RGB; the param is accepted but only used to keep the call chain uniform. To avoid an `unused` warning, prefix it `_theme` OR (cleaner) skip adding it to `highlight_code` and leave that one signature unchanged. **Decision: do NOT add `theme` to `highlight_code`** — its output is entirely syntect RGB and threading a theme there is noise. Add the island comment instead (below).
  - `pub fn render_transcript(msgs, query, agent, theme: &Theme)` (line 161)
  - `pub fn render_transcript_with_terms(msgs, terms, agent, theme: &Theme)` (line 166)
  - `pub fn render_indexed_fallback(content, query, theme: &Theme)` (line 225)
  - `pub fn render_indexed_fallback_with_terms(content, terms, theme: &Theme)` (line 230)
  - `fn highlight_terms(line, terms, theme: &Theme)` (line 265)
  - `fn prefix_first(...)` already takes a `color: Color` param — leave it; its caller (line 183) passes `theme.accent` instead of `theme::ACCENT`.

  Update internal call sites so each builder passes `theme` through:
  - line 163: `render_transcript_with_terms(msgs, &parsed.free_terms(), agent, theme)`
  - lines 182,205: `render_prose(s, theme)`
  - lines 187,210: `highlight_code(text, lang.as_deref())` (unchanged — no theme)
  - line 183: `prefix_first(&mut prose, "› ", theme.accent)`
  - lines 194,198: `theme.agent_color(agent)` (replacing `theme::agent_color(agent)`)
  - line 227: `render_indexed_fallback_with_terms(content, &parsed.free_terms(), theme)`
  - line 238: `let mut body = render_prose(content, theme);`
  - wherever `highlight_terms` is invoked (search within `render_transcript_with_terms` / fallback body): pass `theme`.

  T3 — replace the `Event::Code` arm (lines 121–124) with:

  ```rust
          Event::Code(t) => {
              spans.push(Span::styled(
                  t.to_string(),
                  Style::default().fg(theme.code),
              ));
          }
  ```

  T6 — in `highlight_terms` (lines 282–285), keep `Modifier::REVERSED` but add a doc comment above the function explaining the intentional divergence and reference `theme.match_fg`:

  ```rust
  /// Highlight query terms inside a line. Term matches use `Modifier::REVERSED`
  /// (a glyph-level invert), which is intentionally DIFFERENT from the list
  /// selection's full-row background swap (`theme.selection_bg`): inline term
  /// hits should pop without repainting the whole row's background.
  /// `theme.match_fg` is reserved to unify these two affordances later; for now
  /// we keep REVERSED and accept the theme only to wire the call chain.
  fn highlight_terms(line: &Line<'static>, terms: &[String], theme: &Theme) -> Line<'static> {
  ```

  Since `theme` is unused in the body of `highlight_terms` (we keep REVERSED for now), reference it once to avoid the warning by binding `let _ = theme.match_fg;` at the top of the function body, OR rename the param to `_theme`. **Decision: rename to `_theme`** and update the call site accordingly — cleaner than a throwaway `let`.

  T3 island comment — at `highlight_code` line ~37, above `let theme = &ts.themes["base16-ocean.dark"];`, add:

  ```rust
      // Intentional RGB island: syntect owns these foreground colors; they are
      // deliberately NOT mapped to the semantic Theme roles.
  ```

  (Note: this local is named `theme` and shadows nothing since `highlight_code` takes no `Theme` param — fine.)

- [ ] Update `PreviewState::update` (lines 351–394) to copy the theme and thread it. At the TOP of the function body (before the first `&mut`/`set_preview_matches` use), add:

  ```rust
          let theme = *app.theme();
  ```

  Then pass `&theme` into the two builders:
  - line 385: `.map(|content| render_indexed_fallback_with_terms(&content, terms, &theme))`
  - line 389: `render_transcript_with_terms(&self.transcript, terms, agent, &theme)`

- [ ] Update all standalone `preview.rs` tests (lines ~399–600) that call the changed builders to pass `&crate::tui::theme::Theme::default()` as the new trailing arg. Affected test call sites (verify by `rg -n "render_transcript|render_prose|render_indexed_fallback|highlight_code" src/tui/preview.rs`): lines ~423,441,448,454,466,490,497,514,530,562. For `highlight_code` (line ~552) NO change (it kept its signature).

- [ ] Update `view.rs` test at line 489 (`render_transcript(&transcript, app.query(), AgentId::Claude)`) — add `app.theme()` as the trailing arg: `render_transcript(&transcript, app.query(), AgentId::Claude, app.theme())`.

- [ ] Run: `cargo test --lib tui::preview -- --nocapture` — expect PASS (including the new `inline_code_uses_theme_code_role`).
- [ ] Run: `cargo test --lib -- --nocapture` — expect PASS across the board.
- [ ] Commit:

  ```
  refactor(tui): thread Theme through preview builders; code role + match doc (T3, T6)

  Co-Authored-By: Claude <noreply@anthropic.com>
  ```

---

## Task 6 — Migrate remaining `theme::CONST` / `theme::agent_color` call sites to `Theme`

Convert the rest of `view.rs`, `results_list.rs`, and `help.rs` (`section()`) off the module-level constants and free function, onto `Theme`.

**Files:**
- `src/tui/view.rs`
- `src/tui/results_list.rs`
- `src/tui/help.rs`

- [ ] In `view.rs`, the `render` function (line 51) already has `app`. Replace all remaining `theme::*` reads in functions reachable from `render` that have `app`/`theme` in scope:
  - In `render` body: lines 144,145 (`theme::SELECTED_FG`/`SELECTED_BG`) → `app.theme().selection_fg` / `app.theme().selection_bg`; line 154 (`theme::DIVIDER`) → `app.theme().border`; lines 174,180 (`theme::PREVIEW_TEXT`) → `app.theme().preview_text`. Lines 68,71,73 (query line: `ACCENT`/`SELECTED_FG`/`DIM`) → `app.theme().accent` / `.selection_fg` / `.muted`.
  - `preview_header_lines` (line 364) does not take `app`; add a trailing `theme: &Theme` param and pass `app.theme()` at its call site (line ~173). Inside it, lines 384 (`theme::agent_color(s.agent)` → `theme.agent_color(s.agent)`), 388,390,401,403,412 (`theme::DIM` → `theme.muted`), 396 (`theme::ACCENT` → `theme.accent`).

- [ ] In `results_list.rs`, thread `&Theme` into the public builders:
  - `row_line` (line 15): add trailing `theme: &Theme` param. Update its call site in `view.rs` (line ~125) to pass `app.theme()`.
  - `header_line` (line 35): add trailing `theme: &Theme` param. Update its call site in `view.rs` (line ~116) to pass `app.theme()`.
  - `cell` (line 50) and `enrichment_cell` (line 76): add trailing `theme: &Theme` param; pass it from `row_line`.
  - Replace `theme::DIM` (lines 44,69,71,88,92,93) → `theme.muted`; `theme::agent_color(s.agent)` (line 60) → `theme.agent_color(s.agent)`; `theme::ACCENT` (line 91) → `theme.accent`.
  - Add `use crate::tui::theme::Theme;` to imports (line ~7), keeping `theme` removed if no longer referenced (or keep both; clippy/`cargo build` will warn on unused import — drop `theme` from the `use crate::tui::{theme, view::rel_time};` if nothing else uses it).
  - Update `results_list.rs` tests (lines ~177,190,229,246) calling `row_line`/`header_line` to pass `&crate::tui::theme::Theme::default()`.

- [ ] In `help.rs`: thread `&Theme` so `section()` headings use `theme.accent`. Change `pub fn lines(theme: &Theme) -> Vec<Line<'static>>` (line 10) and `fn section(label, theme: &Theme)` (line 39) → `theme.accent`. Update `render` (which already takes `theme` from Task 4) to call `lines(theme)`. Update the standalone test `help_lists_core_bindings` (line ~93) to call `lines(&crate::tui::theme::Theme::default())`. Add `use crate::tui::theme::Theme;` to imports.

- [ ] Update the two buffer-cell assertions in `view.rs`'s `selected_result_has_marker_and_focus_style` test (lines 633–635) — they reference the about-to-be-removed consts. Replace:
  - `theme::SELECTED_BG` → `crate::tui::theme::Theme::default().selection_bg`
  - `theme::SELECTED_FG` → `crate::tui::theme::Theme::default().selection_fg`

- [ ] Run: `cargo build` — confirm there are NO remaining references to `theme::ACCENT|DIM|DIVIDER|OVERLAY_DIM|PREVIEW_TEXT|SELECTED_BG|SELECTED_FG|agent_color`. Verify with:

  ```sh
  rg -n "theme::(ACCENT|DIM|DIVIDER|OVERLAY_DIM|PREVIEW_TEXT|SELECTED_BG|SELECTED_FG|agent_color)" src/
  ```

  Expected: NO matches.

- [ ] Run: `cargo test --lib -- --nocapture` — expect PASS.
- [ ] Commit:

  ```
  refactor(tui): migrate all call sites from const palette to Theme

  Co-Authored-By: Claude <noreply@anthropic.com>
  ```

---

## Task 7 — Remove the legacy constants and free function

Now that nothing references them, delete the old `const`s and the free `agent_color`.

**Files:**
- `src/tui/theme.rs`

- [ ] Delete the free `pub fn agent_color(...)` (lines 4–10) and all seven `pub const` lines (ACCENT, DIM, DIVIDER, OVERLAY_DIM, PREVIEW_TEXT, SELECTED_BG, SELECTED_FG, lines 12–18). The file should now contain only: the `use` lines, the `Theme` struct, `impl Default`, `impl Theme`, and the `#[cfg(test)] mod tests`.

- [ ] Run: `cargo build` — expect success (no dangling references).
- [ ] Run: `cargo test --lib -- --nocapture` — expect PASS.
- [ ] Commit:

  ```
  refactor(tui): remove legacy const palette and free agent_color

  Co-Authored-By: Claude <noreply@anthropic.com>
  ```

---

## Task 8 — Final verification

**Files:** (none — verification only)

- [ ] Run: `cargo test --lib` — expect ALL pass.
- [ ] Run: `cargo test` — expect ALL pass (integration tests like `index_sync` unaffected, but confirm).
- [ ] Run: `cargo clippy --all-targets -- -D warnings` — expect clean (no unused params/imports; if `highlight_terms`'s `_theme` or any threaded param trips an `unused` lint, address by `_`-prefixing or by actually using it as documented).
- [ ] Confirm no stragglers:

  ```sh
  rg -n "theme::(ACCENT|DIM|DIVIDER|OVERLAY_DIM|PREVIEW_TEXT|SELECTED_BG|SELECTED_FG|agent_color)" src/
  ```

  Expected: NO matches.

- [ ] Confirm every required `Theme` role field exists exactly as named:

  ```sh
  rg -n "bg:|fg:|muted:|accent:|code:|border:|overlay_fg:|overlay_bg:|selection_fg:|selection_bg:|match_fg:|warning:|error:|success:|preview_text:|agent_claude:|agent_codex:|agent_cursor:" src/tui/theme.rs
  ```

  Expected: all 18 fields present in both the struct and `Default`.

- [ ] No commit needed (verification only). Branch is ready for review.

---

## Done Criteria

- `struct Theme` exists with exactly the 18 named roles and a `Default` mapping the legacy palette plus the new `warning`/`error`/`success`/`overlay_bg`/`match_fg`/`code` roles.
- `Theme::agent_color(&self, AgentId) -> Color` replaces the free function.
- T1: YOLO banner + footer warnings paint with `theme.warning` (asserted by buffer-cell test).
- T2: modal backdrop sets a background scrim via `theme.overlay_bg` (asserted by buffer-cell test).
- T3: inline code uses `theme.code`; syntect block carries the "intentional RGB island" comment.
- T6: term-match REVERSED behavior documented as intentionally distinct from selection bg-swap; `match_fg` exposed.
- No config theming wired (out of scope).
- `cargo test` and `cargo clippy --all-targets -- -D warnings` are clean.
