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
  `Known Pressure Points`, not in stable boundary language.

## Data Flow

```text
CLI/config
  -> Engine
  -> Adapters scan and parse session files
  -> SearchIndex stores searchable Session documents
  -> Engine exposes SessionSummary rows, Transcript data, and ResumeCommand data
  -> TUI renders results and selected transcript
  -> Resume runs any prepare step, then execs the selected agent command
```

On launch, the foreground engine opens the existing Tantivy index and searches it
immediately. A background sync opens another index handle, scans available
adapters, parses changed files, commits batches, and sends refresh updates to the
TUI loop. Deletions are scoped to adapters that completed a successful
authoritative scan; unavailable adapters and scan failures preserve existing
rows and surface as sync status warnings.

## Module Responsibilities

- `src/core.rs`: shared domain types such as `Session`, `SessionSummary`,
  `Transcript`, `ResumeCommand`, `AgentId`, `Message`, `Block`, and
  source-agnostic helpers for title derivation, title normalization, transcript
  text filtering, and transcript flattening.
- `src/adapters/`: source-specific integration. Adapters scan files, parse raw
  JSONL into `core` types, extract source-specific metadata candidates, provide
  preview transcripts, and build resume commands.
  The Codex adapter treats plain `.jsonl` and compressed `.jsonl.zst` rollouts
  as representations of the same session and selects the transcript record
  family from persisted `history_mode`, with a non-empty fallback for malformed
  or transitional files.
- `src/query.rs`: parses user query text into a structured query independent of
  Tantivy.
- `src/index.rs`: owns Tantivy schema, upsert/delete, incremental diff helpers,
  and search execution with recency-boosted ranking. Indexed rows use a
  namespaced `agent:id` document key; the raw session id remains on `Session`
  for agent resume commands. Source mtimes and hook-sidecar stamps are tracked
  separately so either kind of change can trigger an incremental reindex.
- `src/engine.rs`: UI-agnostic orchestration for query state, search results,
  transcript loading, resume command construction, debouncing, and background
  sync status.
- `src/tui/`: terminal state and rendering. `App` owns interaction state;
  `view`, `results_list`, `preview`, `help`, `toolbar`, and `keymap` split display
  concerns. `App` also owns the `SearchMode` (simple vs raw): simple mode treats
  the query line as free text and exposes a `toolbar` of Scope/Sort controls that
  compose into the same search the raw DSL drives (`query::compose_simple` +
  `engine.set_sort`), so the Tantivy layer is unaware of the mode. The results
  list has two rendering paths controlled by `[display] row_style`: `card` (default)
  renders multi-line cards with metadata and optional KWIC snippets via manual
  `Rect` layout; `compact` uses the legacy single-line `Table` with the column
  solver. `tui/glyphs.rs` owns the centralized glyph vocabulary (`Glyphs`),
  mirroring `theme::Theme`: a single value chosen once at startup and carried on
  `App` (read via `App::glyphs()`). It has a `nerd` variant (Private Use Area
  icons, the `[display] icons` opt-out default) and an `ascii` variant that
  reproduces the pre-icon look with no tofu; field-icon accessors return the
  empty string in `ascii`. Chrome glyphs (selection marker, accent bar,
  separator, spinner, archived marker, and the field/status icons) resolve
  through `Glyphs`; the preview transcript's content prefixes (`●`/`›`/`•`) stay
  literal in `preview.rs` as a deliberate content-layer exception (icons live in
  chrome, not content). Per-agent mark glyphs come from `Adapter::agent_glyph`
  (B-011) and are injected into `Glyphs` by position in `AgentId::ALL`, so the
  `tui` layer never names an agent-specific glyph literal.
- `src/enrich/`: per-session display enrichment. Fast enrichers are local and
  synchronous; slow enrichers run through `EnrichmentService`.
- `src/tui/columns.rs`: column definitions and responsive width solving.
- `src/resume.rs`: terminal-safe process handoff through `exec`, plus an
  optional run-and-wait prepare step (e.g. `codex unarchive <id>` for archived
  sessions) executed after terminal restore and before the resume `exec`.
- `src/config.rs`: optional TOML config, persisted UI state, and launcher
  override (`[launcher]` section with `{agent}` template for custom resume
  binaries). `[display]` section controls `row_style` (card/compact) and `icons`
  (nerd-font icon layer, default on / opt-out).
- `src/update.rs`: background update checker. Queries GitHub releases API at
  startup (cached 24 hours), detects install method (Homebrew vs cargo), and
  shows a compact upgrade indicator (`↑ v<version>`) in the TUI footer status.

## Stable Boundaries

- **B-001 Raw Agent Boundary:** Raw agent JSON belongs only in adapters.
- **B-002 Tantivy Boundary:** Tantivy types belong only in `index.rs`.
- **B-003 Terminal UI Boundary:** Crossterm and ratatui types belong only in
  `tui/` and the top-level run loop.
