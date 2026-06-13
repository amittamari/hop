# Product Spec: Search And Resume Agent Sessions

Date: 2026-06-05
Status: Current

## Summary

`hop` is a local terminal tool for finding and resuming coding-agent sessions.
It indexes Claude Code and Codex JSONL session files, lets the user search
across prior conversation content, previews a cleaned transcript, and resumes
the selected session in the original agent and working directory.

The product should feel instant on launch, useful with partial remembered text,
and safe around terminal state when handing off to an agent CLI.

## Goals

- Open into an interactive TUI without blocking on a full filesystem scan.
- Search conversation bodies, titles, agents, directories, and dates from one
  query box.
- Make results scannable with agent, repo, branch, title, message count, PR, and
  time metadata.
- Show a readable preview that removes internal command, tool, context, and
  system-reminder noise.
- Resume a selected Claude Code or Codex session by replacing the `hop` process
  with the correct agent command.
- Work with zero configuration while allowing users to override data
  directories, preview behavior, and column layout.

## Non-Goals

- Team-shared indexes or remote session stores.
- Runtime plugin systems for arbitrary agents.
- Non-interactive list, export, or stats modes.
- Full theme and custom keybinding support. Config accepts these fields for
  forward compatibility, but they are reserved.
- Editing, deleting, or mutating source session files.

## Primary Users

- Developers who frequently use Claude Code, Codex, or both.
- Developers who remember fragments of a prior agent conversation but not the
  session ID or exact project directory.
- Developers who want to resume work directly from terminal history without
  opening multiple agent-specific tools.

## Core User Journey

1. The user launches `hop`.
2. The TUI renders immediately from the existing local Tantivy index.
3. A background sync scans available agent data directories and updates the
   index incrementally.
4. The user types remembered words or filters such as `agent:codex`,
   `dir:api`, or `date:week`.
5. Results update after a short debounce and keep the selected row visible.
6. The preview pane shows the selected session transcript with matching query
   terms highlighted.
7. The user presses `Enter`.
8. If the target agent supports yolo resume, `hop` asks whether to resume with
   the dangerous bypass flag.
9. `hop` restores the terminal, changes to the recorded session directory when
   available, and `exec`s the agent resume command.

## Supported Data Sources

### Claude Code

- Default data directory: `~/.claude/projects`.
- Scan shape: top-level `*.jsonl` files under encoded project directories.
- Session ID: JSONL filename stem.
- Directory: first non-empty `cwd` value.
- Branch: first non-empty `gitBranch` value.
- Title: `aiTitle` when present, otherwise `summary`, otherwise first user prose
  message.
- Resume command: `claude --resume <session-id>`.
- Yolo resume command: `claude --dangerously-skip-permissions --resume
  <session-id>`.

### Codex

- Default data directory: `~/.codex`.
- Scan shape: recursive `*.jsonl` files under `sessions/` and
  `archived_sessions/`.
- Session ID: `rollout-<timestamp>-<id>.jsonl` trailing ID, falling back to the
  filename stem.
- Directory: `session_meta.payload.cwd`.
- Branch and repo URL: `session_meta.payload.git.branch` and
  `session_meta.payload.git.repository_url`.
- Title: first user prose message.
- Resume command: `codex resume <session-id>`.
- Yolo resume command: `codex --dangerously-bypass-approvals-and-sandbox resume
  <session-id>`.

## Search Requirements

- Free text searches full indexed transcript content and session titles.
- `agent:<agent>` includes agents. Supported values are `claude` and `codex`.
- `-agent:<agent>`, `!agent:<agent>`, and `agent:claude,!codex` exclude agents.
- `dir:<text>` includes directory substrings.
- `-dir:<text>` excludes directory substrings.
- `repo:<text>` includes sessions whose git remote URL contains the substring;
  `-repo:<text>` excludes them. The remote is stable across worktrees, so this
  matches every worktree of a repo. Sessions outside a git repo have no remote
  and never satisfy a `repo:` include.
- `date:today` and `date:yesterday` use the user's local calendar day.
- `date:week` means sessions in the last 7 days.
- `date:month` means sessions in the last 30 days.
- `date:<Nh|Nd|Nw` means sessions newer than the specified duration.
- `date:>Nh|Nd|Nw` means sessions older than the specified duration.
- Unknown keyword prefixes remain free text rather than failing the query.
- Tab autocomplete should complete unambiguous `agent:` and `date:` values.

