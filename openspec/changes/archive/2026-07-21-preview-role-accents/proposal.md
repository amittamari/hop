## Why

When reading a transcript in the preview panel, user and agent messages are
visually identical — same color, same styling, same lack of glyphs. The only
differentiator is the label text in the thin-rule separator (`user` vs
`claude`), which requires actively reading it. This makes turn boundaries
harder to recognize at a glance.

## What Changes

- **Agent separator colored with brand color**: The thin-rule separator for
  agent turns uses the agent's brand color (amber/purple/green) instead of the
  neutral `theme.border` gray. The agent glyph (already registered via the
  `Adapter` trait) is added to the separator label.
- **User turns stay neutral**: User separators keep their current gray styling
  with no glyph.
- **Message bodies stay unchanged**: Prose and code retain their current
  rendering without role-specific prefixes or decoration.
- **Ascii fallback**: When icons are disabled, the agent glyph in the
  separator is omitted per existing `tui-icons` spec behavior.

## Capabilities

### New Capabilities
- `preview-role-accents`: Visual differentiation of user vs agent turns in the
  transcript preview via color-coded separators.

### Modified Capabilities
- `preview-rendering`: The separator rendering gains role-aware styling (color
  and optional glyph) instead of uniform gray for both roles.

## Impact

- `src/tui/preview.rs` — `thin_rule()` and `render_transcript()` gain
  role-aware separator color and glyph logic.
- `src/tui/theme.rs` — agent brand colors already exist; no new colors needed.
- `src/tui/glyphs.rs` — agent glyphs already registered; consumed in a new
  call site.
- No new dependencies. No config changes. No breaking changes.
