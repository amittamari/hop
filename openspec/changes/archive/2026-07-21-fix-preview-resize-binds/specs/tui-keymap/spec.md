## MODIFIED Requirements

### Requirement: Default bindings
The keymap SHALL define default Ctrl-chord bindings: Ctrl+C (quit), Ctrl+P (toggle preview), Ctrl+U/D (scroll preview up/down), Ctrl+B/N (jump match prev/next), Ctrl+K/L (resize preview smaller/larger), Ctrl+O (open PR), Ctrl+R (toggle search mode).

#### Scenario: Default toggle preview binding
- **WHEN** the keymap is built with no config overrides
- **THEN** Ctrl+P SHALL be bound to TogglePreview

#### Scenario: Default resize preview bindings
- **WHEN** the keymap is built with no config overrides
- **THEN** Ctrl+K SHALL be bound to ResizePreview(-1) (shrink)
- **AND** Ctrl+L SHALL be bound to ResizePreview(1) (grow)

#### Scenario: Resize bindings work on macOS without Mission Control conflict
- **WHEN** the user presses Ctrl+K or Ctrl+L on macOS with default Mission Control shortcuts enabled
- **THEN** the key events SHALL reach the application and trigger preview resize

## ADDED Requirements

### Requirement: Kitty keyboard protocol support
The application SHALL enable the Kitty keyboard protocol's `DISAMBIGUATE_ESCAPE_CODES` flag on terminals that advertise support, and pop the flag on shutdown.

#### Scenario: Enhanced key detection on Kitty-capable terminal
- **WHEN** the terminal supports keyboard enhancement
- **THEN** the application SHALL push `DISAMBIGUATE_ESCAPE_CODES` during init
- **AND** SHALL pop the enhancement flag during shutdown

#### Scenario: Legacy terminal unaffected
- **WHEN** the terminal does not support keyboard enhancement
- **THEN** the application SHALL NOT attempt to push enhancement flags
- **AND** all default Ctrl+letter bindings SHALL still function correctly

#### Scenario: Panic cleanup
- **WHEN** the application panics after enabling keyboard enhancement
- **THEN** the panic hook SHALL pop the enhancement flag before exiting
