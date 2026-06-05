# Architecture, Rust, and TUI Review

**Date:** 2026-06-05  
**Status:** Review artifact for later action-item extraction  
**Scope:** Architecture boundaries, Rust idioms, Ratatui application pattern, TUI UI/UX, and frontend portability.

## Summary

`hop` has a sound baseline: raw agent JSON is mostly normalized in adapters, search is isolated behind Tantivy, query parsing is independent of Tantivy, and TUI rendering is tested with Ratatui's `TestBackend`. The crate is small enough that feature work is still manageable.

The main architectural risk is not broken code, but boundary drift. `main.rs` currently owns too much runtime behavior: preview memoization, sync folding, enrichment scheduling, status construction, and terminal-loop side effects. That makes the current TUI work, but it weakens the promise that the Rust core/backend could be reused cleanly by another frontend.

The most important next step is to harden stable application boundaries before adding more columns, filters, enrichers, or frontend surfaces.

## Sources Reviewed

- `docs/PROJECT.md`
- `docs/ARCHITECTURE.md`
- `README.md`
- `docs/specs/2026-06-04-hop-design.md`
- `docs/specs/2026-06-04-hop-tui-v2-design.md`
- `docs/plans/superpowers-2026-06-04-hop.md`
- `docs/plans/superpowers-2026-06-04-hop-tui-v2.md`
- `src/` and `tests/`
- Ratatui docs:
  - https://ratatui.rs/concepts/application-patterns/
  - https://ratatui.rs/concepts/application-patterns/the-elm-architecture/
  - https://ratatui.rs/concepts/application-patterns/component-architecture/
  - https://ratatui.rs/concepts/rendering/
  - https://ratatui.rs/concepts/layout/

## Verification

- `cargo test`: passed.
- `cargo fmt --check`: passed.
- `cargo clippy --all-targets --all-features -- -D warnings`: failed on idiomatic issues:
  - needless borrow in `src/index.rs`
  - `Preset::from_str` should implement or avoid confusion with `FromStr`
  - `tui::view::render` has too many arguments
  - needless borrows in `src/tui/view.rs`

## Findings

### F-001: Sync can delete valid indexed rows after transient scan unavailability

**Severity:** High  
**Files:** `src/engine.rs`, `src/index.rs`

`Engine::sync_once` and `Engine::spawn_background_sync` build one global `known` map, skip unavailable adapters, then diff against the smaller scan set. If an adapter's data directory temporarily disappears, that agent's previously indexed rows can be treated as deleted.

Related code:

- `src/engine.rs`: `known_mtimes`, adapter availability checks, global `diff`
- `src/index.rs`: `diff`

The first design spec says missing agent data dirs are treated as zero sessions and pruned. That is explicit, but it is risky for removable disks, permission errors, cloud sync races, or transient paths. The current architecture docs say sync is "non-fatal"; destructive pruning on transient unavailability is not consistent with that spirit.

**Action candidate:** Diff and delete only inside an adapter namespace after that adapter scanned successfully. Report unavailable adapters and fatal scan errors separately from parse errors.

### F-002: `main.rs` owns application behavior that should live in engine or TUI state

**Severity:** High  
**Files:** `src/main.rs`, `src/engine.rs`, `src/tui/mod.rs`, `src/tui/view.rs`

`main.rs` is more than orchestration. It owns:

- transcript cache state
- preview line memoization
- preview fallback decisions
- visible-row enrichment enqueueing
- enrichment result folding
- status line construction
- sync update folding
- terminal lifecycle

The TUI v2 spec says `engine.rs` should own the `EnrichmentService`, fold enrichment results through the existing update channel, and drive on-demand transcript parsing. The implementation instead keeps much of that behavior in the terminal runner.

**Action candidate:** Move runtime effects into a UI-agnostic application service or an explicit TEA-like loop:

- `AppModel`: query, selected row, preview state, visible rows, enrichment status
- `Msg`: key input, sync update, search due, enrichment result, selection changed
- `Effect`: search, load transcript, request enrichment, resume, persist UI state
- `view`: pure Ratatui rendering from model plus render resources

### F-003: Backend portability is partial, not ready for an Electron or Swift frontend tomorrow

**Severity:** High  
**Files:** `src/lib.rs`, `src/adapters/mod.rs`, `src/resume.rs`, `src/main.rs`

Reusable pieces:

- `core`
- `query`
- most of `index`
- adapter parsing
- parts of `enrich`

Non-portable or awkward pieces:

- `resume.rs` is Unix-only `exec` handoff.
- `Adapter` mixes file scanning, parsing, transcript loading, resume command construction, and agent identity.
- `main.rs` has the practical backend service loop but it is private to the TUI binary.
- `lib.rs` exposes CLI, TUI, and resume modules as the same public surface as domain/backend code.
- Result rows use `core::Session`, including large `content`, instead of a dedicated frontend DTO.

**Action candidate:** Introduce a backend facade that a GUI can call without importing terminal concepts:

