# Hook-Based Session Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enrich session metadata (branch, worktree, repo_url, cwd, permission mode) via hop-owned hooks instead of relying solely on vendor data.

**Architecture:** `hop meta capture` is called by provider hooks at session start/stop. It reads hook context from stdin, collects git metadata, and writes JSON sidecars to `~/.hop/meta/<agent>/<session-id>.json`. During indexing, a merge step overlays sidecar data onto adapter-parsed sessions. `hop hooks install/uninstall/status` manages hook installation per provider.

**Tech Stack:** Rust, serde_json, clap (subcommands), std::process::Command (git), existing Tantivy index

## Global Constraints

- Rust 2021 edition, existing dependency versions from Cargo.toml
- Hooks must never block or crash the user's coding session — all errors swallowed
- Sidecar writes must be atomic (write to temp, rename) to prevent partial reads
- Uninstall must be a clean inverse of install — leave no trace
- Schema version bumps to 3 (triggers automatic index rebuild)
- `permission_mode` replaces `yolo: bool` on `SessionSummary`; existing `yolo` field kept during transition but deprecated

---

### Task 1: Sidecar Types and Read/Write Module

**Files:**
- Create: `src/hooks/mod.rs`
- Create: `src/hooks/sidecar.rs`
- Modify: `src/lib.rs:1` (add `pub mod hooks;`)
- Test: inline `#[cfg(test)]` in `src/hooks/sidecar.rs`

**Interfaces:**
- Produces: `HookEvent` struct, `Sidecar` struct, `SidecarEvent` struct, `sidecar_dir() -> PathBuf`, `sidecar_path(agent, session_id) -> PathBuf`, `Sidecar::read(path) -> Option<Sidecar>`, `Sidecar::write(&self, path) -> Result<()>`, `Sidecar::append_event(&mut self, event: SidecarEvent)`

- [ ] **Step 1: Write the failing test for sidecar round-trip**

In `src/hooks/sidecar.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::AgentId;

    #[test]
    fn sidecar_roundtrip_write_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        let mut sidecar = Sidecar {
            version: 1,
            session_id: "abc-123".into(),
            agent: AgentId::Claude,
            events: vec![],
        };
        sidecar.append_event(SidecarEvent {
            event: HookEvent::Start,
            timestamp: 1719500000,
            cwd: Some("/home/user/project".into()),
            branch: Some("main".into()),
            repo_url: Some("git@github.com:user/repo.git".into()),
            worktree: None,
            permission_mode: None,
        });
        sidecar.write(&path).unwrap();
        let loaded = Sidecar::read(&path).unwrap();
        assert_eq!(loaded.session_id, "abc-123");
        assert_eq!(loaded.events.len(), 1);
        assert_eq!(loaded.events[0].branch.as_deref(), Some("main"));
    }

    #[test]
    fn sidecar_append_adds_stop_event() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        let mut sidecar = Sidecar {
            version: 1,
            session_id: "abc-123".into(),
            agent: AgentId::Claude,
            events: vec![SidecarEvent {
                event: HookEvent::Start,
                timestamp: 1719500000,
                cwd: Some("/project".into()),
                branch: Some("main".into()),
                repo_url: None,
                worktree: None,
                permission_mode: None,
            }],
        };
        sidecar.append_event(SidecarEvent {
            event: HookEvent::Stop,
            timestamp: 1719500300,
            cwd: Some("/project".into()),
            branch: Some("feature".into()),
            repo_url: None,
            worktree: None,
            permission_mode: None,
        });
        sidecar.write(&path).unwrap();
        let loaded = Sidecar::read(&path).unwrap();
        assert_eq!(loaded.events.len(), 2);
        assert_eq!(loaded.events[1].branch.as_deref(), Some("feature"));
    }

    #[test]
    fn sidecar_read_missing_file_returns_none() {
        assert!(Sidecar::read(std::path::Path::new("/nonexistent/path.json")).is_none());
    }

    #[test]
    fn sidecar_path_builds_correct_location() {
        let path = sidecar_path(AgentId::Claude, "abc-123");
        assert!(path.to_string_lossy().contains("claude"));
        assert!(path.to_string_lossy().contains("abc-123.json"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib hooks::sidecar`
Expected: FAIL — module doesn't exist yet

- [ ] **Step 3: Create the module files and implement types**

In `src/hooks/mod.rs`:
```rust
pub mod sidecar;
```

In `src/hooks/sidecar.rs`:
```rust
use crate::core::AgentId;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    Start,
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidecarEvent {
    pub event: HookEvent,
    pub timestamp: i64,
    pub cwd: Option<String>,
    pub branch: Option<String>,
    pub repo_url: Option<String>,
    pub worktree: Option<String>,
    pub permission_mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sidecar {
    pub version: u32,
    pub session_id: String,
    #[serde(serialize_with = "ser_agent", deserialize_with = "de_agent")]
    pub agent: AgentId,
    pub events: Vec<SidecarEvent>,
}

fn ser_agent<S: serde::Serializer>(agent: &AgentId, s: S) -> std::result::Result<S::Ok, S::Error> {
    s.serialize_str(agent.slug())
}

fn de_agent<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<AgentId, D::Error> {
    let s = String::deserialize(d)?;
    AgentId::from_slug(&s).ok_or_else(|| serde::de::Error::custom(format!("unknown agent: {s}")))
}

impl Sidecar {
    pub fn new(agent: AgentId, session_id: String) -> Self {
        Self {
            version: 1,
            session_id,
            agent,
            events: Vec::new(),
        }
    }

    pub fn append_event(&mut self, event: SidecarEvent) {
        self.events.push(event);
    }

    pub fn read(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(self).context("serializing sidecar")?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, &json)
            .with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, path)
            .with_context(|| format!("renaming to {}", path.display()))?;
        Ok(())
    }

    /// The last event's value for a field, preferring stop over start.
    pub fn last_branch(&self) -> Option<&str> {
        self.events.iter().rev().find_map(|e| e.branch.as_deref())
    }

    pub fn last_repo_url(&self) -> Option<&str> {
        self.events.iter().rev().find_map(|e| e.repo_url.as_deref())
    }

    pub fn last_cwd(&self) -> Option<&str> {
        self.events.iter().rev().find_map(|e| e.cwd.as_deref())
    }

    pub fn last_worktree(&self) -> Option<&str> {
        self.events.iter().rev().find_map(|e| e.worktree.as_deref())
    }
}

pub fn sidecar_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|b| b.home_dir().join(".hop").join("meta"))
        .unwrap_or_else(|| PathBuf::from(".hop/meta"))
}

pub fn sidecar_path(agent: AgentId, session_id: &str) -> PathBuf {
    sidecar_dir().join(agent.slug()).join(format!("{session_id}.json"))
}
```

