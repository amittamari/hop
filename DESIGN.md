---
version: alpha
name: hop
description: >-
  Visual identity for hop, a Rust CLI/TUI (ratatui) that indexes, searches, and
  previews Claude Code / Codex / Cursor session transcripts in the terminal.
  This is a terminal design system: "colors" are ANSI-16 names (which adapt to
  the user's terminal palette) plus true-color RGB accents; "typography" is a
  fixed monospace grid whose only expressive axis is cell attributes (bold, dim,
  italic, reversed); "spacing" is measured in character cells; and shapes are
  single-line box-drawing, so there are no corner radii. Source of truth for the
  palette is src/tui/theme.rs.
colors:
  # Base — deferred to the terminal's own default fg/bg so hop blends in.
  background: transparent      # Color::Reset — the terminal's own background
  foreground: transparent      # Color::Reset — the terminal's own foreground
  # Neutrals & structure (ANSI names adapt to the user's theme).
  muted: darkgray              # Color::DarkGray — secondary text, headers, hints
  border: "#374151"            # Color::Rgb(55,65,81) — dividers, rules, separators
  # Primary accent.
  primary: cyan                # Color::Cyan — prompt, borders, keys, headings
  # Selection.
  on-selection: white          # Color::White — selected-row / live-query text
  selection: "#14535b"         # Color::Rgb(20,83,91) — selected-row background (dark teal)
  # Content & semantic.
  code: yellow                 # Color::Yellow — inline code spans in prose
  warning: yellow              # Color::Yellow — YOLO banner, footer + modal warnings
  error: red                   # Color::Red — reserved
  success: green               # Color::Green — reserved
  preview-text: "#cdd5db"      # Color::Rgb(205,213,219) — preview body + header (light gray)
  # Overlay scrim (dims the app behind modals / help).
  overlay-fg: "#404040"        # Color::Rgb(64,64,64)
  overlay-bg: "#0c0c0c"        # Color::Rgb(12,12,12)
  # Match highlight is drawn with the REVERSED attribute, not a color; this
  # token is reserved to later unify inline-match highlighting on one color.
  match: cyan                  # Color::Cyan — reserved (currently unused)
  # Agent brand colors — the one place hop paints with true color on purpose.
  agent-claude: "#f59e0b"      # amber
  agent-codex: "#8b5cf6"       # violet
  agent-cursor: "#22c55e"      # green
typography:
  # A terminal has one font (the user's monospace) at one size. The design
  # system's only typographic tools are cell attributes. Each role below is a
  # named attribute combination; fontWeight 700 == BOLD, 400 == normal. The
  # `attributes` key is non-standard and documents the ratatui Modifier set.
  body:
    fontFamily: monospace
    fontWeight: 400
    attributes: none
  emphasis:
    fontFamily: monospace
    fontWeight: 700
    attributes: BOLD
  heading:
    fontFamily: monospace
    fontWeight: 700
    attributes: BOLD
  prose-em:
    fontFamily: monospace
    fontWeight: 400
    attributes: ITALIC
  archived:
    fontFamily: monospace
    fontWeight: 400
    attributes: DIM
  match:
    fontFamily: monospace
    fontWeight: 400
    attributes: REVERSED
rounded:
  # No corner radii exist in a character grid. Borders are single-line
  # box-drawing glyphs. This token group is intentionally flat.
  none: 0px
spacing:
  # Unit is one character cell. Widths/heights are column/row counts.
  cell: 1
  column-spacing: 1        # gap between table columns
  toolbar-indent: 2        # leading indent before toolbar controls
  preview-pad-left: 1      # inner padding of the preview pane
  modal-pad-x: 2           # modal / help horizontal padding
  modal-pad-y: 1           # modal / help vertical padding
  list-min-width: 48       # results list never renders narrower than this
  preview-min-width: 100   # body width required before the preview pane appears
  modal-width: 72          # resume/confirm modal width
  help-width: 58           # help overlay width
components:
  search-input:
    textColor: "{colors.on-selection}"
    typography: "{typography.body}"
  search-prompt:
    textColor: "{colors.primary}"       # the " ❯ " glyph
  search-position:
    textColor: "{colors.muted}"         # "  pos/total"
  results-header:
    textColor: "{colors.muted}"
  results-row:
    backgroundColor: transparent
    textColor: "{colors.foreground}"
    typography: "{typography.body}"
  results-row-selected:
    backgroundColor: "{colors.selection}"
    textColor: "{colors.on-selection}"
    typography: "{typography.emphasis}"
  results-row-archived:
    textColor: "{colors.muted}"
    typography: "{typography.archived}"
  selection-marker:
    textColor: "{colors.on-selection}"  # the "❯ " highlight symbol
  agent-badge-claude:
    textColor: "{colors.agent-claude}"
  agent-badge-codex:
    textColor: "{colors.agent-codex}"
  agent-badge-cursor:
    textColor: "{colors.agent-cursor}"
  toolbar-control:
    textColor: "{colors.primary}"       # "Scope: all", "Sort: recent"
  toolbar-control-focused:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-selection}"
    typography: "{typography.emphasis}"
  preview-pane:
    textColor: "{colors.preview-text}"
    typography: "{typography.body}"
    padding: "{spacing.preview-pad-left}"
  preview-divider:
    textColor: "{colors.border}"        # Borders::LEFT + header rule "─"
  preview-heading:
    textColor: "{colors.primary}"
    typography: "{typography.heading}"
  preview-inline-code:
    textColor: "{colors.code}"
  modal:
    textColor: "{colors.muted}"
    padding: "{spacing.modal-pad-x}"
    width: "{spacing.modal-width}"
  modal-border:
    textColor: "{colors.primary}"
  modal-title:
    textColor: "{colors.primary}"
    typography: "{typography.heading}"
  modal-warning:
    textColor: "{colors.warning}"
    typography: "{typography.emphasis}"
  modal-backdrop:
    backgroundColor: "{colors.overlay-bg}"
    textColor: "{colors.overlay-fg}"
  help-overlay:
    padding: "{spacing.modal-pad-x}"
    width: "{spacing.help-width}"
  help-border:
    textColor: "{colors.primary}"
  key-hint:
    textColor: "{colors.primary}"       # keys shown in accent, labels muted
  footer:
    textColor: "{colors.muted}"
  footer-primary:
    typography: "{typography.emphasis}" # the first, primary hint is bold
  spinner:
    textColor: "{colors.muted}"         # braille throbber ⠋⠙⠹…
  warning-banner:
    textColor: "{colors.warning}"       # YOLO / --yolo banner
  empty-state:
    textColor: "{colors.muted}"
