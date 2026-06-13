# TUI Design Review — 2026-06-13

Initial Ratatui design/UX review of the `hop` TUI. Scope is **how the interface
looks and feels**, not correctness. Reviewed at commit `9e1bbf2` across four
isolated areas: layout & responsiveness, theme & styling, widgets/data-fit/states,
and interaction & discoverability.

This is a dated artifact, not stable architecture. Findings here are candidates for
work; promote durable decisions into `docs/ARCHITECTURE.md` (and resolve the
related `P-001` pressure point if the theme work lands).

## Overall Read

The skeleton is solid: a clean header/body/footer split, `Clear`-backed overlays
that are centered and size-guarded, a deliberate single-mode model (typing =
search, arrows = move) that sidesteps focus ambiguity, and styling that already
routes most chrome through `theme.rs`. The weaknesses are concentrated and
consistent: **the app silently assumes a wide, dark terminal, and it
under-communicates state** (empty, loading, scroll position, and which keys exist).

## Cross-Cutting Themes

1. **Responsiveness is the biggest gap.** No tiny-terminal guard on the main
   screen (overlays have one), and the preview is a fixed percentage split with no
   collapse — so at the default 80x24 the results list is already starved to ~38
   columns, and at a 40-column pane both panes are unusable. The footer also clips
   its most important spans (warnings) off the right edge.
2. **State communication is missing.** No empty state, no loading/indexing
   spinner, no scrollbars on either scrollable region, and search matches are
   highlighted in the preview but invisible in the list.
3. **Three sources of truth that drift.** Bindings live in `keymap.rs`, the
   `handle_key` match in `mod.rs`, and hand-typed strings in `help.rs` /
   `FOOTER_HINTS`, guarded only by a brittle substring test. The footer also never
   teaches the preview vocabulary even though the preview is on by default.

## Recommended Order of Attack

1. **Responsiveness pair** (tiny-terminal guard + preview collapse) — what a user
   hits on a normal terminal today.
2. **`struct Theme` refactor** — unlocks the warning/error and overlay-scrim fixes
   and resolves `P-001`.
3. **Empty + loading states** — highest "feels broken vs. feels finished" payoff.
4. **`List`→`Table` migration** and the **single `bindings()` table** — larger
   structural cleanups that also kill latent drift bugs.

---

## Area 1 — Layout & Responsiveness

Files: `src/tui/view.rs`, `src/tui/mod.rs`.

| # | Sev | Location | Finding | Fix |
|---|-----|----------|---------|-----|
| L1 | High | `view.rs:51` | No tiny-terminal guard on the main screen; below ~3 rows the body collapses and results vanish with no message (overlays have a guard, main does not). | Early bail at top of `render`: if `width < 30 \|\| height < 6`, render a centered "terminal too small" `Paragraph` and return. |
| L2 | High | `view.rs:87-96`, `mod.rs:65,159` | Preview never collapses; fixed percentage split starves the list to ~38 cols at 80x24 and makes both panes unusable at 40 cols. | Make the split responsive: below a width threshold drop/stack the preview; give the list side `Constraint::Min(48)` so its grid never starves. |
| L3 | Med | `view.rs:188,214-254` | Footer is a single un-truncated `Line`; appended status/warnings (styled ACCENT, the most important) fall off the right edge on narrow terminals. | `Flex::SpaceBetween` (hints left, status right) dropping static hints first; or order warnings before the static hint so they survive clipping. |
| L4 | Med | `view.rs:56,165` | Body uses `Min(1)` not `Min(0)`; muddies "this is the slack absorber" and contributes to the tiny-height squash. | Use `Min(0)` for the body and rely on the L1 guard for the degenerate case. |
| L5 | Med | `view.rs:100,139-148` | List starts flush at the left edge; no `Scrollbar` despite manual viewport paging, so no position indicator in long result sets. | Add a `Scrollbar` (VerticalRight) driven by `ScrollbarState::new(total).position(selected)`; optional horizontal padding. |
| L6 | Low | `view.rs:52-59,88-92,164-167,205-208` | Legacy `Direction + .split()` returning `Rc<[Rect]>` indexed by number, instead of 0.30 `Layout::vertical([...]).areas()`. | Array-destructure: `let [header, body, footer] = Layout::vertical([...]).areas(area);`. |
| L7 | Low | `view.rs:329-331`, `help.rs:69-71` | Full-box overlays use `Block::default().borders(Borders::ALL)` instead of `Block::bordered()`. | `Block::bordered().title(...)`. |
| L8 | Low | `view.rs:161-184` | Preview header runs straight into the transcript with no seam; gated on `height >= 3`, can leave a 1-row transcript. | Add a 1-row spacer (`Length(2)/Length(1)/Min(0)`) or a bottom border; raise the gate to `>= 5`. |
| L9 | Low | `view.rs:272-277`, `help.rs:60-65` | Hand-rolled centering math duplicated in two overlays. | Extract a `center(area, w, h)` helper using `Flex::Center` on both axes. |

