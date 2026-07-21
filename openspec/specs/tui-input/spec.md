# Capability: TUI Input Handling

## Purpose

Routes keyboard and mouse events to actions: key dispatch, keymap command application, toolbar/search-mode transitions, query-line editing with cursor movement, and the activate (resume vs. confirm-modal) decision.

## Requirements

### Requirement: Global quit
Ctrl+C SHALL always produce `Action::Quit`, even from overlays and modals.

#### Scenario: Ctrl+C from help overlay
- **GIVEN** the help overlay is open
- **WHEN** the user presses Ctrl+C
- **THEN** the action SHALL be `Action::Quit`

### Requirement: Help overlay
When the help overlay is open, all keys SHALL be swallowed except Esc and `?`, which close it.

#### Scenario: Typing while help is open
- **GIVEN** the help overlay is open
- **WHEN** the user presses a letter key
- **THEN** the action SHALL be `Action::None`
- **AND** the help overlay SHALL remain open

#### Scenario: Esc closes help
- **GIVEN** the help overlay is open
- **WHEN** the user presses Esc
- **THEN** the help overlay SHALL close

### Requirement: Yolo modal keys
In the yolo modal: Esc SHALL cancel, Tab SHALL toggle the yolo flag, Enter SHALL confirm with `Action::Resume { index, yolo }`.

#### Scenario: Tab toggles yolo in modal
- **GIVEN** the yolo modal is open with yolo off
- **WHEN** the user presses Tab
- **THEN** yolo SHALL become true

#### Scenario: Enter confirms resume
- **GIVEN** the yolo modal is open at index 3 with yolo on
- **WHEN** the user presses Enter
- **THEN** the action SHALL be `Action::Resume { index: 3, yolo: true }`

### Requirement: Keymap chords
Chord-bound commands (from the keymap) SHALL be checked before the main key dispatch. They produce actions like toggle preview, scroll preview, resize preview, open PR, or toggle search mode.

#### Scenario: Ctrl+P toggles preview
- **GIVEN** the default keymap is active
- **WHEN** the user presses Ctrl+P
- **THEN** the preview visibility SHALL toggle

### Requirement: Esc behavior
In main mode, Esc SHALL clear a non-empty query (producing `Action::Search`). When the query is already empty, Esc SHALL produce `Action::Quit`.

#### Scenario: Esc clears query
- **GIVEN** the query is `"auth bug"`
- **WHEN** the user presses Esc
- **THEN** the query SHALL become empty
- **AND** the action SHALL be `Action::Search`

#### Scenario: Esc quits on empty query
- **GIVEN** the query is empty
- **WHEN** the user presses Esc
- **THEN** the action SHALL be `Action::Quit`

### Requirement: Navigation
Down/Up SHALL move the selection by one row (clamping to bounds) and reset preview scroll. PageDown/PageUp SHALL move by one page.

#### Scenario: Down moves selection
- **GIVEN** the selection is at index 0 with 5 results
- **WHEN** the user presses Down
- **THEN** the selection SHALL be 1
- **AND** preview scroll SHALL reset to 0

#### Scenario: Down clamps at end
- **GIVEN** the selection is at the last result
- **WHEN** the user presses Down
- **THEN** the selection SHALL remain at the last index

### Requirement: Tab in simple mode
Tab SHALL cycle toolbar focus (Query -> Scope -> Sort -> Query); Shift+Tab SHALL cycle in reverse. When no repo is detected, the Scope control SHALL be skipped.

#### Scenario: Tab cycles focus with repo
- **GIVEN** simple mode with a repo detected and focus on Query
- **WHEN** the user presses Tab
- **THEN** focus SHALL move to Scope

#### Scenario: Tab skips scope without repo
- **GIVEN** simple mode with no repo detected and focus on Query
- **WHEN** the user presses Tab
- **THEN** focus SHALL move to Sort (skipping Scope)

### Requirement: Tab in raw mode
Tab SHALL trigger `autocomplete` on the query (e.g. `agent:cl` -> `agent:claude`).

#### Scenario: Tab autocompletes agent
- **GIVEN** raw mode with query `"agent:cl"`
- **WHEN** the user presses Tab
- **THEN** the query SHALL become `"agent:claude"`
- **AND** the action SHALL be `Action::Search`

### Requirement: Toolbar adjustment
When a toolbar control is focused (simple mode only), Left/Right SHALL adjust the control: Scope toggles between ThisRepo and All; Sort cycles through Relevance/Recent/Oldest.

#### Scenario: Right cycles sort forward
- **GIVEN** simple mode with focus on Sort and current sort Relevance
- **WHEN** the user presses Right
- **THEN** the sort SHALL become Recent

### Requirement: Query editing
Printable characters (without Ctrl) SHALL be inserted at the cursor position. Backspace/Delete SHALL remove characters. Home/End SHALL move the cursor to the start/end. Left/Right (when query is focused) SHALL move the cursor by one character, respecting UTF-8 boundaries.

#### Scenario: Character insertion
- **GIVEN** the query is `"at"` with cursor at 1
- **WHEN** the user types `'u'`
- **THEN** the query SHALL become `"aut"` with cursor at 2

#### Scenario: Backspace removes character
- **GIVEN** the query is `"auth"` with cursor at 4
- **WHEN** the user presses Backspace
- **THEN** the query SHALL become `"aut"` with cursor at 3

### Requirement: Help key
`?` (without Ctrl) SHALL open the help overlay instead of typing.

#### Scenario: Question mark opens help
- **GIVEN** the help overlay is closed
- **WHEN** the user presses `?`
- **THEN** the help overlay SHALL open
- **AND** no character SHALL be inserted into the query

### Requirement: Activate logic
Enter SHALL open the yolo confirmation modal when the selected session's agent supports yolo or the session is archived. Otherwise, it SHALL produce `Action::Resume` directly.

#### Scenario: Enter opens modal for yolo-capable agent
- **GIVEN** the selected session's agent supports yolo
- **WHEN** the user presses Enter
- **THEN** the yolo modal SHALL open

#### Scenario: Enter resumes directly for non-yolo agent
- **GIVEN** the selected session's agent does not support yolo and the session is not archived
- **WHEN** the user presses Enter
- **THEN** the action SHALL be `Action::Resume { index, yolo: false }`

### Requirement: Mouse scroll
Mouse wheel events SHALL scroll the preview pane (3 lines per event) when the preview is visible. Non-scroll mouse events SHALL be ignored.

#### Scenario: Scroll down in preview
- **GIVEN** the preview is visible with scroll at 0
- **WHEN** a ScrollDown mouse event occurs
- **THEN** the preview scroll SHALL advance by 3

#### Scenario: Mouse scroll ignored when preview hidden
- **GIVEN** the preview is not visible
- **WHEN** a ScrollDown mouse event occurs
- **THEN** the preview scroll SHALL remain unchanged

### Requirement: Search mode toggle
The toggle command SHALL expand simple-mode state into a raw DSL string (simple->raw) or lift a `repo:` token from the raw query into the Scope control (raw->simple), preserving intent across modes.

#### Scenario: Simple to raw expands scope
- **GIVEN** simple mode with query `"auth"` and repo scope `"me/hop"`
- **WHEN** the search mode is toggled
- **THEN** the mode SHALL become Raw
- **AND** the query SHALL become `"repo:me/hop auth"`
