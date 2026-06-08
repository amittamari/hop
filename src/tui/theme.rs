use crate::core::AgentId;
use ratatui::style::Color;

pub fn agent_color(agent: AgentId) -> Color {
    match agent {
        AgentId::Claude => Color::Rgb(245, 158, 11),
        AgentId::Codex => Color::Rgb(139, 92, 246),
        AgentId::Cursor => Color::Rgb(34, 197, 94),  // green
    }
}

pub const ACCENT: Color = Color::Cyan;
pub const DIM: Color = Color::DarkGray;
pub const DIVIDER: Color = Color::Rgb(55, 65, 81);
pub const OVERLAY_DIM: Color = Color::Rgb(64, 64, 64);
pub const PREVIEW_TEXT: Color = Color::Rgb(205, 213, 219);
pub const SELECTED_BG: Color = Color::Rgb(20, 83, 91);
pub const SELECTED_FG: Color = Color::White;