## Result List Requirements

- Default columns are `agent`, `repo`, `branch`, `title`, `msgs`, `pr`, and
  `time`.
- The title column is the flexible column and remains visible.
- The agent column remains visible.
- Narrow panes drop lower-priority metadata columns before core identity
  columns.
- Column fitting must use terminal display width rather than byte length or
  Unicode scalar count.
- Repo display prefers recorded repo metadata when available and otherwise uses
  a useful local-directory fallback.
- PR enrichment must not block the UI thread.

## Preview Requirements

- Preview re-parses the selected source JSONL file when available.
- Preview uses the same transcript extraction policy as indexed content.
- Preview hides tool results, command tags, environment context,
  system-reminder blocks, external-agent wrapper blocks, and other internal
  noise that should not be part of the user-facing transcript.
- Preview preserves user and agent turns.
- Preview splits fenced code blocks from prose for readable rendering.
- Preview highlights free-text query matches.
- Preview should fall back to indexed content when the source file is
  unavailable and surface a source-unavailable warning.

## Indexing And Sync Requirements

- Launch opens the existing index and performs the initial search before
  background sync completes.
- Background sync uses adapter scans to detect changed and deleted sessions.
- Changed sessions are parsed and upserted by namespaced document key
  `agent:id`.
- Deleted rows are removed only for adapters that completed an authoritative
  scan.
- Unavailable adapters, scan errors, parse errors, and empty sessions are
  non-fatal and should be reflected in sync status.
- Broad scans and filesystem reads must not happen in the render path.

## Resume Requirements

- Resume must restore the terminal before replacing the process.
- Resume must attempt to change to the recorded session directory before `exec`.
- A missing or invalid directory should not block resume.
- Empty resume commands must be rejected.
- Yolo resume must be explicit through the confirmation modal or `--yolo` CLI
  flag.

## CLI Requirements

- `hop` opens the interactive TUI.
- `hop "<query>"` opens with a pre-filled free-text query.
- `hop -a <agent>` composes an `agent:<agent>` query filter.
- `hop -d <text>` composes a `dir:<text>` query filter.
- `hop -r <text>` composes a `repo:<text>` query filter.
- `hop --rebuild` removes the local index before starting and then rebuilds in
  the normal background sync path.
- `hop --yolo` forces yolo resume when the selected adapter supports it.

## Configuration Requirements

- Missing config file means zero-config defaults.
- `[data_dirs]` can override per-agent source directories by agent slug.
- `[preview] visible`, `width_pct`, and `metadata_header` control initial
  preview behavior.
- Preview visibility and width persist across restarts as UI state.
- `[columns] disabled` hides named columns.
- `[columns] order` moves named columns ahead of unspecified columns while
  preserving default relative order for the rest.
- `theme` and `[keybindings]` are accepted but reserved.

## Interaction Requirements

- Typing edits the live search query.
- Arrow keys move result selection and query cursor according to key context.
- `Tab` autocompletes supported filter values when completion is unambiguous.
- `Enter` resumes the selected session or confirms the yolo modal.
- `Esc` clears a non-empty query and quits when the query is already empty.
- `Ctrl+C` quits from any state.
- `Ctrl+P` toggles preview.
- `Ctrl+U` and `Ctrl+D` scroll preview.
- `Ctrl+N` and `Ctrl+B` jump between preview matches.
- `Ctrl+Left` and `Ctrl+Right` resize preview between 20% and 80%.
- `?` opens help and does not type into the query.

## Quality Requirements

- Parser changes should include fixture or unit coverage for affected agent
  JSONL shapes.
- Search and sync changes should include index-level coverage.
- TUI interaction changes should remain testable with ratatui test backends or
  focused model/keymap tests.
- `cargo test` should pass before handing off behavior changes.

## Open Product Questions

- Should directory filters eventually become path-aware instead of substring
  filters?
- Should `hop` add a non-interactive result mode for scripting, despite the
  current TUI-first scope?
- Should theme and keybinding config move from reserved fields to supported
  user-facing behavior?
- Should additional agents be supported through built-in adapters before a
  general plugin model exists?
