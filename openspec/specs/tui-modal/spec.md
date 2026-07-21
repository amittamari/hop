# Capability: TUI Modal

## Purpose

Renders the centered confirmation modal for session resume, showing session details, yolo toggle, archive warnings, and missing-directory warnings.

## Requirements

### Requirement: Centering
The modal SHALL be centered on both axes within the terminal area, clamped to the available dimensions.

#### Scenario: Centered in large terminal
- **GIVEN** a terminal of 100x40
- **WHEN** a 20x10 modal is centered
- **THEN** the modal SHALL be at position (40, 15)

### Requirement: Modal content
The modal SHALL display: session title, directory, and resume command (all truncated to fit). When the session is archived, an archive warning line SHALL appear. When the directory does not exist on disk, a missing-directory warning SHALL appear.

#### Scenario: Archived session warning
- **GIVEN** the selected session is archived
- **WHEN** the modal is rendered
- **THEN** an archive warning line SHALL be visible containing `"archived"`

### Requirement: Yolo toggle
The modal SHALL show the yolo state: "YOLO on: approvals and sandbox may be bypassed" (in warning style) when on, "YOLO off: normal resume" (in muted style) when off.

#### Scenario: Yolo on display
- **GIVEN** the yolo flag is true
- **WHEN** the modal is rendered
- **THEN** the text `"YOLO on"` SHALL appear in warning style

#### Scenario: Yolo off display
- **GIVEN** the yolo flag is false
- **WHEN** the modal is rendered
- **THEN** the text `"YOLO off"` SHALL appear in muted style

### Requirement: Key hints
The modal footer SHALL display: `Tab` toggle yolo, `Enter` resume (or "unarchive & resume" when archived), `Esc` cancel. Keys SHALL be styled in accent color.

#### Scenario: Archived session confirm label
- **GIVEN** the selected session is archived
- **WHEN** the modal is rendered
- **THEN** the Enter hint SHALL read `"unarchive & resume"`

### Requirement: Overlay scrim
The modal SHALL dim the background by applying the overlay foreground/background colors to the full terminal area before rendering the bordered modal widget.

#### Scenario: Background dimmed
- **WHEN** the modal is rendered
- **THEN** the full terminal area SHALL have the overlay foreground and background colors applied

### Requirement: Value truncation
Session title, directory, and command strings SHALL be truncated to fit the modal's inner width using `fit_for_modal`, which delegates to the column solver's `fit` function.

#### Scenario: Long title truncated
- **GIVEN** a session title longer than the modal inner width
- **WHEN** the modal is rendered
- **THEN** the displayed title SHALL be truncated with an ellipsis