Add to `src/lib.rs`:
```rust
pub mod hooks;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib hooks::sidecar`
Expected: PASS — all 4 tests pass

- [ ] **Step 5: Commit**

```bash
git add src/hooks/mod.rs src/hooks/sidecar.rs src/lib.rs
git commit -m "feat: add sidecar types and read/write for hook metadata"
```

---

### Task 2: Git Metadata Collection

**Files:**
- Create: `src/hooks/git_meta.rs`
- Modify: `src/hooks/mod.rs:1` (add `pub mod git_meta;`)
- Test: inline `#[cfg(test)]` in `src/hooks/git_meta.rs`

**Interfaces:**
- Consumes: `SidecarEvent` from Task 1
- Produces: `GitMeta` struct with `branch`, `repo_url`, `worktree` fields; `GitMeta::collect(cwd: &str) -> GitMeta`

- [ ] **Step 1: Write the failing test**

In `src/hooks/git_meta.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_from_valid_git_repo() {
        // Use the hop repo itself as the test subject
        let meta = GitMeta::collect(".");
        // We're in a git repo, so branch and repo_url should be present
        assert!(meta.branch.is_some());
        assert!(meta.repo_url.is_some());
    }

    #[test]
    fn collect_from_non_git_dir() {
        let dir = tempfile::tempdir().unwrap();
        let meta = GitMeta::collect(dir.path().to_str().unwrap());
        assert!(meta.branch.is_none());
        assert!(meta.repo_url.is_none());
        assert!(meta.worktree.is_none());
    }

    #[test]
    fn collect_from_empty_string() {
        let meta = GitMeta::collect("");
        assert!(meta.branch.is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib hooks::git_meta`
Expected: FAIL — module doesn't exist

- [ ] **Step 3: Implement git metadata collection**

In `src/hooks/git_meta.rs`:
```rust
use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct GitMeta {
    pub branch: Option<String>,
    pub repo_url: Option<String>,
    pub worktree: Option<String>,
}

impl GitMeta {
    pub fn collect(cwd: &str) -> Self {
        if cwd.is_empty() {
            return Self::default();
        }
        let branch = git_field(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])
            .filter(|b| b != "HEAD");
        let repo_url = git_field(cwd, &["remote", "get-url", "origin"]);
        let worktree = detect_worktree(cwd);
        Self {
            branch,
            repo_url,
            worktree,
        }
    }
}

fn git_field(dir: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

fn detect_worktree(dir: &str) -> Option<String> {
    let toplevel = git_field(dir, &["rev-parse", "--show-toplevel"])?;
    let common_dir = git_field(dir, &["rev-parse", "--git-common-dir"])?;
    let git_dir = git_field(dir, &["rev-parse", "--git-dir"])?;
    // If git-dir != common-dir, we're in a linked worktree
    if git_dir != common_dir {
        Some(toplevel)
    } else {
        None
    }
}
```

Add to `src/hooks/mod.rs`:
```rust
pub mod git_meta;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib hooks::git_meta`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/hooks/git_meta.rs src/hooks/mod.rs
git commit -m "feat: add git metadata collection for hooks"
```

---

### Task 3: `hop meta capture` Subcommand

**Files:**
- Create: `src/hooks/capture.rs`
- Modify: `src/hooks/mod.rs` (add `pub mod capture;`)
- Modify: `src/cli.rs` (add `Meta` subcommand to `Cli`)
- Modify: `src/main.rs` (dispatch `meta capture` before TUI launch)
- Test: inline `#[cfg(test)]` in `src/hooks/capture.rs`

**Interfaces:**
- Consumes: `Sidecar`, `SidecarEvent`, `sidecar_path()` from Task 1; `GitMeta::collect()` from Task 2
- Produces: `capture(agent: AgentId, event: HookEvent, stdin: &str) -> Result<()>`

- [ ] **Step 1: Write the failing test for stdin parsing and capture**

