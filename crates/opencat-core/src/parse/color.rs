//! 脚本面向的颜色字面量解析：`#rgb` / `#rrggbb` / `#rrggbbaa` / `#rrggbb/50`
//! 与 `rgb()` / `rgba()`。
//!
//! 标准 CSS 声明走 lightningcss（见 [`crate::css`]）；这里只服务脚本 API
//! 传入的裸颜色字符串——它们不是声明块，没有属性名可供 lightningcss 识别。

use crate::style::ColorToken;

/// `#rgb`、`#rrggbb`、`#rrggbbaa`，可带 tailwind 风格的 `/NN` 透明度后缀。
pub fn color_from_hex(value: &str) -> Option<ColorToken> {
    let (hex_part, opacity_suffix) = match value.rsplit_once('/') {
        Some((hex, suffix)) => (hex, Some(suffix)),
        None => (value, None),
    };

    let hex = hex_part.strip_prefix('#')?;
    let (r, g, b, mut a) = match hex.len() {
        3 => {
            let r = parse_hex_nibble(hex.as_bytes()[0])?;
            let g = parse_hex_nibble(hex.as_bytes()[1])?;
            let b = parse_hex_nibble(hex.as_bytes()[2])?;
            (r * 17, g * 17, b * 17, 255)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b, 255)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            (r, g, b, a)
        }
        _ => return None,
    };

    if let Some(suffix) = opacity_suffix {
        let opacity_percent = suffix.parse::<f32>().ok()?;
        let opacity = (opacity_percent / 100.0).clamp(0.0, 1.0);
        a = ((a as f32) * opacity).round().clamp(0.0, 255.0) as u8;
    }

    Some(ColorToken::Custom(r, g, b, a))
}

/// `rgb(r, g, b)` / `rgba(r, g, b, a)`，逗号分隔。
pub fn parse_rgb_function_color(value: &str) -> Option<ColorToken> {
    let lower = value.trim();
    let (inner, has_alpha) = if let Some(inner) = lower
        .strip_prefix("rgba(")
        .and_then(|value| value.strip_suffix(')'))
    {
        (inner, true)
    } else {
        let inner = lower
            .strip_prefix("rgb(")
            .and_then(|value| value.strip_suffix(')'))?;
        (inner, false)
    };

    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    if (!has_alpha && parts.len() != 3) || (has_alpha && parts.len() != 4) {
        return None;
    }

    let r = parse_rgb_channel(parts[0])?;
    let g = parse_rgb_channel(parts[1])?;
    let b = parse_rgb_channel(parts[2])?;
    let a = if has_alpha {
        parse_alpha_channel(parts[3])?
    } else {
        255
    };
    Some(ColorToken::Custom(r, g, b, a))
}

fn parse_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_rgb_channel(value: &str) -> Option<u8> {
    value
        .parse::<f32>()
        .ok()
        .map(|value| value.round().clamp(0.0, 255.0) as u8)
}

fn parse_alpha_channel(value: &str) -> Option<u8> {
    if let Some(percent) = value.strip_suffix('%') {
        let alpha = percent.parse::<f32>().ok()? / 100.0;
        return Some((alpha * 255.0).round().clamp(0.0, 255.0) as u8);
    }

    let alpha = value.parse::<f32>().ok()?;
    let alpha = if alpha <= 1.0 { alpha * 255.0 } else { alpha };
    Some(alpha.round().clamp(0.0, 255.0) as u8)
}
