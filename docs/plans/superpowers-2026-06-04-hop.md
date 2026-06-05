# hop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `hop`, a native-Rust terminal tool that indexes Claude Code + Codex session history into a full-text Tantivy index and lets you fuzzy-search and `exec`-resume into any past session.

**Architecture:** Layered, UI-agnostic core. Pure domain types (`core`) and query parsing (`query`) at the bottom; per-agent file `adapters` behind a trait; a Tantivy-backed `index` with incremental sync; an `engine` that orchestrates an instant-open + background-sync lifecycle over channels; a `ratatui` `tui` that only renders the viewport; and a `resume` handoff that restores the terminal, `chdir`s, and `exec`-replaces the process with the agent CLI.

**Tech Stack:** Rust 1.91, `tantivy` 0.26, `ratatui` 0.30 + `crossterm` 0.29 (via `ratatui::crossterm`), `clap` 4.6 (derive), `simd-json` 0.17 + `serde`, `jiff` 0.2, `directories` 6, `toml` 0.8, `anyhow` + `thiserror`.

**Verified facts this plan relies on (from on-machine research):**
- **Claude Code:** `~/.claude/projects/<encoded-cwd>/<session-uuid>.jsonl` (skip `*/subagents/*`). Each line is a JSON object with top-level `type`. `cwd` field per line. User text = `type:"user"` with `message.content` either a string (drop if it starts with `<command-name>`/`<command-message>`/`<command-args>`/`<local-command-stdout>`/`<local-command-caveat>`) or an array (keep `type:"text"` blocks). Assistant text = `type:"assistant"`, array, keep `type:"text"` blocks only. Drop lines with `isMeta:true` or a top-level `toolUseResult`. Title = first real user prompt. `timestamp` is RFC3339 `...Z`.
- **Codex:** `~/.codex/sessions/<YYYY>/<MM>/<DD>/rollout-*-<uuid>.jsonl` plus flat `~/.codex/archived_sessions/rollout-*-<uuid>.jsonl`. Lines are `{type,timestamp,payload}`. `session_meta` (first line) has `payload.id` + `payload.cwd`. Clean user text = `event_msg` with `payload.type:"user_message"` → `payload.message` (string). Clean assistant text = `event_msg` `payload.type:"agent_message"` → `payload.message`. Title = first `user_message`. YOLO ⟺ any `turn_context` with `payload.approval_policy=="never"` AND `payload.sandbox_policy.type=="danger-full-access"`.
- **Tantivy 0.26:** `TantivyDocument` (struct) vs `Document` (trait); `searcher.doc::<TantivyDocument>(addr)`; `use tantivy::schema::Value` for `.as_str()/.as_u64()`; `commit(&mut self)`; `FuzzyTermQuery::new_prefix(term, distance:u8, transposition_cost_one:bool)`.
- **ratatui 0.30:** `ratatui::init()/restore()`; `Frame::area()` (not `size()`); test via `terminal.backend().assert_buffer(&expected)`; depend on crossterm through `ratatui::crossterm`.

---

## File Structure

```
hop/
├── Cargo.toml                  # bin crate "hop", all deps
├── src/
│   ├── main.rs                 # parse CLI, run engine+tui loop, perform resume exec
│   ├── lib.rs                  # module decls + re-exports (so tests/ can use hop::*)
│   ├── cli.rs                  # clap Parser
│   ├── core.rs                 # AgentId, Session, ScanEntry, SessionId, helpers
│   ├── query.rs                # ParsedQuery, parse(), autocomplete(), DateFilter
│   ├── adapters/
│   │   ├── mod.rs              # Adapter trait, registry (default_adapters)
│   │   ├── claude.rs           # ClaudeAdapter
│   │   └── codex.rs            # CodexAdapter
│   ├── index.rs                # SearchIndex: schema, sync diff, query builder
│   ├── engine.rs               # Engine: state, debounce, background sync, channel
│   ├── resume.rs               # exec handoff (chdir + exec)
│   ├── config.rs               # Config (TOML) + defaults
│   └── tui/
│       ├── mod.rs              # App, Mode, Action, run loop, handle_key
│       ├── theme.rs            # colors / agent badges
│       └── view.rs             # render(): input, list, preview, footer, modal
└── tests/
    ├── fixtures/
    │   ├── claude/<uuid>.jsonl
    │   └── codex/rollout-...-<uuid>.jsonl
    ├── claude_adapter.rs
    ├── codex_adapter.rs
    └── index_sync.rs
```

`lib.rs` exists so integration tests in `tests/` can `use hop::...`. `main.rs` stays thin and calls into the library.

---

## Task 1: Project scaffolding

**Files:**
- Create: `Cargo.toml`
- Create: `src/lib.rs`
- Create: `src/main.rs`

- [ ] **Step 1: Write `Cargo.toml`**

```toml
[package]
name = "hop"
version = "0.1.0"
edition = "2021"
description = "Fast full-text search and resume for coding-agent sessions"
license = "MIT"

[[bin]]
name = "hop"
path = "src/main.rs"

[lib]
name = "hop"
path = "src/lib.rs"

[dependencies]
anyhow = "1"
thiserror = "2"
clap = { version = "4.6", features = ["derive"] }
tantivy = "0.26"
ratatui = "0.30"
serde = { version = "1", features = ["derive"] }
simd-json = "0.17"
jiff = "0.2"
directories = "6"
toml = "0.8"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Write `src/lib.rs`**

```rust
pub mod adapters;
pub mod cli;
pub mod config;
pub mod core;
pub mod engine;
pub mod index;
pub mod query;
pub mod resume;
pub mod tui;
```

- [ ] **Step 3: Write a placeholder `src/main.rs` so the crate compiles**

```rust
fn main() -> anyhow::Result<()> {
    println!("hop");
    Ok(())
}
```

- [ ] **Step 4: Create empty module files so `lib.rs` compiles**

Create each of these with the single line `// placeholder` (they get filled in later tasks):
`src/cli.rs`, `src/config.rs`, `src/core.rs`, `src/engine.rs`, `src/index.rs`, `src/query.rs`, `src/resume.rs`.
Create `src/adapters/mod.rs` with:
```rust
pub mod claude;
pub mod codex;
```
Create `src/adapters/claude.rs`, `src/adapters/codex.rs` with `// placeholder`.
Create `src/tui/mod.rs` with:
```rust
pub mod theme;
pub mod view;
```
Create `src/tui/theme.rs`, `src/tui/view.rs` with `// placeholder`.

- [ ] **Step 5: Verify it builds**

Run: `cargo build`
Expected: compiles (warnings about unused are fine). This downloads/compiles the whole dependency tree — may take a few minutes the first time.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/
git commit -m "chore: scaffold hop crate and module skeleton"
```

---

## Task 2: `core` domain types

**Files:**
- Modify: `src/core.rs` (replace placeholder)

- [ ] **Step 1: Write failing tests at the bottom of `src/core.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_slug_roundtrip() {
        assert_eq!(AgentId::Claude.slug(), "claude");
        assert_eq!(AgentId::Codex.slug(), "codex");
        assert_eq!(AgentId::from_slug("codex"), Some(AgentId::Codex));
        assert_eq!(AgentId::from_slug("nope"), None);
    }

    #[test]
    fn agent_badge() {
        assert_eq!(AgentId::Claude.badge(), "CLAUDE");
        assert_eq!(AgentId::Codex.badge(), "CODEX");
    }

    #[test]
    fn truncate_title_shortens_with_ellipsis() {
        assert_eq!(truncate_title("hello", 10), "hello");
        assert_eq!(truncate_title("hello world", 5), "hell…");
        // collapses internal whitespace/newlines
        assert_eq!(truncate_title("a\n  b\tc", 100), "a b c");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib core`
Expected: FAIL — `AgentId`, `truncate_title` not found.

- [ ] **Step 3: Implement the types above the tests**

```rust
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentId {
    Claude,
    Codex,
}

impl AgentId {
    pub const ALL: [AgentId; 2] = [AgentId::Claude, AgentId::Codex];

    pub fn slug(self) -> &'static str {
        match self {
            AgentId::Claude => "claude",
            AgentId::Codex => "codex",
        }
    }

    pub fn badge(self) -> &'static str {
        match self {
            AgentId::Claude => "CLAUDE",
            AgentId::Codex => "CODEX",
        }
    }

    pub fn from_slug(s: &str) -> Option<AgentId> {
        match s {
            "claude" => Some(AgentId::Claude),
            "codex" => Some(AgentId::Codex),
            _ => None,
        }
    }
}

pub type SessionId = String;

/// Cheap stat-level scan result for one session file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanEntry {
    pub path: PathBuf,
    /// File modification time, unix milliseconds.
    pub mtime: i64,
}

/// A fully parsed session, ready to index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: SessionId,
    pub agent: AgentId,
    pub title: String,
    pub directory: String,
    /// Session timestamp (first message), unix seconds.
    pub timestamp: i64,
    /// Indexed conversation content (user + assistant text only).
    pub content: String,
    pub message_count: u32,
    /// Source file mtime, unix milliseconds (drives incremental sync).
    pub mtime: i64,
    pub yolo: bool,
}

