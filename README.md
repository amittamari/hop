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

Keys: type to search · ↑↓ move · Enter resume · Ctrl+Y yolo · Ctrl+P toggle preview ·
`[` / `]` resize preview · Ctrl+U/D scroll preview · Tab autocomplete · ? help · Esc quit.

## Columns

Each result row is an aligned grid: `AGENT · REPO · BRANCH · TITLE · MSGS · PR · TIME`.
The repo and branch come straight from the conversation data (Claude's `gitBranch`,
Codex's `git.branch`/`repository_url`); the full directory path is shown in the preview
header rather than as a column. Claude titles prefer the recorded `aiTitle`/summary
when present, then fall back to the first real user prompt. The `PR` column is resolved
in the background via the `gh` CLI and cached on disk, so it never blocks the UI (`⟳`
while resolving, `—` if none).
Narrow terminals drop columns by priority (PR → MSGS → TIME → BRANCH → REPO); the title
always survives, but repo and branch get useful width before the title expands.

## Query syntax

| Form | Meaning |
| --- | --- |
| `auth refresh` | free-text terms (fuzzy + exact, over full conversation content) |
| `agent:claude,codex` | restrict to these agents |
| `-agent:codex` / `agent:claude,!codex` | exclude an agent |
| `dir:api` / `-dir:vendor` | directory substring include / exclude |
| `date:today` `date:yesterday` `date:week` `date:month` | recency windows |
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
keymap = "search"   # default; or "modal" for a vim-style navigate mode (Esc to enter)

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
`docs/PROJECT.md` and `docs/ARCHITECTURE.md`. Dated specs and execution plans live
under `docs/specs/` and `docs/superpowers/plans/`.
