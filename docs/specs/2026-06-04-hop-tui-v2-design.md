# hop — TUI v2 Design Spec

**Date:** 2026-06-04
**Status:** Approved design, pre-implementation
**Lineage:** Iteration on the shipped `hop` v1 (see `2026-06-04-hop-design.md`). v1's core flow
(index → search → resume) is verified; this spec redesigns the **experience**: a readable
preview, a columnar result list backed by a pluggable enrichment architecture, and a richer,
configurable keymap with help.

---

## 1. Purpose & motivation

v1 works but is bare-bones and hard to read:

1. **Preview is raw.** It dumps the flat indexed `content` verbatim, so internals like
   `[external_agent_tool_call]`, `<command-*>` tags, slash-command expansions, tool results, and
   system reminders bleed into the conversation. No role separation, no syntax highlighting, no
   match scrolling, no size/visibility control.
2. **Left pane is unreadable.** A single cramped line per row (badge + title + full directory +
   time). No columns; the full directory path dominates and the title is hard to scan.
3. **Almost no keybinds.** Only `↑↓`, Enter, Tab, Esc, typing — no help, no preview controls, no
   paging. `Tab` is ambiguously overloaded as "yolo," colliding with the documented
   `Tab`-autocomplete.

This spec fixes all three while preserving v1's invariants: interactive in < 50 ms, smooth
scrolling, polished/predictable TUI, terminal always restored.

---

## 2. Goals & non-goals

### Goals
- A **clean, readable transcript preview**: internals filtered out, roles separated, prose and code
  formatted, matches highlighted and scrolled to.
- A **columnar, scannable result list** driven by a **pluggable column + enrichment architecture**
  (the "pluggable arch" promised in v1).
- Display **git branch sourced from conversation data**, plus repo and (background) GitHub PR.
- A **richer, configurable keymap** with a help overlay; default always-typing preset and an opt-in
  modal preset.
- Preserve every v1 performance and polish invariant.

### Non-goals
- No raster/image icons (unchanged from v1; colored text badges only).
- No runtime plugin loading — "pluggable" means clean in-tree traits + registries.
- No new GitHub SDK — PR lookup shells out to the existing `gh`/`git` CLIs.
- No re-architecture of the index/search ranking or the resume handoff.

---

## 3. Content model & preview rendering

### 3.1 Structured messages (shared extractor)

The root cause of the unreadable preview is that `Session.content` is a flat, role-less string.
We separate **search content** from **preview structure** via a single shared extractor:

- Each adapter parses a file into `Vec<Message>`:
  ```rust
  enum Role { User, Agent }
  enum Block { Prose(String), Code { lang: Option<String>, text: String } }
  struct Message { role: Role, blocks: Vec<Block> }
  ```
- **All internal-filtering happens here, once**, so search and preview always agree on what counts
  as "conversation." Filtered out: tool calls/results, `<command-name|message|args|stdout|caveat>`,
  slash-command expansions, `[external_agent_tool_call|result|…]` markers, system reminders,
  `isMeta` lines, and `toolUseResult` payloads.
- The **index** stores a flattened string (`content` = prose + code text joined) purely for
  BM25/fuzzy search — schema unchanged, just fed from the structured form.
- `parse()` (for indexing) and the preview both build on this shared extractor, so the two never
  diverge.

### 3.2 Branch & repo from conversation data

Both agents persist git metadata inside the session, so the branch needs **no filesystem lookup**:

- **Claude:** every line carries `"gitBranch":"<branch>"`. Capture the first non-empty value.
- **Codex:** `session_meta.payload.git = { branch, commit_hash, repository_url }`. Capture `branch`
  and `repository_url`.

`core::Session` gains:
- `branch: Option<String>` — captured during parse, **added to the index schema** so it is
  available without re-parsing.
- `repo_url: Option<String>` — from Codex `repository_url` (Claude has none).

Branch reflects the branch **at session time**, which is more meaningful for resume and still works
even if the directory was later deleted.

### 3.3 On-demand preview parse

The preview **re-parses the selected file on demand** into `Vec<Message>` and renders it:

- Re-parsing one JSONL on selection (debounced) is sub-ms to a few ms and keeps the index lean.
- If the source file is gone, the preview falls back to the flat indexed `content` with a dim
  "source unavailable" note.

### 3.4 Rendering (clean transcript)

