//! Centralized glyph vocabulary for the TUI, mirroring [`Theme`]. A single
//! `Glyphs` value is chosen once at startup and threaded read-only through the
//! render path (carried on `App`, like the theme).
//!
//! Two variants exist:
//! - [`Glyphs::nerd`] renders Private Use Area (PUA) nerd-font icons. This is
//!   the default (opt-out via `[display] icons = false`) and requires a patched
//!   Nerd Font.
//! - [`Glyphs::ascii`] reproduces the pre-icon look: field icons contribute the
//!   empty string, so the surrounding text and layout are byte-for-byte what
//!   they were before this facelift. No PUA code point appears, so there is no
//!   tofu on an unpatched font.
//!
//! Field-icon accessors carry a trailing space in the `nerd` variant so callers
//! can unconditionally prepend them to the field text. Structural glyphs
//! (`selection_marker`, `accent_bar`, `sep`, the spinner) are common-plane
//! Unicode that renders in any modern monospace font, so they are identical in
//! both variants.
//!
//! Per architecture rule B-011 this generic layer never names an agent-specific
//! glyph literal: per-agent glyphs are supplied by each [`Adapter`] and injected
//! via [`Glyphs::set_agent_glyph`], keyed by position in [`AgentId::ALL`].
//!
//! [`Theme`]: crate::tui::theme::Theme
//! [`Adapter`]: crate::adapters::Adapter

use crate::core::AgentId;

/// Braille throbber frames, indexed by the per-redraw frame counter. Common-plane
/// glyphs, so identical in both variants — kept as a module constant that the
/// spinner accessor and the legacy `view` re-export both read.
pub(crate) const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// The selected, chosen-once glyph set. See the module docs for the two variants.
#[derive(Debug, Clone)]
pub struct Glyphs {
    nerd: bool,
    /// Per-agent mark glyph, indexed by position in [`AgentId::ALL`]. Populated
    /// from the adapters via [`set_agent_glyph`](Glyphs::set_agent_glyph); empty
    /// until then. Only surfaced in the `nerd` variant.
    agent: [&'static str; AgentId::ALL.len()],
    branch: &'static str,
    repo: &'static str,
    pr: &'static str,
    time: &'static str,
    msgs: &'static str,
    archived: &'static str,
    warning: &'static str,
    success: &'static str,
    error: &'static str,
    selection_marker: &'static str,
    accent_bar: &'static str,
    sep: &'static str,
}

impl Glyphs {
    /// The escape-hatch variant: no icons, pre-facelift text and layout.
    pub const fn ascii() -> Self {
        Self {
            nerd: false,
            agent: [""; AgentId::ALL.len()],
            branch: "",
            repo: "",
            pr: "",
            time: "",
            msgs: "",
            archived: "arch ",
            warning: "",
            success: "",
            error: "",
            selection_marker: "❯ ",
            accent_bar: "▎",
            sep: " · ",
        }
    }

    /// The default variant: nerd-font PUA icons. Field icons carry a trailing
    /// space. Code points are the locked set from the change design (Font
    /// Awesome / Octicons ranges).
    pub const fn nerd() -> Self {
        Self {
            nerd: true,
            agent: [""; AgentId::ALL.len()],
            branch: "\u{f126} ",   // nf-fa-code_fork
            repo: "\u{f07b} ",     // nf-fa-folder
            pr: "\u{f407} ",       // nf-oct-git_pull_request
            time: "\u{f017} ",     // nf-fa-clock_o
            msgs: "\u{f086} ",     // nf-fa-comments
            archived: "\u{f187} ", // nf-fa-archive
            warning: "\u{f071} ",  // nf-fa-exclamation_triangle
            success: "\u{f00c} ",  // nf-fa-check
            error: "\u{f00d} ",    // nf-fa-times
            selection_marker: "❯ ",
            accent_bar: "▎",
            sep: " · ",
        }
    }

    /// Select the variant from the resolved config flag (`[display] icons`).
    pub fn from_icons_enabled(enabled: bool) -> Self {
        if enabled { Self::nerd() } else { Self::ascii() }
    }

    /// Record an agent's mark glyph, supplied by that agent's adapter. Keyed by
    /// position in [`AgentId::ALL`] so this stays agent-agnostic (B-011).
    pub fn set_agent_glyph(&mut self, agent: AgentId, glyph: &'static str) {
        if let Some(i) = AgentId::ALL.iter().position(|a| *a == agent) {
            self.agent[i] = glyph;
        }
    }

