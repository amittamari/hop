## ADDED Requirements

### Requirement: Preview scroll position is clamped to content bounds

The preview scroll offset SHALL never exceed the last line of the transcript
content. All scroll mutation paths — keyboard page-scroll, mouse wheel, and
jump-to-match — SHALL enforce this upper bound so that the preview pane always
shows content when content exists.

#### Scenario: Keyboard scroll down does not overshoot

- **WHEN** the preview pane is visible with a transcript of N lines
- **AND** the user presses Ctrl+D (scroll down) enough times that
  `preview_scroll + scroll_step` would exceed N − 1
- **THEN** `preview_scroll` SHALL be clamped to N − 1
- **AND** the preview pane SHALL display the tail of the transcript

#### Scenario: Mouse wheel scroll down does not overshoot

- **WHEN** the preview pane is visible with a transcript of N lines
- **AND** the user scrolls the mouse wheel down enough that
  `preview_scroll + wheel_delta` would exceed N − 1
- **THEN** `preview_scroll` SHALL be clamped to N − 1

#### Scenario: Scroll down after jump-to-match does not overshoot

- **WHEN** the user jumps to a match near the end of the transcript via Ctrl+N
- **AND** then presses Ctrl+D (scroll down)
- **THEN** `preview_scroll` SHALL be clamped to N − 1
- **AND** the preview pane SHALL NOT be blank

#### Scenario: Content change re-clamps scroll position

- **WHEN** the user has scrolled deep into a long transcript
- **AND** the selection changes to a session with a shorter transcript
- **THEN** `preview_scroll` SHALL be re-clamped to the new content's bounds
  (note: selection change already resets scroll to 0; this covers edge cases
  where the line count is set after a match-jump)

#### Scenario: Scroll up still works normally

- **WHEN** the user presses Ctrl+U (scroll up)
- **THEN** `preview_scroll` SHALL decrease by `scroll_step`, clamped at 0
- **AND** existing lower-bound behavior SHALL be unchanged
