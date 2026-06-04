# hop TUI v2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace hop's bare-bones TUI with a readable clean-transcript preview, a columnar result list backed by a pluggable column/enrichment architecture (including git branch from conversation data and background GitHub PR lookup), and a richer, configurable keymap with a help overlay.

**Architecture:** A single shared per-adapter extractor turns a session file into `Vec<Message>` (roles + prose/code blocks) with all internals filtered once; the index stores a flattened string for search, while the preview re-parses the selected file on demand and renders it. A `columns` layer renders an aligned grid; an `enrich` layer (Fast in-line providers for branch/repo, a Slow background+cached provider for GitHub PR) feeds the cells. A config-selectable keymap (default "search", opt-in "modal") drives all actions.

**Tech Stack:** Rust, ratatui 0.30 + crossterm, tantivy 0.26, simd-json, jiff, clap, `syntect` (code highlighting), `pulldown-cmark` (prose markdown). Background work via std threads + mpsc channels (no async runtime).

**Spec:** `docs/specs/2026-06-04-hop-tui-v2-design.md`

---

## File structure

**Create:**
- `src/enrich/mod.rs` — `Enricher` trait, `EnrichKind`, `EnrichValue`, registry, fast enrichers (branch, repo).
- `src/enrich/gh_pr.rs` — GitHub PR slow enricher + slug parsing.
- `src/enrich/service.rs` — `EnrichmentService` (worker thread + disk cache + channels).
- `src/columns.rs` — `Cell`, `Column`, default column set, responsive width/drop solver.
- `src/tui/keymap.rs` — keymap presets + binding resolution.
- `src/tui/preview.rs` — transcript renderer (syntect + markdown + match highlight/scroll).
- `src/tui/results_list.rs` — column-grid list renderer.
- `src/tui/help.rs` — help overlay renderer.

**Modify:**
- `src/core.rs` — `Role`/`Block`/`Message`, `split_blocks`, `flatten_messages`, `Session.branch`/`repo_url`.
- `src/adapters/mod.rs` — `Adapter::transcript`.
- `src/adapters/claude.rs`, `src/adapters/codex.rs` — shared `extract`, branch/repo capture, `transcript`.
- `src/index.rs` — schema gains `branch`/`repo_url`, `SCHEMA_VERSION` bump.
- `src/config.rs` — `[preview]`, `keymap`, `[columns]`, enricher toggles.
- `src/tui/mod.rs` — App state (preview, keymap, enrichment maps), keymap-driven dispatch.
- `src/tui/view.rs` — split/hidden layout, resize, help overlay, columns, preview wiring.
- `src/lib.rs` — `pub mod enrich; pub mod columns;`.
- `src/main.rs` — spawn enrichment service, on-selection transcript parse, fold enrichment results.
- `Cargo.toml` — `syntect`, `pulldown-cmark`.
- `README.md` — keys, columns, config.

---

## Task 0: Add dependencies

**Files:**
- Modify: `Cargo.toml:16-27`

- [ ] **Step 1: Add the new dependencies**

In `[dependencies]` add:

```toml
syntect = { version = "5.2", default-features = false, features = ["default-syntaxes", "default-themes", "regex-onig"] }
pulldown-cmark = { version = "0.12", default-features = false }
```

- [ ] **Step 2: Verify it resolves and builds**

Run: `cargo build`
Expected: builds (downloads syntect/pulldown-cmark) with no errors.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "build: add syntect and pulldown-cmark"
```

---

## Task 1: Core message model + block/flatten helpers

**Files:**
- Modify: `src/core.rs`

- [ ] **Step 1: Write failing tests**

Add to the `tests` module in `src/core.rs`:

```rust
#[test]
fn split_blocks_separates_fenced_code() {
    let input = "before\n```rust\nfn x() {}\n```\nafter";
    let blocks = split_blocks(input);
    assert_eq!(blocks, vec![
        Block::Prose("before".into()),
        Block::Code { lang: Some("rust".into()), text: "fn x() {}".into() },
        Block::Prose("after".into()),
    ]);
}

#[test]
fn split_blocks_plain_prose_is_single_block() {
    assert_eq!(split_blocks("just text"), vec![Block::Prose("just text".into())]);
}

#[test]
fn split_blocks_unlabeled_fence_has_no_lang() {
    let blocks = split_blocks("```\nraw\n```");
    assert_eq!(blocks, vec![Block::Code { lang: None, text: "raw".into() }]);
}

#[test]
fn flatten_messages_joins_prose_and_code() {
    let msgs = vec![
        Message { role: Role::User, blocks: vec![Block::Prose("hi".into())] },
        Message { role: Role::Agent, blocks: vec![
            Block::Prose("fixed".into()),
            Block::Code { lang: Some("rust".into()), text: "let x=1;".into() },
        ]},
    ];
    assert_eq!(flatten_messages(&msgs), "hi\nfixed\nlet x=1;");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib core::tests`
Expected: FAIL — `Block`, `Message`, `Role`, `split_blocks`, `flatten_messages` not found.

- [ ] **Step 3: Add the types and helpers**

Add near the top of `src/core.rs` (after the `use` line):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Prose(String),
    Code { lang: Option<String>, text: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub blocks: Vec<Block>,
}

/// Split a text body into prose and fenced-code blocks. ```` ```lang ```` opens a
/// code block; a line that is exactly ``` (after trim) closes it. Empty prose runs
/// are dropped. Trailing/leading blank lines within prose are trimmed.
pub fn split_blocks(text: &str) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let mut prose: Vec<&str> = Vec::new();
    let mut code: Vec<&str> = Vec::new();
    let mut lang: Option<String> = None;
    let mut in_code = false;

    let flush_prose = |prose: &mut Vec<&str>, out: &mut Vec<Block>| {
        let joined = prose.join("\n");
        let trimmed = joined.trim();
        if !trimmed.is_empty() {
            out.push(Block::Prose(trimmed.to_string()));
        }
        prose.clear();
    };

    for line in text.lines() {
        let t = line.trim_end();
        if let Some(rest) = t.trim_start().strip_prefix("```") {
            if in_code {
                out.push(Block::Code { lang: lang.take(), text: code.join("\n") });
                code.clear();
                in_code = false;
            } else {
                flush_prose(&mut prose, &mut out);
                let l = rest.trim();
                lang = if l.is_empty() { None } else { Some(l.to_string()) };
                in_code = true;
            }
            continue;
        }
        if in_code {
            code.push(line);
        } else {
            prose.push(line);
        }
    }
    if in_code {
        // unterminated fence: keep what we have as code
        out.push(Block::Code { lang: lang.take(), text: code.join("\n") });
    } else {
        flush_prose(&mut prose, &mut out);
    }
    out
}

/// Flatten messages into a single newline-joined string for the search index.
pub fn flatten_messages(msgs: &[Message]) -> String {
    let mut out = String::new();
    for m in msgs {
        for b in &m.blocks {
            let t = match b {
                Block::Prose(s) => s.trim(),
                Block::Code { text, .. } => text.trim(),
            };
            if t.is_empty() {
                continue;
            }
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(t);
        }
    }
    out
}
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test --lib core::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/core.rs
git commit -m "feat(core): add Message/Block model + split_blocks/flatten helpers"
```

---

## Task 2: Add branch/repo_url to Session and `transcript` to the Adapter trait

This task adds the fields and trait method and fixes every `Session { .. }` literal and adapter impl so the crate compiles. It does not yet populate branch/repo from real data (Tasks 3–4).

**Files:**
- Modify: `src/core.rs` (Session struct), `src/adapters/mod.rs` (trait), `src/adapters/claude.rs`, `src/adapters/codex.rs`, `src/engine.rs` (FakeAdapter + `sess`), `src/index.rs` (`to_session`), `src/tui/mod.rs` (test `sess`), `src/tui/view.rs` (test literal)

- [ ] **Step 1: Add the fields to `Session`**

In `src/core.rs`, in `struct Session`, after `pub yolo: bool,` add:

```rust
    /// Git branch at session time, captured from conversation data when present.
    pub branch: Option<String>,
    /// Git remote URL when the agent records it (Codex). None otherwise.
    pub repo_url: Option<String>,
```

- [ ] **Step 2: Add `transcript` to the trait**

In `src/adapters/mod.rs`, add to `trait Adapter` (after `parse`):

```rust
    /// Re-parse a session file into structured, internals-filtered messages for
    /// the preview. Shares the same extractor as `parse`.
    fn transcript(&self, path: &Path) -> Result<Vec<crate::core::Message>>;
```

Add `use crate::core::Message;`? No — fully-qualified above keeps imports minimal; leave as written.

- [ ] **Step 3: Make the crate compile again — fix every literal and impl**

Run: `rg -n "Session \{" src` to list every literal.

In **`src/index.rs`** `to_session`, add the two fields to the returned `Session`:

```rust
            branch: { let b = get_str(self.f.branch); if b.is_empty() { None } else { Some(b) } },
            repo_url: { let r = get_str(self.f.repo_url); if r.is_empty() { None } else { Some(r) } },
```

(These reference `self.f.branch`/`self.f.repo_url`, added in Task 5. To keep this task self-contained and compiling, temporarily use `branch: None, repo_url: None,` here and replace in Task 5 Step 3.)

So for **this** task, in `src/index.rs` `to_session` add:

```rust
            branch: None,
            repo_url: None,
```

In **`src/claude.rs`** `parse`'s returned `Session`, add:

```rust
            branch: None,
            repo_url: None,
```

and add a temporary `transcript` impl to `impl Adapter for ClaudeAdapter` (replaced in Task 3):

```rust
    fn transcript(&self, _path: &Path) -> Result<Vec<crate::core::Message>> {
        Ok(Vec::new())
    }
```

In **`src/codex.rs`** `parse`'s returned `Session`, add `branch: None, repo_url: None,` and the same temporary `transcript` impl.

In **`src/engine.rs`** test helper `sess`, add `branch: None, repo_url: None,` to the literal, and add to `FakeAdapter`:

```rust
        fn transcript(&self, _path: &Path) -> anyhow::Result<Vec<crate::core::Message>> {
            Ok(Vec::new())
        }
```

In **`src/tui/mod.rs`** test helper `sess`, add `branch: None, repo_url: None,`.

In **`src/tui/view.rs`** test `renders_badge_and_title`, add `branch: None, repo_url: None,` to the `Session` literal.

- [ ] **Step 4: Verify the whole crate compiles and tests pass**

Run: `cargo test`
Expected: PASS (no behavior change yet).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add Session.branch/repo_url and Adapter::transcript (stubs)"
```

---

## Task 3: Claude adapter — shared extractor, branch capture, transcript

**Files:**
- Modify: `src/adapters/claude.rs`
- Test: `tests/claude_adapter.rs`

- [ ] **Step 1: Write failing tests**

Append to `tests/claude_adapter.rs` (adjust the existing `use`/setup to match the file; the adapter is constructed as in existing tests):

```rust
#[test]
fn claude_captures_branch_and_filters_internals() {
    use hop::adapters::claude::ClaudeAdapter;
    use hop::adapters::Adapter;
    use std::fs;

    let tmp = tempfile::tempdir().unwrap();
    let proj = tmp.path().join("proj");
    fs::create_dir_all(&proj).unwrap();
    let file = proj.join("s.jsonl");
    fs::write(&file, concat!(
        r#"{"type":"user","cwd":"/w","gitBranch":"feat/x","timestamp":"2026-06-04T13:20:16.361Z","message":{"role":"user","content":"fix the bug"}}"#, "\n",
        r#"{"type":"user","message":{"role":"user","content":"<command-name>/clear</command-name>"}}"#, "\n",
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"done"},{"type":"tool_use","name":"Bash","id":"t"}]}}"#, "\n",
        r#"{"type":"user","isMeta":true,"message":{"role":"user","content":"meta"}}"#, "\n",
    )).unwrap();

    let a = ClaudeAdapter::new(tmp.path().to_path_buf());
    let s = a.parse(&file).unwrap();
    assert_eq!(s.branch.as_deref(), Some("feat/x"));
    assert!(s.content.contains("fix the bug"));
    assert!(s.content.contains("done"));
    assert!(!s.content.contains("/clear"));
    assert!(!s.content.contains("meta"));
}

#[test]
fn claude_transcript_has_roles_and_code() {
    use hop::adapters::claude::ClaudeAdapter;
    use hop::adapters::Adapter;
    use hop::core::{Block, Role};
    use std::fs;

    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("s.jsonl");
    fs::write(&file, concat!(
        r#"{"type":"user","cwd":"/w","message":{"role":"user","content":"hi"}}"#, "\n",
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"text\n```rust\nfn a(){}\n```"}]}}"#, "\n",
    )).unwrap();

    let a = ClaudeAdapter::new(tmp.path().to_path_buf());
    let msgs = a.transcript(&file).unwrap();
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].role, Role::User);
    assert_eq!(msgs[1].role, Role::Agent);
    assert!(matches!(msgs[1].blocks.last(), Some(Block::Code { lang, .. }) if lang.as_deref() == Some("rust")));
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --test claude_adapter`
Expected: FAIL — branch is `None`, transcript returns empty.

