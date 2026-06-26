---
name: garden-docs
description: >
  Audit project documentation for staleness by comparing what changed in code
  since docs/ was last updated. Use this skill whenever the user mentions
  gardening docs, auditing docs, checking if docs are stale, syncing docs with
  code, doc drift, or asks "are the docs up to date". Also trigger when the user
  says things like "update the docs", "docs are stale", "what's missing from
  docs", or "review documentation freshness".
---

# Doc Gardening

Detect documentation drift — code changes that happened after the last docs/
update — and fix it.

## Mental Model

Documentation rots when code evolves but docs don't. The "drift window" is the
set of commits between the last `docs/` change and HEAD. Every commit in that
window is a potential source of undocumented change. The job is to read those
commits, understand what changed structurally, and decide whether the existing
docs still describe the system accurately.

Not every code commit needs a doc update. Bug fixes, test additions, formatting,
dependency bumps, and internal refactors that don't change boundaries or behavior
are noise. The signal is: did a boundary move, did a module's responsibility
change, did user-facing behavior change, did the data flow change, did a new
concept appear that readers would need to know about.

## Workflow

### 1. Discover the drift window

Find the last commit that touched any file under `docs/`:

```bash
git log -1 --format="%H %ai %s" -- docs/
```

Then find all commits since that one:

```bash
git log --oneline <last-docs-commit>..HEAD
```

If `docs/` has never been touched, the drift window is the full history — flag
this and suggest bootstrapping documentation first rather than gardening.

If HEAD *is* the last docs commit (no drift window), report that docs are
current and stop.

### 2. Analyze the drift window

For each commit in the drift window, examine what changed:

```bash
git diff --stat <last-docs-commit>..HEAD
git diff <last-docs-commit>..HEAD -- src/
```

Focus on structural signals:
- **New files or modules** — a new `src/foo.rs` or `src/adapters/bar.rs` likely
  needs mention in architecture docs.
- **Renamed or deleted files** — existing doc references may be broken.
- **Changed public interfaces** — new exported functions, changed type
  signatures, new CLI flags or subcommands.
- **New dependencies or integrations** — a new external tool or API.
- **Changed data flow** — new pipeline stages, changed boundaries.

Ignore:
- Pure test changes (unless they reveal a new testing pattern worth documenting).
- Formatting, clippy fixes, dependency version bumps with no API change.
- Internal implementation changes that don't affect module boundaries or
  user-visible behavior.

### 3. Map changes to documentation

Read the existing docs to understand what they currently describe. Then for each
significant change from step 2, check whether the docs already cover it.

Build a gap report with this structure:

```
## Doc Drift Report

**Drift window:** <last-docs-commit-hash> → HEAD (<N> commits, <date range>)

### Gaps Found

1. **<short description>**
   - What changed: <commit(s) and what they did>
   - What's stale: <which doc file/section is affected>
   - Suggested fix: <what to add, update, or remove>

2. ...

### No Action Needed

- <commit summary> — internal refactor, no doc impact
- ...
```

Present this report to the user before making changes. If there are no gaps,
say so clearly and stop.

### 4. Apply fixes

After presenting the report, update the docs. Follow these principles:

- **Edit, don't rewrite.** Change the minimum needed to make docs accurate again.
  Preserve voice, structure, and existing content that's still correct.
- **Respect the project's doc hierarchy.** If the project has architecture docs,
  project docs, specs, etc., put information in the right place. Don't dump
  everything into one file.
- **Update, don't append.** If a module description changed, update the existing
  description rather than adding a second one.
- **Remove stale content.** If something was deleted or renamed, remove or update
  references to it. Don't leave dead references.
- **Preserve IDs and structure.** If docs use numbered rules, stable IDs, or
  other reference systems, respect them. Don't renumber existing items.

### 5. Summarize

After applying fixes, give a brief summary of what changed:

```
## Changes Made

- Updated `docs/ARCHITECTURE.md`: added Foo module to module responsibilities,
  updated data flow diagram
- Updated `docs/PROJECT.md`: added new CLI flag `--bar` to user-facing commands
- No changes needed in `docs/specs/` — specs are historical
```

## Edge Cases

- **Monorepo with multiple docs/ dirs**: Search for documentation directories at
  common locations (`docs/`, `doc/`, `documentation/`). If the project uses a
  different convention, ask.
- **No docs/ directory**: Suggest creating foundational docs rather than
  gardening. This skill is for maintaining existing documentation, not
  bootstrapping from scratch.
- **Very large drift window (50+ commits)**: Summarize by area of change rather
  than per-commit analysis. Group related commits and focus on the net effect.
- **CLAUDE.md / AGENTS.md**: These are agent-facing docs that may also drift.
  Include them in the audit if they reference specific files, modules, or
  commands that changed.
