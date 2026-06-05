# Architecture Review

Date: 2026-06-05

This review covers the current `hop` codebase architecture after the TUI v2 work.
It focuses on boundaries, maintainability, and failure modes that matter before
adding more agents or larger features.

## Summary

The codebase has a solid small-app structure:

- `adapters` normalize Claude and Codex JSONL into shared `core` types.
- `query` stays independent from Tantivy.
- `index` owns Tantivy schema, search, and sync diffing.
- `engine` is mostly UI-agnostic orchestration.
- `tui` is split into state, rendering, preview, results, help, and keymap modules.
- `enrich` separates fast local display data from slow background PR lookup.

The main architectural risks are not broad rewrites. They are a few boundary leaks
and scaling assumptions that could become visible as local session history grows.

## Findings

### 1. Preview Loading Re-Scans Adapters

Severity: Medium

`main.rs` reloads the selected transcript by calling `latest_transcript()`, which
re-runs `adapter.scan()` to rediscover the source file path. This happens from the
TUI loop on selection changes.

Relevant files:

- `src/main.rs`
- `src/core.rs`
- `src/adapters/mod.rs`

Impact:

- Preview work can become proportional to total session-file count.
- The TUI loop owns filesystem discovery behavior that belongs closer to engine
  or index state.
- Indexed `Session` rows cannot reliably point back to their source without a
  fresh adapter scan.

Recommended direction:

- Add a stable source key or source path to indexed session metadata.
- Store it through the Tantivy schema.
- Let preview loading resolve the transcript from that stored source reference.
- Move transcript loading/cache ownership out of `main.rs` when practical.

### 2. Directory And Date Filters Are Applied After A Fixed Fetch Cap

Severity: Medium

`SearchIndex::search()` fetches a fixed number of Tantivy hits, then applies
directory and date filters in Rust.

Relevant files:

- `src/index.rs`
- `src/query.rs`

Impact:

- Large histories can miss valid matches if the first capped result set is mostly
  filtered out.
- Restrictive `dir:` filters and older date windows are most exposed.

Recommended direction:

- Push date filtering into the Tantivy query where possible.
- For filters that remain post-query, over-fetch or paginate until enough
  matching rows are collected or the search space is exhausted.
- Add an integration test with many high-scoring filtered-out documents before a
  valid filtered-in document.

### 3. Result Rendering Is Not Truly Viewport-Only

Severity: Medium

`tui::view::render()` builds a `ListItem` for every result on every frame. Each row
can run fast enrichers synchronously, including filesystem fallback for branch
lookup when session metadata is absent.

Relevant files:

- `src/tui/view.rs`
- `src/tui/results_list.rs`
- `src/enrich/mod.rs`

Impact:

- Smooth scrolling depends on total result count, not only visible rows.
- Filesystem fallback can accidentally enter the render hot path for many rows.

Recommended direction:

- Render only the visible row range.
- Cache fast enrichment values per session id/source key.
- Keep filesystem fallback out of per-frame formatting paths.

### 4. Session Identity Is Not Namespaced By Agent

Severity: Medium

Sync merges all adapter scans into maps keyed only by raw session id, and index
upsert/delete also operate by raw id.

Relevant files:

- `src/engine.rs`
- `src/index.rs`
- `src/core.rs`

Impact:

- A Claude and Codex session with the same raw id would collide.
- Adding more adapters increases the risk.
- Deletes and updates can affect the wrong row if ids overlap.

Recommended direction:

- Introduce an indexed document key such as `agent:id`.
- Preserve raw session id separately for agent resume commands.
- Use the namespaced key for sync diffing, index deletes, enrichment maps, and
  preview source lookup.

### 5. Some Extension Hooks Are Declared But Not Wired

Severity: Low

The code parses or exposes extension-facing fields that are not yet consistently
used.

Examples:

- `Adapter::supports_yolo()` exists, but `App::set_results()` currently marks all
  rows as yolo-capable.
- `Config.columns.order`, `theme`, and `keybindings` are parsed but not applied.

Relevant files:

- `src/adapters/mod.rs`
- `src/tui/mod.rs`
- `src/config.rs`

Impact:

- The extension model is less complete than it appears.
- Future agents may assume a config or capability field is active when it is only
  reserved.

Recommended direction:

- Either wire the hooks through or document them as reserved.
- For yolo, pass adapter capability information from engine/app wiring instead of
  assuming every row supports it.

## Suggested Order Of Work

1. Namespace indexed session identity.
2. Store source path/source key for preview lookup.
3. Fix search filtering so capped fetches cannot hide valid results.
4. Make result rendering truly viewport-bounded.
5. Wire or explicitly mark reserved config/capability hooks.

The first two items are related and should be designed together. A namespaced
source key can support sync, preview lookup, enrichment maps, and future adapter
expansion.

## Current Verification

At review time, the existing test suite passed:

```sh
cargo test
```

Result: all unit, integration, and doc-test targets passed.
