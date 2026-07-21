# Capability: Engine

## Purpose

Orchestrates search and sync: owns the search index, adapters, and query state. Provides debounced search execution, transcript loading, resume command building, and both synchronous and background index synchronization with detailed reporting.

## Requirements

### Requirement: Search lifecycle
The engine SHALL hold the current query string, parsed query, sort order, and result set. `set_query` SHALL parse the query and mark a search as pending. `search_due` SHALL return true once the debounce interval (40ms) has elapsed. `search` SHALL execute the query against the index and clear the pending flag.

#### Scenario: Debounced search
- **WHEN** `set_query` is called
- **THEN** `search_due` SHALL return false immediately
- **AND** SHALL return true after 40ms have elapsed

### Requirement: Transcript loading
`transcript_for` SHALL look up the adapter for the session's agent, load the source file via `adapter.transcript()`, and return a `Transcript`. It SHALL return `None` when the source path or adapter is unavailable.

#### Scenario: Source path missing
- **GIVEN** a session summary with `source_path: None`
- **WHEN** `transcript_for` is called
- **THEN** it SHALL return `None`

### Requirement: Resume command building
`resume_command_for` SHALL load the full indexed session, find the matching adapter, build the resume argv, apply launcher rewrites when configured, and attach a `prepare` step for archived sessions (e.g. `codex unarchive`).

#### Scenario: Archived session includes prepare step
- **GIVEN** an archived Codex session
- **WHEN** `resume_command_for` is called
- **THEN** the returned `ResumeCommand.prepare` SHALL contain an unarchive command

#### Scenario: Active session has no prepare step
- **GIVEN** a non-archived session
- **WHEN** `resume_command_for` is called
- **THEN** `prepare` SHALL be `None`

### Requirement: Synchronous sync
`sync_once` SHALL scan all available adapters, diff against the index's known state, parse changed sessions, upsert them (skipping empty and non-interactive sessions), delete removed sessions, and commit. It SHALL return a `SyncReport` with counters for each outcome.

#### Scenario: Empty sessions skipped
- **GIVEN** a session with zero messages or empty content
- **WHEN** sync runs
- **THEN** the session SHALL NOT be indexed and `report.empty_sessions` SHALL increment

#### Scenario: Non-interactive sessions skipped
- **GIVEN** a session where `adapter.is_interactive()` returns false
- **WHEN** sync runs
- **THEN** the session SHALL NOT be indexed and `report.non_interactive_sessions` SHALL increment

#### Scenario: Unavailable adapter preserves rows
- **WHEN** an adapter reports `is_available() == false`
- **THEN** its previously indexed sessions SHALL NOT be deleted

### Requirement: Background sync
`spawn_background_sync` SHALL run the sync on a separate thread, sending `Update::Refresh` after batch commits and `Update::Done` with the final report. The UI thread SHALL call `reload()` to pick up new documents.

#### Scenario: Sync completion message
- **WHEN** background sync finishes successfully
- **THEN** `Update::Done` SHALL be sent with a `SyncReport` containing accurate counters

### Requirement: Sidecar-triggered reindex
When a session's sidecar stamp changes but its file mtime has not, the engine SHALL still reindex the session to pick up hook-captured metadata.

#### Scenario: Sidecar written after initial index
- **GIVEN** a session indexed without a sidecar, then a sidecar is written
- **WHEN** sync runs again with unchanged session mtime
- **THEN** the session SHALL be reindexed with the sidecar metadata

### Requirement: Source path change detection
When a Codex session's file changes from `.jsonl` to `.jsonl.zst` (compression) while preserving mtime, the engine SHALL detect the path change and reindex.

#### Scenario: Compression replaces plain file
- **GIVEN** a session indexed from `rollout.jsonl`, which is then replaced by `rollout.jsonl.zst` with the same mtime
- **WHEN** sync runs
- **THEN** the session SHALL be reindexed and `source_path` SHALL point to the `.jsonl.zst` file
