use ratatui::style::Color;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub bg: String,
    pub fg: String,
    pub primary: String,
    pub secondary: String,
    pub highlight_bg: String,
    pub highlight_fg: String,
    pub border: String,
    pub error: String,
    pub dir_color: String,
    pub file_color: String,
    pub symlink_color: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            bg: "Reset".to_string(),
            fg: "Reset".to_string(),
            primary: "Cyan".to_string(),
            secondary: "DarkGray".to_string(),
            highlight_bg: "Blue".to_string(),
            highlight_fg: "White".to_string(),
            border: "DarkGray".to_string(),
            error: "Red".to_string(),
            dir_color: "Cyan".to_string(),
            file_color: "Reset".to_string(),
            symlink_color: "Magenta".to_string(),
        }
    }
}

pub fn parse_color(color_str: &str) -> Color {
    match color_str.to_lowercase().as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" => Color::Gray,
        "darkgray" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightyellow" => Color::LightYellow,
        "lightblue" => Color::LightBlue,
        "lightmagenta" => Color::LightMagenta,
        "lightcyan" => Color::LightCyan,
        "white" => Color::White,
        "reset" => Color::Reset,
        _ => {
            // hex parsing placeholder
            if color_str.starts_with('#') && color_str.len() == 7 {
                let r = u8::from_str_radix(&color_str[1..3], 16).unwrap_or(255);
                let g = u8::from_str_radix(&color_str[3..5], 16).unwrap_or(255);
                let b = u8::from_str_radix(&color_str[5..7], 16).unwrap_or(255);
                Color::Rgb(r, g, b)
            } else {
                Color::Reset
            }
        }
    }
}
