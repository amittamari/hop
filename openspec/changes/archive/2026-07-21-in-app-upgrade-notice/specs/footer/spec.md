## ADDED Requirements

### Requirement: Upgrade available indicator
The footer's right-side status SHALL display an upgrade-available indicator when a newer version has been detected. The indicator SHALL be styled with `theme.accent` color and formatted as `↑ v<version>` (e.g., `↑ v0.8.1`). The indicator SHALL persist for the entire TUI session once set.

#### Scenario: Update available
- **WHEN** the background update check detects a newer version `0.8.1`
- **THEN** the footer status SHALL show `↑ v0.8.1` styled with `theme.accent`
- **AND** the indicator SHALL remain visible for the rest of the session

#### Scenario: No update available
- **WHEN** the background update check finds the current version is latest
- **THEN** no upgrade indicator SHALL appear in the footer

#### Scenario: Narrow terminal hides status
- **WHEN** the terminal is too narrow to fit both key-hints and right-side status
- **THEN** the upgrade indicator SHALL be hidden along with other status fields (existing priority rule)
