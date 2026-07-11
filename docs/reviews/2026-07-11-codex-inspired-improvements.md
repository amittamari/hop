# Codex-Inspired Improvements — 2026-07-11

Findings from studying the upstream **Codex** codebase (a local checkout of it,
crate root `codex-rs/`) for changes worth making to `hop`. Codex is doubly
relevant: it is the authoritative **writer** of the Codex session files `hop`
parses, and it is itself a mature `ratatui` TUI with its own `/resume` picker, so
it is a source of inspiration on two fronts — adapter correctness and TUI craft.

This is a dated **review / action-item** artifact, not a commitment and not
stable architecture. It is deliberately broader than any single change: the
**Sev/Pri columns are triage signals**, and the intent is to spin off a focused
`docs/plans/` doc **per tier** (or per item) as each is picked up, deciding scope
there. Promote durable decisions into `docs/ARCHITECTURE.md` and resolve the
related pressure points (`P-001`) when the work lands. Do not treat the code
references as frozen — Codex is a moving target and line numbers will drift; the
type/constant names are the durable anchors.

All `codex.rs`/`core.rs`/`view.rs`/etc. references are in `hop`
(`src/…`). All `protocol.rs`/`policy.rs`/`resume_picker.rs`/etc. references are in
Codex (`codex-rs/…`), verified at Codex `main` on 2026-07-11 against real session
files under `~/.codex/sessions`.

## Overall Read

`hop`'s Codex adapter is correct **today** but rests on two format assumptions
Codex is actively moving away from — legacy `event_msg` transcripts and
uncompressed `.jsonl`. Both will fail **silently** (empty transcripts, missing
sessions) rather than loudly, which makes them the priority. Beyond correctness,
Codex writes far richer per-session metadata than `hop` extracts, and its TUI
crate has several cleanly liftable patterns (terminal-adaptive theming, a
`pulldown-cmark` renderer, `two-face` syntax breadth, `insta` snapshots) — most
notably a principled path to resolve the long-standing `P-001` theme pressure
point.

## Suggested Sequencing

If tiers are promoted to plans one at a time, this is the order that front-loads
risk reduction:

1. **Tier 1 correctness** (C1 `response_item`, C2 `.jsonl.zst`) — latent silent
   failures that trip as Codex rolls out paginated history and rollout
   compression. Highest priority despite not being broken *today*.
2. **Title/noise hygiene** (C3) — visible quality bug now; confirmed garbage
   titles on real sessions.
3. **Metadata enrichment** (M1–M3) — cheap, mostly additive, unlocks new columns
   and noise filtering.
4. **Theme refactor** (T1) — resolves `P-001` and is a prerequisite for
   cross-terminal correctness; pairs with the earlier TUI design review.
5. **TUI craft** (T2–T4) and **picker features** (U1–U4) — opportunistic quality,
   adopt selectively.

---

## Tier 1 — Adapter Correctness & Robustness

Files: `src/adapters/codex.rs`, `src/core.rs`. Touches `B-001` (raw agent
boundary), `I-001` (non-fatal sync), `I-004` (shared extraction).

| # | Sev | hop location | Codex reference | Finding | Direction |
|---|-----|--------------|-----------------|---------|-----------|
| C1 | **High** | `codex.rs:59-94` (match), `77-93` (msg extraction) | `policy.rs:104-115` (`should_persist_event_msg`); `protocol.rs:697` (`ThreadHistoryMode`); `models.rs:845,935` (`ContentItem`/`ResponseItem`) | `hop` extracts messages **only** from `event_msg` (`user_message`/`agent_message`). Codex persists those events **only** when `history_mode == Legacy`. In `Paginated` mode the transcript lives in `response_item` `message` records instead. All on-disk sessions are legacy today, but the paginated stack + recent "ordinals" work signal the migration. When flipped, `hop` will produce **empty transcripts and blank titles** for every new Codex session. | Add a `response_item` arm: `payload.type == "message"` → role `user`/`assistant`, text = concatenated `content[]` (`input_text`/`output_text` → text). Prefer `response_item` when present; fall back to `event_msg`. Optionally gate on `SessionMeta.history_mode` (absent ⇒ legacy). Keep search + preview extraction shared per `I-004`. |
| C2 | **High** | `codex.rs:321-342` (`collect_jsonl`), `331` (`extension == "jsonl"`) | `compression.rs:18` (`COMPRESSED_SUFFIX=".zst"`), `:60` (`compressed_rollout_path`), `:149` (`RolloutFile` reads either); `spawn_rollout_compression_worker` | Codex compresses older rollouts to `.jsonl.zst` in a background worker. `hop`'s scan filters `extension == "jsonl"`, so compressed files (extension `zst`) are skipped entirely — `hop` will **silently lose older sessions** once compression activates. None exist on disk yet. | Accept `*.jsonl.zst` in `collect_jsonl` and zstd-decompress before parsing (e.g. `zstd` crate). Ensure incremental mtime tracking still keys correctly on the compressed path. |
| C3 | Med | `codex.rs:108-173` (`clean_event_message`, `DROP_XML_BLOCKS` at 108-111); `core.rs:281-297` (`derive_session_title`) | `protocol.rs:102-121` (context tag constants), `:122-130` (`USER_MESSAGE_BEGIN` / `strip_user_message_prefix`), `:1440-1443` (review-mode events) | `hop` strips only `environment_context` + `system-reminder`. Codex wraps injected context in ~10 tags (`user_instructions`, `apps_instructions`, `skills_instructions`, `plugins_instructions`, `collaboration_mode`, `context_window`, …) — the rest leak into the index and titles. It also does not strip the `## My request for Codex:` prefix, nor skip review-mode boilerplate. Confirmed a real on-disk session titled with `## Code review guidelines:` instead of the actual request. | Extend the drop-list from the Codex tag constants (source of truth). Strip the `USER_MESSAGE_BEGIN` prefix. When deriving titles, skip a leading user message beginning with `## Code review guidelines:`. Keep this cross-agent policy in `core` per `B-010` where Claude needs the same rule. |

