# Architecture

`hop` is a small layered Rust application. The important constraint is that agent
data is normalized at the boundary, then the rest of the system works with stable
domain types.

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
  `Block`, and helpers for titles and transcript flattening.
- `src/adapters/`: source-specific integration. Adapters scan files, parse raw
  JSONL into `core` types, provide preview transcripts, and build resume commands.
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

- Raw agent JSON belongs only in adapters.
- Tantivy types belong only in `index.rs`.
- Crossterm and ratatui types belong only in `tui/` and the top-level run loop.
- External commands such as `gh`, `claude`, and `codex` should be isolated behind
  enrichers, adapters, or resume handoff code.
- `core::Session` is the cross-module contract for indexed and displayed rows.
  It carries the source JSONL path so preview loading can re-parse the selected
  transcript without rediscovering files through an adapter scan.

## Invariants

- Index sync is incremental and non-fatal: parse errors skip individual files.
- Schema changes must bump `SCHEMA_VERSION` in `src/index.rs`.
- Search content and preview transcript should be produced from the same adapter
  extractor so filtering rules do not diverge.
- Slow enrichment must never run on the UI thread.
- Result rendering should stay viewport-bounded; only visible rows should build
  formatted list items or request display enrichment.
- Resume should never leave the terminal in raw mode.

## Known Pressure Points

These are current architectural seams worth tracking before expanding the app:

- `theme` and arbitrary `[keybindings]` config tables are parsed as reserved
  forward-compatible fields but are not applied yet.
- Directory filters are substring filters and therefore still run after Tantivy
  retrieval, but search now paginates until enough filtered-in rows are found or
  the matching hit set is exhausted.

Do not fix these opportunistically during unrelated work, but account for them
when touching the surrounding code.