In `src/hooks/capture.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::AgentId;
    use crate::hooks::sidecar::{HookEvent, Sidecar};

    #[test]
    fn parse_claude_stdin() {
        let input = r#"{"session_id":"abc-123","cwd":"/home/user/project","hook_event_name":"SessionStart"}"#;
        let ctx = parse_hook_stdin(input, AgentId::Claude).unwrap();
        assert_eq!(ctx.session_id, "abc-123");
        assert_eq!(ctx.cwd, "/home/user/project");
    }

    #[test]
    fn parse_codex_stdin() {
        let input = r#"{"session_id":"def-456","cwd":"/work","hook_event_name":"SessionStart"}"#;
        let ctx = parse_hook_stdin(input, AgentId::Codex).unwrap();
        assert_eq!(ctx.session_id, "def-456");
        assert_eq!(ctx.cwd, "/work");
    }

    #[test]
    fn capture_writes_sidecar_to_dir() {
        let dir = tempfile::tempdir().unwrap();
        let sidecar_base = dir.path().to_path_buf();
        let input = r#"{"session_id":"s1","cwd":".","hook_event_name":"SessionStart"}"#;
        capture_to_dir(AgentId::Claude, HookEvent::Start, input, &sidecar_base).unwrap();
        let path = sidecar_base.join("claude").join("s1.json");
        let loaded = Sidecar::read(&path).unwrap();
        assert_eq!(loaded.session_id, "s1");
        assert_eq!(loaded.events.len(), 1);
    }

    #[test]
    fn capture_stop_appends_to_existing() {
        let dir = tempfile::tempdir().unwrap();
        let sidecar_base = dir.path().to_path_buf();
        let start_input = r#"{"session_id":"s1","cwd":".","hook_event_name":"SessionStart"}"#;
        capture_to_dir(AgentId::Claude, HookEvent::Start, start_input, &sidecar_base).unwrap();
        let stop_input = r#"{"session_id":"s1","cwd":".","hook_event_name":"Stop"}"#;
        capture_to_dir(AgentId::Claude, HookEvent::Stop, stop_input, &sidecar_base).unwrap();
        let path = sidecar_base.join("claude").join("s1.json");
        let loaded = Sidecar::read(&path).unwrap();
        assert_eq!(loaded.events.len(), 2);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib hooks::capture`
Expected: FAIL — module doesn't exist

- [ ] **Step 3: Implement capture logic**

In `src/hooks/capture.rs`:
```rust
use crate::core::AgentId;
use crate::hooks::git_meta::GitMeta;
use crate::hooks::sidecar::{HookEvent, Sidecar, SidecarEvent};
use anyhow::{Context, Result};
use std::path::Path;

pub struct HookContext {
    pub session_id: String,
    pub cwd: String,
}

pub fn parse_hook_stdin(input: &str, _agent: AgentId) -> Result<HookContext> {
    let v: serde_json::Value = serde_json::from_str(input).context("parsing hook stdin")?;
    let session_id = v["session_id"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let cwd = v["cwd"]
        .as_str()
        .unwrap_or("")
        .to_string();
    Ok(HookContext { session_id, cwd })
}

pub fn capture_to_dir(
    agent: AgentId,
    event: HookEvent,
    stdin: &str,
    sidecar_base: &Path,
) -> Result<()> {
    let ctx = parse_hook_stdin(stdin, agent)?;
    if ctx.session_id.is_empty() {
        anyhow::bail!("no session_id in hook input");
    }

    let git = GitMeta::collect(&ctx.cwd);
    let ts = jiff::Timestamp::now().as_second();
    let se = SidecarEvent {
        event,
        timestamp: ts,
        cwd: if ctx.cwd.is_empty() { None } else { Some(ctx.cwd) },
        branch: git.branch,
        repo_url: git.repo_url,
        worktree: git.worktree,
        permission_mode: None,
    };

    let path = sidecar_base
        .join(agent.slug())
        .join(format!("{}.json", ctx.session_id));

    let mut sidecar = Sidecar::read(&path)
        .unwrap_or_else(|| Sidecar::new(agent, ctx.session_id));
    sidecar.append_event(se);
    sidecar.write(&path)?;
    Ok(())
}

pub fn capture(agent: AgentId, event: HookEvent, stdin: &str) -> Result<()> {
    let base = crate::hooks::sidecar::sidecar_dir();
    capture_to_dir(agent, event, stdin, &base)
}
```

Add to `src/hooks/mod.rs`:
```rust
pub mod capture;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib hooks::capture`
Expected: PASS

- [ ] **Step 5: Wire into CLI**

Modify `src/cli.rs` — add a `Subcommand` enum alongside the existing struct. The existing flags become the default (no subcommand) behavior:

```rust
use clap::{Parser, Subcommand as ClapSubcommand};

#[derive(Parser, Debug)]
#[command(name = "hop", about = "Search and resume coding-agent sessions")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    // ... existing fields unchanged ...
}

#[derive(ClapSubcommand, Debug)]
pub enum Command {
    /// Manage session metadata hooks.
    Hooks {
        #[command(subcommand)]
        action: HooksAction,
    },
    /// Internal: capture session metadata (called by hooks).
    Meta {
        #[command(subcommand)]
        action: MetaAction,
    },
}

#[derive(ClapSubcommand, Debug)]
pub enum HooksAction {
    /// Install hooks for detected providers.
    Install {
        /// Install for all detected providers without prompting.
        #[arg(long)]
        all: bool,
        /// Install for a specific provider only.
        #[arg(long)]
        provider: Option<String>,
    },
    /// Remove all hop hooks.
    Uninstall {
        #[arg(long)]
        all: bool,
        #[arg(long)]
        provider: Option<String>,
    },
    /// Show hook installation status.
    Status,
}

#[derive(ClapSubcommand, Debug)]
pub enum MetaAction {
    /// Capture session metadata from hook stdin.
    Capture {
        /// Agent name (claude, codex, cursor).
        #[arg(long)]
        agent: String,
        /// Hook event (start, stop).
        #[arg(long)]
        event: String,
    },
}
```

Modify `src/main.rs` — dispatch subcommands before the TUI:

```rust
fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(cmd) = &cli.command {
        return match cmd {
            hop::cli::Command::Meta { action } => match action {
                hop::cli::MetaAction::Capture { agent, event } => {
                    let agent = hop::core::AgentId::from_slug(agent)
                        .ok_or_else(|| anyhow::anyhow!("unknown agent: {agent}"))?;
                    let event = match event.as_str() {
                        "start" => hop::hooks::sidecar::HookEvent::Start,
                        "stop" => hop::hooks::sidecar::HookEvent::Stop,
                        _ => anyhow::bail!("unknown event: {event}"),
                    };
                    let mut stdin = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut stdin)?;
                    hop::hooks::capture::capture(agent, event, &stdin)
                }
            },
            hop::cli::Command::Hooks { action } => {
                // Placeholder for Task 5
                match action {
                    hop::cli::HooksAction::Install { .. } => {
                        eprintln!("hop hooks install: not yet implemented");
                        Ok(())
                    }
                    hop::cli::HooksAction::Uninstall { .. } => {
                        eprintln!("hop hooks uninstall: not yet implemented");
                        Ok(())
                    }
                    hop::cli::HooksAction::Status => {
                        eprintln!("hop hooks status: not yet implemented");
                        Ok(())
                    }
                }
            }
        };
    }

    // ... rest of existing main() unchanged (TUI path) ...
}
```

