use crate::core::AgentId;
use ratatui::style::Color;

/// Semantic color roles for the TUI. Internal only: a single hardcoded
/// default for now (no config wiring). `Copy` so it can be cheaply lifted
/// into locals when a `&mut App` borrow is in scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub muted: Color,
    pub accent: Color,
    pub code: Color,
    pub border: Color,
    pub overlay_fg: Color,
    pub overlay_bg: Color,
    pub selection_fg: Color,
    pub selection_bg: Color,
    pub match_fg: Color,
    pub warning: Color,
    pub error: Color,
    pub success: Color,
    pub preview_text: Color,
    pub agent_claude: Color,
    pub agent_codex: Color,
    pub agent_cursor: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Reset,
            fg: Color::Reset,
            muted: Color::DarkGray,                  // was DIM
            accent: Color::Cyan,                     // was ACCENT
            code: Color::Yellow,                     // was inline Color::Yellow (T3)
            border: Color::Rgb(55, 65, 81),          // was DIVIDER
            overlay_fg: Color::Rgb(64, 64, 64),      // was OVERLAY_DIM
            overlay_bg: Color::Rgb(12, 12, 12),      // NEW: real scrim bg (T2)
            selection_fg: Color::White,              // was SELECTED_FG
            selection_bg: Color::Rgb(20, 83, 91),    // was SELECTED_BG
            match_fg: Color::Cyan,                   // NEW: reserved for future unification (T6)
            warning: Color::Yellow,                  // NEW (T1)
            error: Color::Red,                       // NEW
            success: Color::Green,                   // NEW
            preview_text: Color::Rgb(205, 213, 219), // was PREVIEW_TEXT
            agent_claude: Color::Rgb(245, 158, 11),
            agent_codex: Color::Rgb(139, 92, 246),
            agent_cursor: Color::Rgb(34, 197, 94),
        }
    }
}

impl Theme {
    pub fn agent_color(&self, agent: AgentId) -> Color {
        match agent {
            AgentId::Claude => self.agent_claude,
            AgentId::Codex => self.agent_codex,
            AgentId::Cursor => self.agent_cursor,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::AgentId;

    #[test]
    fn default_theme_distinguishes_warning_error_accent() {
        let t = Theme::default();
        assert_ne!(t.warning, t.accent);
        assert_ne!(t.error, t.accent);
        assert_ne!(t.warning, t.error);
        assert_ne!(t.warning, t.success);
    }

    #[test]
    fn default_theme_maps_legacy_constants() {
        let t = Theme::default();
        assert_eq!(t.muted, Color::DarkGray);
        assert_eq!(t.accent, Color::Cyan);
        assert_eq!(t.selection_fg, Color::White);
        assert_eq!(t.warning, Color::Yellow);
        assert_eq!(t.error, Color::Red);
        assert_eq!(t.success, Color::Green);
    }

    #[test]
    fn agent_color_method_matches_brand_colors() {
        let t = Theme::default();
        assert_eq!(t.agent_color(AgentId::Claude), Color::Rgb(245, 158, 11));
        assert_eq!(t.agent_color(AgentId::Codex), Color::Rgb(139, 92, 246));
        assert_eq!(t.agent_color(AgentId::Cursor), Color::Rgb(34, 197, 94));
    }
}