/// Collapse whitespace runs to single spaces and truncate to `max` chars
/// (counting the ellipsis), trimming ends.
pub fn truncate_title(raw: &str, max: usize) -> String {
    let collapsed: String = {
        let mut out = String::new();
        let mut prev_space = false;
        for c in raw.trim().chars() {
            if c.is_whitespace() {
                if !prev_space {
                    out.push(' ');
                }
                prev_space = true;
            } else {
                out.push(c);
                prev_space = false;
            }
        }
        out
    };
    let count = collapsed.chars().count();
    if count <= max {
        return collapsed;
    }
    let keep = max.saturating_sub(1);
    let mut s: String = collapsed.chars().take(keep).collect();
    s.push('…');
    s
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib core`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/core.rs
git commit -m "feat(core): add AgentId, Session, ScanEntry, title helper"
```

---

## Task 3: `query` parser + autocomplete

**Files:**
- Modify: `src/query.rs` (replace placeholder)

- [ ] **Step 1: Write failing tests at the bottom of `src/query.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::AgentId;

    #[test]
    fn plain_free_text() {
        let q = parse("auth refresh token");
        assert_eq!(q.free_text, "auth refresh token");
        assert!(q.agents.include.is_empty() && q.agents.exclude.is_empty());
        assert!(q.dirs.include.is_empty() && q.dirs.exclude.is_empty());
        assert!(q.date.is_none());
    }

    #[test]
    fn agent_multi_value_and_negation() {
        let q = parse("agent:claude,!codex login");
        assert_eq!(q.agents.include, vec![AgentId::Claude]);
        assert_eq!(q.agents.exclude, vec![AgentId::Codex]);
        assert_eq!(q.free_text, "login");
    }

    #[test]
    fn agent_token_negation_prefix() {
        let q = parse("-agent:codex");
        assert_eq!(q.agents.exclude, vec![AgentId::Codex]);
        assert!(q.agents.include.is_empty());
    }

    #[test]
    fn dir_filters_include_and_exclude() {
        let q = parse("dir:api -dir:vendor bug");
        assert_eq!(q.dirs.include, vec!["api".to_string()]);
        assert_eq!(q.dirs.exclude, vec!["vendor".to_string()]);
        assert_eq!(q.free_text, "bug");
    }

    #[test]
    fn date_keywords_and_comparisons() {
        assert_eq!(parse("date:today").date, Some(DateFilter::Today));
        assert_eq!(parse("date:yesterday").date, Some(DateFilter::Yesterday));
        assert_eq!(parse("date:week").date, Some(DateFilter::LastWeek));
        assert_eq!(parse("date:month").date, Some(DateFilter::LastMonth));
        assert_eq!(parse("date:<1h").date, Some(DateFilter::Within(3600)));
        assert_eq!(parse("date:<2d").date, Some(DateFilter::Within(2 * 86400)));
        assert_eq!(parse("date:>1w").date, Some(DateFilter::OlderThan(7 * 86400)));
    }

    #[test]
    fn date_range_windows() {
        let now = 1_000_000i64;
        assert_eq!(DateFilter::Today.range(now), (Some(now - 86400), Some(now)));
        assert_eq!(
            DateFilter::Yesterday.range(now),
            (Some(now - 2 * 86400), Some(now - 86400))
        );
        assert_eq!(DateFilter::Within(3600).range(now), (Some(now - 3600), Some(now)));
        assert_eq!(DateFilter::OlderThan(3600).range(now), (None, Some(now - 3600)));
    }

    #[test]
    fn autocomplete_agent_value() {
        assert_eq!(autocomplete("agent:cl").as_deref(), Some("agent:claude"));
        assert_eq!(autocomplete("bug agent:co").as_deref(), Some("bug agent:codex"));
        // already complete -> no suggestion
        assert_eq!(autocomplete("agent:claude"), None);
        // free text -> no suggestion
        assert_eq!(autocomplete("auth"), None);
    }

    #[test]
    fn autocomplete_date_value() {
        assert_eq!(autocomplete("date:to").as_deref(), Some("date:today"));
        assert_eq!(autocomplete("date:y").as_deref(), Some("date:yesterday"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib query`
Expected: FAIL — `parse`, `DateFilter`, `autocomplete` not found.

- [ ] **Step 3: Implement the parser above the tests**

```rust
use crate::core::AgentId;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AgentFilter {
    pub include: Vec<AgentId>,
    pub exclude: Vec<AgentId>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DirFilter {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateFilter {
    Today,
    Yesterday,
    LastWeek,
    LastMonth,
    /// Newer than `now - secs`.
    Within(i64),
    /// Older than `now - secs`.
    OlderThan(i64),
}

impl DateFilter {
    /// Inclusive (min, max) timestamp bounds in unix seconds; `None` = unbounded.
    pub fn range(self, now: i64) -> (Option<i64>, Option<i64>) {
        const D: i64 = 86_400;
        match self {
            DateFilter::Today => (Some(now - D), Some(now)),
            DateFilter::Yesterday => (Some(now - 2 * D), Some(now - D)),
            DateFilter::LastWeek => (Some(now - 7 * D), Some(now)),
            DateFilter::LastMonth => (Some(now - 30 * D), Some(now)),
            DateFilter::Within(s) => (Some(now - s), Some(now)),
            DateFilter::OlderThan(s) => (None, Some(now - s)),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ParsedQuery {
    pub free_text: String,
    pub agents: AgentFilter,
    pub dirs: DirFilter,
    pub date: Option<DateFilter>,
}

pub fn parse(input: &str) -> ParsedQuery {
    let mut q = ParsedQuery::default();
    let mut free: Vec<&str> = Vec::new();

    for tok in input.split_whitespace() {
        // A leading '-' or '!' negates the entire keyword token.
        let (negated, body) = match tok.strip_prefix(['-', '!']) {
            Some(rest) if rest.contains(':') => (true, rest),
            _ => (false, tok),
        };

        if let Some((key, val)) = body.split_once(':') {
            match key {
                "agent" => parse_agent(val, negated, &mut q.agents),
                "dir" => {
                    if negated {
                        q.dirs.exclude.push(val.to_string());
                    } else {
                        q.dirs.include.push(val.to_string());
                    }
                }
                "date" => {
                    if let Some(df) = parse_date(val) {
                        q.date = Some(df);
                    }
                }
                _ => free.push(tok),
            }
        } else {
            free.push(tok);
        }
    }

    q.free_text = free.join(" ");
    q
}

fn parse_agent(val: &str, token_negated: bool, out: &mut AgentFilter) {
    for part in val.split(',') {
        let (neg, name) = match part.strip_prefix('!') {
            Some(rest) => (true, rest),
            None => (false, part),
        };
        let neg = neg ^ token_negated;
        if let Some(agent) = AgentId::from_slug(name) {
            if neg {
                out.exclude.push(agent);
            } else {
                out.include.push(agent);
            }
        }
    }
}

fn parse_date(val: &str) -> Option<DateFilter> {
    match val {
        "today" => return Some(DateFilter::Today),
        "yesterday" => return Some(DateFilter::Yesterday),
        "week" => return Some(DateFilter::LastWeek),
        "month" => return Some(DateFilter::LastMonth),
        _ => {}
    }
    let (older, rest) = match val.strip_prefix('>') {
        Some(r) => (true, r),
        None => (false, val.strip_prefix('<').unwrap_or(val)),
    };
    let secs = parse_duration(rest)?;
    Some(if older {
        DateFilter::OlderThan(secs)
    } else {
        DateFilter::Within(secs)
    })
}

fn parse_duration(s: &str) -> Option<i64> {
    let (num, unit) = s.split_at(s.find(|c: char| !c.is_ascii_digit())?);
    let n: i64 = num.parse().ok()?;
    let mult = match unit {
        "h" => 3_600,
        "d" => 86_400,
        "w" => 604_800,
        _ => return None,
    };
    Some(n * mult)
}

/// Tab autocomplete for the last whitespace-delimited token.
/// Returns the full completed input string, or `None` if nothing to complete.
pub fn autocomplete(input: &str) -> Option<String> {
    let last = input.split_whitespace().last()?;
    let prefix_len = input.len() - last.len();
    let prefix = &input[..prefix_len];

    let completion = if let Some(partial) = last.strip_prefix("agent:") {
        complete_value(partial, &["claude", "codex"]).map(|v| format!("agent:{v}"))
    } else if let Some(partial) = last.strip_prefix("date:") {
        complete_value(partial, &["today", "yesterday", "week", "month"])
            .map(|v| format!("date:{v}"))
    } else {
        None
    }?;

    Some(format!("{prefix}{completion}"))
}

fn complete_value(partial: &str, candidates: &[&str]) -> Option<String> {
    if partial.is_empty() {
        return None;
    }
    let matches: Vec<&&str> = candidates.iter().filter(|c| c.starts_with(partial)).collect();
    // Only complete when unambiguous and not already complete.
    match matches.as_slice() {
        [only] if **only != partial => Some((**only).to_string()),
        _ => None,
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib query`
Expected: PASS (8 tests).

- [ ] **Step 5: Commit**

```bash
git add src/query.rs
git commit -m "feat(query): keyword/free-text parser + date filters + autocomplete"
```

---

## Task 4: `adapters` trait + registry

**Files:**
- Modify: `src/adapters/mod.rs`

- [ ] **Step 1: Write the trait and registry (no test yet — exercised by Tasks 5 & 6)**

Replace `src/adapters/mod.rs` with:

```rust
pub mod claude;
pub mod codex;

use crate::core::{AgentId, ScanEntry, Session, SessionId};
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

pub trait Adapter: Send + Sync {
    fn id(&self) -> AgentId;
    /// True if this agent's data directory exists.
    fn is_available(&self) -> bool;
    /// Cheap stat-level scan: session id -> (path, mtime). No file bodies read.
    fn scan(&self) -> Result<HashMap<SessionId, ScanEntry>>;
    /// Full parse of one session file.
    fn parse(&self, path: &Path) -> Result<Session>;
    /// argv for resuming this session (program + args).
    fn resume_command(&self, s: &Session, yolo: bool) -> Vec<String>;
    fn supports_yolo(&self) -> bool;
}

/// Default v1 adapters, honoring config data-dir overrides.
pub fn default_adapters(cfg: &crate::config::Config) -> Vec<Box<dyn Adapter>> {
    vec![
        Box::new(claude::ClaudeAdapter::new(cfg.data_dir(AgentId::Claude))),
        Box::new(codex::CodexAdapter::new(cfg.data_dir(AgentId::Codex))),
    ]
}
```

- [ ] **Step 2: Verify it does not yet compile (depends on config + adapters)**

Run: `cargo build 2>&1 | head -20`
Expected: errors referencing `config::Config::data_dir`, `ClaudeAdapter`, `CodexAdapter`. This is expected — Tasks 5, 6, and 8 resolve them. Do **not** commit yet; proceed to Task 5.

---

## Task 5: `config` (needed by adapters registry)

**Files:**
- Modify: `src/config.rs`

> Done before the adapter impls because the registry references `Config::data_dir`.

- [ ] **Step 1: Write failing tests at the bottom of `src/config.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::AgentId;

    #[test]
    fn defaults_when_no_file() {
        let cfg = Config::default();
        // Claude default ends in .claude/projects, Codex default ends in .codex
        assert!(cfg.data_dir(AgentId::Claude).ends_with("projects"));
        assert!(cfg.data_dir(AgentId::Codex).to_string_lossy().contains(".codex"));
    }

    #[test]
    fn data_dir_override_from_toml() {
        let toml = r#"
            [data_dirs]
            claude = "/custom/claude"
        "#;
        let cfg = Config::from_toml_str(toml).unwrap();
        assert_eq!(cfg.data_dir(AgentId::Claude), std::path::PathBuf::from("/custom/claude"));
        // unset agent falls back to default
        assert!(cfg.data_dir(AgentId::Codex).to_string_lossy().contains(".codex"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib config`
Expected: FAIL — `Config` not found.

- [ ] **Step 3: Implement config**

```rust
use crate::core::AgentId;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub data_dirs: HashMap<String, PathBuf>,
    // theme/keybindings reserved for later tasks; parsed leniently.
    #[serde(default)]
    pub theme: HashMap<String, String>,
    #[serde(default)]
    pub keybindings: HashMap<String, String>,
}

impl Config {
    /// Load from the platform config dir; missing file => defaults.
    pub fn load() -> Result<Config> {
        let Some(dirs) = directories::ProjectDirs::from("dev", "hop", "hop") else {
            return Ok(Config::default());
        };
        let path = dirs.config_dir().join("config.toml");
        if !path.exists() {
            return Ok(Config::default());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        Config::from_toml_str(&text)
    }

    pub fn from_toml_str(s: &str) -> Result<Config> {
        toml::from_str(s).context("parsing config.toml")
    }

    /// Resolved data directory for an agent (config override or default).
    pub fn data_dir(&self, agent: AgentId) -> PathBuf {
        if let Some(p) = self.data_dirs.get(agent.slug()) {
            return p.clone();
        }
        let home = directories::BaseDirs::new()
            .map(|b| b.home_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        match agent {
            AgentId::Claude => home.join(".claude").join("projects"),
            AgentId::Codex => home.join(".codex"),
        }
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib config`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/config.rs src/adapters/mod.rs
git commit -m "feat(config): TOML config with data-dir overrides; add Adapter trait"
```

---

## Task 6: `ClaudeAdapter`

**Files:**
- Modify: `src/adapters/claude.rs`
- Create: `tests/fixtures/claude/sample.jsonl`
- Create: `tests/claude_adapter.rs`

- [ ] **Step 1: Create the fixture `tests/fixtures/claude/sample.jsonl`**

One JSON object per line (exact content matters for assertions):

```
{"type":"user","sessionId":"sample","cwd":"/Users/me/work/api","timestamp":"2026-06-04T13:20:16.361Z","isMeta":null,"message":{"role":"user","content":"fix the auth refresh token bug"}}
{"type":"system","subtype":"local_command","level":"info","content":"<local-command-stdout>noise</local-command-stdout>"}
{"type":"user","sessionId":"sample","cwd":"/Users/me/work/api","timestamp":"2026-06-04T13:20:17.000Z","message":{"role":"user","content":"<command-name>/clear</command-name>"}}
{"type":"assistant","sessionId":"sample","timestamp":"2026-06-04T13:20:46.621Z","message":{"role":"assistant","content":[{"type":"text","text":"The refresh token was dropped on retry."},{"type":"tool_use","name":"Bash","id":"toolu_x"}]}}
{"type":"user","sessionId":"sample","toolUseResult":"ok","timestamp":"2026-06-04T13:20:50.000Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_x","content":"done"}]}}
{"type":"user","sessionId":"sample","isMeta":true,"timestamp":"2026-06-04T13:20:51.000Z","message":{"role":"user","content":"meta note"}}
```

- [ ] **Step 2: Write `tests/claude_adapter.rs`**

```rust
use hop::adapters::claude::ClaudeAdapter;
use hop::adapters::Adapter;
use hop::core::AgentId;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/claude")
        .join(name)
}

