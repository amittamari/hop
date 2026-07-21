# Capability: Cursor Adapter

## Purpose

Parses Cursor agent transcript JSONL files from `~/.cursor/projects/<slug>/agent-transcripts/<uuid>/<uuid>.jsonl` into the core `Session` type. Enriches metadata from Cursor's SQLite `store.db` and resolves workspace paths from `worker.log`.

## Requirements

### Requirement: Scan discovery
The adapter SHALL scan `~/.cursor/projects/` for project directories, then look inside each for `agent-transcripts/<uuid>/<uuid>.jsonl`. Only the canonical file (stem matches the directory UUID) SHALL be scanned; hook sidecars SHALL be skipped.

#### Scenario: Canonical file discovered
- **GIVEN** a directory `<root>/<slug>/agent-transcripts/<uuid>/` containing `<uuid>.jsonl`
- **WHEN** scan is called
- **THEN** the session id SHALL be the UUID and the canonical file SHALL be returned

#### Scenario: Non-canonical files skipped
- **GIVEN** a directory containing a `.jsonl` file whose stem does not match the UUID directory
- **WHEN** scan is called
- **THEN** the file SHALL NOT be included in the scan results

### Requirement: Workspace path resolution
The adapter SHALL read `worker.log` in each project directory to extract the `workspacePath=` value. Results SHALL be cached per project directory.

#### Scenario: Workspace path extracted
- **GIVEN** a `worker.log` containing a line with `workspacePath=/home/user/project`
- **WHEN** the adapter resolves the workspace path
- **THEN** the directory SHALL be `/home/user/project`

### Requirement: Store metadata
The adapter SHALL read session metadata (title, creation time, yolo flag) from the SQLite `store.db` at `~/.cursor/chats/<md5(workspace)>/<uuid>/store.db`. The workspace path is hashed with MD5 to locate the correct database.

#### Scenario: Store.db unavailable
- **WHEN** the store.db file does not exist or cannot be read
- **THEN** the adapter SHALL fall back to `derive_session_title` and file mtime

### Requirement: User query extraction
User messages wrapped in `<user_query>...</user_query>` SHALL have the wrapper stripped, returning only the inner content.

#### Scenario: Wrapper stripped
- **GIVEN** a user message `"context <user_query>fix the bug</user_query> more"`
- **WHEN** `clean_user_text` is called
- **THEN** the result SHALL be `"fix the bug"`

### Requirement: Redacted thinking
Text blocks containing `[REDACTED]` (Cursor's extended-thinking redaction) SHALL be stripped. A block that is entirely `[REDACTED]` SHALL be dropped; a block ending with `[REDACTED]` SHALL have the suffix removed.

#### Scenario: Entirely redacted block dropped
- **GIVEN** a text block with content `"[REDACTED]"`
- **WHEN** `strip_redacted` is called
- **THEN** the result SHALL be an empty string

#### Scenario: Trailing redacted suffix removed
- **GIVEN** a text block with content `"some output [REDACTED]"`
- **WHEN** `strip_redacted` is called
- **THEN** the result SHALL be `"some output"`

### Requirement: Error handling
When a turn ends with `type: "turn_ended"` and `status: "error"` and no agent reply exists, the session SHALL be treated as empty (not indexed).

#### Scenario: Errored turn with no reply
- **GIVEN** a transcript with a user message, then `turn_ended` with `status: "error"`, and no assistant message
- **WHEN** the session is parsed
- **THEN** the messages list SHALL be empty

### Requirement: Resume command
`resume_command` SHALL produce `["cursor-agent", "--resume", "<id>"]`. With yolo, it SHALL prepend `--force`.

#### Scenario: Normal resume
- **GIVEN** a session with id `"uuid-1"`
- **WHEN** `resume_command` is called with `yolo: false`
- **THEN** the result SHALL be `["cursor-agent", "--resume", "uuid-1"]`

#### Scenario: Yolo resume
- **GIVEN** a session with id `"uuid-1"`
- **WHEN** `resume_command` is called with `yolo: true`
- **THEN** the result SHALL be `["cursor-agent", "--force", "--resume", "uuid-1"]`
