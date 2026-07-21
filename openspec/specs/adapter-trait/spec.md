# Capability: Adapter Trait

## Purpose

Defines the `Adapter` trait that each agent-specific parser implements, establishing the contract for session discovery, parsing, transcript extraction, and resume/unarchive commands. Includes shared utilities for timestamp parsing, file mtime, git field resolution, and per-directory git caching.

## Requirements

### Requirement: Trait contract
Every `Adapter` SHALL implement: `id()` returning its `AgentId`, `is_available()` checking whether the data directory exists, `scan()` for cheap stat-level discovery, `parse()` for full session parsing, `transcript()` for preview-quality message extraction, `resume_command()` for building the CLI argv, and `supports_yolo()`.

#### Scenario: Adapter reports availability
- **WHEN** the agent's data directory exists on disk
- **THEN** `is_available()` SHALL return `true`

#### Scenario: Adapter data directory missing
- **WHEN** the agent's data directory does not exist
- **THEN** `is_available()` SHALL return `false`

### Requirement: Agent glyph
Each adapter MAY override `agent_glyph()` to return a Nerd Font Private Use Area code point. The default SHALL be an empty string. Shipped adapters SHALL each return a non-empty, distinct glyph.

#### Scenario: Adapter without override
- **GIVEN** an adapter that does not override `agent_glyph`
- **WHEN** `agent_glyph()` is called
- **THEN** it SHALL return an empty string

#### Scenario: Shipped adapters have distinct glyphs
- **GIVEN** the default adapter set
- **WHEN** each adapter's `agent_glyph()` is called
- **THEN** every glyph SHALL be non-empty and distinct from the others

### Requirement: Interactivity filter
Adapters MAY override `is_interactive()` to classify sessions as non-interactive (e.g. sub-agent threads, memory consolidation). The default SHALL return `true` (interactive). Non-interactive sessions are skipped at index time.

#### Scenario: Default is interactive
- **GIVEN** an adapter that does not override `is_interactive`
- **WHEN** `is_interactive` is called for any session
- **THEN** it SHALL return `true`

### Requirement: Unarchive support
Adapters MAY override `unarchive_command()` to return an argv that unarchives a session before resume. The default SHALL return `None`.

#### Scenario: Default returns None
- **GIVEN** an adapter that does not override `unarchive_command`
- **WHEN** `unarchive_command` is called
- **THEN** it SHALL return `None`

### Requirement: Git field caching
`GitFieldCache` SHALL cache per-directory git lookups so that a rebuild pass spawns at most one `git` process per unique directory, not one per session.

#### Scenario: Repeated lookups for the same directory
- **GIVEN** two sessions sharing the same working directory
- **WHEN** `resolve` is called for both
- **THEN** the resolver function SHALL be invoked only once and the cached result SHALL be returned for the second call

### Requirement: Git remote URL resolution
`git_remote_url` SHALL resolve the `origin` remote URL for a directory. When the directory no longer exists on disk (e.g. a deleted worktree), it SHALL walk ancestor directories until it finds one that can resolve the remote.

#### Scenario: Deleted worktree ancestor fallback
- **GIVEN** a directory that no longer exists but whose parent is a valid git repo
- **WHEN** `git_remote_url` is called
- **THEN** it SHALL resolve the remote from the nearest existing ancestor

### Requirement: Default adapter set
`default_adapters` SHALL return one adapter per supported agent, honoring config data-dir overrides.

#### Scenario: One adapter per agent
- **GIVEN** a default config
- **WHEN** `default_adapters` is called
- **THEN** it SHALL return exactly three adapters with distinct `AgentId`s