#[test]
fn parses_id_cwd_and_excludes_noise() {
    let adapter = ClaudeAdapter::new(PathBuf::from("/unused"));
    let s = adapter.parse(&fixture("sample.jsonl")).unwrap();

    assert_eq!(s.agent, AgentId::Claude);
    assert_eq!(s.id, "sample"); // from filename
    assert_eq!(s.directory, "/Users/me/work/api");

    // content keeps only real user + assistant text
    assert!(s.content.contains("fix the auth refresh token bug"));
    assert!(s.content.contains("The refresh token was dropped on retry."));
    // excluded: local-command output, slash-command, tool_result, tool_use, isMeta
    assert!(!s.content.contains("noise"));
    assert!(!s.content.contains("/clear"));
    assert!(!s.content.contains("done"));
    assert!(!s.content.contains("toolu_x"));
    assert!(!s.content.contains("meta note"));

    // title = first real user prompt; message_count = real text messages
    assert_eq!(s.title, "fix the auth refresh token bug");
    assert_eq!(s.message_count, 2);
    assert!(!s.yolo);
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test --test claude_adapter`
Expected: FAIL — `ClaudeAdapter` not found / does not compile.

- [ ] **Step 4: Implement `src/adapters/claude.rs`**

```rust
use crate::adapters::Adapter;
use crate::core::{truncate_title, AgentId, ScanEntry, Session, SessionId};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const TITLE_MAX: usize = 80;

pub struct ClaudeAdapter {
    /// Root holding `<encoded-cwd>/<session-uuid>.jsonl` (default ~/.claude/projects).
    root: PathBuf,
}

impl ClaudeAdapter {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

#[derive(Deserialize)]
struct Line {
    #[serde(rename = "type")]
    kind: Option<String>,
    cwd: Option<String>,
    timestamp: Option<String>,
    #[serde(rename = "isMeta")]
    is_meta: Option<bool>,
    #[serde(rename = "toolUseResult")]
    tool_use_result: Option<serde_json::Value>,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    content: Option<Content>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Content {
    Text(String),
    Blocks(Vec<Block>),
}

#[derive(Deserialize)]
struct Block {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

const COMMAND_PREFIXES: [&str; 5] = [
    "<command-name>",
    "<command-message>",
    "<command-args>",
    "<local-command-stdout>",
    "<local-command-caveat>",
];

impl Adapter for ClaudeAdapter {
    fn id(&self) -> AgentId {
        AgentId::Claude
    }

    fn is_available(&self) -> bool {
        self.root.is_dir()
    }

    fn scan(&self) -> Result<HashMap<SessionId, ScanEntry>> {
        let mut out = HashMap::new();
        if !self.root.is_dir() {
            return Ok(out);
        }
        for project in std::fs::read_dir(&self.root)?.flatten() {
            let pdir = project.path();
            if !pdir.is_dir() {
                continue;
            }
            for entry in std::fs::read_dir(&pdir)?.flatten() {
                let path = entry.path();
                // top-level *.jsonl only (skip <uuid>/subagents/*)
                if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                let mtime = file_mtime_ms(&entry)?;
                out.insert(id.to_string(), ScanEntry { path, mtime });
            }
        }
        Ok(out)
    }

    fn parse(&self, path: &Path) -> Result<Session> {
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .context("session file has no stem")?
            .to_string();

        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;

        let mut directory = String::new();
        let mut title: Option<String> = None;
        let mut first_ts: Option<i64> = None;
        let mut content = String::new();
        let mut message_count: u32 = 0;

        for line in raw.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let mut buf = line.as_bytes().to_vec();
            let parsed: Line = match simd_json::serde::from_slice(&mut buf) {
                Ok(l) => l,
                Err(_) => continue, // skip malformed line, non-fatal
            };

            if directory.is_empty() {
                if let Some(cwd) = &parsed.cwd {
                    directory = cwd.clone();
                }
            }

            let kind = parsed.kind.as_deref().unwrap_or("");
            let is_user = kind == "user";
            let is_assistant = kind == "assistant";
            if !is_user && !is_assistant {
                continue;
            }
            if parsed.is_meta == Some(true) || parsed.tool_use_result.is_some() {
                continue;
            }

            let text = parsed
                .message
                .as_ref()
                .and_then(|m| m.content.as_ref())
                .and_then(|c| extract_text(c, is_user));
            let Some(text) = text else { continue };
            if text.trim().is_empty() {
                continue;
            }

            if first_ts.is_none() {
                first_ts = parsed.timestamp.as_deref().and_then(parse_ts_secs);
            }
            if title.is_none() && is_user {
                title = Some(truncate_title(&text, TITLE_MAX));
            }
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str(text.trim());
            message_count += 1;
        }

        Ok(Session {
            id,
            agent: AgentId::Claude,
            title: title.unwrap_or_else(|| "(untitled)".to_string()),
            directory,
            timestamp: first_ts.unwrap_or(0),
            content,
            message_count,
            mtime: 0, // filled by engine from ScanEntry
            yolo: false,
        })
    }

    fn resume_command(&self, s: &Session, yolo: bool) -> Vec<String> {
        if yolo {
            vec![
                "claude".into(),
                "--dangerously-skip-permissions".into(),
                "--resume".into(),
                s.id.clone(),
            ]
        } else {
            vec!["claude".into(), "--resume".into(), s.id.clone()]
        }
    }

    fn supports_yolo(&self) -> bool {
        true
    }
}

/// For user lines, a string is real text unless it starts with a command tag.
/// For block arrays (either role), keep only `text` blocks joined by space.
fn extract_text(content: &Content, is_user: bool) -> Option<String> {
    match content {
        Content::Text(s) => {
            if is_user && COMMAND_PREFIXES.iter().any(|p| s.trim_start().starts_with(p)) {
                None
            } else {
                Some(s.clone())
            }
        }
        Content::Blocks(blocks) => {
            let joined: Vec<&str> = blocks
                .iter()
                .filter(|b| b.kind == "text")
                .filter_map(|b| b.text.as_deref())
                .collect();
            if joined.is_empty() {
                None
            } else {
                Some(joined.join(" "))
            }
        }
    }
}

pub(crate) fn parse_ts_secs(s: &str) -> Option<i64> {
    let ts: jiff::Timestamp = s.parse().ok()?;
    Some(ts.as_second())
}

pub(crate) fn file_mtime_ms(entry: &std::fs::DirEntry) -> Result<i64> {
    let modified = entry.metadata()?.modified()?;
    let dur = modified
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    Ok(dur.as_millis() as i64)
}
```

> Note: this adds `serde_json` only for the `toolUseResult` passthrough value. Add it to `Cargo.toml` `[dependencies]`: `serde_json = "1"`.

- [ ] **Step 5: Add `serde_json` dependency**

In `Cargo.toml` under `[dependencies]`, add: `serde_json = "1"`.

- [ ] **Step 6: Run to verify pass**

Run: `cargo test --test claude_adapter`
Expected: PASS (1 test).

- [ ] **Step 7: Commit**

```bash
git add src/adapters/claude.rs tests/fixtures/claude tests/claude_adapter.rs Cargo.toml Cargo.lock
git commit -m "feat(adapters): ClaudeAdapter scan + parse with content exclusion policy"
```

---

## Task 7: `CodexAdapter`

**Files:**
- Modify: `src/adapters/codex.rs`
- Create: `tests/fixtures/codex/rollout-2026-06-04T10-00-00-codexsample.jsonl`
- Create: `tests/codex_adapter.rs`

- [ ] **Step 1: Create fixture `tests/fixtures/codex/rollout-2026-06-04T10-00-00-codexsample.jsonl`**

```
{"type":"session_meta","timestamp":"2026-06-04T10:00:00.000Z","payload":{"id":"codexsample","cwd":"/Users/me/work/web","git":{"branch":"main"}}}
{"type":"turn_context","timestamp":"2026-06-04T10:00:01.000Z","payload":{"approval_policy":"on-request","sandbox_policy":{"type":"workspace-write"}}}
{"type":"event_msg","timestamp":"2026-06-04T10:00:02.000Z","payload":{"type":"user_message","message":"refactor the auth guard"}}
{"type":"response_item","timestamp":"2026-06-04T10:00:03.000Z","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"# AGENTS.md instructions"},{"type":"input_text","text":"<environment_context><cwd>/Users/me/work/web"}]}}
{"type":"event_msg","timestamp":"2026-06-04T10:00:04.000Z","payload":{"type":"agent_message","message":"I split the guard into middleware."}}
{"type":"response_item","timestamp":"2026-06-04T10:00:05.000Z","payload":{"type":"function_call","name":"exec_command","arguments":"ls"}}
{"type":"event_msg","timestamp":"2026-06-04T10:00:06.000Z","payload":{"type":"token_count","total":1234}}
{"type":"turn_context","timestamp":"2026-06-04T10:00:07.000Z","payload":{"approval_policy":"never","sandbox_policy":{"type":"danger-full-access"}}}
```

- [ ] **Step 2: Write `tests/codex_adapter.rs`**

```rust
use hop::adapters::codex::CodexAdapter;
use hop::adapters::Adapter;
use hop::core::AgentId;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/codex")
        .join(name)
}

#[test]
fn parses_meta_clean_text_and_detects_yolo() {
    let adapter = CodexAdapter::new(PathBuf::from("/unused"));
    let s = adapter
        .parse(&fixture("rollout-2026-06-04T10-00-00-codexsample.jsonl"))
        .unwrap();

    assert_eq!(s.agent, AgentId::Codex);
    assert_eq!(s.id, "codexsample"); // from session_meta.payload.id
    assert_eq!(s.directory, "/Users/me/work/web");

    // clean event_msg text only
    assert!(s.content.contains("refactor the auth guard"));
    assert!(s.content.contains("I split the guard into middleware."));
    // injected/tool/meta excluded
    assert!(!s.content.contains("AGENTS.md"));
    assert!(!s.content.contains("environment_context"));
    assert!(!s.content.contains("exec_command"));
    assert!(!s.content.contains("token_count"));

    assert_eq!(s.title, "refactor the auth guard");
    assert_eq!(s.message_count, 2);
    // any turn_context with never + danger-full-access => yolo
    assert!(s.yolo);
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test --test codex_adapter`
Expected: FAIL — `CodexAdapter` not found.

- [ ] **Step 4: Implement `src/adapters/codex.rs`**

```rust
use crate::adapters::claude::{file_mtime_ms, parse_ts_secs};
use crate::adapters::Adapter;
use crate::core::{truncate_title, AgentId, ScanEntry, Session, SessionId};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const TITLE_MAX: usize = 80;

pub struct CodexAdapter {
    /// ~/.codex (we read sessions/ and archived_sessions/ under it).
    root: PathBuf,
}

impl CodexAdapter {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn session_roots(&self) -> Vec<PathBuf> {
        vec![self.root.join("sessions"), self.root.join("archived_sessions")]
    }
}

#[derive(Deserialize)]
struct Line {
    #[serde(rename = "type")]
    kind: String,
    payload: Option<Payload>,
}

#[derive(Deserialize)]
struct Payload {
    // session_meta
    id: Option<String>,
    cwd: Option<String>,
    // turn_context
    approval_policy: Option<String>,
    sandbox_policy: Option<SandboxPolicy>,
    // event_msg
    #[serde(rename = "type")]
    sub: Option<String>,
    message: Option<String>,
}

#[derive(Deserialize)]
struct SandboxPolicy {
    #[serde(rename = "type")]
    kind: String,
}

impl Adapter for CodexAdapter {
    fn id(&self) -> AgentId {
        AgentId::Codex
    }

    fn is_available(&self) -> bool {
        self.session_roots().iter().any(|p| p.is_dir())
    }

    fn scan(&self) -> Result<HashMap<SessionId, ScanEntry>> {
        let mut out = HashMap::new();
        for root in self.session_roots() {
            collect_jsonl(&root, &mut out)?;
        }
        Ok(out)
    }

    fn parse(&self, path: &Path) -> Result<Session> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;

        let mut id = String::new();
        let mut directory = String::new();
        let mut title: Option<String> = None;
        let mut first_ts: Option<i64> = None;
        let mut content = String::new();
        let mut message_count: u32 = 0;
        let mut yolo = false;

        for line in raw.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let mut buf = line.as_bytes().to_vec();
            let parsed: Line = match simd_json::serde::from_slice(&mut buf) {
                Ok(l) => l,
                Err(_) => continue,
            };
            let Some(p) = parsed.payload else { continue };

            match parsed.kind.as_str() {
                "session_meta" => {
                    if let Some(i) = p.id {
                        id = i;
                    }
                    if let Some(c) = p.cwd {
                        directory = c;
                    }
                }
                "turn_context" => {
                    let never = p.approval_policy.as_deref() == Some("never");
                    let danger =
                        p.sandbox_policy.as_ref().map(|s| s.kind.as_str()) == Some("danger-full-access");
                    if never && danger {
                        yolo = true;
                    }
                }
                "event_msg" => {
                    let sub = p.sub.as_deref().unwrap_or("");
                    let is_user = sub == "user_message";
                    let is_agent = sub == "agent_message";
                    if !is_user && !is_agent {
                        continue;
                    }
                    let Some(text) = p.message else { continue };
                    if text.trim().is_empty() {
                        continue;
                    }
                    if title.is_none() && is_user {
                        title = Some(truncate_title(&text, TITLE_MAX));
                    }
                    if !content.is_empty() {
                        content.push('\n');
                    }
                    content.push_str(text.trim());
                    message_count += 1;
                }
                _ => {}
            }
        }

        // first timestamp: re-read meta line is overkill; use file-derived 0 fallback,
        // but prefer the session_meta timestamp captured below.
        // (We re-scan for the first envelope timestamp cheaply.)
        if first_ts.is_none() {
            first_ts = raw
                .lines()
                .find(|l| !l.trim().is_empty())
                .and_then(|l| {
                    let mut b = l.as_bytes().to_vec();
                    let v: serde_json::Value = simd_json::serde::from_slice(&mut b).ok()?;
                    v.get("timestamp").and_then(|t| t.as_str()).and_then(parse_ts_secs)
                });
        }

        if id.is_empty() {
            // fall back to filename uuid (last '-' segment before .jsonl)
            id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|stem| stem.rsplit_once('-').map(|(_, u)| u.to_string()))
                .unwrap_or_else(|| "unknown".to_string());
        }

        Ok(Session {
            id,
            agent: AgentId::Codex,
            title: title.unwrap_or_else(|| "(untitled)".to_string()),
            directory,
            timestamp: first_ts.unwrap_or(0),
            content,
            message_count,
            mtime: 0,
            yolo,
        })
    }

    fn resume_command(&self, s: &Session, yolo: bool) -> Vec<String> {
        if yolo {
            vec![
                "codex".into(),
                "--dangerously-bypass-approvals-and-sandbox".into(),
                "resume".into(),
                s.id.clone(),
            ]
        } else {
            vec!["codex".into(), "resume".into(), s.id.clone()]
        }
    }

    fn supports_yolo(&self) -> bool {
        true
    }
}

/// Recursively collect `rollout-*.jsonl` files keyed by trailing-uuid id.
fn collect_jsonl(dir: &Path, out: &mut HashMap<SessionId, ScanEntry>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl(&path, out)?;
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        // rollout-<ts>-<uuid> => uuid is everything after the last 5 '-'? Use last hyphen group.
        let id = stem.rsplit_once('-').map(|(_, u)| u.to_string()).unwrap_or_else(|| stem.to_string());
        let mtime = file_mtime_ms(&entry)?;
        out.insert(id, ScanEntry { path, mtime });
    }
    Ok(())
}
```

> Note on the id from filename: the real UUID contains hyphens, so `rsplit_once('-')` only captures the last group. This filename-derived id is a **fallback**; `parse()` overrides it with the authoritative `session_meta.payload.id`. `scan()`'s key only needs to be stable per file for the diff, and the engine re-keys by the parsed id on upsert (see Task 9). The fixture's filename uses a hyphen-free id so the fallback test is deterministic.

- [ ] **Step 5: Run to verify pass**

Run: `cargo test --test codex_adapter`
Expected: PASS (1 test).

- [ ] **Step 6: Run the whole adapter+core+query+config suite**

Run: `cargo test --lib && cargo test --test claude_adapter --test codex_adapter`
Expected: all PASS.

- [ ] **Step 7: Commit**

```bash
git add src/adapters/codex.rs tests/fixtures/codex tests/codex_adapter.rs
git commit -m "feat(adapters): CodexAdapter scan + parse + yolo auto-detect"
```

---

## Task 8: `index` — Tantivy schema, build, query

**Files:**
- Modify: `src/index.rs`
- Create: `tests/index_sync.rs`

- [ ] **Step 1: Write `tests/index_sync.rs`**

```rust
use hop::core::{AgentId, ScanEntry, Session};
use hop::index::{diff, SearchIndex};
use hop::query;
use std::collections::HashMap;
use std::path::PathBuf;

