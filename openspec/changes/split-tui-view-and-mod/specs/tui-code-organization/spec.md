# TUI Code Organization

## ADDED Requirements

### Requirement: TUI source files stay within the soft size limit

TUI source files SHALL be kept under ~500 lines by splitting oversized modules
into cohesive submodules along responsibility boundaries, without changing
user-visible behavior or the crate's public module surface.

#### Scenario: view rendering is split into focused submodules

- **WHEN** the TUI render code is organized
- **THEN** render orchestration, footer, card layout, and preview-header
  rendering live in separate submodules under `src/tui/view/`
- **AND** each submodule keeps its own colocated `#[cfg(test)]` tests
- **AND** each resulting `.rs` file is at most 500 lines

#### Scenario: app state and input handling are separated

- **WHEN** the tui module root is organized
- **THEN** shared types (`App`, `Action`, `SearchMode`) remain importable from
  `crate::tui` unchanged
- **AND** state accessors and key/action dispatch live in separate modules
- **AND** each resulting `.rs` file is at most 500 lines

#### Scenario: behavior is preserved

- **WHEN** the refactor is complete
- **THEN** `cargo test` and `cargo test --lib` pass with the redistributed tests
- **AND** no user-visible TUI behavior changes