    /// Whether this is the nerd (icon) variant.
    pub fn nerd_enabled(&self) -> bool {
        self.nerd
    }

    /// The agent's mark glyph, or `""` when icons are disabled or no glyph was
    /// registered for the agent. Bare glyph (no trailing space); callers add the
    /// gap between the mark and the text label.
    pub fn agent(&self, agent: AgentId) -> &'static str {
        if !self.nerd {
            return "";
        }
        AgentId::ALL.iter().position(|a| *a == agent).map(|i| self.agent[i]).unwrap_or("")
    }

    pub fn branch(&self) -> &'static str {
        self.branch
    }
    pub fn repo(&self) -> &'static str {
        self.repo
    }
    pub fn pr(&self) -> &'static str {
        self.pr
    }
    pub fn time(&self) -> &'static str {
        self.time
    }
    pub fn msgs(&self) -> &'static str {
        self.msgs
    }
    /// The archived marker prefixed to a title: `"arch "` in ascii, an archive
    /// icon (with trailing space) in nerd.
    pub fn archived_marker(&self) -> &'static str {
        self.archived
    }
    pub fn warning(&self) -> &'static str {
        self.warning
    }
    pub fn success(&self) -> &'static str {
        self.success
    }
    pub fn error(&self) -> &'static str {
        self.error
    }
    pub fn selection_marker(&self) -> &'static str {
        self.selection_marker
    }
    pub fn accent_bar(&self) -> &'static str {
        self.accent_bar
    }
    pub fn sep(&self) -> &'static str {
        self.sep
    }

    /// The throbber glyph for a given per-redraw frame counter.
    pub fn spinner_frame(&self, frame: u64) -> &'static str {
        SPINNER_FRAMES[(frame as usize) % SPINNER_FRAMES.len()]
    }
}

impl Default for Glyphs {
    /// Defaults to the safe `ascii` variant. Production selects the real variant
    /// from config; this keeps constructs like `App::new()` tofu-free until set.
    fn default() -> Self {
        Self::ascii()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_field_icons_are_empty_and_have_no_pua() {
        let g = Glyphs::ascii();
        for s in
            [g.branch(), g.repo(), g.pr(), g.time(), g.msgs(), g.warning(), g.success(), g.error()]
        {
            assert!(s.is_empty(), "ascii field icon must be empty, got {s:?}");
        }
        // Agent glyph is suppressed in ascii even if one is registered.
        let mut g = Glyphs::ascii();
        g.set_agent_glyph(AgentId::Claude, "\u{f069}");
        assert_eq!(g.agent(AgentId::Claude), "");
        // No accessor exposes a Private Use Area code point in ascii.
        let is_pua = |c: char| ('\u{e000}'..='\u{f8ff}').contains(&c);
        for s in [g.archived_marker(), g.selection_marker(), g.accent_bar(), g.sep()] {
            assert!(!s.chars().any(is_pua), "ascii glyph {s:?} must not contain PUA");
        }
    }

    #[test]
    fn ascii_structural_glyphs_match_pre_change_literals() {
        let g = Glyphs::ascii();
        assert_eq!(g.selection_marker(), "❯ ");
        assert_eq!(g.accent_bar(), "▎");
        assert_eq!(g.sep(), " · ");
        assert_eq!(g.archived_marker(), "arch ");
        assert_eq!(g.spinner_frame(0), SPINNER_FRAMES[0]);
    }

    #[test]
    fn nerd_field_icons_carry_pua_and_trailing_space() {
        let g = Glyphs::nerd();
        let is_pua = |c: char| ('\u{e000}'..='\u{f8ff}').contains(&c);
        for s in
            [g.branch(), g.repo(), g.pr(), g.time(), g.msgs(), g.archived_marker(), g.warning()]
        {
            assert!(s.chars().any(is_pua), "nerd icon {s:?} should contain a PUA glyph");
            assert!(s.ends_with(' '), "nerd field icon {s:?} should carry a trailing space");
        }
    }

    #[test]
    fn nerd_agent_glyph_round_trips_by_agent() {
        let mut g = Glyphs::nerd();
        g.set_agent_glyph(AgentId::Codex, "\u{f120}");
        assert_eq!(g.agent(AgentId::Codex), "\u{f120}");
        // Unregistered agents resolve to empty, not a panic.
        assert_eq!(g.agent(AgentId::Cursor), "");
    }

    #[test]
    fn from_icons_enabled_selects_variant() {
        assert!(Glyphs::from_icons_enabled(true).nerd_enabled());
        assert!(!Glyphs::from_icons_enabled(false).nerd_enabled());
    }
}