**Version-awareness (cross-cutting for C1):** `SessionMeta.history_mode`
(`protocol.rs:3065`) and the new top-level `ordinal` field (`RolloutLine`,
`protocol.rs:3335`) are format-version signals `hop` currently ignores. Reading
`history_mode` to pick the extraction path (and treating absence as legacy) is a
clean, forward-compatible strategy rather than heuristically probing every line.

---

## Tier 2 — Metadata Enrichment

Files: `src/adapters/codex.rs` (`Payload` `183-195`, `Git` `197-201`), `src/core.rs`
(`SessionSummary` ~`203`), `src/columns.rs`, `src/enrich/`. Mostly additive.

| # | Pri | hop gap | Codex reference | Value |
|---|-----|---------|-----------------|-------|
| M1 | High | No `model` captured; `turn_context` arm already parsed at `codex.rs:69-76` | `protocol.rs:3240` (`TurnContextItem.model`, real value e.g. `gpt-5.6-sol`); `SessionMeta.model_provider` `:3048` | A searchable/displayable **model** column. Cheap: the record is already parsed — add the field. Consider a matching Claude model source so the column is cross-agent (`B-010`). |
| M2 | Med | `Git` struct omits commit hash | `GitInfo.commit_hash` `protocol.rs:3347` (real SHA on disk) | Enables showing/searching the commit a session ran against. One-field add. |
| M3 | Med | No `source`/`thread_source` filtering; every rollout indexed equally | `SessionMeta.source` (`SessionSource`, `protocol.rs:2720`), `thread_source` (`ThreadSource`, `:2737`); `INTERACTIVE_SESSION_SOURCES` allowlist `lib.rs:25-32` | Hide sub-agent / `memory_consolidation` / non-interactive threads a user would never resume. Mirror Codex's own interactive allowlist. Ties to `I-001` (empty-session skipping) as a sibling filter. |
| M4 | Low | `permission_mode` collapses to `yolo`/`default` at `codex.rs:264-268`; yolo detection misses newer variants | `AskForApproval::Granular` `protocol.rs:932`; `SandboxPolicy` `:1001-1003` | Surface the real approval policy + sandbox mode instead of a boolean; recognize `Granular`. More honest than the current two-state collapse. |
| M5 | Low | Images in user turns yield empty text and vanish (`Payload.message` is a plain `String`, `codex.rs:194`) | `UserMessageEvent.images` `protocol.rs:2286`; `user_message_preview` emits `[Image]` `:2315-2329` | Mirror Codex's `[Image]` placeholder so image-only turns aren't dropped from `message_count`/titles. |
| M6 | Low | Compaction not counted/surfaced; `message_count` at `codex.rs:258` can misrepresent long sessions | `CompactedItem.message` `protocol.rs:3176`; always persisted `policy.rs:15-18` | Optional: account for `compacted` records so counts reflect compacted history. |

---

## Tier 3 — TUI Craft

Files: `src/tui/theme.rs`, `src/tui/preview.rs`, tests. Resolves `P-001`.
Overlaps the `2026-06-13-tui-design-review.md` theme item.