## Area 2 — Theme & Styling Consistency

Files: `src/tui/theme.rs` (18 lines) + styling usage across `view.rs`,
`results_list.rs`, `preview.rs`, `help.rs`. `theme.rs` is a flat bag of `const`
colors, not a semantic role system — most chrome routes through it (good) but
gaps hide two real defects.

| # | Sev | Location | Finding | Fix |
|---|-----|----------|---------|-----|
| T1 | High | `view.rs:309-318,247-251` | Safety-critical YOLO/warning text is painted `ACCENT` (cyan) — identical to footer labels, PR links, headings. The one "stop and read" string gets no distinct signal. | Add `warning`/`error` roles (e.g. Yellow/Red); reserve ACCENT for neutral emphasis. |
| T2 | High | `view.rs:323-324`, `help.rs:66-67` | `OVERLAY_DIM` sets only `fg`, so the modal backdrop's backgrounds and explicitly-styled cells aren't dimmed — a muddy partial scrim, not a clean one. | Dim via `bg` (or `.dim()`); define an `overlay` role setting both fg and bg. |
| T3 | Med | `preview.rs:123,51,37` | Raw literals bypass the theme: inline code `Color::Yellow`; syntect block uses hardcoded `base16-ocean.dark` RGB that ignores the user's scheme and clashes with the rest of the palette. (~2 literals in draw code + 6 RGB consts in theme.) | Route inline code through a `code` role; pick a syntect theme nearer the app palette or make it themeable. |
| T4 | Med | `theme.rs` (all) | Mixed ANSI + RGB with no policy. `DIM = DarkGray` and `PREVIEW_TEXT = Rgb(205,213,219)` are set as `fg` with no `bg` → light-on-light on a light terminal. The app assumes dark. | Commit to ANSI throughout, or define full bg/fg pairs so the app controls its own contrast. |
| T5 | Med | (codebase-wide) | No base `bg`/`fg` roles — the app never sets its own background. Root cause of T2 and T4. | Define `bg`/`fg` roles, apply to root and overlay surfaces. |
| T6 | Low | `view.rs:142-146`, `preview.rs:284` | Two selection idioms: list uses fixed teal `bg`; preview term-match uses `REVERSED`. A match inside the selected row inverts to low-contrast teal. | Pick one `match` highlight role and apply consistently, or document the difference. |
| T7 | Low | `theme.rs:6-8` | Agent brand colors are hard RGB (coherent, used consistently) but ignore the terminal theme. | Acceptable for brand identity; add a one-line "intentional" note so they aren't "fixed" to ANSI. |

**Modifier use:** reasonable, not flat (BOLD headings/selection, REVERSED matches,
ITALIC emphasis). Gap: "muted" is a color (`DarkGray`) rather than `Modifier::DIM`,
so it can't compose on top of a semantic color.

**Proposed `struct Theme`** (replacing the flat const bag, resolves `P-001`):

```rust
pub struct Theme {
    // base surfaces (currently absent — root cause of contrast/overlay bugs)
    pub bg: Color,
    pub fg: Color,
    // text roles
    pub muted: Color,        // was DIM (DarkGray)
    pub accent: Color,       // was ACCENT (Cyan) — neutral emphasis / links / headings
    pub code: Color,         // was inline Color::Yellow in preview.rs:123
    // structure
    pub border: Color,       // was DIVIDER
    pub overlay_fg: Color,
    pub overlay_bg: Color,   // NEW — real scrim (fixes T2)
    // selection / search match
    pub selection_fg: Color,
    pub selection_bg: Color,
    pub match_fg: Color,     // unify list-selection vs preview-match (T6)
    // status semantics (NEW — fixes T1)
    pub warning: Color,
    pub error: Color,
    pub success: Color,
    // brand identity (kept exact RGB on purpose)
    pub preview_text: Color,
    pub agent_claude: Color,
    pub agent_codex: Color,
    pub agent_cursor: Color,
}
```

## Area 3 — Widgets, Data-Fit & Screen States

Files: `src/tui/results_list.rs`, `src/tui/preview.rs`, `src/columns.rs`.

