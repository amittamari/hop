## Context

hop's TUI renders agent identity and metadata as text and already uses
common-plane Unicode glyphs (`❯ • › ● · — … ─ ▎` and a braille spinner) that
render in any modern monospace font. Those glyph literals are scattered across
`view.rs`, `results_list.rs`, `preview.rs`, `toolbar.rs`, `modal.rs`, `help.rs`,
and `columns.rs` (e.g. `SEP` is inlined as `" · "` in five places). There is no
single owner of the visual vocabulary.

Nerd-font icons differ from what hop uses today in one important way: they live
in the Private Use Area (PUA) and render only when the terminal font is patched.
Without a patched font they show as tofu (`□`). The audience skews to Claude Code
/ Codex power users who typically run patched fonts, so icons can be opt-out —
but only behind a fallback that reproduces today's look exactly.

Constraints:
- Architecture rule B-011: agent-specific knowledge lives in the adapter; generic
  layers must not name an agent literal or match on agent identity for a per-agent
  decision.
- The render path must stay free of network, large scans, and broad filesystem
  work.
- Resume behavior (terminal restore → chdir → exec) must be unaffected.

## Goals / Non-Goals

**Goals:**
- A single centralized glyph set (`Glyphs`) that mirrors `Theme`: chosen once at
  startup, threaded as `&Glyphs`, with `nerd` and `ascii` variants.
- Opt-out via `[display] icons` (default `true`); `ascii` variant is visually
  identical to today (no tofu, no layout shift).
- A subtle icon pass on chrome only: agent mark, metadata fields (branch, repo,
  PR, time, message count), archived marker, and status (warning/success/error).
- Per-agent glyph provided through the `Adapter` trait, honoring B-011.
- Absorb the scattered glyph literals so each glyph has exactly one owner.

**Non-Goals:**
- Terminal image rendering (Kitty / iTerm2 / Sixel) of real agent logos —
  deferred to a follow-up change, scoped to the preview header only.
- Terminal capability auto-detection (querying for font/protocol support).
- Icons in the footer key-hints, card snippet, or transcript prose.
- Making `Theme` / `Glyphs` user-configurable beyond the single `icons` toggle;
  full theming stays reserved.

## Decisions

### Decision: `Glyphs` set mirrors `Theme`, selected once and threaded
Add `src/tui/glyphs.rs` with a `Glyphs` struct exposing accessor methods for each
chrome glyph. Provide `Glyphs::nerd()` and `Glyphs::ascii()` constructors. Select
the variant once from config at startup and thread `&Glyphs` through the render
functions exactly where `&Theme` already flows.

- **Why**: `Theme` is the proven pattern for "one value, chosen once, threaded
  read-only." Reusing its shape keeps the render path uniform and makes the
  fallback a single switch rather than per-call-site conditionals.
- **Alternative considered**: a global/`OnceCell`. Rejected — it hides the
  dependency, complicates tests, and diverges from the `&Theme` convention.
- **Alternative considered**: per-call `if icons { … } else { … }`. Rejected —
  reintroduces the scatter and makes the fallback untrustworthy.

### Decision: `ascii` variant returns empty for field icons, keeping text
Field-icon accessors return `""` in the `ascii` variant and `"<glyph> "` in the
`nerd` variant, so callers unconditionally prepend the accessor result to the
existing text. Structural glyphs that already render everywhere (`·`, `─`, `▎`,
`❯`, spinner) keep their current character in both variants.

- **Why**: guarantees the `ascii` layout equals today's byte-for-byte for the
  text, so the escape hatch can never regress into tofu or shift columns.
- **Alternative considered**: ASCII stand-ins (`[b]`, `#`) for field icons in the
  `ascii` variant. Rejected — that is a *different* look, not a faithful
  fallback, and invites bikeshedding.

### Decision: per-agent glyph via `Adapter` trait (B-011), not the theme layer
Add an agent-agnostic method to the `Adapter` trait (e.g. `fn agent_glyph(&self)
-> char` or `&'static str`) with a safe default, overridden in `claude.rs`,
`codex.rs`, `cursor.rs`. The TUI obtains the glyph via the adapter, never by
matching `AgentId`.

- **Why**: B-011. Glyph choice is agent-specific knowledge and belongs in the
  adapter next to other per-agent quirks.
- **Note**: `Theme::agent_color` currently matches `AgentId` in the `tui` layer,
  which already bends B-011. We do the glyph the correct way rather than
  perpetuate the bend; realigning `agent_color` is out of scope but noted as a
  future cleanup.
- **Alternative considered**: mirror `agent_color` and match `AgentId` inside
  `Glyphs`. Rejected — consistent with existing code but perpetuates the boundary
  violation for brand-new agent knowledge.

### Decision: thorough centralization of existing glyph literals
Move `SELECTION_MARKER`, `SPINNER_FRAMES`, `ACCENT_BAR`, `SEP` (and the ~5
inlined `" · "` copies), `ARCHIVED_MARKER`, and the preview prefixes (`●`, `›`,
`•`) into `Glyphs`. Call sites read from `&Glyphs`.

