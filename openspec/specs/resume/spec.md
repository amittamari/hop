# Capability: Resume

## Purpose

Handles the terminal-safe process handoff: running an optional preparatory command (e.g. unarchiving) and then exec-replacing the hop process with the agent CLI in the session's working directory.

## Requirements

### Requirement: Prepare step
`run_prepare` SHALL execute a preparatory command (e.g. `codex unarchive <id>`) synchronously, inheriting stdio. An empty argv SHALL be a no-op. A non-zero exit status SHALL produce an error. The terminal is already restored when this runs.

#### Scenario: Empty argv is no-op
- **WHEN** `run_prepare` is called with an empty argv
- **THEN** it SHALL return `Ok(())`

#### Scenario: Non-zero exit produces error
- **GIVEN** a prepare command that exits with status 1
- **WHEN** `run_prepare` is called
- **THEN** it SHALL return an error

### Requirement: Exec resume
`exec_resume` SHALL change the current directory to the session's working directory, then exec-replace the process with the resume argv. On success, this call SHALL never return.

#### Scenario: Vanished directory
- **WHEN** the session directory no longer exists
- **THEN** a warning SHALL be printed to stderr but the exec SHALL still proceed (best-effort chdir)

#### Scenario: Empty argv
- **WHEN** `exec_resume` is called with an empty argv
- **THEN** it SHALL return an error containing "empty"