---

# hop DESIGN.md

## Overview

`hop` is a keyboard-driven terminal tool for finding a past coding-agent session
and jumping back into it. Its personality is **quiet, fast, and native to the
terminal**. The UI should feel like it belongs to the shell it runs in, not like
a colorful app dropped on top of one: it defers to the user's own terminal
colors and font, spends color budget only where it carries meaning, and never
decorates for decoration's sake.

The emotional target is *calm competence*. A session list should scan like
`git log` or `fzf` — dense, legible, instantly navigable — while the preview
pane reads like a clean transcript. Motion is minimal (a single braille spinner
for background work); everything else is static so the eye can rest. When in
doubt, remove chrome rather than add it, and let the terminal's own theme show
through.

Because this is a text-mode UI, the usual design primitives are reinterpreted:
color is the ANSI-16 palette (theme-adaptive) plus a few true-color accents;
typography is a fixed monospace grid whose only expressive axis is cell
attributes; layout is measured in character cells; and "shape" is single-line
box-drawing. The tokens above are the normative values; the prose explains how
to apply them.

## Colors

The palette is deliberately small and role-based. It lives in exactly one place,
`src/tui/theme.rs` (`Theme::default`), and is threaded through render via
`RenderModel`. Two principles govern it:

1. **Defer to the terminal.** Base foreground and background are `Color::Reset`,
   and most neutrals and semantics use ANSI-16 names (`darkgray`, `cyan`,
   `yellow`, `red`, `green`, `white`). These re-map to whatever palette the
   user's terminal theme defines, so hop looks at home in Solarized, Dracula,
   or a stock profile without any per-theme work.
2. **Spend true color only on meaning.** Explicit RGB values are reserved for
   things that must look the same everywhere: the selection band, the preview
   body text, structural borders, the overlay scrim, and the per-agent brand
   colors.

- **Primary — Cyan (`cyan`):** the single accent. It marks the interactive edge
  of the UI — the search prompt `❯`, overlay borders, key hints, section
  headings, and resolved metadata. Use it sparingly; if everything is accented,
  nothing is.
- **Muted — Dark Gray (`darkgray`):** the workhorse for anything secondary —
  column headers, timestamps, message counts, footer hints, separators, and the
  spinner. Most of the screen is muted; accent and selection punch through it.
