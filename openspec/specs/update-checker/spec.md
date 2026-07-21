# Capability: Update Checker

## Purpose

Checks for new hop releases on GitHub, caches the result for 24 hours, detects the install method (Homebrew/cargo/unknown), and produces an appropriate upgrade message.

## Requirements

### Requirement: Version check
`check_for_update` SHALL compare the current binary version against the latest GitHub release. It SHALL return `Some(UpdateAvailable)` only when the latest version is strictly newer.

#### Scenario: Current version is latest
- **GIVEN** the cached latest version matches the current binary version
- **WHEN** `check_for_update` is called
- **THEN** it SHALL return `None`

#### Scenario: Newer version available
- **GIVEN** the cached latest version is `"99.0.0"` (newer than current)
- **WHEN** `check_for_update` is called
- **THEN** it SHALL return `Some(UpdateAvailable)` with the latest version

### Requirement: Cache
The update check SHALL cache the latest version string and check timestamp in a JSON file. When the cache is fresh (within 24 hours), no network request SHALL be made.

#### Scenario: Cache hit
- **GIVEN** a cache file written less than 24 hours ago
- **WHEN** `check_for_update` is called
- **THEN** the cached version SHALL be used without a network request

#### Scenario: Cache miss
- **GIVEN** no cache file or a stale cache (older than 24 hours)
- **WHEN** `check_for_update` is called
- **THEN** the latest version SHALL be fetched from GitHub and the cache SHALL be written

### Requirement: Install method detection
The checker SHALL detect the install method from the binary's canonical path: paths containing `/Cellar/` or `/homebrew/` -> Homebrew, `/.cargo/bin/` -> CargoInstall, otherwise Unknown.

#### Scenario: Homebrew Cellar path
- **GIVEN** the binary path is `/opt/homebrew/Cellar/hop/0.2.3/bin/hop`
- **WHEN** the install method is detected
- **THEN** it SHALL be `Homebrew`

#### Scenario: Cargo bin path
- **GIVEN** the binary path is `/Users/me/.cargo/bin/hop`
- **WHEN** the install method is detected
- **THEN** it SHALL be `CargoInstall`

### Requirement: Upgrade message
`upgrade_message` SHALL include the current and latest versions. For Homebrew installs, it SHALL suggest `brew upgrade hop`. For other methods, it SHALL link to the GitHub releases page.

#### Scenario: Homebrew upgrade message
- **GIVEN** an update from v0.2.3 to v0.3.0 with install method Homebrew
- **WHEN** `upgrade_message` is called
- **THEN** the message SHALL contain `"v0.2.3"`, `"v0.3.0"`, and `"brew upgrade hop"`

### Requirement: GitHub API
The fetcher SHALL request the latest release from the GitHub API with a 5-second timeout, parse the `tag_name` field, and strip any `v` prefix.

#### Scenario: Tag name with v prefix
- **GIVEN** the API returns `{"tag_name": "v1.2.3"}`
- **WHEN** the latest version is fetched
- **THEN** the version string SHALL be `"1.2.3"`

#### Scenario: Missing tag_name
- **GIVEN** the API returns a JSON object without `tag_name`
- **WHEN** the latest version is fetched
- **THEN** the result SHALL be `None`
