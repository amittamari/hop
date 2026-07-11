# 🐇 hop

Fast full-text search and resume for coding-agent sessions (**Claude Code**, **Codex**, and **Cursor**).

`hop` aggregates your past Claude Code, Codex, and Cursor sessions into a single full-text index, allowing you to jump straight back into any of them. Type a few words you remember, pick the result, hit Enter, and you are immediately resumed in the original agent and the original working directory.

<p align="center">
  <img src="demo/hop.gif" alt="hop demo: search, preview, and resume coding-agent sessions" width="100%">
</p>

---

## ✨ Features

* **🔎 Full-text search across every session** — A Tantivy index over your entire Claude Code, Codex, and Cursor history. Fuzzy and exact matching across full conversation transcripts, not just titles.
* **🤖 Multi-agent, one index** — Claude Code, Codex, and Cursor sessions live side by side, normalized into a single searchable view.
* **⚡ Instant resume** — Pick a session and `hop` restores the terminal, `chdir`s to the original working directory, and `exec`-replaces itself with the right agent CLI. No copy-pasting paths.
* **🧮 Rich, responsive results grid** — Agent, repo, branch, title, message count, PR status, and time — with columns that gracefully drop on narrow terminals.
* **🧹 Clean transcript previews** — On-demand previews strip tool calls, command tags, and system noise, with syntax-highlighted code and highlighted query matches.
* **🏷️ Powerful query keywords** — Filter by `agent:`, `dir:`, `repo:`, and relative/duration `date:` expressions inline with free-text search.
* **🔗 GitHub PR awareness** — Associated PRs are resolved in the background via the `gh` CLI and cached on disk.
* **🚀 Background streaming index** — Existing data renders instantly on launch; new sessions sync in the background without blocking the UI.

---

## 🚀 Quick Start

### Installation

**Homebrew** (macOS & Linux):

```bash
brew install amittamari/tap/hop
```

**From source:**

```bash
cargo install --locked --path .
```

### Dependencies

`hop` works standalone for searching and previewing sessions. Additional tools
unlock more features:

