## ADDED Requirements

### Requirement: Mouse capture lifecycle
The TUI SHALL enable mouse capture when it starts so that wheel/trackpad scroll
events are delivered to the application, and SHALL disable mouse capture when the
TUI is torn down — including before restoring the terminal to exec-resume a
session — so the terminal returns to normal mouse behavior.

#### Scenario: Entering the TUI
- **WHEN** the TUI starts
- **THEN** mouse capture SHALL be enabled in addition to raw mode and the
  alternate screen

#### Scenario: Leaving the TUI
- **WHEN** the TUI exits normally, or restores the terminal before resuming a
  session
- **THEN** mouse capture SHALL be disabled and the terminal restored to its
  prior state

### Requirement: Wheel scroll targets the conversation preview
A mouse/trackpad scroll event SHALL adjust the conversation preview's vertical
scroll offset when the preview pane is visible, and SHALL NOT change the
sessions-list selection.

#### Scenario: Scrolling down over an open preview
- **WHEN** the preview pane is visible and the user scrolls down
- **THEN** the preview content SHALL scroll down by a small line step and the
  selected session SHALL remain unchanged

#### Scenario: Scrolling up over an open preview
- **WHEN** the preview pane is visible and the preview is scrolled past the top
- **THEN** the preview content SHALL scroll up by a small line step, clamped so
  the offset does not go below the top of the transcript

### Requirement: Wheel scroll granularity
Wheel scrolling SHALL move the preview by a small, fixed number of lines per
scroll event, distinct from (and smaller than) the page-sized step used by the
keyboard scroll commands, so that wheel/trackpad scrolling reads smoothly.

#### Scenario: Single scroll notch
- **WHEN** the user produces one scroll event
- **THEN** the preview offset SHALL change by the small line step, not by a full
  page

### Requirement: Wheel scroll ignored without a preview
When the conversation preview pane is not visible, mouse scroll events SHALL be
ignored and SHALL NOT move the sessions-list selection.

#### Scenario: Scrolling with the preview hidden
- **WHEN** the preview pane is not visible and the user scrolls
- **THEN** the application state SHALL be unchanged and keyboard list navigation
  SHALL continue to work as before
