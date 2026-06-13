# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