- **B-004 External Command Boundary:** External commands such as `gh`, `claude`,
  and `codex` should be isolated behind enrichers, adapters, or resume handoff
  code.
- **B-005 Session Data Shapes:** `core::Session` is the full parsed/indexed
  document shape. `core::SessionSummary` is the display/search result row shape
  and carries the source JSONL path so preview loading can re-parse the selected
  transcript without rediscovering files through an adapter scan. Transcript
  content and resume commands use `core::Transcript` and `core::ResumeCommand`
  instead of overloading result rows.
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
  normalization, command-tag filtering, or transcript flattening, prefer a
  shared core helper over duplicating the policy in each adapter. Width-specific
  truncation remains a rendering concern.
- **B-011 Adapter Vocabulary Containment:** Agent-specific knowledge — format
  quirks, magic field values, enum variants, and per-agent judgments (e.g. which
  Codex `SessionSource` values are non-interactive, which model sentinels are
  synthetic) — must live inside that agent's adapter module. Generic layers
  (`engine`, `core`, `index`, `tui`) must never name an agent-specific constant,
  string literal, or `adapters::<agent>::…` symbol in non-test code; they reach
  adapters only through the `Adapter` trait and `core` types. When a generic layer
  needs an agent-specific decision, add an agent-agnostic `Adapter` trait method
  with a safe default and let the adapter override it (see `Adapter::is_interactive`),
  rather than importing the adapter's logic upward. This extends `B-001` (raw JSON
  stays in adapters) from data to *semantics*, and complements `B-010` (genuinely
  cross-agent policy belongs in `core`).

## Current Invariants

- **I-001 Non-Fatal Sync:** Index sync is incremental and non-fatal: parse
  errors skip individual files, adapter scan failures preserve that adapter's
  indexed rows, and unavailable adapters are reported without forcing deletion.
  Sessions that parse to no usable conversation (zero messages or empty content,
  e.g. a Cursor subagent spawn the model blocked before any reply) are counted as
  `empty_sessions` and skipped rather than indexed. As a sibling filter, sessions
  the producing adapter reports as non-interactive (via `Adapter::is_interactive`;
  Codex sub-agent / memory-consolidation / exec-startup threads) are counted as
  `non_interactive_sessions` and skipped. The judgment stays in the adapter — the
  engine only knows the agent-agnostic concept — and is fail-open, so absent or
  unrecognized sources are always kept.
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
- **I-010 Display Width:** Column, row, modal, and fixed-width terminal fitting
  should use terminal display width rather than Unicode scalar counts.
- **I-011 Modeless Ctrl-Chord Keymap:** The keymap is modeless — typing always
  edits the query, navigation lives on the arrows, and every configurable action
  chord must include Ctrl (`keymap::parse_chord`), so no key does double duty.
  Vim-style modal navigation was considered and declined (issue #32): a persistent
  query field makes bare `j/k/h/l` navigation impossible without a mode; the closest
  peer, Codex's `/resume` picker, is likewise modeless and explicitly suppresses
  bare-key navigation while typing; and `hop` already collapsed an earlier
  `Search`/`Modal` preset system to this single model (commit `5bfa443`). Search
  *complexity* is instead handled by the simple/raw search-mode toggle (which
  chooses how the query line is interpreted), not by adding key modes.

## Application Pattern

The TUI currently follows a small-app Ratatui shape: one `App` model handles key
updates and focused view modules render the model. Keymaps decode raw keys into
TUI-local commands; `tui::Action` is reserved for effects the outer run loop must
perform, such as search, quit, and resume. As interaction grows, prefer a TEA-like
loop over ad hoc state in the runner:

```text
event -> message -> update model -> explicit effects -> render model
```

Effects include search, sync refresh, transcript loading, enrichment requests,
UI-state persistence, and resume handoff. Keeping those effects explicit makes
the terminal UI easier to test and keeps a path open for non-terminal frontends.

## Known Pressure Points

These are current architectural seams worth tracking before expanding the app:

- **P-001 Reserved Config Fields:** The `theme` config table is parsed as a
  reserved forward-compatible field but is not applied yet. (`[keybindings]` is
  now applied — `tui::keymap::Keymap::from_config` resolves Ctrl-chord overrides
  at launch.)
- **P-002 Directory Filter Post-Filtering:** Directory filters are substring
  filters and therefore still run after Tantivy retrieval, but search now
  paginates until enough filtered-in rows are found or the matching hit set is
  exhausted.
- **P-003 Agent Color Matches In The TUI Layer:** `Theme::agent_color`
  (`tui/theme.rs`) matches `AgentId` to a brand RGB inside the generic `tui`
  layer, which bends `B-011`. The newer per-agent glyph deliberately routes
  through `Adapter::agent_glyph` instead; realigning `agent_color` the same way
  (adapter-provided brand color injected into `Theme`) is the intended fix but is
  out of scope for the icon facelift.

Pressure points are not permanent architecture. Do not fix them opportunistically
during unrelated work, but when a pressure point is resolved, update or remove
its `P-*` entry in the same change.
