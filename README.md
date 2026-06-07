# 🐇 hop

Fast full-text search and resume for coding-agent sessions (**Claude Code** + **Codex**).

`hop` aggregates your past Claude Code and Codex sessions into a single full-text index, allowing you to jump straight back into any of them. Type a few words you remember, pick the result, hit Enter, and you are immediately resumed in the original agent and the original working directory.

---

## ✨ Features

* **🔎 Full-text search across every session** — A Tantivy index over your entire Claude Code and Codex history. Fuzzy and exact matching across full conversation transcripts, not just titles.
* **🤖 Multi-agent, one index** — Claude Code and Codex sessions live side by side, normalized into a single searchable view.
* **⚡ Instant resume** — Pick a session and `hop` restores the terminal, `chdir`s to the original working directory, and `exec`-replaces itself with the right agent CLI. No copy-pasting paths.
* **🧮 Rich, responsive results grid** — Agent, repo, branch, title, message count, PR status, and time — with columns that gracefully drop on narrow terminals.
* **🧹 Clean transcript previews** — On-demand previews strip tool calls, command tags, and system noise, with syntax-highlighted code and highlighted query matches.
* **🏷️ Powerful query keywords** — Filter by `agent:`, `dir:`, and relative/duration `date:` expressions inline with free-text search.
* **🔗 GitHub PR awareness** — Associated PRs are resolved in the background via the `gh` CLI and cached on disk.
* **🚀 Background streaming index** — Existing data renders instantly on launch; new sessions sync in the background without blocking the UI.

---

## 🚧 Not Yet Supported

`hop` currently indexes **Claude Code** and **Codex** sessions. The following
providers are not yet supported — contributions are welcome:

* **Gemini CLI**
* **Cursor**
* **Aider**
* **opencode**

Each provider is wired in through a session adapter, so adding one is mostly a
matter of mapping its on-disk session format to `hop`'s `core` types.

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

### Usage Examples

```bash
hop                      # Open the interactive TUI
hop "auth refresh"       # Open the TUI with a pre-filled search query
hop -a claude -d api     # Filter by agent and directory on launch
hop --rebuild            # Wipe and rebuild the search index

```

---

## ⌨️ Keyboard Shortcuts

The query is always live — just start typing to filter. Navigation lives on the
arrows and secondary actions on `Ctrl` chords, so no key ever does double duty.

| Key | Action |
| --- | --- |
| **Typing** | Filters search results |
| `↑` / `↓` | Move selection up / down |
| `PgUp` / `PgDn` | Page viewport up / down |
| `←` / `→` / `Home` / `End` | Move the query cursor |
| `Tab` | Autocomplete keywords (e.g., `agent:cl` → `agent:claude`) |
| `Enter` | **Resume selected session** (prompts for yolo when supported) |
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
| `date:today` / `date:yesterday` | **Relative Date** | Local calendar-day filters |
| `date:week` / `date:month` | **Date Windows** | Broad recency windows |
| `date:<2d` / `date:>1w` | **Duration** | Matches within (`<`) or older than (`>`) durations (`h`/`d`/`w`) |

---

## 📊 Interface & Columns

Each row in the TUI is organized into a dynamic, aligned grid:


$$\text{AGENT} \quad\cdot\quad \text{REPO} \quad\cdot\quad \text{BRANCH} \quad\cdot\quad \text{TITLE} \quad\cdot\quad \text{MSGS} \quad\cdot\quad \text{PR} \quad\cdot\quad \text{TIME}$$

* **Branch & Repo:** Extracted from conversation metadata (Claude’s `gitBranch` / Codex’s `git.branch`). The Repo column prefers Codex's `repository_url`, falling back to the directory's basename. Full paths are displayed in the preview header.
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

```

> ⚙️ **Note:** `theme` and `[keybindings]` tables are accepted for forward-compatibility but are currently reserved and not applied. Preview width and visibility choices persist automatically across restarts.

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