- **Selection — dark teal band (`#14535b`) with white text (`white`):** the one
  strong fill in the UI, applied only to the currently selected result row
  (plus bold). It reads as "you are here" without shouting.
- **Border — slate (`#374151`):** structural lines only — the preview pane's
  left divider, the rule under the preview header, and separators inside
  overlays. Never used for text.
- **Preview text — light gray (`#cdd5db`):** body and header text in the preview
  pane, a touch dimmer than pure white for comfortable long-form reading.
- **Code — Yellow (`yellow`):** inline `code` spans in rendered prose. Fenced
  code blocks are the one exception to the semantic palette — they are syntax-
  highlighted with syntect's `base16-ocean.dark` theme and emit raw RGB per
  token on purpose (see `preview.rs`); do not try to fold those into roles.
- **Warning — Yellow (`yellow`):** the YOLO/`--yolo` banner and cautionary text
  in the footer and resume modal (missing or archived directories).
- **Error / Success (`red` / `green`):** defined and reserved; not yet surfaced.
- **Agent brand colors:** Claude amber (`#f59e0b`), Codex violet (`#8b5cf6`),
  Cursor green (`#22c55e`), dispatched by `Theme::agent_color`. Used for the
  AGENT column badge and the `●` role marker in the preview so the source agent
  is identifiable at a glance.

Match highlighting is drawn with the `REVERSED` attribute rather than a color;
the `match` token (Cyan) is reserved for a future unification of the two
match affordances and is currently unused.

## Typography

There is one typeface — the user's terminal monospace — at one size. The design
system's entire typographic vocabulary is therefore **cell attributes**
(`ratatui::style::Modifier`). Hierarchy comes from attribute + color, never from
size or font family.

- **Emphasis / Heading — BOLD:** selected rows, section and modal titles,
  overlay headings, the primary footer hint, the focused toolbar value, and
  markdown headings/strong. Bold is the "louder" tier; pair it with accent for
  headings and with the selection band for the current row.
- **Prose emphasis — ITALIC:** markdown `*emphasis*` in rendered transcript
  prose only.
- **Archived — DIM:** an entire archived result row is dimmed, pushing it behind
  live sessions without hiding it.
- **Match — REVERSED:** inline query-match highlighting inverts fg/bg at the
  glyph level, in both result titles and the preview. It is intentionally
  distinct from the selection band so a match inside the selected row is still
  visible.

Rule of thumb: at most one attribute tier per span. Don't stack BOLD+REVERSED or
DIM+BOLD — in a terminal these combine unpredictably across emulators.

## Layout

The screen is a single full-height frame divided into four horizontal bands,
top to bottom (`src/tui/view.rs::render`). Spacing is counted in character
cells; there are no gutters — regions are adjacent and separated by attribute or
a single box-drawing line, not by whitespace.

1. **Header — height 1:** the search input row (prompt, live query, position).
2. **Toolbar — height 0–N:** the Scope / Sort controls. Collapses to 0 rows in
   raw-query mode.
3. **Body — flexible (fills remaining height):** the results list, with an
   optional preview pane on the right.
4. **Footer — height 1:** key hints on the left, volatile status on the right,
   laid out with `Flex::SpaceBetween` so the status survives when the terminal
   is narrow.

**Body split.** The preview pane appears only when it is requested *and* the
body is at least `preview-min-width` (100 cells) wide. When shown, the body
splits horizontally into a results list (floored at `list-min-width`, 48 cells,
so its columns never collapse) and a preview taking the remaining width
(default 50%). Below the threshold, the list takes the whole body.

**Responsive columns.** The results table (AGENT · REPO · BRANCH · TITLE · MSGS
· PR · TIME) uses a priority-based solver (`src/tui/columns.rs`): as width
shrinks, low-priority columns drop out entirely, but TITLE and AGENT never drop.
Overflowing cells truncate with `…` — path-like cells truncate from the *start*
(`…spaces/personal/hop`) to keep the meaningful tail.

**Too-small guard.** Below 30×6 cells the UI refuses to draw and shows a single
centered muted message.

## Elevation & Depth

A terminal is flat — there are no shadows or z-height. Depth is expressed three
ways, in increasing strength:

1. **Attribute recession.** Muted text and `DIM` push content back; bold and
   accent bring it forward. This is the primary, everyday hierarchy.
