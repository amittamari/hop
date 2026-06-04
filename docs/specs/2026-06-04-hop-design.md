# hop — Design Spec

**Date:** 2026-06-04
**Status:** Approved design, pre-implementation
**Lineage:** Ground-up Rust rewrite of [`fast-resume`](https://github.com/angristan/fast-resume) (`fr`), a Python TUI for searching and resuming coding-agent sessions.

---

## 1. Purpose

`hop` is a fast terminal tool that aggregates coding-agent session history into a single
full-text-searchable index and lets you jump straight back into any past session — "hop back
into a session." You type a few words you remember saying (or that the agent said), pick the
result, hit Enter, and you're resumed in the original agent, in the original working directory.

It keeps the proven core of `fast-resume` (unified full-text search across agents, fuzzy
matching, direct resume handoff) and rebuilds the execution to fix three concrete problems with
the original:

1. **Scroll lag** in the result list.
2. **Slow-feeling cold indexing** on first run.
3. **Unpolished TUI behavior** — e.g. modals that ignore Esc/Ctrl+C, dead-end states.

`hop` is native Rust end-to-end (the original used Tantivy only via Python bindings), eliminating
the interpreter and binding overhead.

---

## 2. Goals & non-goals

### Goals
- **Interactive in < 50 ms** on every launch, regardless of history size.
- **Smooth scrolling** through thousands of results.
- **Polished, predictable TUI**: no dead-ends, consistent quit/cancel semantics, terminal always
  restored.
- **High-quality search** preserved: exact (BM25, boosted) + typo-tolerant fuzzy ranking over
  full conversation content, not just titles.
- **Clean extension points** so additional agents are added without touching the core.

### Non-goals (v1)
- Supporting every agent up front. v1 ships **Claude Code + Codex** only.
- Feature-completeness with `fast-resume`. `--stats`, update notifications, non-interactive/`--list`
  modes, and the niche agents are explicitly deferred.
- Inline raster/image icons. Replaced by colored text badges (see §4).
- A plugin system loaded at runtime. "Extensible" here means a clean in-tree trait, not dynamic
  plugins.

---

## 3. Scope

| Area | v1 | Deferred |
| --- | --- | --- |
| Agents | Claude Code, Codex | Copilot CLI/VSCode, OpenCode, Crush, Vibe |
| Search | Full-text content, keyword filters, fuzzy + exact ranking | — |
| Keyword syntax | `agent:`, `dir:`, `date:`, negations (`!`/`-`), free text, `Tab` autocomplete | — |
| Resume | Exec handoff, yolo mode (incl. Codex auto-detect) | — |
| Index mgmt | `--rebuild` | — |
| Config | Optional TOML: theme, keybindings, extra data dirs | — |
| Modes | Interactive TUI | `--stats`, `--list`, `--no-tui`, update notifications |

---

## 4. UX decisions (validated via mockups)

- **Result rows — colored text badges.** Each row leads with a short colored agent tag
  (`CLAUDE` in purple, `CODEX` in blue), followed by `title · directory · relative-time`. No
  raster icons. This removes the per-row image-decode cost (a primary scroll-lag culprit) and the
  terminal-compatibility problems (no "Ghostty recommended / iTerm freezes" caveats); it works in
  every terminal with zero font dependencies.
- **Layout — vertical split.** Search input on top; results list on the left; an always-on live
  preview pane on the right; a key-hint footer at the bottom. When a query is active, the preview
  scrolls to the first match and highlights matched terms.
- **TUI polish is a first-class requirement and a tested invariant:**
  - **Esc** and **Ctrl+C** always quit from the main view.
  - **Esc** always closes any modal/overlay (returning to the prior view, choosing nothing).
  - No state the user can enter but not leave.
  - The terminal is always restored to its prior state on exit — normal quit, resume handoff, or
    panic (via a Drop guard around raw-mode/alt-screen setup).

### Row anatomy (illustrative)
```
❯ auth refresh                                          42/512
┌───────────────────────────────┬────────────────────────────┐
 CLAUDE fix auth middleware bug  │ ~/work/api · 2h · 48 msgs
 CODEX  refactor auth guard      │
 CLAUDE add auth retry logic     │ » the auth middleware was
 CODEX  debug login auth flow    │   dropping the refresh
 CLAUDE oauth token refresh      │   token on retry. ␣␣I see…
└───────────────────────────────┴────────────────────────────┘
↑↓ move · enter resume · tab yolo · esc quit
```

---

## 5. Architecture

The system is decomposed into modules with single responsibilities and explicit interfaces. Each
is independently testable; the UI layer is kept free of I/O and indexing logic.

### 5.1 `core` — domain
Pure data types, no I/O.
- `Session { id, agent: AgentId, title, directory, timestamp, content, message_count, mtime, yolo }`
- `AgentId` enum (extensible).
- Title truncation / normalization helpers.

### 5.2 `adapters` — agent integration
A trait plus a registry. Each adapter knows how to find, parse, and resume one agent's sessions.

```rust
trait Adapter {
    fn id(&self) -> AgentId;
    fn is_available(&self) -> bool;                       // data dir exists
    fn scan(&self) -> Result<HashMap<SessionId, ScanEntry>>; // id -> (path, mtime), cheap
    fn parse(&self, path: &Path) -> Result<Session>;      // full parse of one file
    fn resume_command(&self, s: &Session, yolo: bool) -> Vec<String>;
    fn supports_yolo(&self) -> bool;
}
```

- **`scan`** is intentionally cheap (stat-level), so incremental sync can decide what to re-parse
  without reading file bodies.
- v1 impls: `ClaudeAdapter` (JSONL in `~/.claude`), `CodexAdapter` (JSONL in
  `~/.codex/sessions`, yolo auto-detected from session metadata).
- Indexed content = user text messages + assistant text responses. Excluded: tool calls/results,
  meta/system messages, local-command output. (Same content policy as `fast-resume`.)

### 5.3 `query` — query parsing
Parses the raw search string into a structured query:
- Free-text terms.
- `agent:claude,codex`, `-agent:vibe`, `agent:claude,!codex`.
- `dir:substr`, negated dirs.
- `date:today|yesterday|week|month`, `date:<1h`, `date:>1w`, `date:<2d`.
- Drives `Tab` autocomplete (e.g. `agent:cl` → `agent:claude`).

Output is a `ParsedQuery { free_text, agent_filter, dir_filter, date_filter }` consumed by the
index layer. No Tantivy types leak into the parser.

### 5.4 `index` — search engine (Tantivy, native)
- Schema: `id`, `agent`, `title`, `content`, `directory`, `timestamp`, `mtime`, `message_count`,
  `yolo`. A `.schema_version` marker file; on mismatch the index is dropped and rebuilt.
- **Query builder** reproduces `fast-resume` ranking: exact parsed query over `title`+`content`
  boosted ~5×, OR'd with edit-distance-1 prefix fuzzy term queries per word over `title`+`content`
  for typo tolerance. Filters (`agent`/`dir`/`date`) applied as boolean constraints.
- **Incremental sync**: given the index's known `(id → mtime)` map and an adapter's `scan` output,
  re-parse only entries where `mtime > known + 1ms`; upsert them; detect deletions (known-but-absent)
  and remove them. Commits are **batched** so results appear progressively.
- Index lives under the platform cache dir (e.g. `~/.cache/hop/` via the `directories` crate).

### 5.5 `engine` — orchestration (UI-agnostic)
Owns application state and the "feels instant" lifecycle. Knows nothing about ratatui.
- On start: open (or create) the index, run an immediate query against whatever is already indexed,
  and return results synchronously so the UI can render at once.
- Spawn a **background sync task**: for each available adapter, `scan` → diff against index →
  parse changed files → batched upsert/delete → reload reader → push a "refresh" signal over a
  channel.
- Search is **debounced** (~40 ms) to avoid querying on every keystroke.
- Exposes: current results, selection state, query state, and an update channel the UI drains.

### 5.6 `tui` — presentation (ratatui + crossterm)
- Renders search input, results list, preview pane, footer, and modals.
- **Viewport-only rendering**: only rows visible in the current scroll window are formatted/drawn;
  off-screen rows do zero work; no image decoding anywhere.
- Key map centralizes bindings; the quit/cancel invariants from §4 are enforced here and tested.
- Drains the engine's update channel between frames to fold in streamed sessions and search results.

### 5.7 `resume` — handoff
Builds the resume command from the selected session's adapter, then:
1. Tears down the TUI / restores the terminal.
2. `chdir` to the session's directory.
3. `exec`-replaces the current process with the agent CLI (so shell history shows
   `claude --resume …`, not `hop`, and there's no lingering parent process).

Resume command table (v1):

| Agent | Resume | With `--yolo` |
| --- | --- | --- |
| Claude | `claude --resume <id>` | `claude --dangerously-skip-permissions --resume <id>` |
| Codex | `codex resume <id>` | `codex --dangerously-bypass-approvals-and-sandbox resume <id>` |

Codex yolo is auto-detected from session metadata; for agents that support yolo but don't store it,
a modal asks (Tab toggles, Enter confirms, Esc cancels). `--yolo` forces it without prompting.

### 5.8 `config` — optional configuration
- TOML at the platform config dir (e.g. `~/.config/hop/config.toml`).
- Keys: theme colors (per-agent + UI accents), keybinding overrides, additional/override data
  directories per agent.
- Fully optional; zero-config defaults match the validated UX.

### 5.9 `cli` — entry point (clap)
- `hop [QUERY]` — open the TUI, optionally pre-filtered.
- `-a, --agent <name>` — filter by agent.
- `-d, --dir <substr>` — filter by directory.
- `--yolo` — force yolo resume when supported.
- `--rebuild` — wipe and rebuild the index.

---

## 6. Data flow

```
CLI parse ──► initial ParsedQuery
   │
   ▼
engine.start() ─ open/create index ─► immediate query ─► initial results
   │                                                        │
   ├─ spawn background sync ──┐                             ▼
   │                          │                    tui renders (<50ms)
   │   adapters.scan()        │                             │
   │   diff vs index          │   keystroke ─► debounce ─► query ─► update visible rows
   │   parse changed          │                             │
   │   batched upsert/delete  │                             │
   │   reload reader ─────────┴── "refresh" over channel ──►┘  (rows stream in live)
   │
Enter ─► resolve resume cmd ─► restore terminal ─► chdir ─► exec agent CLI
Esc / Ctrl+C ─► clean quit (terminal restored)
```

---

## 7. Performance strategy

| Lever | Effect |
| --- | --- |
| Native Rust + SIMD JSON parsing | Cold parse/index far faster than the Python original |
| Open index immediately, never block on a full build | Interactive in < 50 ms every launch |
| Background, batched, incremental indexing | Sessions stream in; no blocking spinner |
| Viewport-only rendering, no per-row image decode | Smooth scrolling at any list size |
| Debounced search (~40 ms) | No wasted queries while typing |
| Channel-based UI updates | Render thread never blocks on indexing |

---

## 8. Error handling

- **Parse errors** are non-fatal: skip the offending file, continue; surface an optional error
  count in the footer.
- **Index corruption / schema mismatch**: transparently drop and rebuild.
- **Missing agent data dir**: treated as zero sessions for that agent (and its previously-known
  sessions are pruned).
- **Panics**: the terminal is still restored via a Drop guard so the shell is never left in raw
  mode / alt-screen.

---

## 9. Testing strategy

- **Adapter parsing** — unit tests against checked-in fixture session files of real shape (sample
  Claude JSONL and Codex JSONL), covering content extraction and the exclusion policy.
- **Query parser** — unit tests for every keyword form, multi-value, and negation, plus
  autocomplete completion.
- **Index** — integration tests: build from scratch, incremental update (mtime change re-parses),
  deletion detection, schema-version-triggered rebuild, ranking (exact boosted above fuzzy).
- **Resume** — command-generation tests including yolo and Codex auto-detect.
- **TUI behavior** — ratatui `TestBackend` tests asserting the polish invariants: **Esc quits**,
  **Esc closes a modal**, **Ctrl+C quits**, navigation moves selection, debounced query updates
  results.

---

## 10. Distribution

- `cargo install hop`
- Homebrew tap
- Prebuilt macOS/Linux binaries via cargo-dist + GitHub Actions on tagged releases.

---

## 11. Proposed crate layout

```
hop/
├── Cargo.toml
├── src/
│   ├── main.rs            # thin: parse CLI, run engine+tui, perform resume exec
│   ├── cli.rs             # clap definitions
│   ├── core/              # Session, AgentId, helpers
│   ├── adapters/          # trait, registry, claude.rs, codex.rs
│   ├── query.rs           # keyword/free-text parser + autocomplete
│   ├── index.rs           # Tantivy schema, query builder, incremental sync
│   ├── engine.rs          # orchestration, state, background sync, debounce
│   ├── tui/               # app, results_list, preview, modal, keymap, theme
│   ├── resume.rs          # command build + exec handoff
│   └── config.rs          # TOML load + defaults
├── tests/                 # integration tests
│   └── fixtures/          # sample claude/codex session files
└── docs/specs/2026-06-04-hop-design.md
```

### Key dependencies (intended)
`ratatui` + `crossterm` (TUI), `tantivy` (search), `clap` (CLI), `serde` + a SIMD JSON parser
(`sonic-rs`/`simd-json`) (parsing), `directories` (paths), `toml` (config),
`chrono`/`jiff` (dates), `anyhow`/`thiserror` (errors). Background work via std threads + channels
(no async runtime required for v1).
