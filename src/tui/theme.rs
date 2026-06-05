use crate::core::AgentId;
use ratatui::style::Color;

pub fn agent_color(agent: AgentId) -> Color {
    match agent {
        AgentId::Claude => Color::Rgb(245, 158, 11),
        AgentId::Codex => Color::Rgb(139, 92, 246),
    }
}

pub const ACCENT: Color = Color::Cyan;
pub const DIM: Color = Color::DarkGray;