2. **A single structural divider.** The preview pane is separated from the list
   by a one-cell `Borders::LEFT` line in the border color — the only persistent
   "seam" in the layout.
3. **Scrim + border for true overlays.** Modals (resume/confirm) and the help
   panel sit "above" the app: the entire background is repainted with the
   overlay scrim (`overlay-fg` on `overlay-bg`) to recede it, the overlay rect
   is `Clear`ed, and the panel is drawn as a bordered block with an accent
   border. The scrim is what signals modality — not a drop shadow.

## Shapes

The shape language is **single-line box-drawing**, inherited from ratatui's
default border set (`Block::bordered()`, `Borders::LEFT`). There are no rounded
corners in a character grid, which is why the `rounded` token group holds only
`none`. Keep it that way: mixing border sets (double lines, heavy lines, rounded
corners) across overlays would break the uniform, understated frame. Horizontal
rules (e.g. under the preview header) use `─` in the border color.

## Components

Structure and exact values live in the token block; this section is applied
guidance. Component tokens use terminal-appropriate properties — `textColor`,
`backgroundColor`, `typography`, and cell-based `padding`/`width` — so `rounded`
never appears.

### Search input (header)

Prompt glyph ` ❯ ` in accent, live query in white, and a muted `pos/total`
counter trailing it. The hardware cursor is positioned in the query text. When
indexing runs in the background, a muted `spinner indexing N…` is appended —
never a blocking state.

### Results table

A `ratatui::Table` with a muted header row, single-cell column spacing, and a
`❯ ` selection marker. The selected row gets the teal selection band + white +
bold; archived rows are prefixed `arch ` and dimmed whole-row. The AGENT column
renders an uppercase text badge (`CLAUDE`/`CODEX`/`CURSOR`) in that agent's
brand color — badges are text, not icons. Missing enrichment values render as a
muted `—`; a slow-to-resolve cell (e.g. PR) shows the spinner until it resolves.

### Toolbar (Scope / Sort)

Each control is `label: value`, label muted, value in accent. The focused
control inverts to a white-on-accent bold badge so keyboard focus is
unambiguous. Controls are indented two cells and separated by three spaces.

### Preview pane

A left-bordered block (border color) with one cell of left padding; body text in
light gray. An optional three-row header (title + metadata, separated by a `─`
rule) sits above the transcript body. Transcript rendering: user messages get a
`› ` prefix in accent, agent messages a `●` marker in the agent's brand color,
markdown headings/strong → bold, emphasis → italic, inline code → yellow, list
items → `•`. The pane scrolls with wrap; there is no scrollbar widget.

### Overlays (modal & help)

Bordered blocks with an accent border and title, `2×1` padding, fixed widths
(modal 72, help 58), centered via `Flex::Center` over the scrim described in
Elevation. Body labels are bold; warnings are bold in the warning color. Key
hints render the key in accent and its label muted, separated by ` · `.

### Footer

Muted key hints on the left (the first, primary hint bolded), volatile status on
the right. It is one row and must degrade gracefully — the status is allowed to
win the space when the terminal is narrow.

### Spinner

The only animation: a 10-frame braille throbber (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) in muted,
advanced on the ~50 ms redraw tick. Used exclusively to indicate background work
(indexing, pending enrichment). Nothing else moves.

## Do's and Don'ts

- **Do** define every color as a semantic role in `src/tui/theme.rs` and
  reference the role — never hardcode a `Color` at a call site.
- **Do** prefer ANSI-16 names for neutrals and semantics so hop adapts to the
  user's terminal theme; reserve true-color RGB for selection, preview text,
  borders, the scrim, and agent brand colors.
- **Do** convey hierarchy with attributes (muted → normal → bold) and reserve
  the teal band for the single selected row.
- **Do** keep accent (cyan) rare — it should mark the interactive edge, not fill
  the screen.
- **Do** truncate with `…` and drop low-priority columns responsively; keep
  TITLE and AGENT always visible.
- **Don't** stack cell attributes (e.g. BOLD+REVERSED, DIM+BOLD) — they render
  inconsistently across terminal emulators.
- **Don't** introduce a second border style; all frames use the default
  single-line box-drawing set.
- **Don't** paint solid backgrounds beyond the selection band and the overlay
  scrim — the terminal's own background should show through everywhere else.
- **Don't** add motion beyond the single braille spinner, and never block the
  render path on network, disk scans, or large work.
- **Don't** fold syntect's fenced-code-block colors into the semantic palette;
  they are an intentional true-color island.