- **Roles:** user turns prefixed `›` (accent); agent turns prefixed `●` + agent-colored badge.
- **Prose:** light markdown — headers, **bold**, lists, inline `code` — rendered as ratatui spans
  (no raw `#`/`*` noise).
- **Code blocks:** real syntax highlighting via `syntect` (lazy-loaded syntax/theme sets, converted
  to ratatui spans). Language taken from the fence with an alias map (`ts`→`typescript`,
  `py`→`python`, `sh`→`bash`, …); unknown/unlabeled → plain dim text.
- **Match handling:** when a query is active, the preview auto-scrolls to the first match and
  highlights all matched terms (reverse/bold).
- **Performance guard:** syntect highlighting + transcript parse run **only for the selected row**
  and are **memoized per selection**, never touching the launch or scroll hot path.

---

## 4. Left pane: pluggable columns + enrichment

Two cleanly separated layers.

### 4.1 Enrichment layer (data)

```rust
enum EnrichKind { Fast, Slow }     // Fast = sync/data-or-path-derived; Slow = background

trait Enricher {
    fn id(&self) -> &'static str;
    fn kind(&self) -> EnrichKind;
    fn resolve(&self, s: &Session) -> Option<EnrichValue>;
    fn cache_key(&self, s: &Session) -> String;   // e.g. "<repo>@<branch>"
    fn ttl(&self) -> Duration;
}
```

- **Fast enrichers** resolve synchronously for visible rows, no network:
  - `branch` — straight from `Session.branch` (data-derived). FS-git fallback (`.git/HEAD`) only when
    the field is absent (older Claude sessions, non-git dirs).
  - `repo` — Codex `repo_url` basename (e.g. `responsive-editor-packages`); else git-toplevel
    basename of the directory; else directory basename.
- **Slow enrichers** run on a dedicated worker thread with a work queue:
  - `gh_pr` — maps (repo, branch) → PR via the `gh`/`git` CLI. Results cached to disk under the
    cache dir keyed by `cache_key` with a TTL, and pushed back over the **same engine update
    channel** the background index sync already uses. The TUI only enqueues PR resolution for rows
    **currently in the viewport**, so it never does network work for off-screen rows. The cell shows
    `⟳` while pending, then the value (`#4821`) or `—` when resolved/absent.

### 4.2 Column layer (presentation)

```rust
struct Column {
    id: &'static str,
    header: &'static str,
    align: Align,
    priority: u8,            // higher drops first when narrow
    min_width: u16,
    color: fn(&Cell) -> Style,
}
```

