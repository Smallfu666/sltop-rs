use ratatui::style::{Color, Style};

pub const TEXT: Color = Color::White;
pub const MUTED: Color = Color::Gray;
pub const DIM: Color = Color::DarkGray;
pub const ACCENT: Color = Color::Rgb(0x44, 0x99, 0xdd);
pub const SUCCESS: Color = Color::Rgb(0x33, 0xcc, 0x55);
pub const WARNING: Color = Color::Rgb(0xdd, 0xaa, 0x00);
pub const DANGER: Color = Color::Rgb(0xdd, 0x44, 0x44);
pub const INFO: Color = Color::Rgb(0x44, 0xbb, 0xdd);
pub const HEADER_BG: Color = Color::Rgb(0x22, 0x22, 0x33);
pub const SELECTED_BG: Color = Color::Rgb(0x33, 0x44, 0x66);
pub const USER_BG: Color = Color::Rgb(0x33, 0x33, 0x22);
pub const CONFIRM_BG: Color = Color::Rgb(0x44, 0x22, 0x22);
pub const NODE_ALLOC: Color = Color::Rgb(0xcc, 0x44, 0x22);
pub const NODE_MIX: Color = Color::Rgb(0xdd, 0xaa, 0x00);
pub const NODE_IDLE: Color = Color::Rgb(0x33, 0xcc, 0x55);
pub const NODE_DRAIN: Color = Color::Rgb(0x66, 0x66, 0x66);

pub fn state_style(state: &str) -> Style {
    match state {
        "RUNNING" => Style::default().fg(SUCCESS),
        "PENDING" => Style::default().fg(WARNING),
        "COMPLETING" => Style::default().fg(INFO),
        "FAILED" | "CANCELLED" => Style::default().fg(DANGER),
        "TIMEOUT" => Style::default().fg(DANGER),
        _ => Style::default().fg(MUTED),
    }
}


