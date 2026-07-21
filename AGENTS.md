# Agent Map

This file is the short entry point for coding agents. Keep it small. Durable
project knowledge belongs in `docs/`.

## Start Here

- Project contract: `docs/PROJECT.md`
- Architecture map: `docs/ARCHITECTURE.md`
- User-facing usage: `README.md`

## What This Repo Builds

`hop` is a Rust CLI/TUI that indexes Claude Code and Codex session JSONL files,
searches them with Tantivy, previews a cleaned transcript, and exec-resumes the
selected session in the original working directory.

## Working Rules

- Read `docs/RUST_GUIDELINES.md` before starting any implementation work.
- Prefer the existing module boundaries over introducing new framework shape.
- Prefer files under ~500 lines. Crossing it on a file you're editing is a signal
  to split by responsibility — not a hard limit. If a file is legitimately over
  (e.g. dominated by colocated `#[cfg(test)]` tests), add a one-line note near the
  top explaining why, and move on. Don't split modules purely to satisfy the count.
- Keep `main.rs` as orchestration only when possible; reusable behavior belongs
  in library modules.
- Parse external data at adapter boundaries into `core` types before indexing or
  rendering.
- Keep agent-specific knowledge inside its adapter (`B-011`): format quirks, magic
  field values, and per-agent judgments belong in `adapters/<agent>.rs`. Generic
  layers (`engine`, `core`, `index`, `tui`) must not name an agent-specific literal
  or `adapters::<agent>::…` symbol — reach adapters only through the `Adapter` trait.
  When a generic layer needs a per-agent decision, add an agent-agnostic trait
  method with a safe default and override it in the adapter.
- Keep the TUI responsive: no network calls, large scans, or broad filesystem
  work in the render path.
- Preserve resume behavior: restore the terminal before `exec`, then `chdir` and
  replace the process with the agent CLI.
- Update documentation when changing architecture, user-visible behavior, or
  agent-facing workflows.
- Never write absolute local working-directory paths (e.g. `/Users/<name>/...`
  or a personal `~/workspaces/...` layout) into committed files. Reference repos
  and files by their repo-relative path, well-known data dirs (`~/.codex`,
  `~/.claude`), or a neutral description like "a local checkout".

## Common Commands

```sh
cargo test
cargo test --lib
cargo test --test index_sync
cargo run -- --rebuild
```

## Documentation Policy

Use docs as a map, not a manual. Add new docs only when they are stable enough to
help the next agent avoid rediscovery. Prefer updating `docs/PROJECT.md` or
`docs/ARCHITECTURE.md` before creating another top-level document.

Architecture rules in `docs/ARCHITECTURE.md` use stable IDs. Do not renumber
them casually; update or remove stale rules and pressure points when code changes
make them obsolete. Current-state concerns that may be fixed later belong under
`Known Pressure Points`, not in stable boundary or invariant language.
