# 2026-06-05 Review Action Plan

> Source material: `docs/reviews/2026-06-05-architecture-rust-tui-review.md`,
> `docs/reviews/2026-06-05-tui-keymap-review.md`,
> `docs/reviews/2026-06-05-popular-tui-research.md`,
> `docs/ARCHITECTURE.md`, `docs/PROJECT.md`, `README.md`.

## Summary

Address review action items in dependency order: backend data safety first, then
overlapping TUI/query fixes, then `main.rs` boundary refactors, then
frontend-neutral DTOs and polish.

Current baseline before implementation: `cargo test` and `cargo fmt --check`
pass; `cargo clippy --all-targets --all-features -- -D warnings` fails on known
issues from architecture review verification.

## Wave 1: Backend Safety

- [ ] Fix sync deletion safety.
  - Source: architecture review `F-001`, action items `A-001`/`A-002`,
    architecture pressure point `P-005`.
  - Delete indexed rows only for adapters that completed a successful
    authoritative scan.
  - Preserve existing rows when an adapter is unavailable or `scan()` fails.
  - Add sync status for scan/unavailable warnings distinct from parse-error
    counts.

- [ ] Surface adapter quality signals.
  - Source: architecture review `F-007`, action item `A-010`,
    `docs/PROJECT.md` Quality Bar.
  - Track scan failures, parse failures, and successful empty/noisy sessions.
  - Keep malformed files non-fatal.

## Wave 2: TUI Keymap And Query Semantics

- [ ] Fix search-input command conflicts.
  - Source: keymap review `K-001`/recommended action order, popular TUI
    research "Highest Leverage" items 1-3.
  - Make `Ctrl+C` global before overlay/modal handling.
  - Make plain `?`, `[`, and `]` commands only when the query is empty in search
    mode; otherwise they type into the query.
  - Preserve modal navigate commands.

- [ ] Make modal state visible.
  - Source: keymap review `K-003`, architecture review `F-005`, architecture
    pressure point `P-006`, popular TUI research "Mode Visibility".
  - Add visible `SEARCH`/`NAV` indicator.
  - Render mode-specific footer/help text.
  - Update `README.md` key descriptions.

- [ ] Make parsed query semantics authoritative.
  - Source: architecture review `F-004`, action item `A-007`, architecture
    boundary `B-006`.
  - Add `ParsedQuery` helpers for free highlight terms and filter summaries.
  - Use parsed free terms for preview highlighting and indexed fallback.
  - Remove raw-query filter parsing from `main.rs`.

- [ ] Add minimal query editing.
  - Source: keymap review `K-004`, popular TUI research "Highest Leverage" item
    4.
  - Add cursor state and support Left/Right, Home/End, Delete,
    Backspace-before-cursor, `Ctrl+A`, `Ctrl+E`, `Ctrl+W`.
  - Keep `Ctrl+U/D` as preview scroll and document it clearly.

## Wave 3: Runner Boundary Refactor

- [ ] Resolve clippy failures.
  - Source: architecture review Verification and `F-008`.
  - Fix needless borrow in `index.rs`.
  - Implement or rename `Preset::from_str`.
  - Replace `tui::view::render`'s long argument list with a render
    model/context.

- [ ] Move feature state out of `main.rs`.
  - Source: architecture review `F-002`, action items `A-003`/`A-004`,
    architecture boundaries `B-007`/`B-008`, pressure point `P-004`.
  - Move preview loading/memoization into explicit TUI model or engine-facing
    effects.
  - Move enrichment scheduling/result folding into engine/backend state or
    explicit effects.
  - Keep render side-effect-free and viewport-bounded.

## Wave 4: Frontend-Neutral Backend Shapes

- [ ] Split row/session data shapes.
  - Source: architecture review `F-003`/`F-006`, action items
    `A-005`/`A-006`/`A-009`, architecture pressure point `P-003`.
  - Introduce `SessionSummary`, `Transcript`, and `ResumeTarget`/`ResumeCommand`
    equivalents.
  - Stop cloning full indexed `content` into normal TUI result rows.
  - Load indexed content only for fallback preview or explicit source needs.

- [ ] Add a backend facade without splitting crates yet.
  - Source: architecture review "Component Separation and Frontend
    Portability", architecture boundary `B-009`.
  - Expose search, transcript load, resume command construction, sync updates,
    and enrichment requests without Ratatui/Crossterm types.
  - Keep terminal restore and Unix `exec` in CLI/TUI handoff code.

- [ ] Update architecture docs.
  - Source: architecture review "Principles to Promote Into Architecture Docs",
    `docs/ARCHITECTURE.md` documentation policy.
  - Promote durable rules after implementation.
  - Remove or revise resolved pressure points.

## Wave 5: TUI Polish

- [ ] Use terminal display width.
  - Source: architecture review `F-009`, action item `A-011`.
  - Replace `chars().count()` fitting in columns/result rows/modals with
    display-width-aware fitting.
  - Add wide-glyph and combining-mark tests.

- [ ] Make paging viewport-aware.
  - Source: keymap review `K-005`, popular TUI research "Next Layer" item 8.
  - Base PgUp/PgDn and preview scroll quantum on rendered viewport height.

- [ ] Add preview match navigation.
  - Source: popular TUI research "Highest Leverage" item 5.
  - Track match positions and add next/previous match actions without breaking
    live search input.

- [ ] Defer larger features.
  - Source: popular TUI research "Next Layer".
  - Row-context commands, sorting/toggle affordances, command vocabulary, and
    themes should get separate plans.

## Test Plan

- Run after each wave: `cargo test`, `cargo fmt --check`,
  `cargo clippy --all-targets --all-features -- -D warnings`.
- Add engine tests for adapter-scoped deletion and sync warning status.
- Add TUI tests for global `Ctrl+C`, context-sensitive printable commands,
  modal footer/help text, and query cursor editing.
- Add query tests for parsed highlight terms and filter summaries.
- Add index/integration tests if search return shapes or deletion semantics
  change.
- Add width tests for wide glyphs and combining marks in the polish wave.

## Assumptions

- `RTK.md` is referenced by `AGENTS.md` but is absent in the repo; this plan uses
  the available docs.
- No CLI behavior changes except safer sync status, corrected key handling, and
  clearer help/footer text.
- Crate splitting is out of scope; module boundaries come first.
