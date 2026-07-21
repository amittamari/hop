## MODIFIED Requirements

### Requirement: Version check
`check_for_update` SHALL compare the current binary version against the latest GitHub release. It SHALL return `Some(UpdateAvailable)` only when the latest version is strictly newer.

When launched with the TUI, the background update-check thread SHALL send the result as an `Update::UpgradeAvailable` variant through the shared `Sender<Update>` channel instead of returning a `JoinHandle`. The post-exit stderr message SHALL be removed.

When launched with `--version`, the update check SHALL continue to run synchronously and print the verbose upgrade message to stderr.

#### Scenario: Current version is latest
- **GIVEN** the cached latest version matches the current binary version
- **WHEN** `check_for_update` is called
- **THEN** it SHALL return `None`

#### Scenario: Newer version available (TUI path)
- **GIVEN** the latest version is newer than the current version
- **WHEN** the TUI is running
- **THEN** the background thread SHALL send `Update::UpgradeAvailable { latest }` through the channel
- **AND** no upgrade message SHALL be printed to stderr after exit

#### Scenario: Newer version available (--version path)
- **GIVEN** the latest version is newer than the current version
- **WHEN** `hop --version` is run
- **THEN** the verbose upgrade message (versions + install command) SHALL be printed to stderr
