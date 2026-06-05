# hop

Fast full-text search and resume for coding-agent sessions (Claude Code + Codex).

`hop` aggregates your past Claude Code and Codex sessions into a single full-text
index and lets you jump straight back into any of them — type a few words you
remember, pick the result, hit Enter, and you're resumed in the original agent, in
the original working directory.

## Install

    cargo install --path .

## Usage

    hop                      # open the TUI
    hop auth refresh         # pre-filled query
    hop -a claude -d api      # filter by agent + directory
    hop --rebuild             # wipe and rebuild the index

Default keys: type to search · ←/→ Home/End edit query · ↑↓ move · PgUp/PgDn
page by viewport · Enter resume · Ctrl+Y yolo prompt · Ctrl+P toggle preview ·
Ctrl+U/D scroll preview by viewport · Ctrl+N/B next/previous preview match ·
Tab autocomplete · `[` / `]` resize preview when the query is empty · ? help
when the query is empty · Esc quit. With `keymap = "modal"`, Esc switches from
SEARCH to NAV mode, `/` returns to search, and Ctrl+C quits globally.

## Columns

Each result row is an aligned grid: `AGENT · REPO · BRANCH · TITLE · MSGS · PR · TIME`.
The branch comes from conversation data (Claude's `gitBranch`, Codex's
`git.branch`). The repo label prefers Codex's `repository_url` when present, then
falls back to the directory basename; the full directory path is shown in the
preview header rather than as a column. Claude titles prefer the recorded
`aiTitle`/summary when present, then fall back to the first real user prompt.
Titles are whitespace-normalized in the index and truncated only to the rendered
column width.
The `PR` column is resolved in the background via the `gh` CLI and cached on
disk, so it never blocks the UI (`⟳` while resolving, `—` if none).
Narrow terminals drop columns by priority (PR → MSGS → TIME → BRANCH → REPO).
Repo and branch size to visible content when space allows; leftover width goes
to the title.

## Query syntax

| Form | Meaning |
| --- | --- |
| `auth refresh` | free-text terms (fuzzy + exact, over full conversation content) |
| `agent:claude,codex` | restrict to these agents |
| `-agent:codex` / `agent:claude,!codex` | exclude an agent |
| `dir:api` / `-dir:vendor` | directory substring include / exclude |
| `date:today` `date:yesterday` | local calendar-day filters |
| `date:week` `date:month` | recency windows |
| `date:<2d` / `date:>1w` | within / older than a duration (`h`/`d`/`w`) |

Press `Tab` to autocomplete keyword values (e.g. `agent:cl` → `agent:claude`).

## How it works

The index lives under your platform cache dir (e.g. `~/.cache/hop/`). On launch
`hop` opens it immediately and renders whatever is already indexed, then syncs new
sessions in the background so results stream in without a blocking spinner. Resume
restores the terminal, `chdir`s to the session's directory, and `exec`-replaces the
process with the agent CLI.

The preview pane re-parses the selected session on demand into a clean transcript
(internals like tool calls, `<command-*>` tags and system reminders are stripped),
with syntax-highlighted code and the matched query terms highlighted.

## Config

Optional TOML at your platform config dir (e.g. `~/.config/hop/config.toml`); all keys
have zero-config defaults. The chosen preview width/visibility also persist across
restarts automatically.

```toml
keymap = "search"   # default; or "modal" for a vim-style navigate mode (Esc enters navigate)

[preview]
visible = true
width_pct = 50
metadata_header = true

[columns]
disabled = []       # e.g. ["pr"] to turn off the background GitHub PR column
order = []          # e.g. ["agent", "title", "time"]; unspecified columns follow
```

`theme` and `[keybindings]` config tables are accepted for forward compatibility
but are reserved and not applied yet.

## Documentation

For contributor and agent context, start with `AGENTS.md`, then read
`docs/PROJECT.md` and `docs/ARCHITECTURE.md`. Dated specs, review artifacts, and
execution plans live under `docs/specs/`, `docs/reviews/`, and
`docs/plans/`.