- Columns read from `Session` fields + the enrichment map and produce aligned cells, rendered as a
  fixed-width span grid (replacing v1's single cramped `Line`).
- **Responsive:** when the list pane is narrow, columns drop by ascending `priority`
  (`PR` → `BRANCH` → `REPO` → `TIME` → `MSGS`); **TITLE always survives** and flexes to fill.
- **Default column set:** `AGENT` badge · `REPO` · `BRANCH` · `TITLE` (flex, match-highlighted) ·
  `MSGS` · `PR` · `TIME`. The raw **directory column is dropped entirely** — the full path appears
  only in the preview header for context.
- **Config-driven:** `config.toml` can reorder, enable/disable, and re-prioritize columns;
  per-enricher toggles gate the slow providers. Zero-config defaults match the above.

Both registries (enrichers, columns) live alongside the adapter registry, so adding a provider is
in-tree and touches neither `core` nor `tui` internals.

---

## 5. Preview controls & keymap

### 5.1 Preview pane

- Two states: **split** (default) ↔ **hidden**, toggled with `Ctrl+P`.
- **Resize** the split with `[` / `]` (shrink/grow preview, clamped to sane min/max).
- Chosen width + last state **persist** to `config.toml` and survive restarts.
- Long conversations scroll **within** the split (`Ctrl+U`/`Ctrl+D`); there is no separate
  full-screen preview mode.

### 5.2 Keymap — two presets, config-selectable

Default `keymap = "search"` (always-typing); opt-in `keymap = "modal"`.

**Default (search) preset:**

| Key | Action |
| --- | --- |
| *(type)* | edit query (always live) |
| `↑` / `↓` | move selection |
| `PgUp` / `PgDn` | page the list |
| `Ctrl+U` / `Ctrl+D` | scroll preview |
| `[` / `]` | shrink / grow preview |
| `Ctrl+P` | toggle preview (split/hidden) |
| `Tab` | autocomplete keyword value (`agent:cl`→`agent:claude`) |
| `Enter` | resume selected |
| `Ctrl+Y` | resume with **yolo** (force) |
| `?` | help overlay |
| `Esc` / `Ctrl+C` | quit |

**Tab conflict resolved.** v1 overloaded `Tab` as "yolo," colliding with documented
`Tab`-autocomplete. `Tab` is now **keyword autocomplete**. Yolo is reached either via the existing
**resume modal** (Enter on a yolo-capable agent → modal where `Tab` toggles yolo, `Enter` confirms,
`Esc` cancels — unchanged) or directly via **`Ctrl+Y`**. `--yolo` still forces it from the CLI.

**Help overlay (`?`):** a centered modal listing the active keymap's bindings, grouped
(navigation / preview / actions). `Esc` or `?` closes it. Obeys the modal invariants (Esc always
closes, terminal always restored).

**Modal preset** (`keymap = "modal"`): `Esc` leaves the query into a navigate mode where single
letters work (`j`/`k` move, `g`/`G` top/bottom, `p` preview toggle, `/` to search again); all chord
bindings remain available.

---

## 6. Structure (module changes)

Extends the existing layout; does not reorganize it.

- `core` — add `Message`, `Role`, `Block`; `Session` gains `branch` and `repo_url`.
- `adapters` — `Adapter` gains `fn transcript(&self, path) -> Result<Vec<Message>>`; `parse` is
  refactored onto the shared message extractor so filtering lives in one place. Claude & Codex
  extractors extended to capture branch/repo and strip the §3.1 internals.
- `enrich/` *(new)* — `Enricher` trait + registry, `BranchEnricher`, `RepoEnricher`, `GhPrEnricher`,
  and an `EnrichmentService` (worker thread + disk cache + channel push).
- `columns.rs` *(new)* — `Column` + registry + responsive width/drop solver.
- `tui/` — split `view.rs` into focused pieces: `results_list.rs` (column grid), `preview.rs`
  (transcript renderer + syntect/markdown), `help.rs` (overlay), `keymap.rs` (two presets + config
  binding). `tui/mod.rs` (`App`) gains preview state (visible / width / scroll) and dispatches via
  the keymap.
- `config.rs` — add `[preview]` (state, width), `keymap` preset, `[columns]`
  (order / enabled / priority), per-enricher toggles.
- `engine.rs` — owns the `EnrichmentService`, folds enrichment results in via the existing update
  channel; debounced selection drives the on-demand transcript parse for the preview.
- `index.rs` — schema adds `branch` (stored/displayable); schema-version bump triggers the existing
  transparent rebuild.

### New dependencies
`syntect` (code highlighting; lazy-loaded default assets) and `pulldown-cmark` (prose markdown
tokenizing) → both converted to ratatui spans. PR lookup shells out to `gh`/`git` (no GitHub SDK).

---

## 7. Testing strategy

- **Adapter `transcript`** over fixtures: roles correct; code blocks captured with language; branch
  and repo captured (Claude `gitBranch`, Codex `payload.git`); **every** internal class filtered
  (tool calls/results, `<command-*>`, `[external_agent_*]`, system reminders, meta, `toolUseResult`).
- **Preview renderer** (TestBackend): user/agent prefixes; code highlighted; prose markdown; match
  highlighted and scrolled-to; source-unavailable fallback.
- **Column layout:** ordering; responsive drop order; alignment; title-always-survives; directory
  absent.
- **Enrichment:** fast enrichers (branch from data, repo from `repo_url`/temp git dir); slow
  enricher caching + TTL + pending→resolved transition; viewport-only enqueue.
- **Keymap:** both presets; help overlay open/close; `Tab` autocomplete; `Ctrl+Y` yolo; `Ctrl+P`
  toggle; `[`/`]` resize. **Invariants preserved:** Esc/Ctrl+C quit, Esc closes overlays, terminal
  restored.

---

## 8. Performance guards

- Launch still opens the index and renders immediately (< 50 ms); no enrichment or highlighting
  blocks first paint.
- syntect highlight + transcript parse run only for the selected row and are memoized per selection.
- PR enrichment is viewport-scoped, background, and disk-cached with a TTL.
- The column grid formats only visible rows (viewport-only rendering, unchanged from v1).
