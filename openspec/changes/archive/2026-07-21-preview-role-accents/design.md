## Context

The transcript preview in the TUI renders user and agent messages with
identical styling. The only differentiator is the label text in the thin-rule
separator (`── user ──────` vs `── claude ──────`). Agent responses frequently
fill the viewport, pushing the separator off-screen and leaving the reader with
no visual cue about who is speaking.

The codebase already has per-agent brand colors (`theme.agent_color()`) and
per-agent nerd-font glyphs (`glyphs.agent()`). Neither is currently used in the
transcript separator rendering path.

## Goals / Non-Goals

**Goals:**
- Make user vs agent turns distinguishable at a glance without reading text.
- Reuse the existing theme and glyph infrastructure — no new dependencies.

**Non-Goals:**
- Chat-bubble or indentation-based layout changes.
- Role-specific message body decoration.

## Decisions

### D1: Color the agent separator in brand color

The entire thin-rule line for agent turns — prefix dashes, label, and fill
dashes — uses `theme.agent_color(agent)` instead of `theme.border` /
`theme.preview_text`. User separators keep the current gray.

**Why not just the label?** The full-width color band registers peripherally.
Coloring only the label word still requires the eye to land on a small target.

### D2: Add agent glyph to the separator label

When icons are enabled, the agent separator label becomes
`"✱ claude"` (glyph + space + badge). When icons are disabled, the separator
renders the badge text alone — same as today but in brand color.

The glyph is sourced from `glyphs.agent(agent_id)`, preserving B-011 (no
agent-specific literals in generic layers).

## Risks / Trade-offs

- **[Separator scrolls away]** Long agent messages can push the role separator
  off-screen. The simpler visual treatment is preferred over persistent body
  decoration; users can scroll to the turn boundary when needed.