- [ ] **Step 6: Run all tests to verify nothing broke**

Run: `cargo test`
Expected: PASS — all existing tests plus new ones pass

- [ ] **Step 7: Commit**

```bash
git add src/hooks/capture.rs src/hooks/mod.rs src/cli.rs src/main.rs
git commit -m "feat: add hop meta capture subcommand for hook-based metadata"
```

---

### Task 4: Sidecar Merge into SessionSummary and Index

**Files:**
- Modify: `src/core.rs:202-221` (add `worktree`, `permission_mode` to `SessionSummary`; keep `yolo` for transition)
- Modify: `src/index.rs:17` (bump `SCHEMA_VERSION` to 3)
- Modify: `src/index.rs:25-67` (add `worktree`, `permission_mode` fields to schema and `Fields`)
- Modify: `src/index.rs:108-134` (update `upsert` to write new fields)
- Modify: `src/index.rs:329-360` (update `to_summary` to read new fields)
- Modify: `src/engine.rs:211-283` (call sidecar merge after adapter parse)
- Create: `src/hooks/merge.rs` (merge logic)
- Modify: `src/hooks/mod.rs` (add `pub mod merge;`)
- Test: inline `#[cfg(test)]` in `src/hooks/merge.rs`, update existing tests in `src/engine.rs`, `src/index.rs`, `src/enrich/mod.rs`

**Interfaces:**
- Consumes: `Sidecar::read()`, `sidecar_path()` from Task 1
- Produces: `merge_sidecar(summary: &mut SessionSummary)` — mutates a session summary in place with sidecar data

- [ ] **Step 1: Write the failing test for merge logic**

In `src/hooks/merge.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{AgentId, SessionSummary};
    use crate::hooks::sidecar::{HookEvent, Sidecar, SidecarEvent};

    fn base_summary() -> SessionSummary {
        SessionSummary {
            id: "s1".into(),
            agent: AgentId::Claude,
            title: "test".into(),
            directory: "/vendor/path".into(),
            timestamp: 100,
            message_count: 5,
            yolo: false,
            branch: None,
            repo_url: None,
            source_path: None,
            archived: false,
            worktree: None,
            permission_mode: None,
        }
    }

    fn sidecar_with_events(events: Vec<SidecarEvent>) -> Sidecar {
        Sidecar {
            version: 1,
            session_id: "s1".into(),
            agent: AgentId::Claude,
            events,
        }
    }

    #[test]
    fn merge_fills_missing_branch() {
        let mut summary = base_summary();
        let sidecar = sidecar_with_events(vec![SidecarEvent {
            event: HookEvent::Start,
            timestamp: 100,
            cwd: Some("/project".into()),
            branch: Some("feature".into()),
            repo_url: None,
            worktree: None,
            permission_mode: None,
        }]);
        apply_sidecar(&mut summary, &sidecar);
        assert_eq!(summary.branch.as_deref(), Some("feature"));
    }

    #[test]
    fn merge_sidecar_wins_over_vendor_branch() {
        let mut summary = base_summary();
        summary.branch = Some("old-vendor-branch".into());
        let sidecar = sidecar_with_events(vec![
            SidecarEvent {
                event: HookEvent::Start,
                timestamp: 100,
                cwd: None,
                branch: Some("start-branch".into()),
                repo_url: None,
                worktree: None,
                permission_mode: None,
            },
            SidecarEvent {
                event: HookEvent::Stop,
                timestamp: 200,
                cwd: None,
                branch: Some("final-branch".into()),
                repo_url: None,
                worktree: None,
                permission_mode: None,
            },
        ]);
        apply_sidecar(&mut summary, &sidecar);
        assert_eq!(summary.branch.as_deref(), Some("final-branch"));
    }

    #[test]
    fn merge_fills_worktree() {
        let mut summary = base_summary();
        let sidecar = sidecar_with_events(vec![SidecarEvent {
            event: HookEvent::Start,
            timestamp: 100,
            cwd: None,
            branch: None,
            repo_url: None,
            worktree: Some("/worktrees/feature".into()),
            permission_mode: None,
        }]);
        apply_sidecar(&mut summary, &sidecar);
        assert_eq!(summary.worktree.as_deref(), Some("/worktrees/feature"));
    }

    #[test]
    fn merge_sidecar_cwd_wins() {
        let mut summary = base_summary();
        let sidecar = sidecar_with_events(vec![SidecarEvent {
            event: HookEvent::Start,
            timestamp: 100,
            cwd: Some("/sidecar/path".into()),
            branch: None,
            repo_url: None,
            worktree: None,
            permission_mode: None,
        }]);
        apply_sidecar(&mut summary, &sidecar);
        assert_eq!(summary.directory, "/sidecar/path");
    }

    #[test]
    fn merge_preserves_vendor_when_sidecar_field_is_none() {
        let mut summary = base_summary();
        summary.repo_url = Some("vendor-url".into());
        let sidecar = sidecar_with_events(vec![SidecarEvent {
            event: HookEvent::Start,
            timestamp: 100,
            cwd: None,
            branch: None,
            repo_url: None,
            worktree: None,
            permission_mode: None,
        }]);
        apply_sidecar(&mut summary, &sidecar);
        assert_eq!(summary.repo_url.as_deref(), Some("vendor-url"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib hooks::merge`
Expected: FAIL — module doesn't exist and `SessionSummary` lacks new fields

- [ ] **Step 3: Add new fields to `SessionSummary`**

