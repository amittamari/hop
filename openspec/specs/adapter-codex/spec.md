# Capability: Codex Adapter

## Purpose

Parses Codex session rollout files (plain `.jsonl` or zstd-compressed `.jsonl.zst`) from `~/.codex/sessions/` and `~/.codex/archived_sessions/` into the core `Session` type. Handles Codex-specific event schemas, history modes, XML block stripping, and interactivity classification.

## Requirements

### Requirement: Scan discovery
The adapter SHALL recursively scan both `sessions/` and `archived_sessions/` for `.jsonl` and `.jsonl.zst` files. When both compressed and plain versions exist for the same rollout stem, the plain version SHALL be preferred.

#### Scenario: Session id from filename
- **GIVEN** a file named `rollout-2026-07-11T10-00-00-<uuid>.jsonl`
- **WHEN** scan is called
- **THEN** the session id SHALL be the UUID portion after the timestamp prefix

### Requirement: Zstd decompression
When the rollout file ends in `.jsonl.zst`, the adapter SHALL decompress it with zstd before parsing.

#### Scenario: Compressed rollout parsed
- **GIVEN** a rollout file at `rollout-<ts>-<uuid>.jsonl.zst`
- **WHEN** `parse` is called
- **THEN** it SHALL decompress with zstd and produce the same session as the plain equivalent

### Requirement: History mode
The adapter SHALL respect `session_meta.payload.history_mode`. In `paginated` mode, `response_item` messages SHALL be preferred over `event_msg`; in `legacy` mode, `event_msg` SHALL be preferred. Fallback to the other source occurs when the primary is empty.

#### Scenario: Paginated mode prefers response_item
- **GIVEN** a rollout with `history_mode: "paginated"` and both `response_item` and `event_msg` lines
- **WHEN** the session is parsed
- **THEN** messages SHALL be drawn from `response_item` lines

#### Scenario: Legacy mode prefers event_msg
- **GIVEN** a rollout with no `history_mode` (defaults to legacy) and both message types
- **WHEN** the session is parsed
- **THEN** messages SHALL be drawn from `event_msg` lines

### Requirement: Event message cleaning
`clean_event_message` SHALL strip: XML blocks matching known Codex system prefixes (e.g. `<user_instructions>`, `<system-reminder>`), `[external_agent_*]` blocks, command-tag lines, and `<context>` wrappers. The `## My request for Codex:` header SHALL be trimmed from user messages.

#### Scenario: System-reminder XML stripped
- **GIVEN** a message containing a `<system-reminder>...</system-reminder>` block
- **WHEN** `clean_event_message` is called
- **THEN** the XML block SHALL be removed and surrounding text preserved

### Requirement: Interactivity filter
Sessions with `source` or `thread_source` values of `"subagent"`, `"memory_consolidation"`, `"unified_exec_startup"`, or `"internal"` SHALL be classified as non-interactive and skipped at index time. Unknown or absent values SHALL be treated as interactive (fail-open).

#### Scenario: Subagent session non-interactive
- **GIVEN** a session with `source: "subagent"`
- **WHEN** `is_interactive` is called
- **THEN** it SHALL return `false`

#### Scenario: Unknown source treated as interactive
- **GIVEN** a session with `source: "some-future-source"`
- **WHEN** `is_interactive` is called
- **THEN** it SHALL return `true`

### Requirement: Archive detection
A session SHALL be flagged `archived` when its file path contains an `archived_sessions` component.

#### Scenario: File in archived_sessions
- **GIVEN** a rollout file at `~/.codex/archived_sessions/2026/07/rollout.jsonl`
- **WHEN** the session is parsed
- **THEN** `archived` SHALL be `true`

#### Scenario: File in sessions
- **GIVEN** a rollout file at `~/.codex/sessions/2026/07/rollout.jsonl`
- **WHEN** the session is parsed
- **THEN** `archived` SHALL be `false`

### Requirement: Yolo detection
A session SHALL be flagged yolo when `turn_context` reports `approval_policy: "never"` and `sandbox_policy.type: "danger-full-access"`.

#### Scenario: Full yolo flags detected
- **GIVEN** a `turn_context` with `approval_policy: "never"` and `sandbox_policy.type: "danger-full-access"`
- **WHEN** the session is parsed
- **THEN** `yolo` SHALL be `true`

### Requirement: Resume and unarchive commands
`resume_command` SHALL produce `["codex", "resume", "<id>"]`. With yolo, it SHALL prepend `--dangerously-bypass-approvals-and-sandbox`. `unarchive_command` SHALL return `["codex", "unarchive", "<id>"]`.

#### Scenario: Normal resume
- **GIVEN** a session with id `"uuid-1"`
- **WHEN** `resume_command` is called with `yolo: false`
- **THEN** the result SHALL be `["codex", "resume", "uuid-1"]`

#### Scenario: Unarchive command
- **GIVEN** a session with id `"uuid-1"`
- **WHEN** `unarchive_command` is called
- **THEN** the result SHALL be `Some(["codex", "unarchive", "uuid-1"])`
