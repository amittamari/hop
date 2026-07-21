# Capability: Hooks

## Purpose

Session metadata hooks system: captures git metadata at session start/stop events, persists it as sidecar JSON files, merges sidecar data into session summaries at index time, and manages hook installation/uninstallation across agent providers (Claude, Codex, Cursor).

## Requirements

### Requirement: Sidecar format
A `Sidecar` SHALL be a JSON file containing: version (u32), session_id, agent (serialized as slug), and an ordered list of `SidecarEvent`s. Each event records: hook event (Start/Stop), timestamp, and optional cwd, branch, repo_url, worktree, and permission_mode.

#### Scenario: Sidecar round-trip
- **WHEN** a sidecar is written and read back
- **THEN** all fields SHALL be preserved exactly

### Requirement: Sidecar storage
Sidecars SHALL be stored at `~/.hop/meta/<agent_slug>/<session_id>.json`. Writes SHALL use atomic rename (write to `.tmp`, then rename).

#### Scenario: Sidecar path for Claude session
- **GIVEN** agent `Claude` and session id `"abc-123"`
- **WHEN** the sidecar path is computed
- **THEN** it SHALL end with `claude/abc-123.json`

### Requirement: Capture flow
`capture` SHALL parse hook stdin as JSON (extracting `session_id` and `cwd`), collect git metadata from the cwd, create a `SidecarEvent`, and append it to the session's sidecar file (creating it if absent).

#### Scenario: First capture creates sidecar
- **GIVEN** no sidecar file exists for session `"s1"`
- **WHEN** a start event is captured
- **THEN** a sidecar file SHALL be created with one event

#### Scenario: Second capture appends to existing
- **GIVEN** a sidecar file exists with a start event
- **WHEN** a stop event is captured
- **THEN** the sidecar SHALL have two events

### Requirement: Git metadata collection
`GitMeta::collect` SHALL resolve the current branch (`rev-parse --abbrev-ref HEAD`, filtering `HEAD`), origin remote URL, and linked worktree detection (comparing `--git-dir` vs `--git-common-dir`).

#### Scenario: Valid git repo
- **GIVEN** a directory that is a git repository with branch `"test-branch"`
- **WHEN** `GitMeta::collect` is called
- **THEN** `branch` SHALL be `Some("test-branch")` and `repo_url` SHALL be `Some(...)`

#### Scenario: Non-git directory
- **GIVEN** a directory that is not a git repository
- **WHEN** `GitMeta::collect` is called
- **THEN** all fields SHALL be `None`

### Requirement: Sidecar merge
`merge_sidecar` SHALL: (1) attempt live git enrichment as a fallback for missing fields, then (2) overlay the sidecar's last event onto the session summary. The sidecar's final snapshot is authoritative, including `None` values that clear prior state.

#### Scenario: Final null clears vendor values
- **GIVEN** a sidecar whose last event has `branch: None`
- **WHEN** merged onto a summary that had a vendor-provided branch
- **THEN** the summary's branch SHALL become `None`

### Requirement: Sidecar stamp
`sidecar_stamp_in` SHALL produce a cheap file stamp (`mtime_nanos:file_length`) for incremental indexing, so the engine can detect sidecar changes without reading the file.

#### Scenario: Stamp changes on append
- **GIVEN** a sidecar file with one event
- **WHEN** a second event is appended
- **THEN** `sidecar_stamp_in` SHALL return a different value than before the append

### Requirement: Provider detection
`detect_providers` SHALL check for Claude (`~/.claude`), Codex (`~/.codex/config.toml`), and Cursor (`~/.cursor`) directories and report whether hooks are installed for each.

#### Scenario: Claude detected but not installed
- **GIVEN** `~/.claude` exists but no hop hook plugin is installed
- **WHEN** `detect_providers` is called
- **THEN** the Claude entry SHALL have `detected: true` and `installed: false`

### Requirement: Hook installation
Claude and Codex hooks SHALL be installed via their plugin/marketplace systems. Cursor hooks SHALL be installed via a `hooks.json` file. All hooks SHALL invoke `hop meta capture --agent <name> --event <start|stop>`.

#### Scenario: Claude hook command
- **WHEN** the Claude hook JSON is generated
- **THEN** it SHALL contain `"hop meta capture --agent claude --event start"` and `"hop meta capture --agent claude --event stop"`

### Requirement: Hook uninstallation
`uninstall` SHALL remove the hook files/directories for each provider cleanly.

#### Scenario: Clean uninstall
- **GIVEN** hooks are installed for Claude
- **WHEN** uninstall is called
- **THEN** the hook plugin directory SHALL be removed
