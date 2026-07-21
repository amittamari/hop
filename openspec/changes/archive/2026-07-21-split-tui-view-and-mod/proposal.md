## Why

`src/tui/view.rs` (1840 lines) and `src/tui/mod.rs` (1105 lines) are the two
genuine god-files in the codebase — well past the ~500-line soft limit added to
`AGENTS.md`, and unlike the test-dominated files, their bulk is production code
with mixed responsibilities. Splitting them by responsibility improves
navigability and makes future TUI work land in smaller, focused units.

## What Changes

- Split `src/tui/view.rs` into a `view/` module: render orchestration, footer,
  card layout, and preview-header rendering, each carrying its own colocated
  `#[cfg(test)]` block (private access preserved).
- Split `src/tui/mod.rs` into the tui module root plus separate state-accessor
  and input/action-dispatch modules; tests travel with the code they exercise.
- No user-visible behavior change, no public API change beyond module paths.
  All existing `crate::tui::...` re-exports remain valid.

## Capabilities

### New Capabilities

<!-- none: this is an internal refactor with no requirement/behavior change -->

### Modified Capabilities

<!-- none: no spec-level behavior changes; module layout only -->

## Impact

- Code: `src/tui/view.rs`, `src/tui/mod.rs`, and new files under `src/tui/view/`
  and `src/tui/`. Import sites elsewhere in the crate if any symbol paths move
  (mitigated by re-exports).
- Tests: existing inline tests are redistributed, not rewritten; `cargo test`
  must stay green.
- Risk: low — mechanical extraction guarded by the existing test suite.
