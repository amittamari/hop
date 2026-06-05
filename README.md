# 🐇 hop

Fast full-text search and resume for coding-agent sessions (**Claude Code** + **Codex**).

`hop` aggregates your past Claude Code and Codex sessions into a single full-text index, allowing you to jump straight back into any of them. Type a few words you remember, pick the result, hit Enter, and you are immediately resumed in the original agent and the original working directory.

---

## 🚀 Quick Start

### Installation

Build and install from the local source directory:

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

### Default (Search-First) Mode

| Key | Action |
| --- | --- |
| **Typing** | Filters search results |
| `↑` / `↓` | Move selection up / down |
| `PgUp` / `PgDn` | Page viewport up / down |
| `←` / `→` / `Home` / `End` | Edit current search query |
| `Tab` | Autocomplete keywords (e.g., `agent:cl` → `agent:claude`) |
| `Enter` | **Resume selected session** |
| `Ctrl + Y` | Run "yolo prompt" |
| `Ctrl + P` | Toggle the preview pane |
| `Ctrl + U` / `D` | Scroll preview pane up / down by viewport |
| `Ctrl + N` / `B` | Go to next / previous preview match |
| `[` / `]` | Resize preview pane *(only when query is empty)* |
| `?` | Show help menu *(only when query is empty)* |
| `Esc` | Quit |

> 💡 **Modal (Vim-style) Mode:** If `keymap = "modal"` is enabled, `Esc` switches from **SEARCH** to **NAVIGATE** mode, `/` returns to search, and `Ctrl + C` quits globally.

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
# Default is "search". Set to "modal" for vim-style navigation.
keymap = "search"

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
* **`docs/specs/`, `docs/reviews/`, `docs/plans/**` — Dated specifications, review artifacts, and execution roadmaps.