Modify `src/core.rs` — add two fields after `archived`:
```rust
pub struct SessionSummary {
    // ... existing fields ...
    pub archived: bool,
    /// Git worktree path, if the session ran in one.
    pub worktree: Option<String>,
    /// Permission mode: "default", "yolo", "auto". Replaces the boolean `yolo` field.
    pub permission_mode: Option<String>,
}
```

Then fix all existing construction sites. Every place that builds a `SessionSummary` needs `worktree: None, permission_mode: None`. These sites are:
- `src/adapters/claude.rs` — in the `parse` method return
- `src/adapters/codex.rs` — in the `parse` method return
- `src/adapters/cursor.rs` — in the `parse` method return
- `src/engine.rs` — `sess_for()` test helper
- `src/enrich/mod.rs` — `sess()` test helper
- `src/tui/preview.rs` — test helper
- `tests/index_sync.rs` — test helpers
- Any other test helpers that build `SessionSummary`

Add `worktree: None, permission_mode: None` to each. For the Codex adapter, also populate `permission_mode` from the existing yolo detection: if `yolo` is true, set `permission_mode: Some("yolo".into())`, otherwise `Some("default".into())`.

- [ ] **Step 4: Implement merge logic**

In `src/hooks/merge.rs`:
```rust
use crate::core::SessionSummary;
use crate::hooks::sidecar::{sidecar_path, Sidecar};

pub fn apply_sidecar(summary: &mut SessionSummary, sidecar: &Sidecar) {
    if let Some(branch) = sidecar.last_branch() {
        summary.branch = Some(branch.to_string());
    }
    if let Some(repo_url) = sidecar.last_repo_url() {
        summary.repo_url = Some(repo_url.to_string());
    }
    if let Some(cwd) = sidecar.last_cwd() {
        summary.directory = cwd.to_string();
    }
    if let Some(worktree) = sidecar.last_worktree() {
        summary.worktree = Some(worktree.to_string());
    }
}

pub fn merge_sidecar(summary: &mut SessionSummary) {
    let path = sidecar_path(summary.agent, &summary.id);
    if let Some(sidecar) = Sidecar::read(&path) {
        apply_sidecar(summary, &sidecar);
    }
}
```

Add to `src/hooks/mod.rs`:
```rust
pub mod merge;
```

- [ ] **Step 5: Update index schema to v3**

Modify `src/index.rs`:
- Bump `SCHEMA_VERSION` to `3`
- Add `worktree: Field` and `permission_mode: Field` to `Fields` struct
- Add to `build_schema()`:
  ```rust
  worktree: b.add_text_field("worktree", STRING | STORED),
  permission_mode: b.add_text_field("permission_mode", STRING | STORED),
  ```
- Update `upsert()` to write new fields:
  ```rust
  if let Some(w) = &m.worktree {
      doc.add_text(self.f.worktree, w);
  }
  if let Some(pm) = &m.permission_mode {
      doc.add_text(self.f.permission_mode, pm);
  }
  ```
- Update `to_summary()` to read new fields:
  ```rust
  worktree: {
      let w = get_str(self.f.worktree);
      if w.is_empty() { None } else { Some(w) }
  },
  permission_mode: {
      let pm = get_str(self.f.permission_mode);
      if pm.is_empty() { None } else { Some(pm) }
  },
  ```

- [ ] **Step 6: Wire merge into the sync pipeline**

Modify `src/engine.rs` in `sync_index()`, after a session is successfully parsed (around line 257), call merge:

```rust
Ok(mut s) => {
    s.mtime = entry.mtime;
    if s.meta.source_path.is_none() {
        s.meta.source_path = Some(entry.path.clone());
    }
    hop::hooks::merge::merge_sidecar(&mut s.meta);
    // ... rest unchanged ...
}
```

- [ ] **Step 7: Run all tests**

Run: `cargo test`
Expected: PASS — all tests pass with new fields defaulting to `None`

- [ ] **Step 8: Commit**

```bash
git add src/core.rs src/hooks/merge.rs src/hooks/mod.rs src/index.rs src/engine.rs src/adapters/ src/enrich/ src/tui/ tests/
git commit -m "feat: merge sidecar metadata into session index, add worktree and permission_mode fields"
```

---

### Task 5: Provider Detection and Hook Installation/Uninstall

**Files:**
- Create: `src/hooks/providers.rs`
- Modify: `src/hooks/mod.rs` (add `pub mod providers;`)
- Modify: `src/main.rs` (wire `hop hooks install/uninstall/status` dispatch)
- Test: inline `#[cfg(test)]` in `src/hooks/providers.rs`

**Interfaces:**
- Consumes: `AgentId` from `core.rs`
- Produces: `ProviderStatus` struct, `detect_providers() -> Vec<ProviderStatus>`, `install_hook(agent, dry_run) -> Result<String>`, `uninstall_hook(agent) -> Result<String>`, `is_installed(agent) -> bool`

- [ ] **Step 1: Write the failing test for provider detection**