- `BackendService::open(config) -> BackendService`
- `search(ParsedQuery) -> Vec<SessionSummary>`
- `load_transcript(SessionKey) -> Transcript`
- `resume_command(SessionKey, ResumeMode) -> ResumeCommand`
- `sync_updates() -> Receiver<BackendEvent>`
- `request_enrichment(VisibleRange)`

Keep terminal restore and `exec` as a CLI-only handoff layer.

### F-004: Parsed query is not the single source of truth

**Severity:** Medium  
**Files:** `src/query.rs`, `src/main.rs`, `src/tui/preview.rs`

`query.rs` correctly parses structured filters, but downstream code still reinterprets the raw query:

- preview highlighting splits the raw query, so filters like `agent:claude` and `dir:api` can become highlight terms
- footer filter display reparses raw text in `main.rs`
- autocomplete is tied to raw text token handling

**Action candidate:** Expose a display/query-intent model from `query.rs`, such as:

- `ParsedQuery::free_terms()`
- `ParsedQuery::filter_summary()`
- `ParsedQuery::completion_for_cursor(input, cursor)`

Search, preview matching, footer summaries, and autocomplete should all consume parsed query semantics.

### F-005: UI mode language is unclear

**Severity:** Medium  
**Files:** `src/tui/mod.rs`, `src/tui/view.rs`, `src/tui/help.rs`

The default "search" preset is understandable, but modal mode is not self-evident. In modal mode, `Esc` enters navigate mode, yet footer/help text still presents `Esc` as quit in some contexts. The model has `navigate: bool`, but the rendered interface does not make this state obvious.

**Action candidate:** Render mode-specific footer/help text and a visible mode indicator. In modal mode, show whether the user is in search/edit mode or navigate mode. Keep cancel/quit semantics unambiguous.

### F-006: Result rows clone large indexed content into UI state

**Severity:** Medium  
**Files:** `src/index.rs`, `src/main.rs`, `src/tui/mod.rs`

`SearchIndex::to_session` reconstructs full `Session`, including `content`, for every search result. `sync_results_into_app` clones all results into `App`. PR enrichment then explicitly clears `content` before sending requests, which shows that the current row type is too heavy for normal display flow.

**Action candidate:** Split `Session` into narrower shapes:

- `IndexedSession` or `SessionDocument`: includes indexed content and source path
- `SessionSummary`: list row fields only
- `Transcript`: preview messages
- `ResumeTarget`: directory plus command-relevant id/agent

### F-007: Adapter parsing is too forgiving without surfacing quality signals

**Severity:** Medium  
**Files:** `src/adapters/claude.rs`, `src/adapters/codex.rs`

Malformed JSONL lines are silently skipped. `read_dir(...).flatten()` drops filesystem errors. That makes the app robust against noisy histories, but also hides incompatible schema changes and permission issues.

**Action candidate:** Track parse quality:

- number of invalid lines
- whether any meaningful message was extracted
- whether required metadata was missing
- filesystem scan errors by adapter

Use this to distinguish "valid but empty/noisy session" from "the adapter no longer understands this source".

### F-008: TUI rendering API has too many arguments and blurred render dependencies

**Severity:** Low  
**Files:** `src/tui/view.rs`, `src/main.rs`

`tui::view::render` currently takes 11 arguments. This is a local maintainability smell and matches the larger boundary problem: render inputs are scattered between `App`, engine state, enrichment maps, preview state, status state, and modal command state.

**Action candidate:** Introduce a `RenderModel` or fold more state into `App`:

```rust
pub struct RenderModel<'a> {
    pub app: &'a App,
    pub columns: &'a [Column],
    pub preview: &'a PreviewRenderState,
    pub enrichments: &'a EnrichmentRenderState,
    pub status: &'a StatusLine,
    pub now: i64,
}
```

### F-009: Unicode display width is not handled correctly

**Severity:** Low  
**Files:** `src/columns.rs`, `src/tui/view.rs`

Column fitting and modal fitting use `chars().count()`. That is not terminal display width. Wide glyphs, combining marks, and emoji can misalign columns or truncate incorrectly.

**Action candidate:** Use a terminal-width-aware crate or Ratatui text-width utilities when fitting strings for fixed-width cells.

## TUI UX Assessment

The visual direction is pragmatic and mostly good:

- always-live search
- scannable columns
- right-side transcript preview
- viewport-bounded rows
- help overlay
- simple colored agent badges

The design language is clearest when the app is in its default search flow. It is weakest around modes and transient states:

- modal keymap state is not visible enough
- footer text is too dense and static
- pending PR uses a glyph but not a richer status affordance
- preview scroll has no visible position indicator
- search input lacks a visible cursor or editing affordance
- long path/title behavior is functional but not very polished

Compared with the best Ratatui TUIs, this feels usable and fast, but not yet self-explanatory. A polished TUI should make its current mode, selection, pending work, and escape path obvious without requiring help text.

## Ratatui Application Pattern Assessment