- [ ] **Step 3: Refactor the adapter onto a shared extractor**

In `src/adapters/claude.rs`:

Add `gitBranch` to the `Line` struct:

```rust
    #[serde(rename = "gitBranch")]
    git_branch: Option<String>,
```

Add an `Extracted` struct and an `extract` method, and rewrite `parse`/`transcript` on top of it. Replace the body of `parse` and the stub `transcript` with:

```rust
struct Extracted {
    messages: Vec<crate::core::Message>,
    directory: String,
    branch: Option<String>,
    first_ts: Option<i64>,
}

impl ClaudeAdapter {
    fn extract(&self, path: &Path) -> Result<Extracted> {
        use crate::core::{split_blocks, Message, Role};
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        let mut directory = String::new();
        let mut branch: Option<String> = None;
        let mut first_ts: Option<i64> = None;
        let mut messages: Vec<Message> = Vec::new();

        for line in raw.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let mut buf = line.as_bytes().to_vec();
            let parsed: Line = match simd_json::serde::from_slice(&mut buf) {
                Ok(l) => l,
                Err(_) => continue,
            };
            if directory.is_empty() {
                if let Some(cwd) = &parsed.cwd {
                    directory = cwd.clone();
                }
            }
            if branch.is_none() {
                if let Some(b) = parsed.git_branch.as_deref() {
                    if !b.trim().is_empty() {
                        branch = Some(b.to_string());
                    }
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
            let blocks = split_blocks(&text);
            if blocks.is_empty() {
                continue;
            }
            messages.push(Message {
                role: if is_user { Role::User } else { Role::Agent },
                blocks,
            });
        }
        Ok(Extracted { messages, directory, branch, first_ts })
    }
}
```

Now rewrite `parse` to use it:

```rust
    fn parse(&self, path: &Path) -> Result<Session> {
        use crate::core::{flatten_messages, Role};
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .context("session file has no stem")?
            .to_string();
        let ex = self.extract(path)?;
        let title = ex
            .messages
            .iter()
            .find(|m| m.role == Role::User)
            .and_then(|m| m.blocks.iter().find_map(|b| match b {
                crate::core::Block::Prose(s) => Some(s.as_str()),
                _ => None,
            }))
            .map(|t| truncate_title(t, TITLE_MAX))
            .unwrap_or_else(|| "(untitled)".to_string());
        let content = flatten_messages(&ex.messages);
        Ok(Session {
            id,
            agent: AgentId::Claude,
            title,
            directory: ex.directory,
            timestamp: ex.first_ts.unwrap_or(0),
            content,
            message_count: ex.messages.len() as u32,
            mtime: 0,
            yolo: false,
            branch: ex.branch,
            repo_url: None,
        })
    }

    fn transcript(&self, path: &Path) -> Result<Vec<crate::core::Message>> {
        Ok(self.extract(path)?.messages)
    }
```

- [ ] **Step 4: Run to verify pass (and no regressions)**

Run: `cargo test --test claude_adapter && cargo test --lib`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/adapters/claude.rs tests/claude_adapter.rs
git commit -m "feat(claude): shared extractor, branch capture, structured transcript"
```

---

## Task 4: Codex adapter — shared extractor, branch + repo_url, transcript

**Files:**
- Modify: `src/adapters/codex.rs`, `tests/fixtures/codex/rollout-2026-06-04T10-00-00-codexsample.jsonl`
- Test: `tests/codex_adapter.rs`

- [ ] **Step 1: Extend the fixture with a repository_url**

In `tests/fixtures/codex/rollout-2026-06-04T10-00-00-codexsample.jsonl`, change the first line's `git` object to include a URL:

```json
{"type":"session_meta","timestamp":"2026-06-04T10:00:00.000Z","payload":{"id":"codexsample","cwd":"/Users/me/work/web","git":{"branch":"main","repository_url":"git@github.com:me/web.git"}}}
```

- [ ] **Step 2: Write failing tests**

Append to `tests/codex_adapter.rs`:

```rust
#[test]
fn codex_captures_branch_and_repo_url() {
    use hop::adapters::codex::CodexAdapter;
    use hop::adapters::Adapter;
    let path = std::path::Path::new("tests/fixtures/codex/rollout-2026-06-04T10-00-00-codexsample.jsonl");
    let a = CodexAdapter::new(std::path::PathBuf::from("/unused"));
    let s = a.parse(path).unwrap();
    assert_eq!(s.branch.as_deref(), Some("main"));
    assert_eq!(s.repo_url.as_deref(), Some("git@github.com:me/web.git"));
}

#[test]
fn codex_transcript_roles_and_filters_internals() {
    use hop::adapters::codex::CodexAdapter;
    use hop::adapters::Adapter;
    use hop::core::Role;
    let path = std::path::Path::new("tests/fixtures/codex/rollout-2026-06-04T10-00-00-codexsample.jsonl");
    let a = CodexAdapter::new(std::path::PathBuf::from("/unused"));
    let msgs = a.transcript(path).unwrap();
    // only the user_message + agent_message survive; response_item/function_call/token_count dropped
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].role, Role::User);
    assert_eq!(msgs[1].role, Role::Agent);
}
```

- [ ] **Step 3: Add fields to `Payload`/`Git` and refactor onto an extractor**

In `src/adapters/codex.rs`, add a `git` field to `Payload` and a `Git` struct:

```rust
    git: Option<Git>,
```

```rust
#[derive(Deserialize)]
struct Git {
    branch: Option<String>,
    repository_url: Option<String>,
}
```

Add an `Extracted` + `extract`, and rewrite `parse`/`transcript`:

```rust
struct Extracted {
    messages: Vec<crate::core::Message>,
    directory: String,
    branch: Option<String>,
    repo_url: Option<String>,
    first_ts: Option<i64>,
    yolo: bool,
}

impl CodexAdapter {
    fn extract(&self, path: &Path) -> Result<Extracted> {
        use crate::core::{split_blocks, Message, Role};
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        let mut directory = String::new();
        let mut branch = None;
        let mut repo_url = None;
        let mut first_ts: Option<i64> = None;
        let mut messages: Vec<Message> = Vec::new();
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
            if first_ts.is_none() {
                if let Some(ts) = parsed.timestamp.as_deref() {
                    first_ts = parse_ts_secs(ts);
                }
            }
            let Some(p) = parsed.payload else { continue };
            match parsed.kind.as_str() {
                "session_meta" => {
                    if let Some(c) = p.cwd {
                        directory = c;
                    }
                    if let Some(g) = p.git {
                        branch = g.branch.filter(|b| !b.trim().is_empty());
                        repo_url = g.repository_url.filter(|u| !u.trim().is_empty());
                    }
                }
                "turn_context" => {
                    let never = p.approval_policy.as_deref() == Some("never");
                    let danger = p.sandbox_policy.as_ref().map(|s| s.kind.as_str())
                        == Some("danger-full-access");
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
                    let blocks = split_blocks(&text);
                    if blocks.is_empty() {
                        continue;
                    }
                    messages.push(Message {
                        role: if is_user { Role::User } else { Role::Agent },
                        blocks,
                    });
                }
                _ => {}
            }
        }
        Ok(Extracted { messages, directory, branch, repo_url, first_ts, yolo })
    }
}
```

Rewrite `parse`:

```rust
    fn parse(&self, path: &Path) -> Result<Session> {
        use crate::core::{flatten_messages, Block, Role};
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(session_id_from_filename)
            .unwrap_or_else(|| "unknown".to_string());
        let ex = self.extract(path)?;
        let title = ex
            .messages
            .iter()
            .find(|m| m.role == Role::User)
            .and_then(|m| m.blocks.iter().find_map(|b| match b {
                Block::Prose(s) => Some(s.as_str()),
                _ => None,
            }))
            .map(|t| truncate_title(t, TITLE_MAX))
            .unwrap_or_else(|| "(untitled)".to_string());
        let content = flatten_messages(&ex.messages);
        Ok(Session {
            id,
            agent: AgentId::Codex,
            title,
            directory: ex.directory,
            timestamp: ex.first_ts.unwrap_or(0),
            content,
            message_count: ex.messages.len() as u32,
            mtime: 0,
            yolo: ex.yolo,
            branch: ex.branch,
            repo_url: ex.repo_url,
        })
    }

    fn transcript(&self, path: &Path) -> Result<Vec<crate::core::Message>> {
        Ok(self.extract(path)?.messages)
    }
```

- [ ] **Step 4: Run to verify pass (and no regressions)**

Run: `cargo test --test codex_adapter && cargo test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/adapters/codex.rs tests/codex_adapter.rs tests/fixtures/codex/
git commit -m "feat(codex): shared extractor, branch+repo_url capture, transcript"
```

---

## Task 5: Index schema — store branch + repo_url

**Files:**
- Modify: `src/index.rs`
- Test: `tests/index_sync.rs`

- [ ] **Step 1: Write a failing test**

Append to `tests/index_sync.rs` (mirror existing helpers; construct a `Session` with a branch, upsert via `Engine::sync_once` or directly through the index helper used in that file). If the file builds sessions through a `FakeAdapter`, add a session whose `branch` is `Some`. Minimal direct test:

```rust
#[test]
fn branch_roundtrips_through_index() {
    use hop::core::{AgentId, Session};
    use hop::index::SearchIndex;
    use hop::query::ParsedQuery;
    let dir = tempfile::tempdir().unwrap();
    let idx = SearchIndex::open_or_create(dir.path()).unwrap();
    let mut w = idx.writer().unwrap();
    let s = Session {
        id: "a".into(), agent: AgentId::Codex, title: "t".into(),
        directory: "/w".into(), timestamp: 1, content: "hello".into(),
        message_count: 1, mtime: 1, yolo: false,
        branch: Some("feat/x".into()), repo_url: Some("git@github.com:me/web.git".into()),
    };
    idx.upsert(&mut w, &s);
    w.commit().unwrap();
    idx.reload().unwrap();
    let out = idx.search(&ParsedQuery::default(), 100, 10).unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].branch.as_deref(), Some("feat/x"));
    assert_eq!(out[0].repo_url.as_deref(), Some("git@github.com:me/web.git"));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --test index_sync branch_roundtrips`
Expected: FAIL — branch/repo_url come back `None`.

- [ ] **Step 3: Add the fields to schema, upsert, to_session; bump version**

In `src/index.rs`:

Bump version:

```rust
pub const SCHEMA_VERSION: u32 = 2;
```

Add to `struct Fields`:

```rust
    branch: Field,
    repo_url: Field,
```

Add to `build_schema` (use `STRING | STORED`):

```rust
        branch: b.add_text_field("branch", STRING | STORED),
        repo_url: b.add_text_field("repo_url", STRING | STORED),
```

In `upsert`, after the `yolo` line:

```rust
        if let Some(b) = &s.branch {
            doc.add_text(self.f.branch, b);
        }
        if let Some(r) = &s.repo_url {
            doc.add_text(self.f.repo_url, r);
        }
```

In `to_session`, replace the temporary `branch: None, repo_url: None,` from Task 2 with:

```rust
            branch: { let b = get_str(self.f.branch); if b.is_empty() { None } else { Some(b) } },
            repo_url: { let r = get_str(self.f.repo_url); if r.is_empty() { None } else { Some(r) } },
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --test index_sync && cargo test`
Expected: PASS (schema-version bump transparently rebuilds any stale dev index).

- [ ] **Step 5: Commit**

```bash
git add src/index.rs tests/index_sync.rs
git commit -m "feat(index): store branch and repo_url; bump schema to v2"
```

---

## Task 6: Enrichment trait + fast enrichers (branch, repo)

**Files:**
- Create: `src/enrich/mod.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Register the module**

In `src/lib.rs`, add (alphabetical-ish, after `pub mod core;`):

```rust
pub mod columns;
pub mod enrich;
```

