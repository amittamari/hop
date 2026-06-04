use crate::core::AgentId;
use ratatui::style::Color;

pub fn agent_color(agent: AgentId) -> Color {
    match agent {
        AgentId::Claude => Color::Magenta,
        AgentId::Codex => Color::Blue,
    }
}

pub const ACCENT: Color = Color::Cyan;
pub const DIM: Color = Color::DarkGray;
