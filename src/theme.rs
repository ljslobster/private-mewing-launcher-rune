use rune::{Color, TextStyle};

pub fn c(hex: &str) -> Color {
    let h = hex.trim_start_matches('#');
    let byte = |a: usize, b: usize| -> f32 {
        u8::from_str_radix(&h[a..b], 16).unwrap_or(0) as f32 / 255.0
    };
    let decode = |v: f32| {
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    let (r, g, b, a) = match h.len() {
        6 => (decode(byte(0, 2)), decode(byte(2, 4)), decode(byte(4, 6)), 1.0),
        8 => (decode(byte(0, 2)), decode(byte(2, 4)), decode(byte(4, 6)), byte(6, 8)),
        _ => (0.0, 0.0, 0.0, 1.0),
    };
    Color { r, g, b, a }
}

pub const FLOOR: &str = "#0B1017";
pub const SURFACE: &str = "#121B25";
pub const RAISED: &str = "#1A2735";
pub const BORDER: &str = "#263646";
pub const FOREGROUND: &str = "#EFF5FA";
pub const MUTED: &str = "#8495A7";
pub const MUTED_LIGHT: &str = "#A2B2C2";
pub const SECONDARY_TEXT: &str = "#BDCBD8";
pub const ACCENT: &str = "#68E0BC";
pub const ACCENT_SOFT: &str = "#193A35";
pub const WARNING: &str = "#F3C888";
pub const DANGER: &str = "#F48F9E";
pub const INK: &str = "#0B211D";

pub fn accent() -> Color { c(ACCENT) }
pub fn white() -> Color { c(FOREGROUND) }
pub fn style(size: f32, weight: f32) -> TextStyle { TextStyle::new(size, weight) }