Ratatui documents multiple valid patterns: The Elm Architecture, component architecture, and Flux. Ratatui's immediate-mode model also means the app must be disciplined about event loop orchestration and render triggering.

For `hop`, TEA is the best fit right now:

- one main model
- search-first interaction
- a small set of events
- deterministic rendering from state
- side effects that can be described as explicit effects

Component architecture is not necessary yet. Results, preview, help, and modal are mostly render surfaces, not independent stateful components. If the app later adds tabs, multi-pane workflows, or background task dashboards, component architecture may become more appropriate.

Recommended pattern:

```text
Input/Sync/Enrichment event
  -> Msg
  -> update(AppModel, Msg) -> Effects
  -> run Effects
  -> render(AppModel)
```

`main.rs` should run the loop and terminal lifecycle, not own feature state.

## Component Separation and Frontend Portability

If the goal is to write an Electron or Swift frontend using this Rust backend tomorrow, the backend is not portable enough yet.

Portable today:

- parser output types
- query parser
- Tantivy search wrapper
- adapter parse/transcript behavior
- column solver logic, if a GUI wanted the same terminal columns

Not portable enough:

- the backend lifecycle is embedded in `main.rs`
- `Adapter` bundles parse/scan/resume responsibilities
- resume handoff assumes terminal process replacement
- `Session` is too broad as a cross-frontend row DTO
- enrichment scheduling is driven by terminal visible ranges inside `main.rs`
- status/progress events are not modeled as stable backend events

Desired separation:

```text
hop-core
  Domain types, query parsing, adapter extraction, transcript model.

hop-backend
  Index, sync service, search facade, enrichment service, stable events/DTOs.

hop-tui
  Ratatui App model, key handling, render state, terminal loop, exec resume.

future GUI
  Calls backend facade; renders summaries/transcripts; chooses its own resume policy.
```

This does not require splitting crates immediately, but the module boundaries should move in that direction.

## Feature Boundary Exercise

Feasible feature request: add `branch:<name>` query filtering.

Expected clean boundaries:

- `query.rs`: parse `branch:` include/exclude filters
- `index.rs`: query or post-filter branch
- `README.md`: document syntax
- `tests/index_sync.rs` or query tests: cover behavior
- optional autocomplete: complete `branch:` from current result/index metadata

Current spillover risk:

- `main.rs` would need filter-summary logic changes because it reparses raw query text
- preview highlighting would need to ignore `branch:<name>`
- if branch autocomplete needs indexed values, no backend API currently exposes filter suggestions

Conclusion: the feature is feasible, but it reveals the missing invariant that parsed query semantics must drive every consumer.

## Action Item Candidates

These are phrased for extraction into tasks.

- A-001: Change sync deletion logic so unavailable adapters or failed scans cannot prune previously indexed sessions.
- A-002: Add explicit sync error events distinct from parse-error counts.
- A-003: Move enrichment scheduling/result folding out of `main.rs` and into engine/backend state or explicit effects.
- A-004: Move preview transcript loading and preview render-state memoization out of `main.rs`.
- A-005: Introduce backend DTOs (`SessionSummary`, `Transcript`, `ResumeTarget` or equivalent).
- A-006: Add a UI-agnostic backend facade suitable for non-terminal frontends.
- A-007: Make parsed query semantics the single source for search, preview highlights, filter summaries, and autocomplete.
- A-008: Add mode-specific footer/help text and a visible modal-keymap mode indicator.
- A-009: Split row summary data from full indexed content to avoid cloning large content into TUI state.
- A-010: Surface adapter parse quality and scan errors.
- A-011: Replace character-count truncation with terminal-display-width-aware fitting.
- A-012: Address current `cargo clippy --all-targets --all-features -- -D warnings` failures.

## Principles to Promote Into Architecture Docs

These are durable enough to consider adding to `docs/ARCHITECTURE.md`.

- **Parsed query semantics are the source of truth.** Search, preview matching, autocomplete, and filter summaries should not reinterpret the raw query independently.
- **The runner owns lifecycle, not feature state.** `main.rs` should own CLI parsing, terminal/process lifecycle, and wiring. Feature state belongs in engine/backend or TUI model.
- **Effects are explicit and isolated.** Filesystem scans, transcript loads, enrichment requests, persistence, and resume handoff should be triggered as named effects, not hidden in render or generic loop code.
- **Deletion requires a successful authoritative scan.** Incremental sync should only delete rows for an adapter/source that was scanned successfully.
- **Rows are summaries; transcripts and indexed content are separate data.** Display rows should not carry large indexed text unless the caller explicitly requests it.
- **The Rust backend should be frontend-neutral.** Backend APIs should expose stable data and events without Ratatui, Crossterm, terminal restoration, or Unix `exec` assumptions.
- **TUI mode must be visible.** Any modal/navigate/search mode should be reflected in footer/help text and have an obvious escape path.
- **Rendering stays viewport-bounded and side-effect-free.** Render functions may format visible data; they should not request work, perform I/O, or mutate backend state.