In `src/hooks/providers.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_hook_json_generation() {
        let json = claude_hook_entry("start");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], "hop-meta");
        assert!(parsed["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("hop meta capture"));
    }

    #[test]
    fn claude_settings_merge_preserves_existing() {
        let existing = r#"{
            "hooks": {
                "PreToolUse": [{"hooks": [{"type": "command", "command": "echo hi"}]}]
            }
        }"#;
        let merged = merge_claude_hooks(existing).unwrap();
        let v: serde_json::Value = serde_json::from_str(&merged).unwrap();
        // Existing hooks preserved
        assert!(!v["hooks"]["PreToolUse"].is_null());
        // Hop hooks added
        assert!(!v["hooks"]["SessionStart"].is_null());
        assert!(!v["hooks"]["SessionEnd"].is_null());
    }

    #[test]
    fn claude_settings_unmerge_removes_only_hop() {
        let with_hop = r#"{
            "hooks": {
                "PreToolUse": [{"hooks": [{"type": "command", "command": "echo hi"}]}],
                "SessionStart": [{"id": "hop-meta", "hooks": [{"type": "command", "command": "hop meta capture --agent claude --event start"}]}],
                "SessionEnd": [{"id": "hop-meta", "hooks": [{"type": "command", "command": "hop meta capture --agent claude --event stop"}]}]
            }
        }"#;
        let cleaned = unmerge_claude_hooks(with_hop).unwrap();
        let v: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
        assert!(!v["hooks"]["PreToolUse"].is_null());
        assert!(v["hooks"]["SessionStart"].is_null() || v["hooks"]["SessionStart"].as_array().unwrap().is_empty());
    }

    #[test]
    fn codex_plugin_hooks_json() {
        let json = codex_hooks_json();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(!v["hooks"]["SessionStart"].is_null());
        assert!(!v["hooks"]["Stop"].is_null());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib hooks::providers`
Expected: FAIL — module doesn't exist

- [ ] **Step 3: Implement provider detection and hook management**

In `src/hooks/providers.rs`:
```rust
use crate::core::AgentId;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

const HOP_HOOK_ID: &str = "hop-meta";

#[derive(Debug)]
pub struct ProviderStatus {
    pub agent: AgentId,
    pub detected: bool,
    pub installed: bool,
    pub config_path: PathBuf,
    pub best_effort: bool,
}

pub fn home_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn detect_providers() -> Vec<ProviderStatus> {
    let home = home_dir();
    vec![
        detect_claude(&home),
        detect_codex(&home),
        detect_cursor(&home),
    ]
}

fn detect_claude(home: &Path) -> ProviderStatus {
    let config_path = home.join(".claude").join("settings.json");
    let detected = home.join(".claude").exists();
    let installed = detected && is_claude_installed(&config_path);
    ProviderStatus {
        agent: AgentId::Claude,
        detected,
        installed,
        config_path,
        best_effort: false,
    }
}

fn detect_codex(home: &Path) -> ProviderStatus {
    let plugin_dir = home.join(".codex").join(".tmp").join("plugins").join("plugins").join("hop");
    let config_path = plugin_dir.join("hooks.json");
    let detected = home.join(".codex").exists();
    let installed = config_path.exists();
    ProviderStatus {
        agent: AgentId::Codex,
        detected,
        installed,
        config_path,
        best_effort: false,
    }
}

fn detect_cursor(home: &Path) -> ProviderStatus {
    let config_path = home.join(".cursor").join("hooks.json");
    let detected = home.join(".cursor").exists();
    let installed = detected && is_cursor_installed(&config_path);
    ProviderStatus {
        agent: AgentId::Cursor,
        detected,
        installed,
        config_path,
        best_effort: true,
    }
}

fn is_claude_installed(path: &Path) -> bool {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return false,
    };
    text.contains(HOP_HOOK_ID)
}

fn is_cursor_installed(path: &Path) -> bool {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return false,
    };
    text.contains("hop meta capture")
}

// --- Claude ---

pub fn claude_hook_entry(event: &str) -> String {
    let cli_event = match event {
        "SessionStart" => "start",
        "SessionEnd" => "stop",
        _ => event,
    };
    format!(
        r#"{{"id":"{HOP_HOOK_ID}","hooks":[{{"type":"command","command":"hop meta capture --agent claude --event {cli_event}"}}]}}"#
    )
}

pub fn merge_claude_hooks(existing_json: &str) -> Result<String> {
    let mut v: serde_json::Value =
        serde_json::from_str(existing_json).context("parsing settings.json")?;
    let hooks = v
        .as_object_mut()
        .context("settings.json is not an object")?
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));
    let hooks_obj = hooks
        .as_object_mut()
        .context("hooks is not an object")?;

    let start_entry: serde_json::Value = serde_json::from_str(&claude_hook_entry("SessionStart"))?;
    let end_entry: serde_json::Value = serde_json::from_str(&claude_hook_entry("SessionEnd"))?;

    for (event_name, entry) in [("SessionStart", start_entry), ("SessionEnd", end_entry)] {
        let arr = hooks_obj
            .entry(event_name)
            .or_insert_with(|| serde_json::json!([]));
        let arr = arr.as_array_mut().context("hook event is not an array")?;
        arr.retain(|e| e.get("id").and_then(|i| i.as_str()) != Some(HOP_HOOK_ID));
        arr.push(entry);
    }
    serde_json::to_string_pretty(&v).context("serializing settings.json")
}

pub fn unmerge_claude_hooks(existing_json: &str) -> Result<String> {
    let mut v: serde_json::Value = serde_json::from_str(existing_json).context("parsing")?;
    if let Some(hooks) = v.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        for (_event, arr) in hooks.iter_mut() {
            if let Some(a) = arr.as_array_mut() {
                a.retain(|e| e.get("id").and_then(|i| i.as_str()) != Some(HOP_HOOK_ID));
            }
        }
        // Remove empty arrays
        let empty_keys: Vec<String> = hooks
            .iter()
            .filter(|(_, v)| v.as_array().map_or(false, |a| a.is_empty()))
            .map(|(k, _)| k.clone())
            .collect();
        for k in empty_keys {
            hooks.remove(&k);
        }
    }
    serde_json::to_string_pretty(&v).context("serializing")
}

pub fn install_claude(home: &Path) -> Result<String> {
    let path = home.join(".claude").join("settings.json");
    let existing = if path.exists() {
        std::fs::read_to_string(&path).context("reading settings.json")?
    } else {
        "{}".to_string()
    };
    let merged = merge_claude_hooks(&existing)?;
    std::fs::write(&path, &merged).context("writing settings.json")?;
    Ok(format!(
        "Claude Code: added SessionStart and SessionEnd hooks to {}",
        path.display()
    ))
}

pub fn uninstall_claude(home: &Path) -> Result<String> {
    let path = home.join(".claude").join("settings.json");
    if !path.exists() {
        return Ok("Claude Code: no settings.json found, nothing to remove".into());
    }
    let existing = std::fs::read_to_string(&path)?;
    let cleaned = unmerge_claude_hooks(&existing)?;
    std::fs::write(&path, &cleaned)?;
    Ok(format!("Claude Code: removed hop hooks from {}", path.display()))
}

// --- Codex ---

pub fn codex_hooks_json() -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "hooks": {
            "SessionStart": [{"id": HOP_HOOK_ID, "hooks": [{"type": "command", "command": "hop meta capture --agent codex --event start"}]}],
            "Stop": [{"id": HOP_HOOK_ID, "hooks": [{"type": "command", "command": "hop meta capture --agent codex --event stop"}]}]
        }
    }))
    .unwrap()
}

pub fn install_codex(home: &Path) -> Result<String> {
    let plugin_dir = home
        .join(".codex")
        .join(".tmp")
        .join("plugins")
        .join("plugins")
        .join("hop");
    std::fs::create_dir_all(&plugin_dir)?;
    let hooks_path = plugin_dir.join("hooks.json");
    std::fs::write(&hooks_path, codex_hooks_json())?;
    Ok(format!("Codex: installed hop plugin at {}", plugin_dir.display()))
}

pub fn uninstall_codex(home: &Path) -> Result<String> {
    let plugin_dir = home
        .join(".codex")
        .join(".tmp")
        .join("plugins")
        .join("plugins")
        .join("hop");
    if plugin_dir.exists() {
        std::fs::remove_dir_all(&plugin_dir)?;
        Ok(format!("Codex: removed hop plugin from {}", plugin_dir.display()))
    } else {
        Ok("Codex: no hop plugin found, nothing to remove".into())
    }
}

// --- Cursor ---

pub fn install_cursor(home: &Path) -> Result<String> {
    let path = home.join(".cursor").join("hooks.json");
    let existing = if path.exists() {
        std::fs::read_to_string(&path).context("reading hooks.json")?
    } else {
        r#"{"hooks":{},"version":1}"#.to_string()
    };
    let mut v: serde_json::Value = serde_json::from_str(&existing)?;
    let hooks = v
        .as_object_mut()
        .context("not an object")?
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));
    let hooks_obj = hooks.as_object_mut().context("hooks not object")?;
    let stop_arr = hooks_obj
        .entry("stop")
        .or_insert_with(|| serde_json::json!([]));
    let arr = stop_arr.as_array_mut().context("stop not array")?;
    arr.retain(|e| {
        e.get("command")
            .and_then(|c| c.as_str())
            .map_or(true, |c| !c.contains("hop meta capture"))
    });
    arr.push(serde_json::json!({"command": "hop meta capture --agent cursor --event stop"}));
    let json = serde_json::to_string_pretty(&v)?;
    std::fs::write(&path, &json)?;
    Ok(format!(
        "Cursor: added stop hook to {} [best-effort]",
        path.display()
    ))
}

pub fn uninstall_cursor(home: &Path) -> Result<String> {
    let path = home.join(".cursor").join("hooks.json");
    if !path.exists() {
        return Ok("Cursor: no hooks.json found, nothing to remove".into());
    }
    let existing = std::fs::read_to_string(&path)?;
    let mut v: serde_json::Value = serde_json::from_str(&existing)?;
    if let Some(hooks) = v.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        if let Some(stop) = hooks.get_mut("stop").and_then(|s| s.as_array_mut()) {
            stop.retain(|e| {
                e.get("command")
                    .and_then(|c| c.as_str())
                    .map_or(true, |c| !c.contains("hop meta capture"))
            });
        }
    }
    let json = serde_json::to_string_pretty(&v)?;
    std::fs::write(&path, &json)?;
    Ok(format!("Cursor: removed hop hooks from {}", path.display()))
}

pub fn install_provider(agent: AgentId, home: &Path) -> Result<String> {
    match agent {
        AgentId::Claude => install_claude(home),
        AgentId::Codex => install_codex(home),
        AgentId::Cursor => install_cursor(home),
    }
}

pub fn uninstall_provider(agent: AgentId, home: &Path) -> Result<String> {
    match agent {
        AgentId::Claude => uninstall_claude(home),
        AgentId::Codex => uninstall_codex(home),
        AgentId::Cursor => uninstall_cursor(home),
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib hooks::providers`
Expected: PASS

