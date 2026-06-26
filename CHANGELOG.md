# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.6](https://github.com/amittamari/hop/compare/v0.2.5...v0.2.6) - 2026-06-26

### Fixed

- *(tui)* prettify confirmation modal to match help overlay ([#38](https://github.com/amittamari/hop/pull/38))
- *(tui)* improve preview header UX ([#42](https://github.com/amittamari/hop/pull/42))
- *(ci)* use CHANGELOG.md for release notes instead of auto-generated

### Other

- update demo gif
- embed SessionSummary in Session, move columns to tui, extract modal
- optimize dependencies and binary size (14M → 8M)
- *(skills)* add `garden-docs` skill

## [0.2.5](https://github.com/amittamari/hop/compare/v0.2.4...v0.2.5) - 2026-06-26

### Added

- *(search)* boost recent sessions in free-text ranking ([#43](https://github.com/amittamari/hop/pull/43))

### Other

- fmt
- sync architecture and project docs with recent code changes
- document runtime dependencies and add gh to Homebrew formula
- update demo gif

## [0.2.4](https://github.com/amittamari/hop/compare/v0.2.3...v0.2.4) - 2026-06-26

### Added

- *(update)* notify when a newer version is available

### Fixed

- *(cli)* append trailing space to initial query for immediate typing
- *(resume)* warn when session directory does not exist ([#31](https://github.com/amittamari/hop/pull/31))
- *(adapters)* resolve repo from ancestor dirs for deleted worktrees ([#37](https://github.com/amittamari/hop/pull/37))
- *(preview)* clear stale transcript when results become empty ([#23](https://github.com/amittamari/hop/pull/23))

## [0.2.3](https://github.com/amittamari/hop/compare/v0.2.2...v0.2.3) - 2026-06-17

### Added

- *(config)* add custom launcher command for session resume ([#39](https://github.com/amittamari/hop/pull/39))

### Fixed

- *(cursor)* skip blocked subagent sessions ([#33](https://github.com/amittamari/hop/pull/33))

### Other

- add VHS demo recording and embed in README ([#36](https://github.com/amittamari/hop/pull/36))

## [0.2.2](https://github.com/amittamari/hop/compare/v0.2.1...v0.2.2) - 2026-06-13

### Added

- *(tui)* open the selected session's PR in the browser ([#29](https://github.com/amittamari/hop/pull/29))

### Other

- rename product spec

## [0.2.1](https://github.com/amittamari/hop/compare/v0.2.0...v0.2.1) - 2026-06-13

### Added

- *(codex)* handle archived sessions ([#27](https://github.com/amittamari/hop/pull/27))
- *(cli)* auto-scope to the current repo on launch ([#26](https://github.com/amittamari/hop/pull/26))
- *(tui)* make Ctrl-chord keybindings configurable via config.toml ([#25](https://github.com/amittamari/hop/pull/25))
- *(tui)* add scroll affordances for results and preview
- *(tui)* add empty, indexing, and animated-glyph screen states ([#24](https://github.com/amittamari/hop/pull/24))
- *(repo)* canonical repo column + repo: filter across worktrees ([#22](https://github.com/amittamari/hop/pull/22))

### Fixed

- *(ci)* use git-only mode so release-plz detects releases from tags
- *(command-tag)* filter more command tags
- *(cursor)* strip `[REDACTED]` message

### Other

- *(release-plz)* trigger on push to master
- *(tui)* bindings
- *(tui)* improve tui responsiveness and layout
- Revert "feat(tui): add scroll affordances for results and preview"
- *(tui)* simplify table rendering and app plumbing
- *(tui)* results list ([#21](https://github.com/amittamari/hop/pull/21))
- *(tui)* introduce semantic Theme system ([#17](https://github.com/amittamari/hop/pull/17))
- TUI design review and execution plans ([#16](https://github.com/amittamari/hop/pull/16))
- harden workflow token permissions; change release-plz trigger ([#14](https://github.com/amittamari/hop/pull/14))
- enforce conventional PR titles and required checks ([#12](https://github.com/amittamari/hop/pull/12))
- disable cargo-semver-checks in release-plz

## [0.2.0](https://github.com/amittamari/hop/compare/v0.1.0...v0.2.0) - 2026-06-11

### Added

- *(adapters)* add Cursor CLI chat support

### Fixed

- *(ci)* push tap formula from empty repo

### Other

- automate releases with release-plz + add CI workflow
- add 'make install'
- *(readme)* reorder quick start section
- *(release)* drop completed one-time tap setup section
- *(brew)* rename tap repo to homebrew-tap