fn sess(id: &str, title: &str, content: &str, agent: AgentId, ts: i64, mtime: i64) -> Session {
    Session {
        id: id.into(),
        agent,
        title: title.into(),
        directory: "/work/api".into(),
        timestamp: ts,
        content: content.into(),
        message_count: 1,
        mtime,
        yolo: false,
    }
}

#[test]
fn build_search_and_reconstruct() {
    let dir = tempfile::tempdir().unwrap();
    let mut idx = SearchIndex::open_or_create(dir.path()).unwrap();

    let mut w = idx.writer().unwrap();
    idx.upsert(&mut w, &sess("a", "auth refresh", "token bug", AgentId::Claude, 100, 1));
    idx.upsert(&mut w, &sess("b", "unrelated", "nothing here", AgentId::Codex, 90, 1));
    w.commit().unwrap();
    idx.reload().unwrap();

    let q = query::parse("auth");
    let results = idx.search(&q, 1000, 50).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "a");
    assert_eq!(results[0].title, "auth refresh");
    assert_eq!(results[0].agent, AgentId::Claude);
}

#[test]
fn exact_ranks_above_fuzzy() {
    let dir = tempfile::tempdir().unwrap();
    let mut idx = SearchIndex::open_or_create(dir.path()).unwrap();
    let mut w = idx.writer().unwrap();
    idx.upsert(&mut w, &sess("exact", "refactor", "refactor", AgentId::Claude, 100, 1));
    idx.upsert(&mut w, &sess("fuzzy", "refacter", "refacter", AgentId::Claude, 100, 1));
    w.commit().unwrap();
    idx.reload().unwrap();

    let q = query::parse("refactor");
    let results = idx.search(&q, 1000, 50).unwrap();
    assert_eq!(results[0].id, "exact"); // exact boosted above edit-distance-1
}