- [ ] **Step 5: Wire `hop hooks` subcommands into main**

Replace the placeholder `Command::Hooks` dispatch in `src/main.rs` with real implementations that call the provider functions. The `install` command (without `--all` or `--provider`) should detect providers and print an interactive menu using stdin. For the first iteration, `--all` and `--provider` flags are sufficient; interactive picker can be added later.

```rust
hop::cli::Command::Hooks { action } => {
    let home = hop::hooks::providers::home_dir();
    match action {
        hop::cli::HooksAction::Install { all, provider } => {
            let providers = hop::hooks::providers::detect_providers();
            let targets: Vec<_> = if let Some(name) = provider {
                let agent = hop::core::AgentId::from_slug(name)
                    .ok_or_else(|| anyhow::anyhow!("unknown provider: {name}"))?;
                providers.into_iter().filter(|p| p.agent == agent).collect()
            } else if *all {
                providers.into_iter().filter(|p| p.detected).collect()
            } else {
                // Interactive: show detected, ask user
                let detected: Vec<_> = providers.into_iter().filter(|p| p.detected).collect();
                if detected.is_empty() {
                    eprintln!("No providers detected.");
                    return Ok(());
                }
                eprintln!("Detected providers:");
                for (i, p) in detected.iter().enumerate() {
                    let effort = if p.best_effort { " [best-effort]" } else { "" };
                    let status = if p.installed { " (already installed)" } else { "" };
                    eprintln!("  {}. {}{}{}", i + 1, p.agent.badge(), effort, status);
                }
                eprint!("Install for all? [Y/n] ");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if input.trim().eq_ignore_ascii_case("n") {
                    return Ok(());
                }
                detected
            };
            for p in &targets {
                match hop::hooks::providers::install_provider(p.agent, &home) {
                    Ok(msg) => eprintln!("{msg}"),
                    Err(e) => eprintln!("Failed to install for {}: {e}", p.agent.badge()),
                }
            }
            Ok(())
        }
        hop::cli::HooksAction::Uninstall { all, provider } => {
            let home = hop::hooks::providers::home_dir();
            let providers = hop::hooks::providers::detect_providers();
            let targets: Vec<_> = if let Some(name) = provider {
                let agent = hop::core::AgentId::from_slug(name)
                    .ok_or_else(|| anyhow::anyhow!("unknown provider: {name}"))?;
                providers.into_iter().filter(|p| p.agent == agent).collect()
            } else {
                providers.into_iter().filter(|p| p.installed).collect()
            };
            for p in &targets {
                match hop::hooks::providers::uninstall_provider(p.agent, &home) {
                    Ok(msg) => eprintln!("{msg}"),
                    Err(e) => eprintln!("Failed to uninstall for {}: {e}", p.agent.badge()),
                }
            }
            Ok(())
        }
        hop::cli::HooksAction::Status => {
            let providers = hop::hooks::providers::detect_providers();
            for p in &providers {
                let detected = if p.detected { "detected" } else { "not found" };
                let installed = if p.installed { "installed" } else { "not installed" };
                let effort = if p.best_effort { " [best-effort]" } else { "" };
                eprintln!("{}: {} / {}{}", p.agent.badge(), detected, installed, effort);
            }
            Ok(())
        }
    }
}
```

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: PASS