| # | Sev | Location | Finding | Fix |
|---|-----|----------|---------|-----|
| W1 | High | `view.rs:122-148`, `results_list.rs:15-48`, `columns.rs:121-214` | Tabular data (AGENT/REPO/BRANCH/TITLE/MSGS/PR/TIME) rendered as a hand-formatted `List`; the app has hand-rolled a `Table` (bespoke column solver, `fit`, manual gaps, separately-rendered offset header). Header/body alignment depends on two code paths staying in sync. | Use `Table` with `Row`/`Cell`, `widths` from the column model, `.header(...)`, `.column_spacing(1)`, `.highlight_symbol("❯ ")`. Keep priority-based column-dropping to *select* columns; let `Table` own width/align/truncation. |
| W2 | High | `view.rs:122-148,62-63` | No empty state; zero results renders a blank box, only `0/0` in the search line. Can't distinguish "no matches" / "no index yet" / "broken". | Centered muted `Paragraph` in the list area; branch on `app.query().is_empty()` for "type to search" vs "no matches". |
| W3 | High | `main.rs:39`, `engine.rs`, (no spinner in `tui/`) | No loading/indexing state; cold start or large corpus shows an empty list with no sign work is happening. | Add a throbber (or frame-cycled braille) + "indexing N…" driven by an `is_indexing` flag through the existing status channel. |
| W4 | Med | `view.rs:178-184,139-148` (no `Scrollbar` anywhere) | Neither the preview `Paragraph` (scrolled by raw offset) nor the list shows a scrollbar — no position-within-content indicator. | Pair each region with `Scrollbar` + `ScrollbarState` from content length/offset; reserve a 1-col gutter. |
| W5 | Med | `preview.rs:265-301` (good) vs `results_list.rs:50-96` (none) | Search matches are highlighted in the preview but invisible in the list, at the surface the user scans fastest. | Thread `free_terms()` into `row_line`/`cell`, reuse `highlight_terms` on the TITLE cell. |
| W6 | Med | `results_list.rs:90-94`, `view.rs:235-240` | Pending slow-enricher renders a static `⟳` that reads as a spinner but never moves. | Animate via a frame tick (tie to W3's throbber) or use an unambiguous static `…`. |
| W7 | Low | `mod.rs:315-320` | Preview scroll clamped only at the top; can scroll past content into blank space. | Clamp to `lines.len().saturating_sub(viewport_height)`; W4's scrollbar makes the overshoot visible. |
| W8 | Low | `view.rs:380-416,165` | Preview-header `title · directory` line isn't ellipsized; a long path wraps/clips the 2-row header. | Run the directory through `columns::fit` against inner width (as the modal already does). |

**What's already good:** selection + scroll state survive frames (reconstructed
from `app.selected()`, persisted `preview_scroll`), preview is a proper
`Paragraph` with `.wrap(Wrap { trim: false })`, and list truncation is wide-glyph /
combining-mark correct with tested narrow-terminal column dropping.

## Area 4 — Interaction & Discoverability

Files: `src/tui/keymap.rs`, `src/tui/help.rs`, `src/tui/mod.rs`, `src/tui/view.rs`.
The single-mode, search-first model is well-chosen and sidesteps focus ambiguity.

| # | Sev | Location | Finding | Fix |
|---|-----|----------|---------|-----|
| H1 | High | `keymap.rs:23-33`, `mod.rs:177-301`, `help.rs:10-37`, `view.rs:212` | Bindings live in three places (keymap, the `handle_key` match, hand-typed help/footer strings); guarded only by a brittle substring test. Help/footer will drift from behavior. | One `bindings()` table (`{keys, group, label}`) feeding help, footer (filtered to a "primary" subset), and ideally dispatch; test that every entry is reachable in `handle_key`. |
| H2 | High | `view.rs:212`, `mod.rs:64-66` | Footer omits the entire preview vocabulary (`Ctrl+P/U/D/N/B/←→`) though the preview is on by default — signature features hidden behind `?`. | Append preview hints when preview is visible and width allows; width-budget so it degrades, not clips. |
| H3 | Med | `mod.rs:186-209` | `?` is swallowed inside the yolo modal, yet the global footer (still drawn) advertises "? help". | Open help from the modal too, or accept it (the modal has its own legend) and treat the modal footer as the source of truth there. |
| H4 | Med | `help.rs:13-35` | Help columns aligned by hand-counted spaces; a longer key string breaks the column. | Render rows as two `Span`s, pad the key column to `max(key.len())` programmatically (falls out of H1). |
| H5 | Med | `view.rs:122-148,62-63` | No empty/no-results guidance; center is blank and Enter does nothing silently. | Centered hint (duplicate of W2): "No sessions match. Esc to clear." / "Type to search…". |
| H6 | Low | `view.rs:135-148` | Selection feedback is strong and test-covered, but disappears when the list is empty (tied to H5). | Resolved by H5/W2. |
| H7 | Low | `keymap.rs:1-3` | `j/k`, `g/G`, `n/N` are intentionally not nav keys (typing edits the query); a vim user pressing `j` types into search. | No change — documented trade-off; keep "↑↓ move" prominent in the footer. Flagged so it isn't "fixed" by adding `j/k`. |
| H8 | Low | `keymap.rs:27` | `Ctrl+D` scrolls the preview but is the conventional EOF/quit chord in shells. | Acceptable (`Ctrl+C`/`Esc` quit and are advertised); revisit only if requested. |
