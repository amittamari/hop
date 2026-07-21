## Why

The TUI presents agent identity, git/PR/session metadata, and status purely as
text. It already relies on common-plane Unicode glyphs (`❯ • › ● · — … ─ ▎` and
the braille spinner), so the terminal is not treated as ASCII-only — yet none of
that visual vocabulary is centralized, and there is no icon layer that makes the
chrome scan quickly. A subtle nerd-font icon pass makes the app feel modern and
lets metadata read at a glance, at effectively zero render cost.

The one real risk is that nerd-font icons live in the Private Use Area (PUA) and
only render in a patched font; without one they show as tofu (`□`). hop's users
skew heavily toward Claude Code / Codex power users who typically run patched
fonts, which justifies an opt-out default — but only if there is a clean,
tofu-free fallback for everyone else.

## What Changes

- Introduce a `Glyphs` set (new TUI module) that mirrors `Theme`: chosen once at
  startup and threaded as `&Glyphs` alongside `&Theme`. Two variants:
  - `Glyphs::nerd()` — PUA icons (default).
  - `Glyphs::ascii()` — reproduces today's look with zero tofu (escape hatch).
- Add `[display] icons` config (bool, default `true` → opt-out). `false` selects
  `Glyphs::ascii()`.
- Apply a **subtle** icon set to chrome only: agent mark (glyph + short text in
  the existing brand color), branch, repo, PR, time/clock, message count, and the
  archived marker. Wire up status glyphs (warning / success / error) using the
  currently-unused `theme.warning` / `theme.success` / `theme.error` roles.
- Route the per-agent glyph through the `Adapter` trait (agent-agnostic method
  with a safe default, overridden per adapter), per architecture rule B-011, so
  no agent-specific glyph literal lives in a generic layer.
- Centralize the scattered glyph literals into `Glyphs`: `SELECTION_MARKER`,
  `SPINNER_FRAMES`, `ACCENT_BAR`, `SEP` (and the ~5 inlined `" · "` copies),
  `ARCHIVED_MARKER`, and the preview prefixes (`●` agent dot, `›` user prefix,
  `•` bullet). One owner per glyph is what makes the fallback switch trustworthy.
- **Restraint by design**: no icons in footer key-hints, card snippet text, or
  transcript prose. Icons live in chrome, not content.
- **Out of scope (deferred)**: terminal image rendering (Kitty / iTerm2 / Sixel)
  of real agent logos. That is a future change layered on top of this fallback
  ladder, scoped to the preview header only.

## Capabilities

### New Capabilities
- `tui-icons`: A centralized glyph vocabulary with a nerd/ascii fallback switch,
  the `[display] icons` opt-out config, the per-agent glyph via the `Adapter`
  trait, and the set of chrome surfaces that render icons (agent mark, metadata
  fields, status indicators). Guarantees the ascii variant is visually
  equivalent to today (no tofu, no layout shift).

### Modified Capabilities
<!-- None. The icon layer is additive over `card-rows` (the agent badge stays
     colored; a glyph is prefixed when icons are enabled) and does not change any
     existing requirement. -->

## Impact

- **New code**: `src/tui/glyphs.rs` (the `Glyphs` set). Threading `&Glyphs`
  through the render path alongside `&Theme`.
- **Config**: `DisplayConfig` in `src/config.rs` gains an `icons` field
  (default `true`).
- **Adapters**: `Adapter` trait (`src/adapters/mod.rs`) gains an agent-glyph
  method with a safe default; `claude.rs` / `codex.rs` / `cursor.rs` override it.
- **Touched render sites**: `view.rs`, `results_list.rs`, `preview.rs`,
  `toolbar.rs`, `modal.rs`, `help.rs`, `columns.rs` — call sites that currently
  hardcode glyph literals move to `&Glyphs`.
- **No new dependencies** (ratatui 0.30 unchanged); no network or filesystem work
  added to the render path.
- **Docs**: update `docs/ARCHITECTURE.md` (new `Glyphs` boundary, agent-glyph via
  `Adapter`) and `README.md` (the `[display] icons` option and font requirement).
