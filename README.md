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

Keys: type to search · ↑↓ move · Enter resume · Tab yolo · Esc quit.

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
