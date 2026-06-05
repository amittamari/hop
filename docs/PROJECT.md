# Project

`hop` is a fast terminal tool for searching and resuming coding-agent sessions.
It currently supports Claude Code and Codex.

The core user journey is:

1. Open `hop`.
2. Search by remembered words, agent, directory, or date.
3. Preview a cleaned transcript.
4. Resume the selected session in the original agent and directory.

## Product Contract

- Launch should render immediately from the existing local index.
- Background sync should update the index without blocking the TUI.
- Search should cover conversation content, not only titles.
- Preview should show user/agent turns and hide internal noise such as tool calls,
  command tags, meta lines, and system reminders.
- Resume should restore the terminal, `chdir` to the session directory, and
  `exec` the appropriate agent command.

## Scope

In scope:

- Claude Code sessions under the configured Claude data directory.
- Codex sessions under the configured Codex data directory.
- Full-text search with keyword filters.
- TUI preview, columns, background PR enrichment, and yolo resume.
- Optional TOML config with zero-config defaults.

Out of scope for now:

- Runtime plugins.
- Non-interactive list/stat modes.
- Supporting every coding agent.
- Remote or team-shared indexes.

## User-Facing Commands

```sh
hop
hop auth refresh
hop -a claude -d api
hop --rebuild
```

See `README.md` for current keybindings and query syntax.

## Data Sources

- Claude Code: JSONL sessions parsed by `src/adapters/claude.rs`.
- Codex: JSONL sessions parsed by `src/adapters/codex.rs`.
- GitHub PR enrichment: optional background lookup through the `gh` CLI.

Adapters are responsible for turning source-specific JSONL shapes into stable
`core::Session` and `core::Message` values. Downstream modules should not depend
on raw Claude or Codex JSON. When multiple adapters need the same source-agnostic
policy, keep that policy in `core` helpers and have adapters supply only their
source-specific candidates. Examples include title fallback/normalization and
shared transcript text filtering/flattening.

## Quality Bar

- `cargo test` should pass before handing off changes.
- Fixtures should cover parser behavior whenever session JSON handling changes.
- Search/index changes should include integration coverage in `tests/index_sync.rs`
  or focused unit tests in `src/index.rs`.
- TUI behavior should stay testable through ratatui `TestBackend` tests.

## Documentation Sources

- `docs/ARCHITECTURE.md` is the current architecture map.
- `docs/specs/` contains dated design specs and may include historical decisions.
- `docs/plans/` contains execution plans. Treat them as history unless
  a current task explicitly says a plan is active.
- `docs/reviews/` contains dated review artifacts and action-item source material.
  Promote only durable rules into `docs/ARCHITECTURE.md`; leave temporary findings
  in review docs or `Known Pressure Points` until resolved.
