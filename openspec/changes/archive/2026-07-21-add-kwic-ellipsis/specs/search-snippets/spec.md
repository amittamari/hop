## ADDED Requirements

### Requirement: Snippet ellipsis indicators

When a KWIC snippet is rendered on a card row, the display SHALL include leading and trailing `...` (three ASCII dots) to indicate the snippet is a fragment of longer content. Tantivy's inter-fragment `...` separators pass through unchanged.

#### Scenario: Leading ellipsis on snippet
- **WHEN** a snippet is rendered on a card row
- **THEN** the displayed text SHALL begin with `...` followed by the snippet content

#### Scenario: Trailing ellipsis on snippet
- **WHEN** a snippet is rendered on a card row
- **THEN** the displayed text SHALL end with `...` after the snippet content

#### Scenario: Ellipsis style
- **WHEN** ellipsis indicators are rendered (leading or trailing)
- **THEN** they SHALL use the muted text style, consistent with non-highlighted snippet text

#### Scenario: Ellipsis within width budget
- **WHEN** a snippet is rendered on a narrow terminal
- **THEN** the leading and trailing ellipsis SHALL be included within the available width budget, reducing the visible context text rather than causing overflow
