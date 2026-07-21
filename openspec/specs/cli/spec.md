# Capability: CLI

## Purpose

Defines the command-line interface for hop: positional query, flag filters, subcommands for hook management and metadata capture, and the logic for composing the initial query and resolving auto-scoping behavior.

## Requirements

### Requirement: Positional query
The CLI SHALL accept an optional positional `query` argument to pre-fill the search input.

#### Scenario: Query pre-fills search
- **WHEN** `hop "auth bug"` is invoked
- **THEN** the TUI SHALL open with the search input pre-filled with `"auth bug"`

### Requirement: Filter flags
The CLI SHALL accept `--agent`, `--dir`, and `--repo` flags for filtering, `--all` to disable auto-scoping, `--yolo` for force-yolo resume, and `--rebuild` to wipe and rebuild the index.

#### Scenario: Agent flag filters results
- **WHEN** `hop --agent claude` is invoked
- **THEN** the initial query SHALL contain `agent:claude`

### Requirement: Initial query composition
`initial_query` SHALL compose the effective query string from positional and flag filters. When `auto_repo` is set, it SHALL be prepended as a `repo:` token.

#### Scenario: All filters combined
- **GIVEN** `--agent claude --dir api --repo hop` and positional query `"auth"`
- **WHEN** `initial_query(None)` is called
- **THEN** the result SHALL be `"agent:claude dir:api repo:hop auth "`

#### Scenario: Auto-repo prepended
- **GIVEN** positional query `"auth"` and auto_repo `"me/hop"`
- **WHEN** `initial_query(Some("me/hop"))` is called
- **THEN** the result SHALL be `"repo:me/hop auth "`

### Requirement: Auto-repo scoping
`wants_auto_repo` SHALL return true when no `--all`, no `--repo`, and no `repo:` or `-repo:` token appears in the positional query. This enables automatic scoping to the current repo.

#### Scenario: Bare invocation enables auto-repo
- **WHEN** `hop` is invoked with no flags or query
- **THEN** `wants_auto_repo` SHALL return true

#### Scenario: Explicit all disables auto-repo
- **WHEN** `hop --all` is invoked
- **THEN** `wants_auto_repo` SHALL return false

### Requirement: DSL detection
`query_has_dsl` SHALL return true when the positional query contains filter keyword tokens (`agent:`, `dir:`, `repo:`, `date:`), triggering raw search mode.

#### Scenario: Query with agent filter triggers DSL mode
- **WHEN** the positional query is `"agent:claude auth"`
- **THEN** `query_has_dsl` SHALL return true

#### Scenario: Plain text does not trigger DSL mode
- **WHEN** the positional query is `"auth bug"`
- **THEN** `query_has_dsl` SHALL return false

### Requirement: Hooks subcommand
`hop hooks install`, `hop hooks uninstall`, and `hop hooks status` SHALL manage session metadata hooks for detected providers.

#### Scenario: Install hooks
- **WHEN** `hop hooks install --all` is invoked
- **THEN** hooks SHALL be installed for all detected providers

### Requirement: Meta subcommand
`hop meta capture --agent <name> --event <start|stop>` SHALL capture session metadata from hook stdin (internal, called by installed hooks).

#### Scenario: Capture start event
- **WHEN** `hop meta capture --agent claude --event start` is invoked with session JSON on stdin
- **THEN** a sidecar file SHALL be created for that session
