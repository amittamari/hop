# Architecture

`hop` is a small layered Rust application. The important constraint is that agent
data is normalized at the boundary, then the rest of the system works with stable
domain types.

## How To Use This Document

This file is the current architecture contract, not a historical review. Keep it
durable and update it when the implementation changes. Rules use stable IDs so
reviews, plans, and issues can refer to them without ambiguity.

- Do not renumber existing IDs unless the rule itself is removed. Gaps are OK.
- Add new IDs for new rules.
- When a rule is no longer true or no longer desired, update or remove it in the
  same change that fixes the code.
- Put temporary defects, migration notes, and known-but-not-yet-fixed concerns in
  `Known Pressure Points` or a dated `docs/reviews/` artifact, not in stable
  boundary language.

## Data Flow

```text
CLI/config
  -> Engine
  -> Adapters scan and parse session files
  -> SearchIndex stores searchable Session documents
  -> TUI renders results and selected transcript
  -> Resume execs the selected agent command
```

On launch, the foreground engine opens the existing Tantivy index and searches it
immediately. A background sync opens another index handle, scans available
adapters, parses changed files, commits batches, and sends refresh updates to the
TUI loop.

## Module Responsibilities

- `src/core.rs`: shared domain types such as `Session`, `AgentId`, `Message`,
  `Block`, and source-agnostic helpers for title derivation, title
  normalization, and transcript flattening.
- `src/adapters/`: source-specific integration. Adapters scan files, parse raw
  JSONL into `core` types, extract source-specific metadata candidates, provide
  preview transcripts, and build resume commands.
- `src/query.rs`: parses user query text into a structured query independent of
  Tantivy.
- `src/index.rs`: owns Tantivy schema, upsert/delete, incremental diff helpers,
  and search execution. Indexed rows use a namespaced `agent:id` document key;
  the raw session id remains on `Session` for agent resume commands.
- `src/engine.rs`: UI-agnostic orchestration for query state, search results,
  debouncing, and background sync.
- `src/tui/`: terminal state and rendering. `App` owns interaction state;
  `view`, `results_list`, `preview`, `help`, and `keymap` split display concerns.
- `src/enrich/`: per-session display enrichment. Fast enrichers are local and
  synchronous; slow enrichers run through `EnrichmentService`.
- `src/columns.rs`: column definitions and responsive width solving.
- `src/resume.rs`: terminal-safe process handoff through `exec`.
- `src/config.rs`: optional TOML config and persisted UI state.

## Stable Boundaries

- **B-001 Raw Agent Boundary:** Raw agent JSON belongs only in adapters.
- **B-002 Tantivy Boundary:** Tantivy types belong only in `index.rs`.
- **B-003 Terminal UI Boundary:** Crossterm and ratatui types belong only in
  `tui/` and the top-level run loop.
- **B-004 External Command Boundary:** External commands such as `gh`, `claude`,
  and `codex` should be isolated behind enrichers, adapters, or resume handoff
  code.
- **B-005 Session Contract:** `core::Session` is the cross-module contract for
  indexed and displayed rows. It carries the source JSONL path so preview loading
  can re-parse the selected transcript without rediscovering files through an
  adapter scan.
- **B-006 Query Semantics Boundary:** Parsed query semantics belong in
  `src/query.rs`. Search, preview matching, autocomplete, and filter summaries
  should consume the parsed query shape rather than reinterpreting raw query text
  independently.
- **B-007 Runner Boundary:** The runner owns lifecycle and wiring, not feature
  state. `main.rs` should stay close to CLI parsing, terminal/process lifecycle,
  and top-level service wiring; reusable behavior belongs in `engine`,
  backend-facing modules, or `tui::App`.
- **B-008 Render Boundary:** Rendering is a terminal concern only. Render
  functions may format visible data, but should not request background work,
  perform filesystem/process I/O, or mutate backend state.
- **B-009 Frontend-Neutral Backend:** Keep the Rust backend frontend-neutral
  where possible. Backend-facing APIs should expose stable data and events
  without assuming ratatui, crossterm, terminal restoration, or Unix `exec`
  handoff.
- **B-010 Core Derivation Boundary:** Source-specific parsing belongs in
  adapters, but cross-agent derivation policy belongs in `core`. If Claude and
  Codex need the same rule, such as title fallback order, title whitespace
  normalization, or transcript flattening, prefer a shared core helper over
  duplicating the policy in each adapter. Width-specific truncation remains a
  rendering concern.

## Current Invariants

- **I-001 Non-Fatal Sync:** Index sync is incremental and non-fatal: parse
  errors skip individual files.
- **I-003 Schema Versioning:** Schema changes must bump `SCHEMA_VERSION` in
  `src/index.rs`.
- **I-004 Shared Transcript Extraction:** Search content and preview transcript
  should be produced from the same adapter extractor so filtering rules do not
  diverge.
- **I-005 No Slow Work On UI Thread:** Slow enrichment must never run on the UI
  thread.
- **I-006 Viewport-Bounded Rendering:** Result rendering should stay
  viewport-bounded; only visible rows should build formatted list items or
  request display enrichment.
- **I-009 Terminal Restoration:** Resume should never leave the terminal in raw
  mode.

## Application Pattern

The TUI currently follows a small-app Ratatui shape: one `App` model handles key
updates and focused view modules render the model. As interaction grows, prefer a
TEA-like loop over ad hoc state in the runner:

```text
event -> message -> update model -> explicit effects -> render model
```

Effects include search, sync refresh, transcript loading, enrichment requests,
UI-state persistence, and resume handoff. Keeping those effects explicit makes
the terminal UI easier to test and keeps a path open for non-terminal frontends.

## Known Pressure Points

These are current architectural seams worth tracking before expanding the app:

- **P-001 Reserved Config Fields:** `theme` and arbitrary `[keybindings]` config
  tables are parsed as reserved forward-compatible fields but are not applied
  yet.
- **P-002 Directory Filter Post-Filtering:** Directory filters are substring
  filters and therefore still run after Tantivy retrieval, but search now
  paginates until enough filtered-in rows are found or the matching hit set is
  exhausted.
- **P-003 Broad Session Type:** The current `core::Session` is used both as
  indexed document and display row. If frontend portability or memory pressure
  becomes important, split row summaries, indexed content, transcript data, and
  resume targets into narrower types.
- **P-004 Runner-Orchestrated Preview And Enrichment:** Enrichment and preview
  memoization currently have some orchestration in the top-level TUI runner.
  Prefer moving durable behavior into engine/backend or explicit TUI
  model/effect state before adding more providers or preview modes.
- **P-005 Sync Deletion Safety Gap:** Current sync diffs all known rows against
  all scanned rows, so a missing or failed adapter scan can be interpreted as
  deletions. Target behavior: delete rows only for a source that scanned
  successfully and authoritatively, and surface failed scans as sync errors.
- **P-006 TUI Mode Visibility Gap:** Modal-keymap navigate/search state is not
  explicit enough in the rendered UI. Target behavior: help and footer text
  should match the current mode, and every modal or navigate mode should have an
  obvious escape path.

Pressure points are not permanent architecture. Do not fix them opportunistically
during unrelated work, but when a pressure point is resolved, update or remove
its `P-*` entry in the same change.