#[test]
fn agent_filter_applies() {
    let dir = tempfile::tempdir().unwrap();
    let mut idx = SearchIndex::open_or_create(dir.path()).unwrap();
    let mut w = idx.writer().unwrap();
    idx.upsert(&mut w, &sess("c", "deploy", "ship it", AgentId::Claude, 100, 1));
    idx.upsert(&mut w, &sess("x", "deploy", "ship it", AgentId::Codex, 100, 1));
    w.commit().unwrap();
    idx.reload().unwrap();

    let q = query::parse("deploy agent:codex");
    let results = idx.search(&q, 1000, 50).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].agent, AgentId::Codex);
}

#[test]
fn incremental_diff_detects_changes_and_deletions() {
    let mut known: HashMap<String, i64> = HashMap::new();
    known.insert("keep".into(), 100);
    known.insert("changed".into(), 100);
    known.insert("deleted".into(), 100);

    let mut scanned: HashMap<String, ScanEntry> = HashMap::new();
    scanned.insert("keep".into(), ScanEntry { path: PathBuf::from("k"), mtime: 100 });
    scanned.insert("changed".into(), ScanEntry { path: PathBuf::from("c"), mtime: 500 });
    scanned.insert("new".into(), ScanEntry { path: PathBuf::from("n"), mtime: 10 });

    let (changed, deleted) = diff(&known, &scanned);
    let mut changed_ids: Vec<&String> = changed.iter().map(|(id, _)| id).collect();
    changed_ids.sort();
    assert_eq!(changed_ids, vec![&"changed".to_string(), &"new".to_string()]);
    assert_eq!(deleted, vec!["deleted".to_string()]);
}

#[test]
fn empty_query_returns_all_sorted_by_recency() {
    let dir = tempfile::tempdir().unwrap();
    let mut idx = SearchIndex::open_or_create(dir.path()).unwrap();
    let mut w = idx.writer().unwrap();
    idx.upsert(&mut w, &sess("old", "a", "x", AgentId::Claude, 100, 1));
    idx.upsert(&mut w, &sess("new", "b", "y", AgentId::Claude, 200, 1));
    w.commit().unwrap();
    idx.reload().unwrap();

    let q = query::parse("");
    let results = idx.search(&q, 1000, 50).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, "new"); // newest first
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --test index_sync`
Expected: FAIL — `SearchIndex`, `diff` not found.

- [ ] **Step 3: Implement `src/index.rs`**

```rust
use crate::core::{AgentId, ScanEntry, Session, SessionId};
use crate::query::ParsedQuery;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::{
    AllQuery, BooleanQuery, BoostQuery, FuzzyTermQuery, Occur, Query, QueryParser, TermQuery,
};
use tantivy::schema::{Field, IndexRecordOption, Schema, Value, FAST, STORED, STRING, TEXT};
use tantivy::{Index, IndexReader, IndexWriter, TantivyDocument, Term};

pub const SCHEMA_VERSION: u32 = 1;
const EXACT_BOOST: f32 = 5.0;
const FETCH_CAP: usize = 5_000;
const WRITER_HEAP: usize = 50_000_000;

struct Fields {
    id: Field,
    agent: Field,
    title: Field,
    content: Field,
    directory: Field,
    timestamp: Field,
    mtime: Field,
    message_count: Field,
    yolo: Field,
}

pub struct SearchIndex {
    index: Index,
    reader: IndexReader,
    f: Fields,
}

fn build_schema() -> (Schema, Fields) {
    let mut b = Schema::builder();
    let f = Fields {
        id: b.add_text_field("id", STRING | STORED),
        agent: b.add_text_field("agent", STRING | STORED),
        title: b.add_text_field("title", TEXT | STORED),
        content: b.add_text_field("content", TEXT | STORED),
        directory: b.add_text_field("directory", STRING | STORED),
        timestamp: b.add_u64_field("timestamp", FAST | STORED),
        mtime: b.add_u64_field("mtime", STORED),
        message_count: b.add_u64_field("message_count", STORED),
        yolo: b.add_u64_field("yolo", STORED),
    };
    (b.build(), f)
}

impl SearchIndex {
    /// Open the index at `dir`, creating it if absent. If the on-disk schema
    /// version marker mismatches, the index is dropped and rebuilt empty.
    pub fn open_or_create(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let marker = dir.join(".schema_version");
        let version_ok = std::fs::read_to_string(&marker)
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            == Some(SCHEMA_VERSION);

        if !version_ok && dir.join("meta.json").exists() {
            // wipe stale index contents
            for entry in std::fs::read_dir(dir)?.flatten() {
                let _ = std::fs::remove_file(entry.path());
            }
        }

        let (schema, f) = build_schema();
        let index = match Index::open_in_dir(dir) {
            Ok(i) => i,
            Err(_) => Index::create_in_dir(dir, schema.clone())
                .with_context(|| format!("creating index in {}", dir.display()))?,
        };
        std::fs::write(&marker, SCHEMA_VERSION.to_string())?;

        let reader = index.reader()?;
        Ok(Self { index, reader, f })
    }

    pub fn writer(&self) -> Result<IndexWriter> {
        Ok(self.index.writer(WRITER_HEAP)?)
    }

    pub fn reload(&self) -> Result<()> {
        self.reader.reload()?;
        Ok(())
    }

    pub fn upsert(&self, w: &mut IndexWriter, s: &Session) {
        w.delete_term(Term::from_field_text(self.f.id, &s.id));
        let mut doc = TantivyDocument::default();
        doc.add_text(self.f.id, &s.id);
        doc.add_text(self.f.agent, s.agent.slug());
        doc.add_text(self.f.title, &s.title);
        doc.add_text(self.f.content, &s.content);
        doc.add_text(self.f.directory, &s.directory);
        doc.add_u64(self.f.timestamp, s.timestamp.max(0) as u64);
        doc.add_u64(self.f.mtime, s.mtime.max(0) as u64);
        doc.add_u64(self.f.message_count, s.message_count as u64);
        doc.add_u64(self.f.yolo, s.yolo as u64);
        let _ = w.add_document(doc);
    }

    pub fn delete(&self, w: &mut IndexWriter, id: &str) {
        w.delete_term(Term::from_field_text(self.f.id, id));
    }

    /// id -> mtime for every indexed session (drives incremental diff).
    pub fn known_mtimes(&self) -> Result<HashMap<SessionId, i64>> {
        let searcher = self.reader.searcher();
        let n = searcher.num_docs().max(1) as usize;
        let hits = searcher.search(&AllQuery, &TopDocs::with_limit(n))?;
        let mut map = HashMap::new();
        for (_, addr) in hits {
            let doc: TantivyDocument = searcher.doc(addr)?;
            let id = doc.get_first(self.f.id).and_then(|v| v.as_str());
            let mtime = doc.get_first(self.f.mtime).and_then(|v| v.as_u64());
            if let (Some(id), Some(m)) = (id, mtime) {
                map.insert(id.to_string(), m as i64);
            }
        }
        Ok(map)
    }

    /// Run a parsed query. `now` is unix seconds (for date filtering).
    pub fn search(&self, q: &ParsedQuery, now: i64, limit: usize) -> Result<Vec<Session>> {
        let searcher = self.reader.searcher();

        // --- build the text/all query plus agent constraints ---
        let mut clauses: Vec<(Occur, Box<dyn Query>)> = Vec::new();

        if q.free_text.trim().is_empty() {
            clauses.push((Occur::Must, Box::new(AllQuery)));
        } else {
            let qp = QueryParser::for_index(&self.index, vec![self.f.title, self.f.content]);
            let exact = qp
                .parse_query(&sanitize(&q.free_text))
                .unwrap_or_else(|_| Box::new(AllQuery));
            let mut should: Vec<(Occur, Box<dyn Query>)> =
                vec![(Occur::Should, Box::new(BoostQuery::new(exact, EXACT_BOOST)))];
            for word in q.free_text.split_whitespace() {
                let w = word.to_lowercase();
                for field in [self.f.title, self.f.content] {
                    let fz = FuzzyTermQuery::new_prefix(Term::from_field_text(field, &w), 1, true);
                    should.push((Occur::Should, Box::new(fz)));
                }
            }
            clauses.push((Occur::Must, Box::new(BooleanQuery::new(should))));
        }

        // agent include (any-of) / exclude
        if !q.agents.include.is_empty() {
            let any: Vec<(Occur, Box<dyn Query>)> = q
                .agents
                .include
                .iter()
                .map(|a| {
                    (
                        Occur::Should,
                        Box::new(TermQuery::new(
                            Term::from_field_text(self.f.agent, a.slug()),
                            IndexRecordOption::Basic,
                        )) as Box<dyn Query>,
                    )
                })
                .collect();
            clauses.push((Occur::Must, Box::new(BooleanQuery::new(any))));
        }
        for a in &q.agents.exclude {
            clauses.push((
                Occur::MustNot,
                Box::new(TermQuery::new(
                    Term::from_field_text(self.f.agent, a.slug()),
                    IndexRecordOption::Basic,
                )),
            ));
        }

        let query = BooleanQuery::new(clauses);

        // --- collect (recency order for empty free-text, else score) ---
        let addrs: Vec<tantivy::DocAddress> = if q.free_text.trim().is_empty() {
            searcher
                .search(
                    &query,
                    &TopDocs::with_limit(FETCH_CAP).order_by_u64_field("timestamp", tantivy::Order::Desc),
                )?
                .into_iter()
                .map(|(_, a)| a)
                .collect()
        } else {
            searcher
                .search(&query, &TopDocs::with_limit(FETCH_CAP))?
                .into_iter()
                .map(|(_, a)| a)
                .collect()
        };

        // --- reconstruct + post-filter dir & date ---
        let date_range = q.date.map(|d| d.range(now));
        let mut out = Vec::new();
        for addr in addrs {
            let doc: TantivyDocument = searcher.doc(addr)?;
            let s = self.to_session(&doc);
            if !dir_ok(&s.directory, &q) {
                continue;
            }
            if let Some((lo, hi)) = date_range {
                if let Some(lo) = lo {
                    if s.timestamp < lo {
                        continue;
                    }
                }
                if let Some(hi) = hi {
                    if s.timestamp > hi {
                        continue;
                    }
                }
            }
            out.push(s);
            if out.len() >= limit {
                break;
            }
        }
        Ok(out)
    }

    fn to_session(&self, doc: &TantivyDocument) -> Session {
        let get_str = |f: Field| doc.get_first(f).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let get_u64 = |f: Field| doc.get_first(f).and_then(|v| v.as_u64()).unwrap_or(0);
        Session {
            id: get_str(self.f.id),
            agent: AgentId::from_slug(&get_str(self.f.agent)).unwrap_or(AgentId::Claude),
            title: get_str(self.f.title),
            directory: get_str(self.f.directory),
            timestamp: get_u64(self.f.timestamp) as i64,
            content: get_str(self.f.content),
            message_count: get_u64(self.f.message_count) as u32,
            mtime: get_u64(self.f.mtime) as i64,
            yolo: get_u64(self.f.yolo) != 0,
        }
    }
}