- **Why**: one owner per glyph is what makes the fallback switch trustworthy and
  removes the pre-existing scatter the inventory surfaced. The facelift is the
  natural moment to pay this down.
- **Trade-off**: touches many files in one change. Mitigated by the mechanical
  nature of the edits and existing render tests.
- **Implementation refinement (content-layer exception):** the preview
  transcript's prefixes (`●` agent dot, `›` user prefix, `•` bullet, `─` rules)
  were left literal in `preview.rs` rather than routed through `Glyphs`. They are
  common-plane glyphs identical in both variants (so relocating them changes no
  behavior), they live in the *content* layer (the spec's own restraint keeps
  icons in chrome, not content), and threading `&Glyphs` through the transcript/
  prose render chain would churn ~15 signatures and their tests for zero visual
  gain. The centralized set therefore owns the chrome glyphs (selection marker,
  accent bar, separator, spinner, archived marker, agent/field/status icons); the
  content prefixes stay put. The search-input prompt `" ❯ "` likewise stays
  literal (input chrome, outside the relocation list).

### Decision: icons config lives on `DisplayConfig`
Add `icons: bool` (default `true`) to `DisplayConfig` in `src/config.rs`, beside
`row_style`. The reserved `theme` HashMap stays untouched.

- **Why**: `[display]` already governs presentation (`row_style`); `icons` is the
  same category of knob. A dedicated bool is clearer than overloading the
  reserved `theme` map.

### Decision: locked glyph code points
The `nerd` variant uses the following glyphs. The **Nerd Fonts class name is the
source of truth**; the code point is the convenience value to embed. All picks
are single-advance glyphs from the long-stable Font Awesome / Octicons ranges
that ship in every patched Nerd Font. Agent marks are evocative approximations —
no official Anthropic / OpenAI / Cursor logo exists in Nerd Fonts.

| Surface  | Nerd Fonts class            | Code point | Color         | Rationale            |
|----------|-----------------------------|------------|---------------|----------------------|
| claude   | `nf-fa-asterisk`            | `U+F069`   | brand (amber) | Anthropic sunburst-ish |
| codex    | `nf-fa-terminal`            | `U+F120`   | brand (purple)| code / CLI           |
| cursor   | `nf-fa-i_cursor`            | `U+F246`   | brand (green) | editor caret         |
| branch   | `nf-fa-code_fork`           | `U+F126`   | (field text)  | git branch           |
| repo     | `nf-fa-folder`              | `U+F07B`   | (field text)  | repo / dir           |
| pr       | `nf-oct-git_pull_request`   | `U+F407`   | (field text)  | pull request         |
| time     | `nf-fa-clock_o`             | `U+F017`   | (field text)  | relative time        |
| msgs     | `nf-fa-comments`            | `U+F086`   | (field text)  | message count        |
| archived | `nf-fa-archive`             | `U+F187`   | muted         | archived marker      |
| warning  | `nf-fa-exclamation_triangle`| `U+F071`   | `theme.warning` | warning status     |
| success  | `nf-fa-check`               | `U+F00C`   | `theme.success` | success status     |
| error    | `nf-fa-times`               | `U+F00D`   | `theme.error` | error status         |

- **Implementation note**: confirm `nf-oct-git_pull_request` against the current
  Nerd Fonts cheat sheet before embedding — the Octicons block has shifted code
  points historically. If it has drifted, resolve by class name. The Font Awesome
  picks (`F0xx`–`F2xx`) are stable.
- **Why FA/Octicons over Material Design**: the FA/Octicons ranges have shipped
  unchanged across Nerd Font releases far longer, minimizing tofu risk on older
  patched fonts.

## Risks / Trade-offs

- **[Tofu when default-on meets an unpatched font]** → The `ascii` escape hatch is
  documented in `README.md`; the `nerd` glyphs are confined to chrome so even a
  missed toggle degrades to a few stray boxes, never broken layout (widths are
  driven by the accompanying text, not the glyph).
- **[Glyph width / double-width cells]** → Some nerd glyphs advance two cells in
  some terminals. Mitigation: field widths are computed from the text, and the
  card/preview already truncate with ellipsis; pick single-advance glyphs and add
  a render test asserting `ascii` output is unchanged.
- **[Large diff across many render files]** → Centralization edits are mechanical;
  rely on existing TUI tests plus a new test that `Glyphs::ascii()` produces
  pre-change strings for the agent mark and metadata line.
- **[Scope creep toward images]** → Explicitly deferred; the fallback ladder built
  here is exactly what a future image tier degrades onto, so nothing is wasted.

## Migration Plan

Additive and config-gated; no data migration. Default flips the look to `nerd`
on next launch; users on unpatched fonts set `[display] icons = false`. Rollback
is the same toggle. Ship with `README.md` noting the patched-font requirement and
the opt-out.

## Open Questions

- Whether modal field labels and help section headings receive icons in this
  change or are left for a later polish pass (spec marks them optional).

(Glyph code points are locked — see "Decision: locked glyph code points".)