(`columns` is created in Task 9; add both now and create empty `src/columns.rs` with `// placeholder` so the crate compiles, or add `pub mod columns;` only in Task 9. To avoid a compile break, add only `pub mod enrich;` here and add `pub mod columns;` in Task 9.)

So add now only:

```rust
pub mod enrich;
```

- [ ] **Step 2: Write the module with failing tests**

Create `src/enrich/mod.rs`:

```rust
//! Pluggable per-session enrichment. Fast enrichers resolve inline for visible
//! rows; slow enrichers resolve in the background (see `service`).

use crate::core::Session;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrichKind {
    /// Pure/cheap; safe to call synchronously while rendering visible rows.
    Fast,
    /// May block or hit the network; must run off the UI thread.
    Slow,
}

/// A resolved enrichment value for one session, ready to display in a cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrichValue {
    pub text: String,
}

pub trait Enricher: Send + Sync {
    fn id(&self) -> &'static str;
    fn kind(&self) -> EnrichKind;
    fn resolve(&self, s: &Session) -> Option<EnrichValue>;
    /// Cache key for slow enrichers; unused for fast ones.
    fn cache_key(&self, _s: &Session) -> String {
        String::new()
    }
    fn ttl(&self) -> Duration {
        Duration::from_secs(0)
    }
}

/// Branch: from `Session.branch`, falling back to `.git/HEAD` of the directory.
pub struct BranchEnricher;

impl Enricher for BranchEnricher {
    fn id(&self) -> &'static str {
        "branch"
    }
    fn kind(&self) -> EnrichKind {
        EnrichKind::Fast
    }
    fn resolve(&self, s: &Session) -> Option<EnrichValue> {
        let b = s
            .branch
            .clone()
            .or_else(|| branch_from_git_head(&s.directory))?;
        Some(EnrichValue { text: b })
    }
}

fn branch_from_git_head(dir: &str) -> Option<String> {
    if dir.is_empty() {
        return None;
    }
    let head = std::fs::read_to_string(Path::new(dir).join(".git").join("HEAD")).ok()?;
    head.trim()
        .strip_prefix("ref: refs/heads/")
        .map(|s| s.to_string())
}

/// Repo: `repo_url` basename when present, else the directory basename.
pub struct RepoEnricher;

impl Enricher for RepoEnricher {
    fn id(&self) -> &'static str {
        "repo"
    }
    fn kind(&self) -> EnrichKind {
        EnrichKind::Fast
    }
    fn resolve(&self, s: &Session) -> Option<EnrichValue> {
        if let Some(url) = &s.repo_url {
            if let Some(name) = repo_name_from_url(url) {
                return Some(EnrichValue { text: name });
            }
        }
        let base = Path::new(&s.directory).file_name()?.to_string_lossy().to_string();
        if base.is_empty() {
            None
        } else {
            Some(EnrichValue { text: base })
        }
    }
}

/// `git@github.com:owner/repo.git` or `https://github.com/owner/repo(.git)` -> `repo`.
pub fn repo_name_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches(".git");
    let last = trimmed.rsplit(['/', ':']).next()?;
    if last.is_empty() {
        None
    } else {
        Some(last.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{AgentId, Session};

    fn sess(branch: Option<&str>, repo_url: Option<&str>, dir: &str) -> Session {
        Session {
            id: "a".into(), agent: AgentId::Claude, title: "t".into(),
            directory: dir.into(), timestamp: 1, content: String::new(),
            message_count: 0, mtime: 0, yolo: false,
            branch: branch.map(|s| s.to_string()),
            repo_url: repo_url.map(|s| s.to_string()),
        }
    }

    #[test]
    fn branch_from_data() {
        assert_eq!(
            BranchEnricher.resolve(&sess(Some("feat/x"), None, "/w")).unwrap().text,
            "feat/x"
        );
    }

    #[test]
    fn repo_from_url_then_dir() {
        assert_eq!(repo_name_from_url("git@github.com:me/web.git").as_deref(), Some("web"));
        assert_eq!(repo_name_from_url("https://github.com/me/web").as_deref(), Some("web"));
        assert_eq!(
            RepoEnricher.resolve(&sess(None, Some("git@github.com:me/web.git"), "/a/b")).unwrap().text,
            "web"
        );
        assert_eq!(
            RepoEnricher.resolve(&sess(None, None, "/a/myproj")).unwrap().text,
            "myproj"
        );
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --lib enrich`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/lib.rs src/enrich/mod.rs
git commit -m "feat(enrich): Enricher trait + branch/repo fast enrichers"
```

---

## Task 7: GitHub PR slow enricher

**Files:**
- Create: `src/enrich/gh_pr.rs`
- Modify: `src/enrich/mod.rs` (`pub mod gh_pr;`)

- [ ] **Step 1: Write failing tests**

Create `src/enrich/gh_pr.rs`:

```rust
//! GitHub PR enricher: maps (repo, branch) -> PR number via the `gh` CLI.
//! Slow (network); resolved in the background and disk-cached.

use super::{EnrichKind, EnrichValue, Enricher};
use crate::core::Session;
use crate::enrich::repo_name_from_url;
use std::time::Duration;

pub struct GhPrEnricher;

impl Enricher for GhPrEnricher {
    fn id(&self) -> &'static str {
        "gh_pr"
    }
    fn kind(&self) -> EnrichKind {
        EnrichKind::Slow
    }
    fn resolve(&self, s: &Session) -> Option<EnrichValue> {
        let branch = s.branch.as_deref()?;
        if branch.is_empty() || branch == "master" || branch == "main" {
            return None;
        }
        let num = gh_pr_number(branch, s.repo_url.as_deref(), &s.directory)?;
        Some(EnrichValue { text: format!("#{num}") })
    }
    fn cache_key(&self, s: &Session) -> String {
        let repo = s
            .repo_url
            .as_deref()
            .and_then(repo_name_from_url)
            .unwrap_or_else(|| s.directory.clone());
        format!("{}@{}", repo, s.branch.as_deref().unwrap_or(""))
    }
    fn ttl(&self) -> Duration {
        Duration::from_secs(60 * 60) // 1h
    }
}

/// Run `gh pr list --head <branch> ...` and return the first PR number, if any.
/// Uses `--repo owner/repo` when derivable from the URL, else runs in `dir`.
fn gh_pr_number(branch: &str, repo_url: Option<&str>, dir: &str) -> Option<u64> {
    use std::process::Command;
    let mut cmd = Command::new("gh");
    cmd.args(["pr", "list", "--head", branch, "--state", "all", "--limit", "1", "--json", "number"]);
    if let Some(slug) = repo_url.and_then(owner_repo_from_url) {
        cmd.args(["--repo", &slug]);
    } else if !dir.is_empty() {
        cmd.current_dir(dir);
    }
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_pr_number(&String::from_utf8_lossy(&out.stdout))
}

/// Parse `[{"number":4821}]` -> 4821.
pub fn parse_pr_number(json: &str) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v.as_array()?.first()?.get("number")?.as_u64()
}

/// `git@github.com:owner/repo.git` / `https://github.com/owner/repo` -> `owner/repo`.
pub fn owner_repo_from_url(url: &str) -> Option<String> {
    let t = url.trim().trim_end_matches(".git");
    if let Some(rest) = t.split("github.com").nth(1) {
        let rest = rest.trim_start_matches([':', '/']);
        if rest.matches('/').count() >= 1 && !rest.is_empty() {
            return Some(rest.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pr_number() {
        assert_eq!(parse_pr_number(r#"[{"number":4821}]"#), Some(4821));
        assert_eq!(parse_pr_number("[]"), None);
        assert_eq!(parse_pr_number("garbage"), None);
    }

    #[test]
    fn owner_repo_extraction() {
        assert_eq!(owner_repo_from_url("git@github.com:me/web.git").as_deref(), Some("me/web"));
        assert_eq!(owner_repo_from_url("https://github.com/me/web").as_deref(), Some("me/web"));
        assert_eq!(owner_repo_from_url("file:///tmp/x"), None);
    }

    #[test]
    fn skips_default_branches() {
        use crate::core::{AgentId, Session};
        let s = Session {
            id: "a".into(), agent: AgentId::Claude, title: "t".into(),
            directory: "/w".into(), timestamp: 1, content: String::new(),
            message_count: 0, mtime: 0, yolo: false,
            branch: Some("main".into()), repo_url: None,
        };
        assert_eq!(GhPrEnricher.resolve(&s), None);
    }
}
```

In `src/enrich/mod.rs` add near the top:

```rust
pub mod gh_pr;
```

- [ ] **Step 2: Run the tests**

Run: `cargo test --lib enrich::gh_pr`
Expected: PASS (the pure parsers are tested; the live `gh` call is not exercised in unit tests).

- [ ] **Step 3: Commit**

```bash
git add src/enrich/mod.rs src/enrich/gh_pr.rs
git commit -m "feat(enrich): GitHub PR slow enricher (gh CLI + parsers)"
```

---

## Task 8: Enrichment service (worker thread + disk cache)

**Files:**
- Create: `src/enrich/service.rs`
- Modify: `src/enrich/mod.rs` (`pub mod service;`)

- [ ] **Step 1: Write failing tests**

Create `src/enrich/service.rs`:

```rust
//! Background resolution of slow enrichers with a disk cache.
//!
//! The UI sends `EnrichRequest`s (a session + which enricher); a worker thread
//! resolves them (checking/populating the on-disk cache) and returns
//! `EnrichResult`s the UI folds into its render state.

use super::{EnrichValue, Enricher};
use crate::core::Session;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct EnrichRequest {
    pub session: Session,
    pub enricher: &'static str,
}

pub struct EnrichResult {
    pub session_id: String,
    pub enricher: &'static str,
    /// None = resolved-but-absent (render as "—"); Some = a value.
    pub value: Option<EnrichValue>,
}

#[derive(Serialize, Deserialize, Default)]
struct CacheFile {
    /// cache_key -> (text-or-empty, fetched_at_unix_secs)
    entries: HashMap<String, (String, u64)>,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Pure cache-hit check used by the worker and by tests.
fn cache_lookup(cache: &CacheFile, key: &str, ttl_secs: u64) -> Option<Option<EnrichValue>> {
    let (text, fetched) = cache.entries.get(key)?;
    if now_secs().saturating_sub(*fetched) > ttl_secs {
        return None; // stale
    }
    if text.is_empty() {
        Some(None)
    } else {
        Some(Some(EnrichValue { text: text.clone() }))
    }
}

pub struct EnrichmentService {
    pub req_tx: Sender<EnrichRequest>,
    pub res_rx: Receiver<EnrichResult>,
    _handle: std::thread::JoinHandle<()>,
}

impl EnrichmentService {
    /// Spawn the worker. `enrichers` are the slow ones to service; `cache_path`
    /// is the JSON cache file (created/loaded lazily).
    pub fn spawn(
        enrichers: Vec<Box<dyn Enricher>>,
        cache_path: PathBuf,
    ) -> EnrichmentService {
        let (req_tx, req_rx) = std::sync::mpsc::channel::<EnrichRequest>();
        let (res_tx, res_rx) = std::sync::mpsc::channel::<EnrichResult>();
        let handle = std::thread::spawn(move || {
            let mut cache: CacheFile = std::fs::read_to_string(&cache_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();

            while let Ok(req) = req_rx.recv() {
                let Some(enr) = enrichers.iter().find(|e| e.id() == req.enricher) else {
                    continue;
                };
                let key = enr.cache_key(&req.session);
                let ttl = enr.ttl().as_secs();
                let value = match cache_lookup(&cache, &key, ttl) {
                    Some(hit) => hit,
                    None => {
                        let resolved = enr.resolve(&req.session);
                        cache.entries.insert(
                            key.clone(),
                            (resolved.as_ref().map(|v| v.text.clone()).unwrap_or_default(), now_secs()),
                        );
                        if let Some(parent) = cache_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Ok(s) = serde_json::to_string(&cache) {
                            let _ = std::fs::write(&cache_path, s);
                        }
                        resolved
                    }
                };
                let _ = res_tx.send(EnrichResult {
                    session_id: req.session.id.clone(),
                    enricher: req.enricher,
                    value,
                });
            }
        });
        EnrichmentService { req_tx, res_rx, _handle: handle }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{AgentId, Session};
    use crate::enrich::{EnrichKind, Enricher};
    use std::time::Duration;

    struct FakeEnricher;
    impl Enricher for FakeEnricher {
        fn id(&self) -> &'static str { "fake" }
        fn kind(&self) -> EnrichKind { EnrichKind::Slow }
        fn resolve(&self, s: &Session) -> Option<EnrichValue> {
            Some(EnrichValue { text: format!("v:{}", s.id) })
        }
        fn cache_key(&self, s: &Session) -> String { s.id.clone() }
        fn ttl(&self) -> Duration { Duration::from_secs(3600) }
    }

    fn sess(id: &str) -> Session {
        Session {
            id: id.into(), agent: AgentId::Claude, title: "t".into(),
            directory: "/w".into(), timestamp: 1, content: String::new(),
            message_count: 0, mtime: 0, yolo: false, branch: None, repo_url: None,
        }
    }

    #[test]
    fn resolves_and_caches_to_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = tmp.path().join("gh_pr.json");
        let svc = EnrichmentService::spawn(vec![Box::new(FakeEnricher)], cache.clone());
        svc.req_tx.send(EnrichRequest { session: sess("a"), enricher: "fake" }).unwrap();
        let r = svc.res_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(r.session_id, "a");
        assert_eq!(r.value.unwrap().text, "v:a");
        // cache file written
        assert!(cache.exists());
    }
}
```

In `src/enrich/mod.rs` add:

```rust
pub mod service;
```

- [ ] **Step 2: Run the test**

Run: `cargo test --lib enrich::service`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/enrich/mod.rs src/enrich/service.rs
git commit -m "feat(enrich): background enrichment service with disk cache"
```

---

## Task 9: Columns layer (cells + responsive solver)

**Files:**
- Create: `src/columns.rs`
- Modify: `src/lib.rs` (`pub mod columns;`)

- [ ] **Step 1: Register module**

In `src/lib.rs` add:

```rust
pub mod columns;
```

- [ ] **Step 2: Write the module with failing tests**

Create `src/columns.rs`:

```rust
//! Pluggable result-list columns and the responsive layout solver. Pure logic:
//! produces per-row cell text and resolved widths; rendering lives in the TUI.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
}

/// A column definition. `flex` columns absorb leftover width (only TITLE).
#[derive(Debug, Clone)]
pub struct Column {
    pub id: &'static str,
    pub header: &'static str,
    pub align: Align,
    /// Higher drops first when the pane is narrow. `u8::MAX` = never drop.
    pub priority: u8,
    pub min_width: u16,
    pub flex: bool,
}

/// The default v1 column set (directory intentionally absent).
pub fn default_columns() -> Vec<Column> {
    vec![
        Column { id: "agent",  header: "",       align: Align::Left,  priority: u8::MAX, min_width: 6,  flex: false },
        Column { id: "repo",   header: "REPO",   align: Align::Left,  priority: 30,      min_width: 8,  flex: false },
        Column { id: "branch", header: "BRANCH", align: Align::Left,  priority: 40,      min_width: 8,  flex: false },
        Column { id: "title",  header: "TITLE",  align: Align::Left,  priority: u8::MAX, min_width: 12, flex: true  },
        Column { id: "msgs",   header: "MSGS",   align: Align::Right, priority: 10,      min_width: 4,  flex: false },
        Column { id: "pr",     header: "PR",     align: Align::Left,  priority: 50,      min_width: 5,  flex: false },
        Column { id: "time",   header: "TIME",   align: Align::Right, priority: 20,      min_width: 4,  flex: false },
    ]
}

const GAP: u16 = 1;

/// Decide which columns are visible and their widths for a given pane width.
/// Drops columns by descending `priority` until the rest fit; TITLE always
/// survives and flexes to fill leftover space.
pub fn solve_layout(columns: &[Column], total_width: u16) -> Vec<(usize, u16)> {
    let mut kept: Vec<usize> = (0..columns.len()).collect();

    let needed = |kept: &[usize]| -> u16 {
        let cols: u16 = kept.iter().map(|&i| columns[i].min_width).sum();
        let gaps = (kept.len().saturating_sub(1)) as u16 * GAP;
        cols + gaps
    };

    // Drop highest-priority (largest number, != MAX) columns until it fits.
    while needed(&kept) > total_width && kept.len() > 1 {
        let drop = kept
            .iter()
            .copied()
            .filter(|&i| columns[i].priority != u8::MAX)
            .max_by_key(|&i| columns[i].priority);
        match drop {
            Some(i) => kept.retain(|&k| k != i),
            None => break, // only un-droppable columns remain
        }
    }

    // Assign widths: min_width to each, extra to the flex column.
    let used = needed(&kept);
    let extra = total_width.saturating_sub(used);
    kept.into_iter()
        .map(|i| {
            let w = if columns[i].flex {
                columns[i].min_width + extra
            } else {
                columns[i].min_width
            };
            (i, w)
        })
        .collect()
}

/// Pad/truncate `s` to exactly `width` columns per `align`.
pub fn fit(s: &str, width: u16, align: Align) -> String {
    let w = width as usize;
    let len = s.chars().count();
    if len == w {
        return s.to_string();
    }
    if len > w {
        if w == 0 {
            return String::new();
        }
        let keep = w.saturating_sub(1);
        let mut out: String = s.chars().take(keep).collect();
        out.push('…');
        return out;
    }
    let pad = " ".repeat(w - len);
    match align {
        Align::Left => format!("{s}{pad}"),
        Align::Right => format!("{pad}{s}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_always_survives_when_very_narrow() {
        let cols = default_columns();
        let layout = solve_layout(&cols, 14);
        let ids: Vec<&str> = layout.iter().map(|&(i, _)| cols[i].id).collect();
        assert!(ids.contains(&"title"));
        assert!(ids.contains(&"agent"));
    }

    #[test]
    fn pr_drops_before_repo_when_narrow() {
        let cols = default_columns();
        // width that forces some drops but not all
        let layout = solve_layout(&cols, 40);
        let ids: Vec<&str> = layout.iter().map(|&(i, _)| cols[i].id).collect();
        // pr (priority 50) drops before branch (40) / repo (30)
        if !ids.contains(&"repo") {
            assert!(!ids.contains(&"pr"));
        }
        assert!(ids.contains(&"title") && ids.contains(&"agent"));
    }

    #[test]
    fn flex_column_absorbs_extra_width() {
        let cols = default_columns();
        let layout = solve_layout(&cols, 200);
        let title_w = layout.iter().find(|&&(i, _)| cols[i].id == "title").unwrap().1;
        assert!(title_w > 12, "title should grow past its min on a wide pane");
    }

    #[test]
    fn fit_pads_and_truncates() {
        assert_eq!(fit("ab", 4, Align::Left), "ab  ");
        assert_eq!(fit("ab", 4, Align::Right), "  ab");
        assert_eq!(fit("abcdef", 4, Align::Left), "abc…");
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --lib columns`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/lib.rs src/columns.rs
git commit -m "feat(columns): pluggable columns + responsive layout solver"
```

---

## Task 10: Config — preview, keymap, columns toggles

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Write failing tests**

Add to the `tests` module in `src/config.rs`:

```rust
#[test]
fn preview_and_keymap_defaults() {
    let cfg = Config::default();
    assert!(cfg.preview.visible);
    assert_eq!(cfg.preview.width_pct, 50);
    assert_eq!(cfg.keymap, "search");
}

#[test]
fn preview_and_keymap_from_toml() {
    let toml = r#"
        keymap = "modal"
        [preview]
        visible = false
        width_pct = 40
    "#;
    let cfg = Config::from_toml_str(toml).unwrap();
    assert!(!cfg.preview.visible);
    assert_eq!(cfg.preview.width_pct, 40);
    assert_eq!(cfg.keymap, "modal");
}

#[test]
fn disabled_columns_from_toml() {
    let toml = r#"
        [columns]
        disabled = ["pr", "msgs"]
    "#;
    let cfg = Config::from_toml_str(toml).unwrap();
    assert!(cfg.columns.disabled.contains(&"pr".to_string()));
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib config`
Expected: FAIL — `preview`, `keymap`, `columns` fields don't exist.

- [ ] **Step 3: Add config structs**

In `src/config.rs`, add structs and fields:

```rust
#[derive(Debug, Deserialize)]
pub struct PreviewConfig {
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default = "default_width_pct")]
    pub width_pct: u16,
}

fn default_true() -> bool { true }
fn default_width_pct() -> u16 { 50 }

impl Default for PreviewConfig {
    fn default() -> Self {
        PreviewConfig { visible: true, width_pct: 50 }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct ColumnsConfig {
    #[serde(default)]
    pub disabled: Vec<String>,
    /// Optional explicit order (column ids); empty = default order.
    #[serde(default)]
    pub order: Vec<String>,
}

fn default_keymap() -> String { "search".to_string() }
```

Add fields to `struct Config`:

```rust
    #[serde(default)]
    pub preview: PreviewConfig,
    #[serde(default = "default_keymap")]
    pub keymap: String,
    #[serde(default)]
    pub columns: ColumnsConfig,
```

Update the `#[derive(Debug, Default, Deserialize)]` on `Config`: since `keymap` now needs a non-empty default and `preview` a custom default, replace the derived `Default` with a manual impl so `Config::default()` matches the serde defaults:

```rust
impl Default for Config {
    fn default() -> Self {
        Config {
            data_dirs: HashMap::new(),
            theme: HashMap::new(),
            keybindings: HashMap::new(),
            preview: PreviewConfig::default(),
            keymap: default_keymap(),
            columns: ColumnsConfig::default(),
        }
    }
}
```

Remove `Default` from the `#[derive(...)]` on `Config` (keep `Debug, Deserialize`).

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib config && cargo test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): preview, keymap, and columns settings"
```

---

## Task 11: Keymap presets + Action expansion

**Files:**
- Create: `src/tui/keymap.rs`
- Modify: `src/tui/mod.rs` (`pub mod keymap;`, `Action` variants, dispatch)

- [ ] **Step 1: Expand `Action` and add the keymap module declaration**

In `src/tui/mod.rs`, add to the `Action` enum:

```rust
    /// Scroll the preview pane.
    ScrollPreview(i16),
    /// Grow/shrink the preview split (+1 grow, -1 shrink).
    ResizePreview(i8),
    /// Toggle preview visibility.
    TogglePreview,
    /// Toggle the help overlay.
    Help,
```

Add at the top of `src/tui/mod.rs`:

```rust
pub mod keymap;
```

- [ ] **Step 2: Write the keymap module with failing tests**

Create `src/tui/keymap.rs`:

```rust
//! Keymap presets. The default "search" preset keeps the query always-live and
//! puts actions on arrows/PgUp-Dn/Ctrl-chords. The "modal" preset adds a
//! navigate mode where single letters act.

use crate::tui::Action;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Search,
    Modal,
}

impl Preset {
    pub fn from_str(s: &str) -> Preset {
        match s {
            "modal" => Preset::Modal,
            _ => Preset::Search,
        }
    }
}

/// Resolve a key to an action that is independent of mode/query editing. These
/// chords work in both presets. Returns None if the key isn't a bound chord.
pub fn chord_action(key: &KeyEvent) -> Option<Action> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match (key.code, ctrl) {
        (KeyCode::Char('c'), true) => Some(Action::Quit),
        (KeyCode::Char('p'), true) => Some(Action::TogglePreview),
        (KeyCode::Char('u'), true) => Some(Action::ScrollPreview(-8)),
        (KeyCode::Char('d'), true) => Some(Action::ScrollPreview(8)),
        (KeyCode::Char('y'), true) => Some(Action::Resume { index: 0, yolo: true }), // index filled by App
        (KeyCode::PageUp, _) => Some(Action::ScrollPreview(-8)),
        (KeyCode::PageDown, _) => Some(Action::ScrollPreview(8)),
        (KeyCode::Char('['), false) => Some(Action::ResizePreview(-1)),
        (KeyCode::Char(']'), false) => Some(Action::ResizePreview(1)),
        (KeyCode::Char('?'), false) => Some(Action::Help),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }
    fn plain(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn ctrl_chords_map() {
        assert_eq!(chord_action(&ctrl('p')), Some(Action::TogglePreview));
        assert!(matches!(chord_action(&ctrl('u')), Some(Action::ScrollPreview(n)) if n < 0));
        assert!(matches!(chord_action(&ctrl('y')), Some(Action::Resume { yolo: true, .. })));
        assert_eq!(chord_action(&ctrl('c')), Some(Action::Quit));
    }

    #[test]
    fn bracket_resizes_and_question_helps() {
        assert_eq!(chord_action(&plain(KeyCode::Char('['))), Some(Action::ResizePreview(-1)));
        assert_eq!(chord_action(&plain(KeyCode::Char(']'))), Some(Action::ResizePreview(1)));
        assert_eq!(chord_action(&plain(KeyCode::Char('?'))), Some(Action::Help));
    }

    #[test]
    fn preset_parsing() {
        assert_eq!(Preset::from_str("modal"), Preset::Modal);
        assert_eq!(Preset::from_str("search"), Preset::Search);
        assert_eq!(Preset::from_str("nonsense"), Preset::Search);
    }
}
```

Note: `chord_action` only covers unambiguous chords (Ctrl-chords + PgUp/PgDn). The printable chars `[`, `]`, `?` are bound as resize/help **only when the query is empty**, otherwise they type literally — that nuance lives in `App::handle_key` (Task 17) and is covered by a test there. Selection arrows (`↑`/`↓`) are handled directly by `App`, not as chords.

- [ ] **Step 3: Run the tests**

Run: `cargo test --lib tui::keymap`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/tui/mod.rs src/tui/keymap.rs
git commit -m "feat(tui): keymap presets + expanded Action set"
```

---

## Task 12: Syntect code highlighting helper

**Files:**
- Create: `src/tui/preview.rs`
- Modify: `src/tui/mod.rs` (`pub mod preview;`)

- [ ] **Step 1: Declare the module**

In `src/tui/mod.rs` add:

```rust
pub mod preview;
```

- [ ] **Step 2: Write the highlighter with a failing test**

Create `src/tui/preview.rs`:

```rust
//! Transcript preview rendering: code highlighting (syntect), prose markdown
//! (pulldown-cmark), and assembling messages into scrollable, match-highlighted
//! lines.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
static THEMES: OnceLock<ThemeSet> = OnceLock::new();

fn map_lang(l: &str) -> &str {
    match l {
        "js" => "javascript",
        "ts" => "typescript",
        "py" => "python",
        "rb" => "ruby",
        "sh" => "bash",
        "yml" => "yaml",
        "rs" => "rust",
        other => other,
    }
}

/// Highlight a code block into indented ratatui lines. Lazily loads syntect's
/// default assets on first use; safe to call from the render path (memoize at
/// the call site per selection).
pub fn highlight_code(code: &str, lang: Option<&str>) -> Vec<Line<'static>> {
    let ps = SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines);
    let ts = THEMES.get_or_init(ThemeSet::load_defaults);
    let theme = &ts.themes["base16-ocean.dark"];
    let syntax = lang
        .map(map_lang)
        .and_then(|l| ps.find_syntax_by_token(l))
        .unwrap_or_else(|| ps.find_syntax_plain_text());
    let mut h = HighlightLines::new(syntax, theme);
    let mut out = Vec::new();
    for line in code.lines() {
        let ranges = h.highlight_line(line, ps).unwrap_or_default();
        let mut spans: Vec<Span<'static>> = vec![Span::raw("  ")];
        for (style, text) in ranges {
            let c = style.foreground;
            spans.push(Span::styled(
                text.to_string(),
                Style::default().fg(Color::Rgb(c.r, c.g, c.b)),
            ));
        }
        out.push(Line::from(spans));
    }
    if out.is_empty() {
        out.push(Line::from(Span::raw("  ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_rust_into_indented_lines() {
        let lines = highlight_code("fn main() {}", Some("rust"));
        assert_eq!(lines.len(), 1);
        // first span is the 2-space indent
        let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.starts_with("  "));
        assert!(text.contains("fn main"));
    }

    #[test]
    fn unknown_lang_falls_back_to_plain() {
        let lines = highlight_code("x = 1", Some("nope-lang"));
        assert_eq!(lines.len(), 1);
    }
}
```

- [ ] **Step 3: Run the test**

Run: `cargo test --lib tui::preview`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/tui/mod.rs src/tui/preview.rs
git commit -m "feat(preview): syntect code highlighting helper"
```

---

## Task 13: Prose markdown rendering

**Files:**
- Modify: `src/tui/preview.rs`

- [ ] **Step 1: Write failing tests**

Add to the `tests` module in `src/tui/preview.rs`:

```rust
#[test]
fn prose_plain_text_one_line() {
    let lines = render_prose("hello world");
    let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(text.trim(), "hello world");
}

#[test]
fn prose_bullets_get_marker() {
    let lines = render_prose("- one\n- two");
    let joined: String = lines.iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(joined.contains("• one"));
    assert!(joined.contains("• two"));
}

#[test]
fn prose_bold_is_styled_bold() {
    let lines = render_prose("**strong**");
    let bold = lines.iter().flat_map(|l| &l.spans)
        .any(|s| s.content.contains("strong") && s.style.add_modifier.contains(ratatui::style::Modifier::BOLD));
    assert!(bold);
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib tui::preview::tests::prose`
Expected: FAIL — `render_prose` undefined.

- [ ] **Step 3: Implement `render_prose`**

Add to `src/tui/preview.rs`:

```rust
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use ratatui::style::Modifier;

/// Render a prose (non-code) markdown string into styled lines. Handles
/// headings (bold), strong/emphasis, inline code, and list items.
pub fn render_prose(text: &str) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut bold = false;
    let mut italic = false;
    let mut in_item = false;

    let flush = |spans: &mut Vec<Span<'static>>, lines: &mut Vec<Line<'static>>| {
        lines.push(Line::from(std::mem::take(spans)));
    };

    for ev in Parser::new(text) {
        match ev {
            Event::Start(Tag::Heading { .. }) => bold = true,
            Event::End(TagEnd::Heading(_)) => {
                bold = false;
                flush(&mut spans, &mut lines);
            }
            Event::Start(Tag::Strong) => bold = true,
            Event::End(TagEnd::Strong) => bold = false,
            Event::Start(Tag::Emphasis) => italic = true,
            Event::End(TagEnd::Emphasis) => italic = false,
            Event::Start(Tag::Item) => {
                in_item = true;
                spans.push(Span::raw("• "));
            }
            Event::End(TagEnd::Item) => {
                in_item = false;
                flush(&mut spans, &mut lines);
            }
            Event::End(TagEnd::Paragraph) => flush(&mut spans, &mut lines),
            Event::SoftBreak | Event::HardBreak => {
                if in_item {
                    spans.push(Span::raw(" "));
                } else {
                    flush(&mut spans, &mut lines);
                }
            }
            Event::Code(t) => {
                spans.push(Span::styled(t.to_string(), Style::default().fg(Color::Yellow)));
            }
            Event::Text(t) => {
                let mut style = Style::default();
                if bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if italic {
                    style = style.add_modifier(Modifier::ITALIC);
                }
                spans.push(Span::styled(t.to_string(), style));
            }
            _ => {}
        }
    }
    if !spans.is_empty() {
        flush(&mut spans, &mut lines);
    }
    if lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib tui::preview`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tui/preview.rs
git commit -m "feat(preview): markdown prose rendering via pulldown-cmark"
```

---

## Task 14: Assemble transcript + match highlight + scroll-to-match

**Files:**
- Modify: `src/tui/preview.rs`

- [ ] **Step 1: Write failing tests**

Add to the `tests` module in `src/tui/preview.rs`:

```rust
use crate::core::{Block, Message, Role};

fn msgs() -> Vec<Message> {
    vec![
        Message { role: Role::User, blocks: vec![Block::Prose("fix the auth bug".into())] },
        Message { role: Role::Agent, blocks: vec![
            Block::Prose("the refresh token dropped".into()),
            Block::Code { lang: Some("rust".into()), text: "fn refresh() {}".into() },
        ]},
    ]
}

#[test]
fn transcript_has_role_prefixes() {
    let lines = render_transcript(&msgs(), "", crate::core::AgentId::Claude);
    let joined: String = lines.iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
        .collect::<Vec<_>>().join("\n");
    assert!(joined.contains("› fix the auth bug"));
    assert!(joined.contains("● CLAUDE"));
    assert!(joined.contains("fn refresh"));
}

#[test]
fn first_match_line_is_found() {
    let lines = render_transcript(&msgs(), "refresh", crate::core::AgentId::Claude);
    let idx = first_match_line(&lines, "refresh");
    assert!(idx.is_some());
}

#[test]
fn match_terms_highlighted() {
    let lines = render_transcript(&msgs(), "auth", crate::core::AgentId::Claude);
    let any_reverse = lines.iter().flat_map(|l| &l.spans)
        .any(|s| s.content.contains("auth") && s.style.add_modifier.contains(ratatui::style::Modifier::REVERSED));
    assert!(any_reverse);
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib tui::preview::tests::transcript`
Expected: FAIL — `render_transcript`, `first_match_line` undefined.

- [ ] **Step 3: Implement assembly, match-highlight, and scroll helper**

Add to `src/tui/preview.rs`:

```rust
use crate::core::{AgentId, Block, Message, Role};
use crate::tui::theme;

/// Render a full transcript into lines, applying query-term highlighting.
pub fn render_transcript(msgs: &[Message], query: &str, _agent: AgentId) -> Vec<Line<'static>> {
    let terms: Vec<String> = query.split_whitespace().map(|t| t.to_lowercase()).collect();
    let mut out: Vec<Line<'static>> = Vec::new();

    for (mi, m) in msgs.iter().enumerate() {
        if mi > 0 {
            out.push(Line::from(""));
        }
        match m.role {
            Role::User => {
                for b in &m.blocks {
                    if let Block::Prose(s) = b {
                        let mut prose = render_prose(s);
                        prefix_first(&mut prose, "› ", theme::ACCENT);
                        out.extend(prose);
                    } else if let Block::Code { lang, text } = b {
                        out.extend(highlight_code(text, lang.as_deref()));
                    }
                }
            }
            Role::Agent => {
                out.push(Line::from(vec![
                    Span::styled("● ", Style::default().fg(theme::agent_color(_agent))),
                    Span::styled("CLAUDE", Style::default().fg(theme::agent_color(_agent)).add_modifier(Modifier::BOLD)),
                ]));
                for b in &m.blocks {
                    match b {
                        Block::Prose(s) => {
                            let mut prose = render_prose(s);
                            indent(&mut prose, "  ");
                            out.extend(prose);
                        }
                        Block::Code { lang, text } => {
                            out.extend(highlight_code(text, lang.as_deref()));
                        }
                    }
                }
            }
        }
    }
    if !terms.is_empty() {
        for line in &mut out {
            *line = highlight_terms(line, &terms);
        }
    }
    out
}

/// Use the agent badge text for the role line. `render_transcript` hardcodes
/// "CLAUDE"; override per-agent here.
fn _badge(agent: AgentId) -> &'static str {
    agent.badge()
}

fn prefix_first(lines: &mut [Line<'static>], prefix: &'static str, color: Color) {
    if let Some(first) = lines.first_mut() {
        let mut spans = vec![Span::styled(prefix, Style::default().fg(color))];
        spans.append(&mut first.spans);
        *first = Line::from(spans);
    }
}

fn indent(lines: &mut [Line<'static>], pad: &'static str) {
    for l in lines.iter_mut() {
        let mut spans = vec![Span::raw(pad)];
        spans.append(&mut l.spans);
        *l = Line::from(spans);
    }
}

/// Re-split a line's spans so any occurrence of a term is reverse-highlighted.
fn highlight_terms(line: &Line<'static>, terms: &[String]) -> Line<'static> {
    let mut out: Vec<Span<'static>> = Vec::new();
    for span in &line.spans {
        let text = span.content.to_string();
        let lower = text.to_lowercase();
        let mut idx = 0usize;
        while idx < text.len() {
            // find the earliest term match at or after idx
            let next = terms
                .iter()
                .filter_map(|t| lower[idx..].find(t.as_str()).map(|p| (idx + p, t.len())))
                .min_by_key(|&(p, _)| p);
            match next {
                Some((p, len)) => {
                    if p > idx {
                        out.push(Span::styled(text[idx..p].to_string(), span.style));
                    }
                    out.push(Span::styled(
                        text[p..p + len].to_string(),
                        span.style.add_modifier(Modifier::REVERSED),
                    ));
                    idx = p + len;
                }
                None => {
                    out.push(Span::styled(text[idx..].to_string(), span.style));
                    break;
                }
            }
        }
        if text.is_empty() {
            out.push(span.clone());
        }
    }
    Line::from(out)
}

/// Index of the first line containing any term (case-insensitive); for scroll.
pub fn first_match_line(lines: &[Line<'static>], query: &str) -> Option<usize> {
    let terms: Vec<String> = query.split_whitespace().map(|t| t.to_lowercase()).collect();
    if terms.is_empty() {
        return None;
    }
    lines.iter().position(|l| {
        let text: String = l.spans.iter().map(|s| s.content.as_ref()).collect::<String>().to_lowercase();
        terms.iter().any(|t| text.contains(t.as_str()))
    })
}
```

Replace the hardcoded `"CLAUDE"` badge with the agent badge: in `render_transcript`, change the role-line `Span::styled("CLAUDE", ...)` to `Span::styled(_agent.badge(), ...)` and rename `_agent` to `agent` (drop the leading underscore) throughout the function signature and body. Remove the now-unused `_badge` helper.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib tui::preview`
Expected: PASS (`transcript_has_role_prefixes` finds `● CLAUDE` because the fixture agent is Claude).

- [ ] **Step 5: Commit**

```bash
git add src/tui/preview.rs
git commit -m "feat(preview): assemble transcript with role prefixes + match highlight/scroll"
```

---

## Task 15: Results-list column-grid renderer

**Files:**
- Create: `src/tui/results_list.rs`
- Modify: `src/tui/mod.rs` (`pub mod results_list;`)

- [ ] **Step 1: Declare the module**

In `src/tui/mod.rs` add:

```rust
pub mod results_list;
```

- [ ] **Step 2: Write the renderer with failing tests**

Create `src/tui/results_list.rs`:

```rust
//! Renders the result list as an aligned column grid using the `columns`
//! solver, the fast enrichers, and a resolved-slow-value lookup.

use crate::columns::{fit, solve_layout, Align, Column};
use crate::core::Session;
use crate::enrich::{EnrichKind, Enricher};
use crate::tui::{theme, view::rel_time};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use std::collections::HashMap;

/// Build one display line for a session given the resolved layout. `resolved`
/// maps (session_id, enricher_id) -> displayed text for slow enrichers; a
/// missing slow value renders as the pending glyph.
pub fn row_line(
    s: &Session,
    layout: &[(usize, u16)],
    columns: &[Column],
    enrichers: &[Box<dyn Enricher>],
    resolved: &HashMap<(String, &'static str), Option<String>>,
    now: i64,
) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (n, &(ci, width)) in layout.iter().enumerate() {
        if n > 0 {
            spans.push(Span::raw(" "));
        }
        let col = &columns[ci];
        let (text, style) = cell(s, col, enrichers, resolved, now);
        spans.push(Span::styled(fit(&text, width, col.align), style));
    }
    Line::from(spans)
}

fn cell(
    s: &Session,
    col: &Column,
    enrichers: &[Box<dyn Enricher>],
    resolved: &HashMap<(String, &'static str), Option<String>>,
    now: i64,
) -> (String, Style) {
    match col.id {
        "agent" => (s.agent.badge().to_string(), Style::default().fg(theme::agent_color(s.agent))),
        "title" => (s.title.clone(), Style::default()),
        "msgs" => (
            if s.message_count > 0 { s.message_count.to_string() } else { "-".into() },
            Style::default().fg(theme::DIM),
        ),
        "time" => (rel_time(s.timestamp, now), Style::default().fg(theme::DIM)),
        other => enrichment_cell(other, s, enrichers, resolved),
    }
}

fn enrichment_cell(
    id: &str,
    s: &Session,
    enrichers: &[Box<dyn Enricher>],
    resolved: &HashMap<(String, &'static str), Option<String>>,
) -> (String, Style) {
    let Some(enr) = enrichers.iter().find(|e| e.id() == id) else {
        return (String::new(), Style::default());
    };
    match enr.kind() {
        EnrichKind::Fast => (
            enr.resolve(s).map(|v| v.text).unwrap_or_else(|| "—".into()),
            Style::default().fg(theme::DIM),
        ),
        EnrichKind::Slow => match resolved.get(&(s.id.clone(), enr.id())) {
            Some(Some(text)) => (text.clone(), Style::default().fg(theme::ACCENT)),
            Some(None) => ("—".into(), Style::default().fg(theme::DIM)),
            None => ("⟳".into(), Style::default().fg(theme::DIM)),
        },
    }
}

/// Convenience: solve the layout for a given width.
pub fn layout_for(columns: &[Column], width: u16) -> Vec<(usize, u16)> {
    solve_layout(columns, width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::columns::default_columns;
    use crate::core::{AgentId, Session};
    use crate::enrich::{BranchEnricher, RepoEnricher};

    fn sess() -> Session {
        Session {
            id: "a".into(), agent: AgentId::Claude, title: "fix auth".into(),
            directory: "/work/api".into(), timestamp: 0, content: String::new(),
            message_count: 12, mtime: 0, yolo: false,
            branch: Some("feat/auth".into()), repo_url: None,
        }
    }

    #[test]
    fn row_renders_repo_branch_title() {
        let cols = default_columns();
        let layout = layout_for(&cols, 120);
        let enr: Vec<Box<dyn Enricher>> = vec![Box::new(BranchEnricher), Box::new(RepoEnricher)];
        let resolved = HashMap::new();
        let line = row_line(&sess(), &layout, &cols, &enr, &resolved, 3600);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("CLAUDE"));
        assert!(text.contains("api"));        // repo from dir basename
        assert!(text.contains("feat/auth"));  // branch from data
        assert!(text.contains("fix auth"));   // title
        assert!(text.contains("12"));         // msgs
    }

    #[test]
    fn pending_pr_shows_glyph() {
        let cols = default_columns();
        let layout = layout_for(&cols, 120);
        let enr: Vec<Box<dyn Enricher>> =
            vec![Box::new(crate::enrich::gh_pr::GhPrEnricher)];
        let resolved = HashMap::new();
        let line = row_line(&sess(), &layout, &cols, &enr, &resolved, 0);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("⟳"));
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --lib tui::results_list`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/tui/mod.rs src/tui/results_list.rs
git commit -m "feat(tui): column-grid result list renderer"
```

---

## Task 16: Help overlay

**Files:**
- Create: `src/tui/help.rs`
- Modify: `src/tui/mod.rs` (`pub mod help;`)

- [ ] **Step 1: Declare the module**

In `src/tui/mod.rs` add:

```rust
pub mod help;
```

- [ ] **Step 2: Write the overlay with a failing test**

Create `src/tui/help.rs`:

```rust
//! Centered help overlay listing the active keymap.

use crate::tui::keymap::Preset;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

pub fn lines(preset: Preset) -> Vec<Line<'static>> {
    let mut out = vec![
        Line::from(Span::raw("Navigation")),
        Line::from("  ↑/↓        move selection"),
        Line::from("  PgUp/PgDn  page list / scroll preview"),
        Line::from("  Ctrl+U/D   scroll preview"),
        Line::from(""),
        Line::from(Span::raw("Preview")),
        Line::from("  Ctrl+P     toggle preview"),
        Line::from("  [ / ]      shrink / grow preview"),
        Line::from(""),
        Line::from(Span::raw("Actions")),
        Line::from("  Enter      resume"),
        Line::from("  Ctrl+Y     resume (yolo)"),
        Line::from("  Tab        autocomplete keyword"),
        Line::from("  ?          toggle this help"),
        Line::from("  Esc/Ctrl+C quit"),
    ];
    if preset == Preset::Modal {
        out.push(Line::from(""));
        out.push(Line::from(Span::raw("Modal mode")));
        out.push(Line::from("  Esc        leave query → navigate"));
        out.push(Line::from("  j/k g/G    move / top-bottom"));
        out.push(Line::from("  / p        search / preview"));
    }
    out
}

/// Render the overlay centered over the frame.
pub fn render(f: &mut Frame, preset: Preset) {
    let area = f.area();
    let w = 44u16.min(area.width.saturating_sub(2));
    let body = lines(preset);
    let h = (body.len() as u16 + 2).min(area.height.saturating_sub(2));
    let rect = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);
    let block = Block::default().borders(Borders::ALL).title(" help ");
    f.render_widget(
        Paragraph::new(body).block(block).alignment(Alignment::Left).style(Style::default()),
        rect,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_help_lists_core_bindings() {
        let l = lines(Preset::Search);
        let text: String = l.iter()
            .map(|x| x.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect::<Vec<_>>().join("\n");
        assert!(text.contains("Ctrl+P"));
        assert!(text.contains("Ctrl+Y"));
        assert!(text.contains("Tab"));
        assert!(!text.contains("Modal mode"));
    }

    #[test]
    fn modal_help_adds_modal_section() {
        let text: String = lines(Preset::Modal).iter()
            .map(|x| x.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect::<Vec<_>>().join("\n");
        assert!(text.contains("Modal mode"));
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --lib tui::help`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/tui/mod.rs src/tui/help.rs
git commit -m "feat(tui): help overlay"
```

---

## Task 17: App state + keymap-driven dispatch

**Files:**
- Modify: `src/tui/mod.rs`

- [ ] **Step 1: Write failing tests**

Add to the `tests` module in `src/tui/mod.rs`:

```rust
#[test]
fn ctrl_p_toggles_preview() {
    let mut app = app_with(1);
    assert!(app.preview_visible());
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    assert!(!app.preview_visible());
}

#[test]
fn brackets_resize_preview_width() {
    let mut app = app_with(1);
    let before = app.preview_width_pct();
    app.handle_key(key(KeyCode::Char(']')));
    assert!(app.preview_width_pct() > before);
    app.handle_key(key(KeyCode::Char('[')));
    app.handle_key(key(KeyCode::Char('[')));
    assert!(app.preview_width_pct() < before);
}

#[test]
fn question_toggles_help_and_esc_closes_it() {
    let mut app = app_with(1);
    app.handle_key(key(KeyCode::Char('?')));
    assert!(app.help_open());
    assert_eq!(app.handle_key(key(KeyCode::Esc)), Action::None);
    assert!(!app.help_open());
}

#[test]
fn ctrl_y_resumes_selected_with_yolo() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Down)); // select index 1
    match app.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL)) {
        Action::Resume { index, yolo } => { assert_eq!(index, 1); assert!(yolo); }
        other => panic!("expected yolo resume, got {other:?}"),
    }
}

#[test]
fn brackets_type_into_query_when_query_nonempty() {
    let mut app = app_with(1);
    app.handle_key(key(KeyCode::Char('a')));
    let act = app.handle_key(key(KeyCode::Char('[')));
    assert_eq!(act, Action::Search);
    assert_eq!(app.query(), "a[");
}

#[test]
fn tab_autocompletes_keyword_value() {
    let mut app = app_with(1);
    for c in "agent:cl".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    assert_eq!(app.handle_key(key(KeyCode::Tab)), Action::Search);
    assert_eq!(app.query(), "agent:claude");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib tui::tests`
Expected: FAIL — new methods/dispatch missing.

- [ ] **Step 3: Add state, getters, and dispatch**

In `src/tui/mod.rs`, add fields to `struct App`:

```rust
    preview_visible: bool,
    preview_width_pct: u16,
    preview_scroll: u16,
    help_open: bool,
    keymap: keymap::Preset,
```

Initialize in `App::new()`:

```rust
            preview_visible: true,
            preview_width_pct: 50,
            preview_scroll: 0,
            help_open: false,
            keymap: keymap::Preset::Search,
```

Add getters/setters:

```rust
    pub fn preview_visible(&self) -> bool { self.preview_visible }
    pub fn preview_width_pct(&self) -> u16 { self.preview_width_pct }
    pub fn preview_scroll(&self) -> u16 { self.preview_scroll }
    pub fn help_open(&self) -> bool { self.help_open }
    pub fn set_keymap(&mut self, p: keymap::Preset) { self.keymap = p; }
    pub fn set_preview(&mut self, visible: bool, width_pct: u16) {
        self.preview_visible = visible;
        self.preview_width_pct = width_pct.clamp(20, 80);
    }
```

Replace the **entire existing `handle_key` method** with the version below. Overlays and chords are handled before query editing; `Tab` now autocompletes the query (via `crate::query::autocomplete`) instead of triggering yolo; `Esc` quits (modal-preset navigate behavior is added in Task 21):

```rust
    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.kind == KeyEventKind::Release {
            return Action::None; // ignore key-release (Windows)
        }
        // Help overlay swallows keys (Esc/? close it).
        if self.help_open {
            if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') {
                self.help_open = false;
            }
            return Action::None;
        }
        // Ctrl+C always quits.
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit;
        }
        // Yolo confirmation modal.
        if let Mode::YoloModal { index, yolo } = self.mode {
            return match key.code {
                KeyCode::Esc => { self.mode = Mode::Main; Action::None }
                KeyCode::Tab => { self.mode = Mode::YoloModal { index, yolo: !yolo }; Action::None }
                KeyCode::Enter => { self.mode = Mode::Main; Action::Resume { index, yolo } }
                _ => Action::None,
            };
        }
        // Unambiguous chords (Ctrl-chords, paging).
        if let Some(act) = keymap::chord_action(&key) {
            return self.apply_chord(act);
        }
        // `[`, `]`, `?` act as chords only when the query is empty (else they type).
        if self.query.is_empty() {
            match key.code {
                KeyCode::Char('[') => return self.apply_chord(Action::ResizePreview(-1)),
                KeyCode::Char(']') => return self.apply_chord(Action::ResizePreview(1)),
                KeyCode::Char('?') => { self.help_open = true; return Action::None; }
                _ => {}
            }
        }
        // Main search handling.
        match key.code {
            KeyCode::Esc => Action::Quit,
            KeyCode::Down => {
                if !self.results.is_empty() {
                    self.selected = (self.selected + 1).min(self.results.len() - 1);
                }
                self.preview_scroll = 0;
                Action::None
            }
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                self.preview_scroll = 0;
                Action::None
            }
            KeyCode::Enter => self.activate(false),
            KeyCode::Tab => {
                if let Some(completed) = crate::query::autocomplete(&self.query) {
                    self.query = completed;
                    self.preview_scroll = 0;
                    Action::Search
                } else {
                    Action::None
                }
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.preview_scroll = 0;
                Action::Search
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.query.push(c);
                self.preview_scroll = 0;
                Action::Search
            }
            _ => Action::None,
        }
    }
```

This replaces the old `match self.mode { ... }` body entirely. `activate(false)` (Enter) still opens the yolo modal for yolo-capable agents; `Ctrl+Y` (via `apply_chord`) resumes with yolo directly.

Add the `apply_chord` helper:

```rust
    fn apply_chord(&mut self, act: Action) -> Action {
        match act {
            Action::TogglePreview => {
                self.preview_visible = !self.preview_visible;
                Action::None
            }
            Action::ResizePreview(d) => {
                let next = self.preview_width_pct as i32 + (d as i32) * 5;
                self.preview_width_pct = next.clamp(20, 80) as u16;
                Action::None
            }
            Action::ScrollPreview(d) => {
                let next = self.preview_scroll as i32 + d as i32;
                self.preview_scroll = next.max(0) as u16;
                Action::None
            }
            Action::Help => { self.help_open = true; Action::None }
            Action::Resume { yolo, .. } => {
                if self.results.is_empty() { Action::None }
                else { Action::Resume { index: self.selected, yolo } }
            }
            other => other,
        }
    }
```

(The full `handle_key` above already resets `preview_scroll` on every selection/query change, so no separate edit is needed.)

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib tui`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tui/mod.rs
git commit -m "feat(tui): preview/help/keymap state + chord dispatch"
```

---

## Task 18: View — split/hidden layout, columns, preview, help

**Files:**
- Modify: `src/tui/view.rs`

- [ ] **Step 1: Update the failing render test**

Replace the `renders_badge_and_title` test body in `src/tui/view.rs` with a call through the new signature (the renderer now needs enrichers + resolved map + a transcript). Add:

```rust
#[test]
fn renders_columns_and_preview() {
    use crate::enrich::{BranchEnricher, Enricher, RepoEnricher};
    use crate::core::{Block, Message, Role};
    use std::collections::HashMap;

    let mut app = App::new();
    app.set_results(vec![Session {
        id: "a".into(), agent: AgentId::Claude, title: "fix auth".into(),
        directory: "/work/api".into(), timestamp: 0, content: "hello".into(),
        message_count: 3, mtime: 0, yolo: false,
        branch: Some("feat/auth".into()), repo_url: None,
    }]);
    let enr: Vec<Box<dyn Enricher>> = vec![Box::new(RepoEnricher), Box::new(BranchEnricher)];
    let resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
    let transcript = vec![Message { role: Role::User, blocks: vec![Block::Prose("fix auth".into())] }];

    let lines = crate::tui::preview::render_transcript(&transcript, app.query(), AgentId::Claude);
    let base = crate::tui::preview::first_match_line(&lines, app.query()).unwrap_or(0) as u16;

    let backend = TestBackend::new(100, 12);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| render(f, &app, 100, &enr, &resolved, &lines, base)).unwrap();
    let buf = term.backend().buffer().clone();
    let text: String = buf.content().iter().map(|c| c.symbol()).collect();
    assert!(text.contains("CLAUDE"));
    assert!(text.contains("fix auth"));
    assert!(text.contains("feat/auth"));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib tui::view`
Expected: FAIL — `render` signature mismatch.

- [ ] **Step 3: Rewrite `render`**

Replace the body of `render` in `src/tui/view.rs` (keep `rel_time` as-is). New signature and body:

```rust
use crate::columns::default_columns;
use crate::enrich::Enricher;
use crate::tui::{help, results_list, theme, App};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;
use std::collections::HashMap;

pub fn render(
    f: &mut Frame,
    app: &App,
    now: i64,
    enrichers: &[Box<dyn Enricher>],
    resolved: &HashMap<(String, &'static str), Option<String>>,
    preview_lines: &[Line<'static>],
    match_base: u16,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());

    // search input
    let header = Line::from(vec![
        Span::raw("❯ "),
        Span::raw(app.query().to_string()),
        Span::raw(format!("   {}/{}", app.results().len(), app.results().len())).fg(theme::DIM),
    ]);
    f.render_widget(Paragraph::new(header), chunks[0]);

    // body: list (| preview)
    let (list_area, preview_area) = if app.preview_visible() {
        let pw = app.preview_width_pct();
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100 - pw), Constraint::Percentage(pw)])
            .split(chunks[1]);
        (body[0], Some(body[1]))
    } else {
        (chunks[1], None)
    };

    // column grid
    let cols = default_columns();
    let list_inner_w = list_area.width.saturating_sub(if preview_area.is_some() { 1 } else { 0 });
    let layout = results_list::layout_for(&cols, list_inner_w);
    let items: Vec<ListItem> = app
        .results()
        .iter()
        .map(|s| ListItem::new(results_list::row_line(s, &layout, &cols, enrichers, resolved, now)))
        .collect();
    let mut state = ListState::default();
    if !app.results().is_empty() {
        state.select(Some(app.selected()));
    }
    let list_block = if preview_area.is_some() {
        Block::default().borders(Borders::RIGHT)
    } else {
        Block::default()
    };
    let list = List::new(items)
        .block(list_block)
        .highlight_style(ratatui::style::Style::default().bg(theme::ACCENT));
    f.render_stateful_widget(list, list_area, &mut state);

    // preview (lines are pre-rendered/memoized by the caller per selection+query)
    if let Some(area) = preview_area {
        let scroll = match_base.saturating_add(app.preview_scroll());
        f.render_widget(Paragraph::new(preview_lines.to_vec()).scroll((scroll, 0)), area);
    }

    // footer
    let footer = if app.modal_open() {
        "tab toggle yolo · enter confirm · esc cancel"
    } else {
        "↑↓ move · enter resume · ctrl+y yolo · ctrl+p preview · [ ] size · ? help · esc quit"
    };
    f.render_widget(Paragraph::new(footer).fg(theme::DIM), chunks[2]);

    // help overlay (drawn last, on top)
    if app.help_open() {
        help::render(f, app.keymap_preset());
    }
}
```

Add a `keymap_preset()` getter to `App` in `src/tui/mod.rs`:

```rust
    pub fn keymap_preset(&self) -> keymap::Preset { self.keymap }
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib tui::view && cargo test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tui/view.rs src/tui/mod.rs
git commit -m "feat(tui): split/hidden layout, column grid, preview, help overlay"
```

---

## Task 19: main.rs wiring — enrichment service, on-selection transcript, config

**Files:**
- Modify: `src/main.rs`

This task has no unit tests (it's the integration shell); it is verified by building and a manual smoke run.

- [ ] **Step 1: Build the slow-enricher service and fast-enricher set in `main`**

In `src/main.rs`, after `let config = Config::load()?;`, add helpers and state. Add imports:

```rust
use hop::core::Message;
use hop::enrich::gh_pr::GhPrEnricher;
use hop::enrich::service::{EnrichRequest, EnrichmentService};
use hop::enrich::{BranchEnricher, Enricher, RepoEnricher};
use hop::tui::preview;
use std::collections::HashMap;
```

Add a cache-path helper next to `index_dir`:

```rust
fn enrich_cache_path() -> std::path::PathBuf {
    directories::ProjectDirs::from("dev", "hop", "hop")
        .map(|d| d.cache_dir().join("enrich").join("gh_pr.json"))
        .unwrap_or_else(|| std::path::PathBuf::from(".hop-enrich.json"))
}
```

Build the fast enrichers (respecting `config.columns.disabled`) and spawn the service for the slow ones:

```rust
    let fast_enrichers: Vec<Box<dyn Enricher>> = vec![Box::new(RepoEnricher), Box::new(BranchEnricher)];
    let pr_enabled = !config.columns.disabled.iter().any(|d| d == "pr");
    let service = if pr_enabled {
        Some(EnrichmentService::spawn(vec![Box::new(GhPrEnricher)], enrich_cache_path()))
    } else {
        None
    };
```

- [ ] **Step 2: Thread preview/keymap config into the App and pass new render args**

Change `run_tui`'s signature to accept the enrichers, the service, and config-derived UI prefs, and to own the resolved map + transcript. Replace `run_tui` with:

```rust
fn run_tui(
    engine: &mut Engine,
    updates: std::sync::mpsc::Receiver<Update>,
    fast_enrichers: &[Box<dyn Enricher>],
    service: Option<&EnrichmentService>,
    config: &Config,
) -> Result<Option<(hop::core::Session, bool)>> {
    let mut terminal = ratatui::init();
    let mut app = App::new();
    app.set_query(engine.query().to_string());
    app.set_keymap(hop::tui::keymap::Preset::from_str(&config.keymap));
    app.set_preview(config.preview.visible, config.preview.width_pct);
    sync_results_into_app(engine, &mut app);

    // slow-enrichment state
    let mut resolved: HashMap<(String, &'static str), Option<String>> = HashMap::new();
    let mut requested: std::collections::HashSet<(String, &'static str)> = Default::default();

    // currently-previewed transcript (re-parsed on selection change)
    let mut transcript: Vec<Message> = Vec::new();
    let mut transcript_for: Option<String> = None;
    // memoized rendered preview lines, rebuilt only when (selection, query) changes
    let mut preview_lines: Vec<ratatui::text::Line<'static>> = Vec::new();
    let mut preview_key: Option<(String, String)> = None;
    let mut preview_base: u16 = 0;

    let outcome = (|| -> Result<Option<(hop::core::Session, bool)>> {
        loop {
            // re-parse the selected session's transcript on demand (debounced by selection identity)
            let sel_id = engine.results().get(app.selected()).map(|s| s.id.clone());
            if app.preview_visible() && sel_id != transcript_for {
                transcript = match engine.results().get(app.selected()) {
                    Some(s) => engine
                        .adapter_for(s.agent)
                        .and_then(|a| latest_transcript(engine, a, s))
                        .unwrap_or_default(),
                    None => Vec::new(),
                };
                transcript_for = sel_id.clone();
            }

            // rebuild memoized preview lines when selection or query changes
            let pkey = (sel_id.clone().unwrap_or_default(), app.query().to_string());
            if app.preview_visible() && preview_key.as_ref() != Some(&pkey) {
                let agent = engine
                    .results()
                    .get(app.selected())
                    .map(|s| s.agent)
                    .unwrap_or(hop::core::AgentId::Claude);
                preview_lines = preview::render_transcript(&transcript, app.query(), agent);
                preview_base = preview::first_match_line(&preview_lines, app.query())
                    .map(|i| i as u16)
                    .unwrap_or(0);
                preview_key = Some(pkey);
            }

            let now = jiff::Timestamp::now().as_second();
            terminal.draw(|f| {
                hop::tui::view::render(f, &app, now, fast_enrichers, &resolved, &preview_lines, preview_base)
            })?;

            // request PR enrichment for visible rows (cap to first ~200 results)
            if let Some(svc) = service {
                for s in engine.results().iter().take(200) {
                    let key = (s.id.clone(), "gh_pr");
                    if !requested.contains(&key) {
                        requested.insert(key);
                        let _ = svc.req_tx.send(EnrichRequest { session: s.clone(), enricher: "gh_pr" });
                    }
                }
                while let Ok(r) = svc.res_rx.try_recv() {
                    resolved.insert((r.session_id, r.enricher), r.value.map(|v| v.text));
                }
            }

            if !app.modal_open() {
                while let Ok(update) = updates.try_recv() {
                    if let Update::Refresh = update {
                        engine.reload()?;
                        engine.search()?;
                        sync_results_into_app(engine, &mut app);
                        transcript_for = None; // force re-parse next frame
                        preview_key = None;
                    }
                }
            }

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    match app.handle_key(key) {
                        Action::Quit => return Ok(None),
                        Action::Search => engine.set_query(app.query().to_string()),
                        Action::Resume { index, yolo } => {
                            if let Some(s) = engine.results().get(index).cloned() {
                                return Ok(Some((s, yolo)));
                            }
                        }
                        _ => {}
                    }
                }
            }

            if !app.modal_open() && engine.search_due() {
                engine.search()?;
                sync_results_into_app(engine, &mut app);
                transcript_for = None;
                preview_key = None;
            }
        }
    })();

    ratatui::restore();
    outcome
}

/// Re-parse the on-disk file for `s` into a transcript. The path isn't stored on
/// the Session, so re-scan the adapter to find it by id (cheap stat-level scan).
fn latest_transcript(
    _engine: &Engine,
    adapter: &dyn hop::adapters::Adapter,
    s: &hop::core::Session,
) -> Option<Vec<Message>> {
    let scanned = adapter.scan().ok()?;
    let entry = scanned.get(&s.id)?;
    adapter.transcript(&entry.path).ok()
}
```

- [ ] **Step 3: Update the `main` call site**

In `main`, change the `run_tui` call to:

```rust
    let pending = run_tui(&mut engine, updates, &fast_enrichers, service.as_ref(), &config)?;
```

- [ ] **Step 4: Build and smoke-test**

Run: `cargo build`
Expected: builds clean.

Run: `cargo run` (in a real terminal with existing Claude/Codex history)
Expected, by inspection:
- Result list shows `CLAUDE/CODEX` badges, `REPO`, `BRANCH`, `TITLE`, `MSGS`, `PR` (`⟳`→`#NNN`/`—`), `TIME`.
- Preview shows a clean transcript (`›`/`●` prefixes, highlighted code, no `<command-*>`/tool noise).
- `Ctrl+P` hides/shows preview; `[`/`]` resize it; `?` opens help; `Esc` closes help, then quits; `Ctrl+Y` resumes with yolo.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire enrichment service, on-demand transcript, preview/keymap config"
```

---

## Task 20: Docs

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update keys, columns, and config sections**

In `README.md`, replace the "Keys:" line with:

```
Keys: type to search · ↑↓ move · Enter resume · Ctrl+Y yolo · Ctrl+P toggle preview ·
      [ / ] resize preview · Ctrl+U/D scroll preview · Tab autocomplete · ? help · Esc quit
```

Add a "Columns" subsection documenting `AGENT · REPO · BRANCH · TITLE · MSGS · PR · TIME`, that the directory is shown in the preview header, and that `PR` is resolved in the background via `gh` and cached.

Add a "Config" subsection with an example `config.toml`:

```toml
keymap = "search"   # or "modal"

[preview]
visible = true
width_pct = 50

[columns]
disabled = []       # e.g. ["pr"] to turn off the GitHub PR column
```

- [ ] **Step 2: Verify the full suite once more**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: document v2 keys, columns, and config"
```

---

## Task 21: Modal keymap preset — navigate mode

**Files:**
- Modify: `src/tui/mod.rs`

- [ ] **Step 1: Write failing tests**

Add to the `tests` module in `src/tui/mod.rs`:

```rust
#[test]
fn search_preset_esc_still_quits() {
    let mut app = app_with(3); // default = search preset
    assert_eq!(app.handle_key(key(KeyCode::Esc)), Action::Quit);
}

#[test]
fn modal_esc_enters_navigate_then_letters_move() {
    let mut app = app_with(3);
    app.set_keymap(keymap::Preset::Modal);
    // Esc enters navigate mode instead of quitting
    assert_eq!(app.handle_key(key(KeyCode::Esc)), Action::None);
    // letters now navigate
    app.handle_key(key(KeyCode::Char('j')));
    assert_eq!(app.selected(), 1);
    app.handle_key(key(KeyCode::Char('k')));
    assert_eq!(app.selected(), 0);
    app.handle_key(key(KeyCode::Char('G')));
    assert_eq!(app.selected(), 2);
    // '/' returns to search so letters type again
    app.handle_key(key(KeyCode::Char('/')));
    assert_eq!(app.handle_key(key(KeyCode::Char('a'))), Action::Search);
    assert_eq!(app.query(), "a");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib tui::tests::modal`
Expected: FAIL — `navigate` field / `handle_navigate` missing; modal Esc currently quits.

- [ ] **Step 3: Add the navigate field, dispatch branch, and handler**

In `src/tui/mod.rs`, add a field to `struct App`:

```rust
    navigate: bool,
```

Initialize in `App::new()`:

```rust
            navigate: false,
```

In `handle_key`, immediately after the `chord_action` block and before the `// `[`, `]`, `?`` block, insert:

```rust
        // Modal preset: navigate mode consumes letter keys.
        if self.keymap == keymap::Preset::Modal && self.navigate {
            return self.handle_navigate(key);
        }
```

Change the main-match `Esc` arm from `KeyCode::Esc => Action::Quit,` to:

```rust
            KeyCode::Esc => {
                if self.keymap == keymap::Preset::Modal {
                    self.navigate = true; // leave query → navigate
                    Action::None
                } else {
                    Action::Quit
                }
            }
```

Add the navigate handler method to `impl App`:

```rust
    fn handle_navigate(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => Action::Quit,
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.results.is_empty() {
                    self.selected = (self.selected + 1).min(self.results.len() - 1);
                }
                self.preview_scroll = 0;
                Action::None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                self.preview_scroll = 0;
                Action::None
            }
            KeyCode::Char('g') => {
                self.selected = 0;
                self.preview_scroll = 0;
                Action::None
            }
            KeyCode::Char('G') => {
                self.selected = self.results.len().saturating_sub(1);
                self.preview_scroll = 0;
                Action::None
            }
            KeyCode::Char('p') => {
                self.preview_visible = !self.preview_visible;
                Action::None
            }
            KeyCode::Char('?') => {
                self.help_open = true;
                Action::None
            }
            KeyCode::Char('/') => {
                self.navigate = false; // back to live search
                Action::None
            }
            KeyCode::Enter => self.activate(false),
            _ => Action::None,
        }
    }
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib tui`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tui/mod.rs
git commit -m "feat(tui): modal keymap preset navigate mode"
```

---

## Task 22: Persist preview state across restarts

Spec §5.1 says the chosen preview width/visibility survive restarts. To avoid
clobbering the user's hand-written `config.toml` (comments/formatting), persist to
a dedicated `ui_state.toml` in the cache dir; on launch it overrides the config
defaults.

**Files:**
- Modify: `src/config.rs`, `src/main.rs`

- [ ] **Step 1: Write a failing test**

Add to the `tests` module in `src/config.rs`:

```rust
#[test]
fn ui_state_roundtrips() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("ui_state.toml");
    UiState { preview_visible: false, preview_width_pct: 35 }.save(&p).unwrap();
    let loaded = UiState::load(&p).unwrap();
    assert!(!loaded.preview_visible);
    assert_eq!(loaded.preview_width_pct, 35);
    assert!(UiState::load(&tmp.path().join("absent.toml")).is_none());
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib config::tests::ui_state`
Expected: FAIL — `UiState` undefined.

- [ ] **Step 3: Add `UiState` to config**

In `src/config.rs`, change the serde import:

```rust
use serde::{Deserialize, Serialize};
```

Add the type and its load/save:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiState {
    pub preview_visible: bool,
    pub preview_width_pct: u16,
}

impl UiState {
    pub fn load(path: &std::path::Path) -> Option<UiState> {
        let text = std::fs::read_to_string(path).ok()?;
        toml::from_str(&text).ok()
    }

    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string(self).context("serializing ui_state")?;
        std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib config`
Expected: PASS.

- [ ] **Step 5: Wire load/save into `main`**

In `src/main.rs`, add `use hop::config::UiState;` to the imports and a path helper next to `index_dir`:

```rust
fn ui_state_path() -> std::path::PathBuf {
    directories::ProjectDirs::from("dev", "hop", "hop")
        .map(|d| d.cache_dir().join("ui_state.toml"))
        .unwrap_or_else(|| std::path::PathBuf::from(".hop-ui-state.toml"))
}
```

In `main`, compute the initial preview state (saved state overrides config) before calling `run_tui`:

```rust
    let ui_path = ui_state_path();
    let init_preview = UiState::load(&ui_path)
        .map(|u| (u.preview_visible, u.preview_width_pct))
        .unwrap_or((config.preview.visible, config.preview.width_pct));
```

Change the `run_tui` call to pass them:

```rust
    let pending = run_tui(
        &mut engine,
        updates,
        &fast_enrichers,
        service.as_ref(),
        &config,
        init_preview,
        ui_path,
    )?;
```

Update `run_tui`'s signature to accept the two new params:

```rust
fn run_tui(
    engine: &mut Engine,
    updates: std::sync::mpsc::Receiver<Update>,
    fast_enrichers: &[Box<dyn Enricher>],
    service: Option<&EnrichmentService>,
    config: &Config,
    init_preview: (bool, u16),
    ui_path: std::path::PathBuf,
) -> Result<Option<(hop::core::Session, bool)>> {
```

Replace the preview-init line inside `run_tui`:

```rust
    app.set_preview(init_preview.0, init_preview.1);
```

And after the event loop's `ratatui::restore();`, persist the final state (the closure's borrow of `app` has ended, so `app` is readable again):

```rust
    ratatui::restore();
    let _ = UiState {
        preview_visible: app.preview_visible(),
        preview_width_pct: app.preview_width_pct(),
    }
    .save(&ui_path);
    outcome
```

- [ ] **Step 6: Build and smoke-test persistence**

Run: `cargo build`
Expected: builds clean.

Manual: `cargo run`, resize the preview with `]` a few times, quit with `Esc`, relaunch — the preview keeps the new width.

- [ ] **Step 7: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "feat: persist preview width/visibility across restarts"
```

---

## Self-review notes (for the implementer)

- **Spec coverage:** §3 content model/preview → Tasks 1,3,4,12,13,14,18,19; §3.2 branch/repo from data → Tasks 3,4,5; §4 columns+enrichment → Tasks 6,7,8,9,15; §5.1 preview controls+persistence → Tasks 10,17,18,22; §5.2 keymap+help → Tasks 11,16,17,21; §6 module changes → all; §7 testing → per-task tests; §8 perf → memoization below.
- **Preview memoization (Tasks 18/19):** the renderer takes pre-rendered lines; `run_tui` rebuilds them only when `(selection, query)` changes. This honors §8 — syntect/markdown run per selection, not per frame.
- **Tab autocomplete (Task 17):** wired to the existing tested `query::autocomplete`; yolo moved to `Ctrl+Y`. Resolves the v1 Tab overload. Covered by `tab_autocompletes_keyword_value`.
- **`[` `]` `?` ambiguity (Task 17):** act as chords only when the query is empty; otherwise they type. Ctrl-chords and PgUp/PgDn always act. Covered by `brackets_type_into_query_when_query_nonempty`.
- **`Ctrl+Y` index (Tasks 11 & 17):** `chord_action` returns a placeholder `index: 0`; `App::apply_chord` substitutes `self.selected`. Verified by `ctrl_y_resumes_selected_with_yolo`.
- **Modal preset (Task 21):** opt-in via `keymap = "modal"`; `Esc` enters a navigate mode (`j/k/g/G/p/?//`). Default `search` preset is unchanged (`Esc` quits).
- **Persistence deviation (Task 22):** spec §5.1 says width/state persist "to config.toml". The plan persists to a dedicated `ui_state.toml` in the cache dir instead, to avoid rewriting the user's hand-authored config (comments/formatting). Behavior (survives restarts) matches the spec intent.
- **Transcript path lookup (Task 19):** Sessions don't carry their file path, so `latest_transcript` re-scans the adapter by id. Acceptable: `scan` is stat-level and runs only on selection change.
- **PR for default branches:** `GhPrEnricher` returns `None` for `main`/`master`, rendering `—`.
- If any `Session { .. }` literal is missed when adding fields (Task 2), `cargo build` will name the file and line — add `branch: None, repo_url: None,`.