fn dir_ok(directory: &str, q: &ParsedQuery) -> bool {
    let d = directory.to_lowercase();
    q.dirs.include.iter().all(|i| d.contains(&i.to_lowercase()))
        && !q.dirs.exclude.iter().any(|e| d.contains(&e.to_lowercase()))
}

/// Escape characters that would make tantivy's QueryParser error out.
fn sanitize(s: &str) -> String {
    s.replace(['+', '-', '!', '^', '~', '*', '?', ':', '(', ')', '[', ']', '{', '}', '"'], " ")
}

/// Pure incremental diff. Returns (changed[(id, entry)], deleted[id]).
/// Changed = scanned mtime > known + 1ms, or id absent from known.
pub fn diff(
    known: &HashMap<SessionId, i64>,
    scanned: &HashMap<SessionId, ScanEntry>,
) -> (Vec<(SessionId, ScanEntry)>, Vec<SessionId>) {
    let mut changed = Vec::new();
    for (id, entry) in scanned {
        match known.get(id) {
            Some(&m) if entry.mtime <= m + 1 => {}
            _ => changed.push((id.clone(), entry.clone())),
        }
    }
    let deleted: Vec<SessionId> = known
        .keys()
        .filter(|id| !scanned.contains_key(*id))
        .cloned()
        .collect();
    (changed, deleted)
}
```

> If `TopDocs::order_by_u64_field` is not present under that exact name in 0.26, use `.order_by_fast_field::<u64>("timestamp", tantivy::Order::Desc)` — the `timestamp` field is declared `FAST`. Confirm by checking `cargo doc -p tantivy --open` or the compiler error, and adjust the one call site.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --test index_sync`
Expected: PASS (5 tests). If the `order_by_*` call fails to compile, apply the alternate from the note above, then rerun.

- [ ] **Step 5: Commit**

```bash
git add src/index.rs tests/index_sync.rs
git commit -m "feat(index): tantivy schema, ranked query builder, incremental diff"
```

---

## Task 9: `engine` — orchestration, debounce, background sync

**Files:**
- Modify: `src/engine.rs`

- [ ] **Step 1: Write failing tests at the bottom of `src/engine.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::Adapter;
    use crate::core::{AgentId, ScanEntry, Session, SessionId};
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    struct FakeAdapter {
        sessions: Vec<Session>,
    }

    impl Adapter for FakeAdapter {
        fn id(&self) -> AgentId { AgentId::Claude }
        fn is_available(&self) -> bool { true }
        fn scan(&self) -> anyhow::Result<HashMap<SessionId, ScanEntry>> {
            Ok(self.sessions.iter().map(|s| {
                (s.id.clone(), ScanEntry { path: PathBuf::from(&s.id), mtime: s.mtime })
            }).collect())
        }
        fn parse(&self, path: &Path) -> anyhow::Result<Session> {
            let id = path.to_string_lossy().to_string();
            self.sessions.iter().find(|s| s.id == id).cloned()
                .ok_or_else(|| anyhow::anyhow!("not found"))
        }
        fn resume_command(&self, s: &Session, _yolo: bool) -> Vec<String> {
            vec!["echo".into(), s.id.clone()]
        }
        fn supports_yolo(&self) -> bool { true }
    }

    fn sess(id: &str, title: &str) -> Session {
        Session {
            id: id.into(), agent: AgentId::Claude, title: title.into(),
            directory: "/d".into(), timestamp: 100, content: title.into(),
            message_count: 1, mtime: 10, yolo: false,
        }
    }

    #[test]
    fn sync_then_search_finds_indexed_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let adapters: Vec<Box<dyn Adapter>> =
            vec![Box::new(FakeAdapter { sessions: vec![sess("a", "auth bug"), sess("b", "deploy")] })];
        let mut engine = Engine::new(dir.path(), adapters).unwrap();

        // synchronous full sync (the blocking core that the bg thread also calls)
        engine.sync_once().unwrap();

        engine.set_query("auth");
        engine.search().unwrap();
        assert_eq!(engine.results().len(), 1);
        assert_eq!(engine.results()[0].id, "a");
    }

    #[test]
    fn deletion_pruned_on_resync() {
        let dir = tempfile::tempdir().unwrap();
        let adapters: Vec<Box<dyn Adapter>> =
            vec![Box::new(FakeAdapter { sessions: vec![sess("a", "auth")] })];
        let mut engine = Engine::new(dir.path(), adapters).unwrap();
        engine.sync_once().unwrap();
        engine.set_query("");
        engine.search().unwrap();
        assert_eq!(engine.results().len(), 1);

        // adapter now returns nothing -> session pruned
        let empty: Vec<Box<dyn Adapter>> = vec![Box::new(FakeAdapter { sessions: vec![] })];
        engine.replace_adapters(empty);
        engine.sync_once().unwrap();
        engine.search().unwrap();
        assert_eq!(engine.results().len(), 0);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib engine`
Expected: FAIL — `Engine` not found.

- [ ] **Step 3: Implement `src/engine.rs`**

```rust
use crate::adapters::Adapter;
use crate::core::Session;
use crate::index::{diff, SearchIndex};
use crate::query::{self, ParsedQuery};
use anyhow::Result;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const DEBOUNCE: Duration = Duration::from_millis(40);

/// Message pushed from the background sync thread to the UI loop.
pub enum Update {
    /// New sessions were indexed; UI should re-run its current search.
    Refresh,
    /// Sync finished; carries count of files that failed to parse.
    Done { parse_errors: usize },
}

pub struct Engine {
    index: SearchIndex,
    adapters: Vec<Box<dyn Adapter>>,
    query: String,
    parsed: ParsedQuery,
    results: Vec<Session>,
    limit: usize,
    last_keystroke: Option<Instant>,
}

impl Engine {
    pub fn new(index_dir: &Path, adapters: Vec<Box<dyn Adapter>>) -> Result<Self> {
        let index = SearchIndex::open_or_create(index_dir)?;
        Ok(Self {
            index,
            adapters,
            query: String::new(),
            parsed: ParsedQuery::default(),
            results: Vec::new(),
            limit: 500,
            last_keystroke: None,
        })
    }

    pub fn results(&self) -> &[Session] {
        &self.results
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn set_query(&mut self, q: impl Into<String>) {
        self.query = q.into();
        self.parsed = query::parse(&self.query);
        self.last_keystroke = Some(Instant::now());
    }

    /// True if enough time has elapsed since the last keystroke to query.
    pub fn debounce_ready(&self) -> bool {
        match self.last_keystroke {
            Some(t) => t.elapsed() >= DEBOUNCE,
            None => true,
        }
    }

    pub fn search(&mut self) -> Result<()> {
        let now = jiff::Timestamp::now().as_second();
        self.results = self.index.search(&self.parsed, now, self.limit)?;
        self.last_keystroke = None;
        Ok(())
    }

    pub fn adapter_for(&self, agent: crate::core::AgentId) -> Option<&dyn Adapter> {
        self.adapters.iter().find(|a| a.id() == agent).map(|b| b.as_ref())
    }

    #[cfg(test)]
    pub fn replace_adapters(&mut self, adapters: Vec<Box<dyn Adapter>>) {
        self.adapters = adapters;
    }

    /// Full synchronous sync pass: scan all adapters, diff, parse changed,
    /// upsert, delete removed, commit, reload. Returns parse-error count.
    pub fn sync_once(&mut self) -> Result<usize> {
        let known = self.index.known_mtimes()?;
        let mut writer = self.index.writer()?;
        let mut parse_errors = 0usize;

        // gather scans, keyed across all adapters
        let mut all_scanned = std::collections::HashMap::new();
        let mut owner = std::collections::HashMap::new(); // id -> adapter index
        for (ai, adapter) in self.adapters.iter().enumerate() {
            if !adapter.is_available() {
                continue;
            }
            for (id, entry) in adapter.scan()? {
                owner.insert(id.clone(), ai);
                all_scanned.insert(id, entry);
            }
        }

        let (changed, deleted) = diff(&known, &all_scanned);
        for id in &deleted {
            self.index.delete(&mut writer, id);
        }
        for (id, entry) in &changed {
            let ai = owner[id];
            match self.adapters[ai].parse(&entry.path) {
                Ok(mut s) => {
                    s.mtime = entry.mtime;
                    self.index.upsert(&mut writer, &s);
                }
                Err(_) => parse_errors += 1,
            }
        }
        writer.commit()?;
        self.index.reload()?;
        Ok(parse_errors)
    }

    /// Spawn the background sync on its own thread. The thread sends `Refresh`
    /// then `Done` over the returned receiver. Uses a fresh index handle so the
    /// UI's engine keeps serving searches meanwhile.
    pub fn spawn_background_sync(
        index_dir: std::path::PathBuf,
        adapters: Vec<Box<dyn Adapter>>,
    ) -> (Receiver<Update>, std::thread::JoinHandle<()>) {
        let (tx, rx): (Sender<Update>, Receiver<Update>) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let result = (|| -> Result<usize> {
                let index = SearchIndex::open_or_create(&index_dir)?;
                let known = index.known_mtimes()?;
                let mut writer = index.writer()?;
                let mut parse_errors = 0usize;

                let mut all_scanned = std::collections::HashMap::new();
                let mut owner = std::collections::HashMap::new();
                for (ai, adapter) in adapters.iter().enumerate() {
                    if !adapter.is_available() {
                        continue;
                    }
                    for (id, entry) in adapter.scan()? {
                        owner.insert(id.clone(), ai);
                        all_scanned.insert(id, entry);
                    }
                }
                let (changed, deleted) = diff(&known, &all_scanned);
                for id in &deleted {
                    index.delete(&mut writer, id);
                }
                // batch commits every 200 upserts so rows stream in
                let mut since_commit = 0;
                for (id, entry) in &changed {
                    let ai = owner[id];
                    match adapters[ai].parse(&entry.path) {
                        Ok(mut s) => {
                            s.mtime = entry.mtime;
                            index.upsert(&mut writer, &s);
                            since_commit += 1;
                        }
                        Err(_) => parse_errors += 1,
                    }
                    if since_commit >= 200 {
                        let _ = writer.commit();
                        let _ = index.reload();
                        let _ = tx.send(Update::Refresh);
                        since_commit = 0;
                    }
                }
                let _ = writer.commit();
                let _ = index.reload();
                Ok(parse_errors)
            })();
            let _ = tx.send(Update::Refresh);
            let _ = tx.send(Update::Done {
                parse_errors: result.unwrap_or(0),
            });
        });
        (rx, handle)
    }

    /// Re-open the reader (after a background commit) so subsequent searches see new docs.
    pub fn reload(&self) -> Result<()> {
        self.index.reload()
    }
}

// silence unused warnings for fields wired by the TUI loop later
#[allow(dead_code)]
fn _assert_send_sync<T: Send>() {}
let _ = Arc::new(Mutex::new(0)); // (removed in real impl; see note)
```

