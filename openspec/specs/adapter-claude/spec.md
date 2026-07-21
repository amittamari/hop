# Capability: Claude Adapter

## Purpose

Parses Claude Code session JSONL files from `~/.claude/projects/<encoded-cwd>/<uuid>.jsonl` into the core `Session` type. Handles Claude-specific field names, content formats, and command-tag filtering.

## Requirements

### Requirement: Scan discovery
The adapter SHALL scan `~/.claude/projects/` (or configured override), iterating project subdirectories for top-level `.jsonl` files. Subagent directories SHALL be skipped.

#### Scenario: Session file naming
- **GIVEN** a file at `<root>/<project>/<uuid>.jsonl`
- **WHEN** scan is called
- **THEN** the session id SHALL be the file stem (the UUID)

### Requirement: JSONL parsing
Each line SHALL be deserialized with simd-json. Lines with `type: "user"` or `type: "assistant"` that are not meta (`isMeta: true`) and not tool-use results SHALL be extracted as messages. Other line types SHALL be skipped.

#### Scenario: Meta lines skipped
- **GIVEN** a JSONL line with `type: "assistant"` and `isMeta: true`
- **WHEN** the file is parsed
- **THEN** the line SHALL NOT produce a message

#### Scenario: Tool-use results skipped
- **GIVEN** a JSONL line with a `toolUseResult` field
- **WHEN** the file is parsed
- **THEN** the line SHALL NOT produce a message

### Requirement: Title extraction
The adapter SHALL prefer `aiTitle` over `summary` for the session title. When neither is present, it SHALL fall back to `derive_session_title` (first user prose).

#### Scenario: aiTitle preferred over summary
- **GIVEN** a session with both `aiTitle: "Fix auth"` and `summary: "Auth work"`
- **WHEN** the session is parsed
- **THEN** the title SHALL be `"Fix auth"`

### Requirement: Directory and branch
The adapter SHALL capture `cwd` from the first line that provides it, and `gitBranch` as the session branch.

#### Scenario: First cwd wins
- **GIVEN** two JSONL lines with different `cwd` values
- **WHEN** the session is parsed
- **THEN** the directory SHALL be the `cwd` from the first line

### Requirement: Model extraction
The adapter SHALL record the last non-synthetic model string from assistant turns. Model strings starting with `<` (e.g. `<synthetic>`) SHALL be ignored.

#### Scenario: Synthetic model ignored
- **GIVEN** an assistant turn with `model: "<synthetic>"`
- **WHEN** the session is parsed
- **THEN** that model string SHALL NOT be recorded

### Requirement: Content filtering
User message text starting with a command tag (per `is_command_tag_line`) SHALL be dropped. Block-array content SHALL keep only `text`-typed blocks.

#### Scenario: Command tag dropped
- **GIVEN** a user line with content `"<command-name>/clear</command-name>"`
- **WHEN** the session is parsed
- **THEN** the line SHALL NOT produce a message

### Requirement: Resume command
`resume_command` SHALL produce `["claude", "--resume", "<id>"]`. With yolo, it SHALL prepend `--dangerously-skip-permissions`.

#### Scenario: Normal resume
- **GIVEN** a session with id `"abc-123"`
- **WHEN** `resume_command` is called with `yolo: false`
- **THEN** the result SHALL be `["claude", "--resume", "abc-123"]`

#### Scenario: Yolo resume
- **GIVEN** a session with id `"abc-123"`
- **WHEN** `resume_command` is called with `yolo: true`
- **THEN** the result SHALL be `["claude", "--dangerously-skip-permissions", "--resume", "abc-123"]`

### Requirement: Repo URL resolution
The adapter SHALL resolve `repo_url` from the session's `cwd` via `GitFieldCache` at parse time, since Claude transcripts carry no git remote field.

#### Scenario: Repo resolved from cwd
- **GIVEN** a session with `cwd` pointing to a git repository with an `origin` remote
- **WHEN** the session is parsed
- **THEN** `repo_url` SHALL be set to the origin remote URL
