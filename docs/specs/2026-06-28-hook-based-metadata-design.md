# Hook-Based Session Metadata

**Issue**: [#64](https://github.com/amittamari/hop/issues/64)
**Date**: 2026-06-28
**Status**: Draft

## Problem

hop extracts session metadata (branch, repo, cwd, worktree, permission mode)
by scraping vendor-specific JSONL, SQLite, and log files. Each provider has
gaps:

- **Worktree**: completely untracked — no vendor provides it.
- **Branch (Cursor)**: always `None` — Cursor records no git branch info.
- **Repo URL**: requires the repo to still exist on disk at index time
  (Claude and Cursor resolve via live `git remote`).
- **Branch changes**: only the first value is captured for Claude sessions.
- **Permission mode**: unreliable across vendors — Claude always returns
  `false` for yolo, Codex checks two policy fields, Cursor reads a SQLite
  boolean. No vendor distinguishes `auto` mode.
- **cwd (Cursor)**: parsed from an unstructured `worker.log` line
  (`workspacePath=...`); if the log is missing or the format changes, cwd is
  lost — and that cascades to losing title, timestamp, and yolo too.

## Solution

Hop-owned hooks that call `hop meta capture` at session start and stop.
The command collects git metadata and writes JSON sidecars to hop-owned
storage. During indexing, sidecar data is merged with vendor data.

## Architecture

### Sidecar Storage

Location: `~/.hop/meta/<agent>/<session-id>.json`

Example: `~/.hop/meta/claude/5bb29e19-3ca4-403e-a3fe-d011e814aaef.json`

```json
{
  "version": 1,
  "session_id": "5bb29e19-3ca4-403e-a3fe-d011e814aaef",
  "agent": "claude",
  "events": [
    {
      "event": "start",
      "timestamp": 1719500000,
      "cwd": "/Users/user/project",
      "branch": "feature-hooks",
      "repo_url": "git@github.com:user/repo.git",
      "worktree": null,
      "permission_mode": null
    },
    {
      "event": "stop",
      "timestamp": 1719500300,
      "cwd": "/Users/user/project",
      "branch": "main",
      "repo_url": "git@github.com:user/repo.git",
      "worktree": null,
      "permission_mode": null
    }
  ]
}
```

Design notes:

- **Event list, not latest-wins** — both start and stop snapshots are stored
  so hop can show the initial branch, detect branch changes, and use the final
  state as the authoritative value for display and search.
- **`version` field** — allows schema evolution without breaking existing
  sidecars.
- **`permission_mode`** — string: `"default"`, `"yolo"`, `"auto"`, or `null`.
  Replaces the boolean `yolo` field. Populated as `null` for now since hook
  stdin does not reliably provide this; vendor data remains the primary source.
- **`worktree`** — path to the worktree root if in one, `null` otherwise.
- **Append-only during a session** — start event creates the file, stop event
  reads and appends to the events array.

### `hop meta capture` Command

Not user-facing. Called by provider hooks.

```
hop meta capture --agent <claude|codex|cursor> --event <start|stop>
```

Behavior:

1. **Read stdin** — parse the provider's hook JSON to extract `session_id`
   and `cwd`.
2. **Collect git metadata from `cwd`**:
   - `branch`: `git rev-parse --abbrev-ref HEAD` (detached HEAD → `None`)
   - `repo_url`: `git remote get-url origin` (no remote → `None`)
   - `worktree`: compare `git rev-parse --show-toplevel` with
     `git rev-parse --git-common-dir` — if they differ, it's a worktree;
     store the toplevel path
   - `permission_mode`: `null` for now (see sidecar notes above)
3. **Write sidecar**:
   - On `start`: create `~/.hop/meta/<agent>/<session-id>.json` with one
     event.
   - On `stop`: read existing file, append the stop event, write back.
   - If the file doesn't exist on `stop` (hooks installed mid-session),
     create it with just the stop event.
4. **Fail silently** — hooks must never block or break the user's session.
   All errors are swallowed. Optionally log to `~/.hop/logs/` for debugging.

### Provider Hook Installation

#### Claude Code (full support)

**Hook events**: `SessionStart`, `SessionEnd`

**Hook input (stdin JSON)**: `session_id`, `cwd`, `transcript_path`,
`hook_event_name`

**Install**: Merge into `~/.claude/settings.json` under `hooks`:

```json
{
  "hooks": {
    "SessionStart": [{
      "hooks": [{
        "type": "command",
        "command": "hop meta capture --agent claude --event start"
      }]
    }],
    "SessionEnd": [{
      "hooks": [{
        "type": "command",
        "command": "hop meta capture --agent claude --event stop"
      }]
    }]
  }
}
```

**Merge strategy**: Read existing settings, append hop entries alongside
existing hooks. Each hop hook group carries an `"id": "hop-meta"` field
so it can be identified for removal:

```json
{
  "id": "hop-meta",
  "hooks": [{
    "type": "command",
    "command": "hop meta capture --agent claude --event start"
  }]
}
```

**Uninstall**: Scan hook arrays for entries with `"id": "hop-meta"` and
remove them. If a hook array becomes empty, remove the key.

**Detection**: `~/.claude` directory exists with `settings.json`.

#### Codex (full support)

**Hook events**: `SessionStart`, `Stop`

**Hook input (stdin JSON)**: `session_id`, `cwd`, `hook_event_name`

**Install**: Create a hop plugin directory at
`~/.codex/.tmp/plugins/plugins/hop/` with a `hooks.json`:

```json
{
  "hooks": {
    "SessionStart": [{
      "hooks": [{
        "type": "command",
        "command": "hop meta capture --agent codex --event start"
      }]
    }],
    "Stop": [{
      "hooks": [{
        "type": "command",
        "command": "hop meta capture --agent codex --event stop"
      }]
    }]
  }
}
```

Note: Codex uses `Stop` (fires each time the agent stops) rather than
`SessionEnd`. The last `Stop` before exit captures the final state.

**Uninstall**: Remove the `~/.codex/.tmp/plugins/plugins/hop/` directory.

**Detection**: `~/.codex` directory exists with `config.toml`.

#### Cursor (best-effort)

**Hook events**: `stop` only — no session start event, no `session_id` or
`cwd` in hook input.

**Install**: Merge into `~/.cursor/hooks.json`:

```json
{
  "hooks": {
    "stop": [{
      "command": "hop meta capture --agent cursor --event stop"
    }]
  }
}
```

**Limitations**:

- No session start event.
- No `session_id` or `cwd` in hook input — `hop meta capture` would need to
  infer cwd from the process environment or accept that Cursor sidecars will
  be incomplete.
- Primary enrichment for Cursor happens at **index time**: when hop indexes a
  Cursor session, it runs git commands against the workspace path (extracted
  from `worker.log` or `meta.json`) to fill in branch, repo_url, and
  worktree.

**Uninstall**: Remove hop entries from `~/.cursor/hooks.json`.

**Detection**: `~/.cursor` directory exists or
`~/Library/Application Support/Cursor/` exists.

### CLI Surface

#### `hop hooks install`

1. Auto-detect installed providers.
2. Show interactive picker with checkboxes — detected providers pre-selected,
   Cursor marked as best-effort.
3. Install hooks for selected providers.
4. Print summary of what was installed and where.

Flags:
- `--all` — skip picker, install for all detected providers.
- `--provider <name>` — install for a specific provider only.

#### `hop hooks uninstall`

1. Detect which providers have hop hooks installed.
2. Remove exactly what `install` added.
3. Print what was removed.

Flags:
- `--all` — uninstall from all providers.
- `--provider <name>` — uninstall from a specific provider.

#### `hop hooks status`

Show installed state per provider:

```
Claude Code: installed (SessionStart + SessionEnd)
Codex: installed (SessionStart + Stop)
Cursor: not installed
```

### Index Integration

#### Merge Strategy

After each adapter parses a session into `SessionSummary`, a merge step
checks for a matching sidecar at `~/.hop/meta/<agent>/<session-id>.json`.

| Field             | Primary source         | Fallback                       |
|-------------------|------------------------|--------------------------------|
| `cwd`             | Sidecar (when present) | Vendor data                    |
| `branch`          | Sidecar (last event)   | Vendor JSONL                   |
| `repo_url`        | Sidecar                | Vendor JSONL → live `git remote` |
| `worktree`        | Sidecar                | None (new field)               |
| `permission_mode` | Vendor data            | Sidecar (future)               |
| `title`           | Vendor data            | —                              |
| `timestamp`       | Vendor data            | Sidecar                        |

Sidecar wins for git fields and cwd because it captures state at the actual
moment of session start/stop. Vendor wins for title, timestamp, and
permission mode because these are reliably provided by vendors.

#### Schema Changes

New fields on `SessionSummary`:

- `worktree: Option<String>` — worktree path, `None` if not in a worktree.
- `permission_mode: Option<String>` — replaces `yolo: bool`. Values:
  `"default"`, `"yolo"`, `"auto"`, or `None`.

Tantivy index schema bumps to version 3 with new stored fields `worktree`
and `permission_mode`. This triggers an automatic index rebuild on first run.

The `yolo` field on `SessionSummary` and in the index schema is deprecated
in favor of `permission_mode`. Migration: `yolo: true` → `permission_mode:
Some("yolo")`, `yolo: false` → `permission_mode: Some("default")`.
