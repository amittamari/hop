# Tier 1 Codex Adapter Correctness

## Goal

Prevent Codex sessions from silently disappearing or producing empty/noisy
transcripts as Codex adopts paginated history and compressed rollout files.
This plan implements C1–C3 from the dated Codex-inspired improvements review.

## Changes

- Parse legacy `event_msg` and paginated `response_item` message records into
  separate candidates, then select one transcript family using
  `session_meta.history_mode`. Treat missing or unknown modes as legacy and use
  the other family only when the preferred candidate is empty.
- Build response messages from ordered `input_text`/`output_text` items for user
  and assistant roles. Pass both transcript formats through the same cleaning,
  block splitting, indexing, and preview path.
- Discover and read `rollout-*.jsonl.zst` alongside plain JSONL, key both by the
  same session ID, prefer a plain sibling during representation transitions,
  and reparse when the stored physical source path changes.
- Remove Codex-injected instruction/context blocks and the
  `## My request for Codex:` prefix. When deriving a Codex title, skip leading
  user prose beginning with `## Code review guidelines:` while retaining that
  prose in the transcript and search content.
- Add `zstd` as a direct dependency. No index schema bump is required because
  existing stored fields and domain types are unchanged.

## Verification

- Adapter tests cover legacy/paginated selection, non-empty fallbacks, ordered
  response text, ignored roles/content, injected-context cleaning, compressed
  scan/parse/preview behavior, sibling precedence, and corrupt streams.
- Codex adapter tests cover review-boilerplate title fallback.
- Index tests verify persisted source paths are available to incremental sync.
- Run `cargo test` as the acceptance check.