> Remove the stray last two lines (`_assert_send_sync` / `Arc` line) — they are illustrative only and won't compile at module scope. The real module ends after the `impl Engine` block. (Listed here so you don't add `Arc`/`Mutex` imports you don't need.)

Correct the imports line to just:
```rust
use std::sync::mpsc::{self, Receiver, Sender};
```
(drop the `Arc`/`Mutex` import).

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib engine`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/engine.rs
git commit -m "feat(engine): sync orchestration, debounce, background sync thread"
```

---

## Task 10: `resume` — exec handoff

**Files:**
- Modify: `src/resume.rs`

> Command *generation* is already covered by adapter unit tests (Tasks 6, 7). This module is the unix `chdir`+`exec` handoff, which replaces the process and so can't be unit-tested for its success path; we test the precondition guard.

- [ ] **Step 1: Write a failing test at the bottom of `src/resume.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_argv_is_rejected() {
        let err = exec_resume("/tmp", &[]).unwrap_err();
        assert!(err.to_string().contains("empty"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib resume`
Expected: FAIL — `exec_resume` not found.

- [ ] **Step 3: Implement `src/resume.rs`**

```rust
use anyhow::{bail, Result};
use std::os::unix::process::CommandExt;
use std::process::Command;

/// chdir to `directory`, then exec-replace this process with `argv`.
/// On success this never returns. Returns Err only if exec/setup fails.
pub fn exec_resume(directory: &str, argv: &[String]) -> Result<std::convert::Infallible> {
    if argv.is_empty() {
        bail!("cannot resume: empty command");
    }
    if !directory.is_empty() {
        // best-effort chdir; a vanished dir shouldn't block resume
        let _ = std::env::set_current_dir(directory);
    }
    let err = Command::new(&argv[0]).args(&argv[1..]).exec();
    bail!("failed to exec {}: {err}", argv[0]);
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib resume`
Expected: PASS (1 test).

- [ ] **Step 5: Commit**

```bash
git add src/resume.rs
git commit -m "feat(resume): chdir + exec handoff with empty-argv guard"
```

---

## Task 11: `tui` theme + app state machine (polish invariants)

**Files:**
- Modify: `src/tui/theme.rs`
- Modify: `src/tui/mod.rs`

> The §4 polish invariants (Esc quits, Esc closes modal, Ctrl+C quits, navigation moves selection) live in `App::handle_key`, which is pure and unit-testable without a backend.

- [ ] **Step 1: Implement `src/tui/theme.rs`**

```rust
use crate::core::AgentId;
use ratatui::style::Color;

pub fn agent_color(agent: AgentId) -> Color {
    match agent {
        AgentId::Claude => Color::Magenta,
        AgentId::Codex => Color::Blue,
    }
}

pub const ACCENT: Color = Color::Cyan;
pub const DIM: Color = Color::DarkGray;
```

- [ ] **Step 2: Write failing tests at the bottom of `src/tui/mod.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{AgentId, Session};
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn sess(id: &str) -> Session {
        Session {
            id: id.into(), agent: AgentId::Claude, title: id.into(),
            directory: "/d".into(), timestamp: 1, content: String::new(),
            message_count: 0, mtime: 0, yolo: false,
        }
    }

    fn app_with(n: usize) -> App {
        let mut app = App::new();
        app.set_results((0..n).map(|i| sess(&format!("s{i}"))).collect());
        app
    }

    #[test]
    fn esc_quits_main_view() {
        let mut app = app_with(3);
        assert_eq!(app.handle_key(key(KeyCode::Esc)), Action::Quit);
    }

    #[test]
    fn ctrl_c_quits() {
        let mut app = app_with(3);
        let k = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(app.handle_key(k), Action::Quit);
    }

    #[test]
    fn esc_closes_modal_without_quitting() {
        let mut app = app_with(3);
        app.open_yolo_modal();
        assert!(app.modal_open());
        assert_eq!(app.handle_key(key(KeyCode::Esc)), Action::None);
        assert!(!app.modal_open());
    }

    #[test]
    fn down_moves_selection() {
        let mut app = app_with(3);
        assert_eq!(app.selected(), 0);
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.selected(), 1);
        // clamps at the end
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.selected(), 2);
    }

    #[test]
    fn typing_updates_query_and_requests_search() {
        let mut app = app_with(0);
        assert_eq!(app.handle_key(key(KeyCode::Char('a'))), Action::Search);
        assert_eq!(app.handle_key(key(KeyCode::Char('b'))), Action::Search);
        assert_eq!(app.query(), "ab");
        assert_eq!(app.handle_key(key(KeyCode::Backspace)), Action::Search);
        assert_eq!(app.query(), "a");
    }

    #[test]
    fn enter_on_yolo_agent_opens_modal_then_confirms_resume() {
        let mut app = app_with(1); // Claude supports yolo
        assert_eq!(app.handle_key(key(KeyCode::Enter)), Action::None);
        assert!(app.modal_open());
        // Tab toggles yolo, Enter confirms
        app.handle_key(key(KeyCode::Tab));
        match app.handle_key(key(KeyCode::Enter)) {
            Action::Resume { yolo, .. } => assert!(yolo),
            other => panic!("expected resume, got {other:?}"),
        }
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test --lib tui`
Expected: FAIL — `App`, `Action`, etc. not found.

- [ ] **Step 4: Implement `src/tui/mod.rs`**

```rust
pub mod theme;
pub mod view;

use crate::core::Session;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// What the run loop should do after a key event.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
    /// Query changed; the loop should (debounced) re-search.
    Search,
    /// Resume the selected session.
    Resume { index: usize, yolo: bool },
}

#[derive(Debug, PartialEq, Eq)]
enum Mode {
    Main,
    /// Yolo confirmation for the pending session index; `yolo` is the toggle.
    YoloModal { index: usize, yolo: bool },
}

pub struct App {
    query: String,
    results: Vec<Session>,
    selected: usize,
    mode: Mode,
    /// Set by the loop so the App knows which agents need a yolo prompt.
    yolo_supported: Vec<bool>,
}

impl App {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            mode: Mode::Main,
            yolo_supported: Vec::new(),
        }
    }

    pub fn query(&self) -> &str {
        &self.query
    }
    pub fn set_query(&mut self, q: String) {
        self.query = q;
    }
    pub fn results(&self) -> &[Session] {
        &self.results
    }
    pub fn selected(&self) -> usize {
        self.selected
    }
    pub fn modal_open(&self) -> bool {
        matches!(self.mode, Mode::YoloModal { .. })
    }

    pub fn set_results(&mut self, results: Vec<Session>) {
        // mark which rows support yolo (test default: Claude/Codex both do)
        self.yolo_supported = results.iter().map(|_| true).collect();
        self.results = results;
        if self.selected >= self.results.len() {
            self.selected = self.results.len().saturating_sub(1);
        }
    }

    /// Test/helper: directly mark whether the row's agent supports yolo.
    pub fn set_yolo_supported(&mut self, flags: Vec<bool>) {
        self.yolo_supported = flags;
    }

    pub fn open_yolo_modal(&mut self) {
        self.mode = Mode::YoloModal {
            index: self.selected,
            yolo: false,
        };
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.kind == KeyEventKind::Release {
            return Action::None; // ignore key-release (Windows)
        }
        // Ctrl+C always quits, in any mode.
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit;
        }

        match self.mode {
            Mode::YoloModal { index, yolo } => match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::Main; // close, choose nothing
                    Action::None
                }
                KeyCode::Tab => {
                    self.mode = Mode::YoloModal { index, yolo: !yolo };
                    Action::None
                }
                KeyCode::Enter => {
                    self.mode = Mode::Main;
                    Action::Resume { index, yolo }
                }
                _ => Action::None,
            },
            Mode::Main => match (key.code, key.modifiers) {
                (KeyCode::Esc, _) => Action::Quit,
                (KeyCode::Down, _) => {
                    if !self.results.is_empty() {
                        self.selected = (self.selected + 1).min(self.results.len() - 1);
                    }
                    Action::None
                }
                (KeyCode::Up, _) => {
                    self.selected = self.selected.saturating_sub(1);
                    Action::None
                }
                (KeyCode::Enter, _) => self.activate(false),
                (KeyCode::Tab, _) => self.activate(true),
                (KeyCode::Backspace, _) => {
                    self.query.pop();
                    Action::Search
                }
                (KeyCode::Char(c), m) if !m.contains(KeyModifiers::CONTROL) => {
                    self.query.push(c);
                    Action::Search
                }
                _ => Action::None,
            },
        }
    }

    /// Enter (yolo=false) or Tab (yolo=true). If the agent supports yolo and the
    /// caller didn't force it, open the confirmation modal; else resume directly.
    fn activate(&mut self, force_yolo: bool) -> Action {
        if self.results.is_empty() {
            return Action::None;
        }
        let idx = self.selected;
        let supports = self.yolo_supported.get(idx).copied().unwrap_or(false);
        if force_yolo {
            return Action::Resume { index: idx, yolo: true };
        }
        if supports {
            self.mode = Mode::YoloModal { index: idx, yolo: false };
            Action::None
        } else {
            Action::Resume { index: idx, yolo: false }
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test --lib tui`
Expected: PASS (6 tests).

- [ ] **Step 6: Commit**

```bash
git add src/tui/mod.rs src/tui/theme.rs
git commit -m "feat(tui): App state machine with Esc/Ctrl+C/modal polish invariants"
```

---

## Task 12: `tui::view` — rendering + a TestBackend render test

**Files:**
- Modify: `src/tui/view.rs`

- [ ] **Step 1: Implement `src/tui/view.rs`**