| # | Pri | hop today | Codex reference | Direction | Cost |
|---|-----|-----------|-----------------|-----------|------|
| T1 | High | Hardcoded `Theme` struct assuming a dark, truecolor terminal (`theme.rs`); `P-001` open | `color.rs` (`is_light`/`blend`/`perceptual_distance`); `terminal_palette.rs` (OSC-11 bg probe, `stdout_color_level`, `best_color` 256/16-color quantizer); `style.rs` (semantic style *functions* taking terminal bg) | Reframe `theme.rs` as semantic role functions branching on light/dark. Adopt `is_light`/`blend`/`perceptual_distance` (**zero new deps**). Optionally add OSC-11 bg detection + `best_color` quantization (adds `supports-color`) so RGB degrades gracefully on non-truecolor terminals. **Resolves `P-001`.** | Low → Med |
| T2 | Med | syntect + `base16-ocean.dark`, no size guard (`preview.rs`) | `render/highlight.rs`: `two-face` (~250 langs, 32 themes, light/dark auto-select); `convert_syntect_color` alpha-aware; 512KB/10k-line guardrail; language-alias `find_syntax` | Add `two-face` beside syntect for breadth + adaptive theme; copy the "drop bg/italic/underline, respect terminal palette" color policy and the size guardrail. | Med |
| T3 | Med | Hand-rolled markdown→lines in `preview.rs` | `markdown.rs` + `markdown_render.rs` (`pulldown-cmark` `Writer`, `MarkdownStyles` `:87-123`) | Replace hand-rolled rendering with a trimmed `pulldown-cmark` event loop + a small `MarkdownStyles` struct (headings, inline code, fences, lists, quotes). Skip table/fence-unwrapping unless needed. | Med |
| T4 | Med | `TestBackend` only | `insta` (55 files, 103 snapshots); plain-text-line snapshots for markdown (`assert_snapshot!(plain_lines(&t).join("\n"))`); optional `VT100Backend` for ANSI/color assertions | Add `insta` snapshot tests over existing `TestBackend`; use plain-text-line snapshots for the markdown renderer to catch structure regressions without color churn. Supports the `I-006` viewport-bounded rendering guarantees under test. | Low |
| T5 | Low | Hand-rolled braille spinner; `Paragraph::scroll` with no position indicator | `shimmer.rs` (time-synced shimmer sweep, degrades on non-truecolor); `pager_overlay.rs:234-250` (percentage bottom-bar); `render/line_utils.rs` (dep-free `prefix_lines`/`line_to_static`) | Optional flourishes: percentage scroll indicator (simpler than a `Scrollbar` widget — cross-refs review item L5), copyable `line_utils` helpers, shimmer "thinking" animation. | Low |

---

## Tier 4 — Picker / UX Features

Codex's picker is `codex-rs/tui/src/resume_picker.rs`; list loading is
`codex-rs/rollout/src/list.rs`. Notably, its own filter is **plain substring**
(`Row::matches_query`), so `hop`'s Tantivy search is already stronger — these are
additive features, not parity fixes. Respect `B-008` (render boundary) and
`I-005`/`I-006` (no slow work on / bounded UI thread) when adopting.

| # | Pri | Idea | Codex reference | Note for hop |
|---|-----|------|-----------------|--------------|
| U1 | Med | Optional **fuzzy mode** complementary to Tantivy, with bolded match indices | `utils/fuzzy-match/src/lib.rs` (`fuzzy_match`: subsequence + prefix bonus, returns highlight indices); `nucleo` in `file-search` for scale; highlight render `selection_popup_common.rs:450-467` | Keep Tantivy for full-text; fuzzy narrows names/paths. Would extend `SearchMode` (currently simple/raw) — check `I-011` modeless keymap constraints. |
| U2 | Med | **Inline expand-selected-row** preview as an alternative to the side pane on narrow terminals | Codex Ctrl+E `toggle_selected_expansion` + `render_transcript_preview_lines` | Directly addresses review item L2 (preview never collapses) for narrow widths without a second pane. |
| U3 | Low | **Header-only** list-metadata reads; derive timestamp/id from filename | `list.rs:964` (`parse_timestamp_uuid_from_filename`); `read_head_summary` `:1109-1168`; `reverse_jsonl_scanner.rs` (tail reads in 8KB chunks) | Only relevant where `hop` reads more than needed to build a row. `hop` already parses id from filename correctly (`codex.rs:313-318`); the head-only preview read is the borrowable part. |
| U4 | Low | UX polish: once-per-load **stable relative-time** reference (no per-row flicker); priority-degrading footer hints; richer empty/loading states | `resume_picker.rs:3116-3137` (relative time), `2139-2372` (footer hint priority/degradation), `3170-3195` (empty/loading copy) | Footer-hint degradation cross-refs review item L3. Empty/loading states cross-ref the review's "state communication" theme. |

---

## Cross-References

- **`P-001` (theme config not applied):** T1 is the principled resolution path.
- **`2026-06-13-tui-design-review.md`:** T1 (theme refactor), T5/U4 (scroll
  indicator L5, footer clipping L3), U2 (preview collapse L2) overlap that review.
- **Architecture rules touched:** `B-001`, `B-010` (adapter vs core derivation
  for C1/C3/M1), `I-001`/`I-004` (sync + shared extraction for C1–C3), `I-011`
  (keymap constraint for U1), `B-008`/`I-005`/`I-006` (render boundary for Tier 4).
- **`I-003` schema versioning:** M1–M2 add indexed fields → bump `SCHEMA_VERSION`.

## Candidate First Plan

If a first `docs/plans/` doc is spun off from this review, **Tier 1 (C1 + C3)** is
the natural scope: parse `response_item` messages (future-proofs transcripts) and
tighten title/noise stripping (fixes visible garbage titles). Both are contained
to `codex.rs`/`core`, both want new fixtures under the existing parser test
pattern (`PROJECT.md` quality bar), and together they measurably improve today's
output while removing the paginated-mode time bomb. (C2 `.jsonl.zst` is
Tier 1 too but independent — it can share that plan or get its own.)
