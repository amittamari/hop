# Capability: Enrichment

## Purpose

Pluggable per-session enrichment system with fast (inline) and slow (background) enrichers, a background worker with disk cache, and built-in enrichers for branch, repo name, and GitHub PR number.

## Requirements

### Requirement: Enricher trait
Each enricher SHALL implement `id()`, `kind()` (Fast or Slow), and `resolve()`. Slow enrichers SHALL additionally implement `cache_key()` and `ttl()`.

#### Scenario: Fast enricher requires no cache key
- **GIVEN** a Fast enricher
- **WHEN** `cache_key` is called
- **THEN** it SHALL return an empty string (the default)

### Requirement: Branch enricher
`BranchEnricher` SHALL be a Fast enricher that returns the session's `branch` field when present.

#### Scenario: Branch present
- **GIVEN** a session with `branch: Some("feat/x")`
- **WHEN** `BranchEnricher.resolve` is called
- **THEN** the result SHALL be `Some(EnrichValue { text: "feat/x" })`

#### Scenario: Branch absent
- **GIVEN** a session with `branch: None`
- **WHEN** `BranchEnricher.resolve` is called
- **THEN** the result SHALL be `None`

### Requirement: Repo enricher
`RepoEnricher` SHALL be a Fast enricher that extracts the repo name from `repo_url` (basename of the URL, stripping `.git`), falling back to the directory basename.

#### Scenario: Repo name from URL
- **GIVEN** a session with `repo_url: "git@github.com:me/web.git"`
- **WHEN** `RepoEnricher.resolve` is called
- **THEN** the result SHALL be `Some(EnrichValue { text: "web" })`

#### Scenario: Fallback to directory basename
- **GIVEN** a session with no `repo_url` and directory `/a/myproj`
- **WHEN** `RepoEnricher.resolve` is called
- **THEN** the result SHALL be `Some(EnrichValue { text: "myproj" })`

### Requirement: Repo slug
`repo_slug_from_url` SHALL extract `owner/repo` from git remote URLs, keeping the owner for uniqueness across repos sharing a basename.

#### Scenario: SSH URL
- **GIVEN** URL `"git@github.com:me/web.git"`
- **WHEN** `repo_slug_from_url` is called
- **THEN** it SHALL return `Some("me/web")`

#### Scenario: HTTPS URL
- **GIVEN** URL `"https://github.com/me/web"`
- **WHEN** `repo_slug_from_url` is called
- **THEN** it SHALL return `Some("me/web")`

### Requirement: GitHub PR enricher
`GhPrEnricher` SHALL be a Slow enricher that maps (repo, branch) to a PR number via `gh pr list`. Default branches (`main`, `master`) and empty branches SHALL be skipped. Results SHALL be cached with a 1-hour TTL keyed by `owner/repo@branch`.

#### Scenario: Default branch skipped
- **GIVEN** a session with `branch: "main"`
- **WHEN** `GhPrEnricher.resolve` is called
- **THEN** it SHALL return `None`

### Requirement: PR browser opening
`open_pr_in_browser` SHALL launch `gh pr view <n> --web` for a resolved PR label (e.g. `"#4821"`). Unparseable labels SHALL be rejected without launching. Stdout/stderr SHALL be silenced.

#### Scenario: Unparseable label rejected
- **GIVEN** a PR label `"—"`
- **WHEN** `open_pr_in_browser` is called
- **THEN** it SHALL return `false` without launching any process

### Requirement: Enrichment service
`EnrichmentService` SHALL spawn a background worker thread that receives `EnrichRequest`s, checks the disk cache, resolves via the enricher when stale/missing, writes the cache, and sends `EnrichResult`s back.

#### Scenario: Result returned and cached
- **GIVEN** a request for session `"a"` with enricher `"fake"`
- **WHEN** the service resolves the request
- **THEN** the result SHALL be sent back on the result channel
- **AND** the cache file SHALL be written to disk

### Requirement: Enrichment state
`EnrichmentState` SHALL track which sessions have been requested and resolved, deduplicate requests, and drain results from the service channel.

#### Scenario: Duplicate request not resent
- **GIVEN** a session `"a"` has already been requested
- **WHEN** `request_visible` is called again with the same session
- **THEN** no additional request SHALL be sent to the service