| Dependency | Purpose | Notes |
| --- | --- | --- |
| **git** | Resolve repo names and branches at index time | Effectively always present on developer machines |
| **[gh](https://cli.github.com/)** | PR column and `Ctrl+O` open-in-browser | Without it, the PR column stays empty — everything else works fine |
| **[claude](https://docs.anthropic.com/en/docs/claude-code)** | Resume Claude Code sessions | Only needed if you use Claude Code |
| **[codex](https://github.com/openai/codex)** | Resume Codex sessions | Only needed if you use Codex |
| **[cursor-agent](https://www.cursor.com/)** | Resume Cursor sessions | Only needed if you use Cursor |

### Usage Examples

```bash
hop                      # Open the TUI, auto-scoped to the current repo (if any)
hop "auth refresh"       # Pre-filled query, still scoped to the current repo
hop --all                # Search across all repos (disable auto-scoping)
hop -a claude -d api     # Filter by agent and directory on launch
hop -r hop               # Filter to one repo across all its worktrees
hop --rebuild            # Wipe and rebuild the search index
hop hooks install --all  # Install metadata hooks for detected agents
hop hooks status         # Show metadata-hook installation status
hop hooks uninstall      # Remove installed hop metadata hooks
```

Metadata hooks capture the final working directory and Git state at session
start and stop. Codex hooks are installed as a local Codex plugin; Claude and
Cursor hooks are merged into their existing hook configuration.

---

## 🤖 Supported Agents

`hop` indexes **Claude Code**, **Codex**, and **Cursor** sessions side by side.
What `hop` can show depends on what each agent records on disk, so coverage
varies by column:

| Capability | Claude Code | Codex | Cursor |
| --- | :---: | :---: | :---: |
| Full-text search & clean preview | ✅ | ✅ | ✅ |
| Resume in the original directory | ✅ | ✅ | ✅ |
| Yolo / skip-permissions resume | ✅ | ✅ | ✅ |
| Session title | ✅ | ✅ | ✅ |
| Working directory + `dir:` filter | ✅ | ✅ | ✅ <sup>†</sup> |
| Repo column + `repo:` filter | ✅ | ✅ | ✅ |
| Branch column | ✅ | ✅ | ❌ |
| PR column | ✅ | ✅ | ❌ <sup>‡</sup> |

<sup>†</sup> Cursor doesn't store the working directory in its transcript; `hop`
recovers it from the session's `worker.log`, so it's unavailable when that log is
missing.
<sup>‡</sup> The PR column is keyed off the branch, so it's empty wherever the
branch is unknown.

**Repo** is resolved from the git remote (`git remote get-url origin`) once per
directory at index time, so it's identical across every worktree of a repo.
**Branch** comes straight from agent metadata (Claude's `gitBranch`, Codex's
`git.branch`); Cursor records none, so it's left blank rather than guessed.

### Not yet supported

The following providers aren't wired up yet — contributions are welcome. Each
provider is added through a session adapter, so it's mostly a matter of mapping
its on-disk session format to `hop`'s `core` types.

* **Gemini CLI**
* **Aider**
* **opencode**

---

## ⌨️ Keyboard Shortcuts

The query is always live — just start typing to filter. Navigation lives on the
arrows and secondary actions on `Ctrl` chords, so no key ever does double duty.
The `Ctrl` chords below are rebindable via `[keybindings]` in `config.toml` (see
[Configuration](#️-configuration)).

| Key | Action |
| --- | --- |
| **Typing** | Filters search results |
| `↑` / `↓` | Move selection up / down |
| `PgUp` / `PgDn` | Page viewport up / down |
| `←` / `→` / `Home` / `End` | Move the query cursor |
| `Tab` | Autocomplete keywords (e.g., `agent:cl` → `agent:claude`) |
| `Enter` | **Resume selected session** (prompts for yolo when supported) |
| `Ctrl + O` | Open the selected session's PR in the browser (when one is resolved) |
| `Ctrl + P` | Toggle the preview pane |
| `Ctrl + U` / `D` | Scroll preview pane up / down by viewport |
| `Ctrl + N` / `B` | Go to next / previous preview match |
| `Ctrl + ←` / `→` | Resize the preview pane |
| `?` | Show help menu |
| `Esc` | Clear the query, or quit when it's already empty |
| `Ctrl + C` | Quit |

---

## 🔍 Query Syntax

Advanced filtering keywords can be used alongside regular free-text search.

| Example | Filter Type | Description |
| --- | --- | --- |
| `auth refresh` | **Free-text** | Matches terms (fuzzy + exact) across full conversation history |
| `agent:claude,codex` | **Include Agent** | Restrict results to specific agents |
| `-agent:codex` or `agent:claude,!codex` | **Exclude Agent** | Exclude specific agents from results |
| `dir:api` / `-dir:vendor` | **Directory** | Substring include or exclude on directory paths |
| `repo:hop` / `-repo:vendor` | **Repository** | Substring include or exclude on the git remote URL — matches every worktree of a repo |
| `date:today` / `date:yesterday` | **Relative Date** | Local calendar-day filters |
| `date:week` / `date:month` | **Date Windows** | Broad recency windows |
| `date:<2d` / `date:>1w` | **Duration** | Matches within (`<`) or older than (`>`) durations (`h`/`d`/`w`) |

> **Auto-scope:** When launched from inside a git repo, `hop` prepends a `repo:owner/name`
> filter for you so you see that repo's sessions first. Pass `--all` to search every repo,
> or just edit/delete the `repo:` token in the query bar to broaden mid-session. Supplying
> your own `-r`/`repo:` filter (or running outside a git repo) disables auto-scoping.

---

## 📊 Interface & Columns

Each row in the TUI is organized into a dynamic, aligned grid:


$$\text{AGENT} \quad\cdot\quad \text{REPO} \quad\cdot\quad \text{BRANCH} \quad\cdot\quad \text{TITLE} \quad\cdot\quad \text{MSGS} \quad\cdot\quad \text{PR} \quad\cdot\quad \text{TIME}$$

* **Branch & Repo:** The **Repo** column shows the repository name parsed from the git remote URL, resolved once per directory at index time (`git remote get-url origin`) — so worktrees of the same repo collapse to one consistent name instead of showing distinct folder names. Sessions outside a git repo fall back to the directory's basename. The **Branch** column comes from agent metadata where recorded; full paths are shown in the preview header, and the Branch column distinguishes worktrees at a glance. Per-agent coverage is summarized in the **Supported Agents** table above.
* **Titles:** Uses the recorded AI title/summary if available, otherwise falls back to the first user prompt. Titles are whitespace-normalized.
* **PR Column:** Resolved asynchronously in the background using the `gh` CLI and cached on disk. Shows `⟳` while loading, and `—` if no PR is associated.
* **Responsive Layout:** Narrow terminals automatically drop columns based on priority:

$$\text{PR} \rightarrow \text{MSGS} \rightarrow \text{TIME} \rightarrow \text{BRANCH} \rightarrow \text{REPO}$$



Leftover width is dynamically allocated to the conversation Title.

---

## 🧠 How It Works

* **Streaming Index:** The index is stored in your platform's cache directory (e.g., `~/.cache/hop/`). On launch, existing data renders instantly. New sessions sync seamlessly in the background without blocking the UI.
* **Instant Resume:** When you select a session, `hop` restores the terminal state, changes the directory (`chdir`), and `exec`-replaces the process directly with the respective agent CLI.
* **Clean Previews:** The preview pane re-parses selected sessions on demand. It strips out internal noise (tool calls, `<command-*>` tags, system reminders) and displays a clean transcript with syntax-highlighted code and highlighted query matches.

---

## ⚙️ Configuration

An optional configuration file can be created in your platform's config directory (e.g., `~/.config/hop/config.toml`).

```toml
[preview]
visible = true
width_pct = 50
metadata_header = true

[columns]
disabled = []   # e.g., ["pr"] to disable background GitHub PR resolution
order = []      # e.g., ["agent", "title", "time"]. Unspecified columns follow naturally.

[keybindings]
# Rebind any Ctrl-chord action. Values must include `ctrl` (the chord-only
# invariant keeps chords from colliding with query editing). Unset commands keep
# their default. Invalid values, unknown command names, and conflicts are logged
# to stderr at launch and fall back to the default rather than failing.
toggle_preview        = "ctrl+p"   # default
scroll_preview_up     = "ctrl+u"
scroll_preview_down   = "ctrl+d"
jump_match_prev       = "ctrl+b"
jump_match_next       = "ctrl+n"
resize_preview_smaller = "ctrl+left"
resize_preview_larger  = "ctrl+right"
open_pr                = "ctrl+o"
quit                  = "ctrl+c"

```

Binding values accept letters (`ctrl+t`), digits, and named keys
(`ctrl+left`, `ctrl+right`, `ctrl+up`, `ctrl+down`, `ctrl+home`, `ctrl+end`,
`ctrl+pageup`, `ctrl+pagedown`, `ctrl+space`). `Ctrl + C` always quits regardless
of the `quit` binding, as an emergency exit. The help overlay (`?`) reflects your
active bindings.

> ⚙️ **Note:** The `theme` table is accepted for forward-compatibility but is
> currently reserved and not applied. Preview width and visibility choices persist
> automatically across restarts.

---

## 📄 Documentation Index

For deeper technical context, explore the following documentation files:

* **`AGENTS.md`** — Overview of contributor and agent context.
* **`docs/PROJECT.md`** — Core project goals and scope.
* **`docs/ARCHITECTURE.md`** — System design and internals.
* **`docs/specs/`, `docs/reviews/`, `docs/plans/`** — Dated specifications, review artifacts, and execution roadmaps.

---

## 🙏 Credits

Inspired by [angristan/fast-resume](https://github.com/angristan/fast-resume),
which also served as a reference for the Claude Code and Codex session adapters.
