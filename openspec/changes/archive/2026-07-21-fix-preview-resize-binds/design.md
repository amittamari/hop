## Context

The preview resize keybindings (`resize_preview_smaller` / `resize_preview_larger`)
default to Ctrl+Left / Ctrl+Right. On macOS, these key combinations are intercepted
at the OS level by Mission Control (switch Spaces) before the terminal application
ever receives them. The application code is correct — unit tests pass and the resize
logic works — but the keystroke never arrives.

A secondary issue is that the app does not enable the Kitty keyboard protocol, which
limits reliable detection of modifier+arrow and modifier+punctuation keys in
terminals that support it.

## Goals / Non-Goals

**Goals:**

- Preview resize works out of the box on macOS and Linux.
- New defaults are universally reliable without depending on the Kitty protocol.
- Kitty keyboard protocol is enabled for improved modifier-key detection.
- Existing config overrides continue to work; only unoverridden defaults change.

**Non-Goals:**

- Adding new resize increments or gestures (mouse drag, variable step size).
- Changing any other default keybinding.
- Supporting Ctrl+punctuation as default bindings (requires Kitty; not universal).

## Decisions

### D-1: Change defaults to Ctrl+K (shrink) / Ctrl+L (grow)

**Choice:** Replace the default `resize_preview_smaller` chord from Ctrl+Left to
Ctrl+K and `resize_preview_larger` from Ctrl+Right to Ctrl+L.

**Why Ctrl+K / Ctrl+L:**

- Both are plain Ctrl+letter chords that work in all terminals without the Kitty
  protocol (they produce standard control characters 0x0B / 0x0C in legacy mode).
- Neither conflicts with macOS system shortcuts or existing hop bindings
  (taken: C, P, U, D, B, N, O, R).
- K and L are physically adjacent on QWERTY, with K left of L — matching the
  spatial metaphor of shrink-left / grow-right.
- "L for larger" is a reasonable mnemonic.

**Alternatives considered:**

- *Keep Ctrl+Left/Right:* Broken on macOS — the whole reason for this change.
- *Ctrl+H / Ctrl+L:* Ctrl+H (0x08) is indistinguishable from Backspace in legacy
  terminals. Would break backspace-to-delete in non-Kitty terminals.
- *Ctrl+, / Ctrl+. (comma/period):* Intuitive "< / >" mnemonic, but
  Ctrl+punctuation has no standard encoding in legacy terminal mode — requires
  Kitty protocol, making it a worse default than the current broken state on Linux.
- *Ctrl+- / Ctrl+=:* Universal zoom idiom, but same Kitty-only limitation as
  punctuation.

### D-2: Enable Kitty keyboard protocol

**Choice:** Push `KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES` on terminal
init and pop it on shutdown (alongside the existing mouse-capture
enable/disable). Gate the push behind `crossterm::supports_keyboard_enhancement()`
so legacy terminals are unaffected.

**Why:** The Kitty protocol lets crossterm distinguish Ctrl+H from Backspace,
modifier+arrow from plain arrow, and modifier+punctuation from plain punctuation.
This benefits users who rebind to Ctrl+Left/Right (where Mission Control is
disabled), Ctrl+punctuation, or other extended chords.

**Alternatives considered:**

- *Skip Kitty entirely:* Works for the default binding fix but leaves modifier-key
  detection fragile for user-configured overrides.
- *Enable full Kitty flags (report all keys, etc.):* Overkill; only
  `DISAMBIGUATE_ESCAPE_CODES` is needed and is the least-intrusive flag.

## Risks / Trade-offs

- **Existing muscle memory:** Users accustomed to Ctrl+Left/Right (e.g. on Linux
  where it worked) will need to relearn or add an explicit override in config.
  → Mitigation: document the change in the README; Ctrl+Left/Right can be restored
  via `[keybindings]`.
- **Kitty protocol interaction:** Enabling `DISAMBIGUATE_ESCAPE_CODES` changes the
  escape sequences crossterm receives. If crossterm 0.29 has any parsing edge cases,
  key events could arrive differently.
  → Mitigation: gate behind `supports_keyboard_enhancement()`; only enable on
  terminals that advertise support. The flag is the lowest-impact Kitty enhancement.
- **Terminal cleanup on panic:** If the process panics after pushing the protocol
  flag but before popping it, the terminal could be left in enhanced mode.
  → Mitigation: `ratatui::init()` already installs a panic hook that restores the
  terminal; extend it (or the existing drop guard) to also pop keyboard enhancement.