- [ ] **Step 7: Manual smoke test**

Run: `cargo run -- hooks status`
Expected: Prints detection and installation status for each provider

- [ ] **Step 8: Commit**

```bash
git add src/hooks/providers.rs src/hooks/mod.rs src/main.rs
git commit -m "feat: add hop hooks install/uninstall/status for all providers"
```

---

### Task 6: Cursor Index-Time Enrichment

**Files:**
- Modify: `src/adapters/cursor.rs` (add git metadata collection at parse time when no sidecar exists)
- Modify: `src/hooks/merge.rs` (add cursor-specific enrichment fallback)
- Test: update existing Cursor adapter tests in `tests/cursor_adapter.rs`

**Interfaces:**
- Consumes: `GitMeta::collect()` from Task 2, `sidecar_path()` from Task 1
- Produces: Cursor sessions now have `branch` and `repo_url` populated at parse time

- [ ] **Step 1: Write the failing test**

Add to `tests/cursor_adapter.rs` a test that verifies branch is populated when parsing a session in a git repo. This requires constructing a test fixture inside a temp git repo.

Alternatively, add a unit test in `src/hooks/merge.rs`:
```rust
#[test]
fn cursor_enrichment_fills_branch_at_index_time() {
    let mut summary = base_summary();
    summary.agent = AgentId::Cursor;
    summary.branch = None;
    summary.directory = ".".into(); // current dir is a git repo
    enrich_from_git_if_needed(&mut summary);
    assert!(summary.branch.is_some());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib hooks::merge::tests::cursor_enrichment`
Expected: FAIL — function doesn't exist

- [ ] **Step 3: Implement index-time git enrichment**

In `src/hooks/merge.rs`, add a function that runs after sidecar merge for sessions that still have no branch:

```rust
use crate::hooks::git_meta::GitMeta;

pub fn enrich_from_git_if_needed(summary: &mut SessionSummary) {
    if summary.branch.is_some() && summary.repo_url.is_some() {
        return;
    }
    if summary.directory.is_empty() {
        return;
    }
    let git = GitMeta::collect(&summary.directory);
    if summary.branch.is_none() {
        summary.branch = git.branch;
    }
    if summary.repo_url.is_none() {
        summary.repo_url = git.repo_url;
    }
    if summary.worktree.is_none() {
        summary.worktree = git.worktree;
    }
}
```

Update `merge_sidecar` to also call this:
```rust
pub fn merge_sidecar(summary: &mut SessionSummary) {
    let path = sidecar_path(summary.agent, &summary.id);
    if let Some(sidecar) = Sidecar::read(&path) {
        apply_sidecar(summary, &sidecar);
    }
    enrich_from_git_if_needed(summary);
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib hooks::merge`
Expected: PASS

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/hooks/merge.rs
git commit -m "feat: add index-time git enrichment for sessions without sidecar data"
```

---

### Task 7: Update Existing Adapter Tests and Integration Tests

**Files:**
- Modify: `tests/index_sync.rs` (add `worktree: None, permission_mode: None` to test helpers)
- Modify: `tests/claude_adapter.rs` (verify existing tests pass with new fields)
- Modify: `tests/codex_adapter.rs` (verify existing tests pass with new fields)
- Modify: `tests/cursor_adapter.rs` (verify existing tests pass with new fields)

**Interfaces:**
- Consumes: All changes from Tasks 1-6

This task is primarily about ensuring all existing tests compile and pass with the new `SessionSummary` fields. Most of the work was done in Task 4 step 3, but integration tests in `tests/` may need updating too.

- [ ] **Step 1: Run the full test suite**

Run: `cargo test`
Expected: If any tests fail, identify which ones need the new fields added

- [ ] **Step 2: Fix any remaining test compilation errors**

Add `worktree: None, permission_mode: None` to any `SessionSummary` construction in integration tests that wasn't caught in Task 4.

- [ ] **Step 3: Run full test suite again**

Run: `cargo test`
Expected: PASS — all tests green

- [ ] **Step 4: Commit if any changes were needed**

```bash
git add tests/
git commit -m "test: update integration tests for new SessionSummary fields"
```