```rust
use crate::tui::{theme, App};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

/// Relative-time label from a unix-seconds timestamp.
pub fn rel_time(ts: i64, now: i64) -> String {
    let s = (now - ts).max(0);
    if s >= 86_400 {
        format!("{}d", s / 86_400)
    } else if s >= 3_600 {
        format!("{}h", s / 3_600)
    } else if s >= 60 {
        format!("{}m", s / 60)
    } else {
        format!("{s}s")
    }
}

pub fn render(f: &mut Frame, app: &App, now: i64) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // search input
            Constraint::Min(1),    // body (list | preview)
            Constraint::Length(1), // footer
        ])
        .split(f.area());

    // --- search input ---
    let header = Line::from(vec![
        Span::raw("❯ "),
        Span::raw(app.query()),
        Span::raw(format!("   {}/{}", app.results().len(), app.results().len())).fg(theme::DIM),
    ]);
    f.render_widget(Paragraph::new(header), chunks[0]);

    // --- body: list | preview ---
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    let items: Vec<ListItem> = app
        .results()
        .iter()
        .map(|s| {
            ListItem::new(Line::from(vec![
                Span::raw(s.agent.badge()).fg(theme::agent_color(s.agent)),
                Span::raw(" "),
                Span::raw(s.title.clone()),
                Span::raw(format!("  · {} · {}", s.directory, rel_time(s.timestamp, now))).fg(theme::DIM),
            ]))
        })
        .collect();

    let mut state = ListState::default();
    if !app.results().is_empty() {
        state.select(Some(app.selected()));
    }
    let list = List::new(items)
        .block(Block::default().borders(Borders::RIGHT))
        .highlight_style(Style::default().bg(theme::ACCENT));
    f.render_stateful_widget(list, body[0], &mut state);

    // --- preview ---
    let preview_text = app
        .results()
        .get(app.selected())
        .map(|s| s.content.clone())
        .unwrap_or_default();
    f.render_widget(Paragraph::new(preview_text), body[1]);

    // --- footer ---
    let footer = if app.modal_open() {
        "tab toggle yolo · enter confirm · esc cancel"
    } else {
        "↑↓ move · enter resume · tab yolo · esc quit"
    };
    f.render_widget(Paragraph::new(footer).fg(theme::DIM), chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{AgentId, Session};
    use crate::tui::App;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn rel_time_units() {
        assert_eq!(rel_time(0, 30), "30s");
        assert_eq!(rel_time(0, 120), "2m");
        assert_eq!(rel_time(0, 7200), "2h");
        assert_eq!(rel_time(0, 2 * 86400), "2d");
    }

    #[test]
    fn renders_badge_and_title() {
        let mut app = App::new();
        app.set_results(vec![Session {
            id: "a".into(), agent: AgentId::Claude, title: "fix auth".into(),
            directory: "/w".into(), timestamp: 0, content: "hello".into(),
            message_count: 1, mtime: 0, yolo: false,
        }]);
        let backend = TestBackend::new(60, 6);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| render(f, &app, 100)).unwrap();
        let buf = term.backend().buffer().clone();
        let text: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(text.contains("CLAUDE"));
        assert!(text.contains("fix auth"));
    }
}
```

- [ ] **Step 2: Run to verify pass**

Run: `cargo test --lib tui::view`
Expected: PASS (2 tests). If `buf.content()` / `symbol()` accessor names differ in ratatui 0.30, switch the assertion to render into `Buffer::with_lines(...)` and use `term.backend().assert_buffer(&expected)` instead (compiler will guide).

- [ ] **Step 3: Commit**

```bash
git add src/tui/view.rs
git commit -m "feat(tui): vertical-split render (badge list + preview + footer)"
```

---

## Task 13: `cli` + `main` wiring (instant open, bg sync, run loop, resume)

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Implement `src/cli.rs`**

```rust
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "hop", version, about = "Search and resume coding-agent sessions")]
pub struct Cli {
    /// Pre-fill the search query.
    pub query: Option<String>,

    /// Filter by agent (claude|codex).
    #[arg(short, long)]
    pub agent: Option<String>,

    /// Filter by directory substring.
    #[arg(short, long)]
    pub dir: Option<String>,

    /// Force yolo resume when supported.
    #[arg(long)]
    pub yolo: bool,

    /// Wipe and rebuild the index before starting.
    #[arg(long)]
    pub rebuild: bool,
}

impl Cli {
    /// Compose the initial query string from positional + flag filters.
    pub fn initial_query(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(a) = &self.agent {
            parts.push(format!("agent:{a}"));
        }
        if let Some(d) = &self.dir {
            parts.push(format!("dir:{d}"));
        }
        if let Some(q) = &self.query {
            parts.push(q.clone());
        }
        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_query_composes_filters() {
        let cli = Cli {
            query: Some("auth".into()),
            agent: Some("claude".into()),
            dir: Some("api".into()),
            yolo: false,
            rebuild: false,
        };
        assert_eq!(cli.initial_query(), "agent:claude dir:api auth");
    }
}
```

- [ ] **Step 2: Run the CLI test**

Run: `cargo test --lib cli`
Expected: PASS (1 test).

- [ ] **Step 3: Implement `src/main.rs`**

```rust
use anyhow::Result;
use clap::Parser;
use hop::adapters::{self, Adapter};
use hop::cli::Cli;
use hop::config::Config;
use hop::engine::{Engine, Update};
use hop::resume;
use hop::tui::{view, Action, App};
use ratatui::crossterm::event::{self, Event};
use std::time::Duration;

fn index_dir() -> std::path::PathBuf {
    directories::ProjectDirs::from("dev", "hop", "hop")
        .map(|d| d.cache_dir().join("index"))
        .unwrap_or_else(|| std::path::PathBuf::from(".hop-index"))
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;
    let dir = index_dir();

    if cli.rebuild && dir.exists() {
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Build adapters twice: one set for the foreground engine, one moved to the bg thread.
    let fg_adapters = adapters::default_adapters(&config);
    let bg_adapters = adapters::default_adapters(&config);

    let mut engine = Engine::new(&dir, fg_adapters)?;
    engine.set_query(cli.initial_query());
    engine.search()?; // immediate results from whatever is already indexed

    // background sync streams new sessions in
    let (updates, _handle) = Engine::spawn_background_sync(dir.clone(), bg_adapters);

    // resume request escapes the TUI loop so we exec AFTER restoring the terminal
    let pending = run_tui(&mut engine, updates)?;

    if let Some((session, yolo)) = pending {
        let agent = engine
            .adapter_for(session.agent)
            .map(|a| a.resume_command(&session, yolo || cli.yolo))
            .unwrap_or_default();
        // terminal already restored by run_tui's Drop/restore
        resume::exec_resume(&session.directory, &agent)?;
    }
    Ok(())
}

/// Runs the event loop. Returns Some((session, yolo)) if the user chose to resume.
fn run_tui(
    engine: &mut Engine,
    updates: std::sync::mpsc::Receiver<Update>,
) -> Result<Option<(hop::core::Session, bool)>> {
    let mut terminal = ratatui::init();
    let mut app = App::new();
    app.set_query(engine.query().to_string());
    sync_results_into_app(engine, &mut app);

    let outcome = (|| -> Result<Option<(hop::core::Session, bool)>> {
        loop {
            let now = jiff::Timestamp::now().as_second();
            terminal.draw(|f| view::render(f, &app, now))?;

            // fold in any streamed sessions
            while let Ok(update) = updates.try_recv() {
                if let Update::Refresh = update {
                    engine.reload()?;
                    engine.search()?;
                    sync_results_into_app(engine, &mut app);
                }
            }

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    match app.handle_key(key) {
                        Action::Quit => return Ok(None),
                        Action::Search => {
                            engine.set_query(app.query().to_string());
                            engine.search()?;
                            sync_results_into_app(engine, &mut app);
                        }
                        Action::Resume { index, yolo } => {
                            if let Some(s) = engine.results().get(index).cloned() {
                                return Ok(Some((s, yolo)));
                            }
                        }
                        Action::None => {}
                    }
                }
            }
        }
    })();

    ratatui::restore();
    outcome
}

fn sync_results_into_app(engine: &Engine, app: &mut App) {
    let results: Vec<hop::core::Session> = engine.results().to_vec();
    let yolo_flags: Vec<bool> = results.iter().map(|_| true).collect(); // v1: both agents support yolo
    app.set_results(results);
    app.set_yolo_supported(yolo_flags);
}
```

> `ratatui::restore()` runs on every exit path (including the `?` early-returns inside the closure, because we capture the closure result then always call `restore`). The Drop-guard requirement from §4/§8 is additionally satisfied by `ratatui::init()`, which installs a panic hook that restores the terminal.

- [ ] **Step 4: Build the whole binary**

Run: `cargo build`
Expected: compiles. Fix any signature mismatches the compiler reports (e.g. `Engine::query()` visibility) — they should be minor.

- [ ] **Step 5: Full test suite**

Run: `cargo test`
Expected: ALL tests pass (lib + 3 integration files).

- [ ] **Step 6: Smoke-test against real data**

Run: `cargo run -- --help`
Expected: clap help text listing `[QUERY]`, `--agent`, `--dir`, `--yolo`, `--rebuild`.

Then (interactive — you drive it): `cargo run` and confirm the TUI opens, shows sessions, scrolls with ↑↓, Esc quits and your terminal is restored. Do **not** press Enter unless you actually want to resume a session (it will exec the agent CLI).

- [ ] **Step 7: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "feat(cli): wire engine + tui + background sync + resume exec"
```

---

## Task 14: Final verification + README

**Files:**
- Create: `README.md`

- [ ] **Step 1: Run the full suite with warnings surfaced**

Run: `cargo test && cargo clippy --all-targets 2>&1 | tail -30`
Expected: tests pass; address any clippy errors (warnings optional).

- [ ] **Step 2: Write a minimal `README.md`**

```markdown
# hop

Fast full-text search and resume for coding-agent sessions (Claude Code + Codex).

## Install

    cargo install --path .

## Usage

    hop                      # open the TUI
    hop auth refresh         # pre-filled query
    hop -a claude -d api      # filter by agent + directory
    hop --rebuild             # wipe and rebuild the index

Keys: type to search · ↑↓ move · Enter resume · Tab yolo · Esc quit.

Query syntax: `agent:claude,codex`, `-agent:codex`, `dir:api`, `-dir:vendor`,
`date:today|yesterday|week|month`, `date:<2d`, `date:>1w`. Tab autocompletes.
```

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: add README with usage and query syntax"
```

---

## Self-review notes (spec coverage)

- §4 row badges → Task 12 (`agent.badge()` + `agent_color`). Vertical split layout → Task 12.
- §4 polish invariants (Esc/Ctrl+C/modal/restore) → Task 11 tests + Task 13 `ratatui::restore()` + panic hook.
- §5.1 core → Task 2. §5.2 adapters + content policy → Tasks 4/6/7. §5.3 query → Task 3.
- §5.4 index (schema, version marker, ranking, incremental, deletions) → Task 8. §5.5 engine (instant open, bg sync, debounce, channel) → Task 9.
- §5.6 tui (viewport list, keymap) → Tasks 11/12. §5.7 resume table + exec + yolo modal → Tasks 6/7 (commands), 10 (exec), 11 (modal). §5.8 config → Task 5. §5.9 cli flags → Task 13.
- §8 error handling: parse errors non-fatal (skip line / count) → Tasks 6/7/9; schema rebuild → Task 8; missing dir → adapters `is_available`/empty scan; panic restore → Task 13.
- §9 testing strategy → tests in Tasks 2,3,6,7,8,9,11,12.

**Deferred / simplified for v1 (flag to reviewer):**
- Date windows are rolling (now-relative), not calendar-accurate day boundaries.
- `dir`/`date` filters are applied as post-search filters over stored docs (agent filter is a Tantivy constraint); fine at v1 scale, revisit if perf needs it.
- Matched-term preview highlighting + auto-scroll-to-first-match (§4) is not yet implemented; the preview shows raw content. Add as a follow-up once core flow is verified.
- Theme/keybinding config keys are parsed but not yet applied (only `data_dirs` is wired).
