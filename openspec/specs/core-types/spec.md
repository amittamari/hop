# Capability: Core Types

## Purpose

Defines the domain model shared across all layers: session identity, message structure, transcript types, and text-processing utilities used by adapters, the index, and the TUI.

## Requirements

### Requirement: Agent identity
`AgentId` SHALL enumerate all supported coding agents (Claude, Codex, Cursor) with slug, badge, and round-trip conversion helpers.

#### Scenario: Slug round-trip
- **GIVEN** any `AgentId` variant
- **WHEN** `from_slug(agent.slug())` is called
- **THEN** it SHALL return `Some(agent)`

#### Scenario: Unknown slug
- **WHEN** `from_slug` is called with an unrecognized string
- **THEN** it SHALL return `None`

### Requirement: Document key namespacing
Session ids are unique only within an agent. The `document_key` function SHALL produce a composite key of the form `agent_slug:raw_id` to uniquely identify a session across all agents.

#### Scenario: Overlapping raw ids
- **GIVEN** a Claude session and a Codex session both with raw id `"abc"`
- **WHEN** `document_key` is called for each
- **THEN** the keys SHALL be `"claude:abc"` and `"codex:abc"` respectively

### Requirement: Message structure
A `Message` SHALL carry a `Role` (User or Agent) and a list of `Block`s. Each `Block` SHALL be either `Prose(String)` or `Code { lang, text }`.

#### Scenario: User message with mixed blocks
- **GIVEN** a user message containing plain text and a fenced code block
- **WHEN** the message is constructed
- **THEN** it SHALL have `role: User` and `blocks` containing both `Prose` and `Code` variants

### Requirement: Fenced code block splitting
`split_blocks` SHALL parse a text body into alternating prose and fenced-code blocks. A line starting with triple backticks opens a code block; a line of only backticks (three or more) closes it. Empty prose runs SHALL be dropped; trailing/leading blank lines within prose SHALL be trimmed.

#### Scenario: Mixed prose and code
- **GIVEN** text with prose, a fenced code block, and more prose
- **WHEN** `split_blocks` is called
- **THEN** it SHALL return `[Prose, Code, Prose]` with the code block's language tag preserved

#### Scenario: Unterminated fence
- **GIVEN** text with an opening fence but no closing fence
- **WHEN** `split_blocks` is called
- **THEN** the remaining lines SHALL be captured as a `Code` block

### Requirement: Content flattening
`flatten_messages` SHALL join all message blocks (prose and code) into a single newline-separated string suitable for full-text indexing.

#### Scenario: Multiple messages with prose and code
- **GIVEN** two messages, one with prose and one with prose and code
- **WHEN** `flatten_messages` is called
- **THEN** all non-empty block texts SHALL be joined with newlines

### Requirement: Command tag filtering
`is_command_tag_line` SHALL return true for lines starting with internal harness tags (e.g. `<command-name>`, `<bash-input>`, `<task-notification>`) so adapters can strip tooling noise from user-visible transcripts.

#### Scenario: Known command tag detected
- **GIVEN** a line starting with `<bash-input>`
- **WHEN** `is_command_tag_line` is called
- **THEN** it SHALL return `true`

#### Scenario: Regular text not flagged
- **GIVEN** a line containing normal conversation text
- **WHEN** `is_command_tag_line` is called
- **THEN** it SHALL return `false`

### Requirement: Title derivation
`derive_session_title` SHALL prefer an explicit title when present, falling back to the first user prose message, and returning `"(untitled)"` when neither exists. Whitespace SHALL be normalized (collapsed to single spaces).

#### Scenario: Explicit title provided
- **GIVEN** an explicit title `"fix  auth  bug"` and a list of messages
- **WHEN** `derive_session_title` is called
- **THEN** it SHALL return `"fix auth bug"` (whitespace collapsed)

#### Scenario: No title and no messages
- **GIVEN** no explicit title and an empty message list
- **WHEN** `derive_session_title` is called
- **THEN** it SHALL return `"(untitled)"`

### Requirement: Title truncation
`truncate_title` SHALL collapse whitespace and truncate to a maximum character count with a trailing ellipsis when the title exceeds the limit.

#### Scenario: Title exceeds max length
- **GIVEN** a title `"hello world"` and max length 5
- **WHEN** `truncate_title` is called
- **THEN** it SHALL return `"hell…"`

#### Scenario: Title within limit
- **GIVEN** a title `"hello"` and max length 10
- **WHEN** `truncate_title` is called
- **THEN** it SHALL return the title unchanged
