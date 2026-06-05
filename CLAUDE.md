# Claude Code Guide

Follow `AGENTS.md` first. This file exists so Claude Code and other tools that
look for `CLAUDE.md` land on the same repository map.

Claude-specific context:

- Claude session parsing lives in `src/adapters/claude.rs`.
- Claude fixtures live in `tests/fixtures/claude/`.
- Claude resume command generation is part of the `Adapter` implementation.
- Do not duplicate transcript-cleaning rules outside the adapter extractor and
  shared `core::{Message, Block, Role}` model.

For project intent and architecture, read:

- `docs/PROJECT.md`
- `docs/ARCHITECTURE.md`
