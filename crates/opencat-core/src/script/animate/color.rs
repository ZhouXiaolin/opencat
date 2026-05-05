//! HSLA color parsing + lerp + back-conversion. Used by `__animate_color`.

#[derive(Clone, Copy, Debug)]
pub struct HSLA {
    pub h: f32,
    pub s: f32,
    pub l: f32,
    pub a: f32,
}

pub fn parse_color(input: &str) -> Option<HSLA> {
    let s = input.trim();
    if let Some(rest) = s.strip_prefix('#') { return parse_hex(rest); }
    if let Some(rest) = s.strip_prefix("rgb(").and_then(|r| r.strip_suffix(')')) {
        return parse_rgb_args(rest, 1.0).map(|(r, g, b, _)| rgb_to_hsl(r, g, b, 1.0));
    }
    if let Some(rest) = s.strip_prefix("rgba(").and_then(|r| r.strip_suffix(')')) {
        if let Some((r, g, b, a)) = parse_rgba_args(rest) {
            return Some(rgb_to_hsl(r, g, b, a));
        }
    }
    if let Some(rest) = s.strip_prefix("hsl(").and_then(|r| r.strip_suffix(')')) {
        return parse_hsl_args(rest, 1.0);
    }
    if let Some(rest) = s.strip_prefix("hsla(").and_then(|r| r.strip_suffix(')')) {
        return parse_hsla_args(rest);
    }
    None
}

fn parse_hex(hex: &str) -> Option<HSLA> {
    let (r, g, b, a) = match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            (r, g, b, 1.0f32)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b, 1.0f32)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            (r, g, b, a as f32 / 255.0)
        }
        _ => return None,
    };
    Some(rgb_to_hsl(r, g, b, a))
}

fn parse_hsl_args(args: &str, default_alpha: f32) -> Option<HSLA> {
    let parts: Vec<&str> = args.split(',').collect();
    if parts.len() != 3 { return None; }
    let h = parts[0].trim().trim_end_matches("deg").parse().ok()?;
    let s = parse_percentage(parts[1])?;
    let l = parse_percentage(parts[2])?;
    Some(HSLA { h, s: s / 100.0, l: l / 100.0, a: default_alpha })
}

fn parse_hsla_args(args: &str) -> Option<HSLA> {
    let parts: Vec<&str> = args.split(',').collect();
    if parts.len() != 4 { return None; }
    let h = parts[0].trim().trim_end_matches("deg").parse().ok()?;
    let s = parse_percentage(parts[1])?;
    let l = parse_percentage(parts[2])?;
    let a = parts[3].trim().parse().ok()?;
    Some(HSLA { h, s: s / 100.0, l: l / 100.0, a })
}

fn parse_rgb_args(args: &str, default_alpha: f32) -> Option<(u8, u8, u8, f32)> {
    let parts: Vec<&str> = args.split(',').collect();
    if parts.len() != 3 { return None; }
    Some((parts[0].trim().parse().ok()?, parts[1].trim().parse().ok()?, parts[2].trim().parse().ok()?, default_alpha))
}

fn parse_rgba_args(args: &str) -> Option<(u8, u8, u8, f32)> {
    let parts: Vec<&str> = args.split(',').collect();
    if parts.len() != 4 { return None; }
    Some((parts[0].trim().parse().ok()?, parts[1].trim().parse().ok()?, parts[2].trim().parse().ok()?, parts[3].trim().parse().ok()?))
}

fn parse_percentage(s: &str) -> Option<f32> { s.trim().trim_end_matches('%').parse().ok() }

fn rgb_to_hsl(r: u8, g: u8, b: u8, a: f32) -> HSLA {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-6 { return HSLA { h: 0.0, s: 0.0, l, a }; }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if (max - r).abs() < 1e-6 {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < 1e-6 {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    HSLA { h: h * 60.0, s, l, a }
}

pub fn lerp_hsla(from: &HSLA, to: &HSLA, t: f32) -> HSLA {
    let mut hue_diff = to.h - from.h;
    if hue_diff > 180.0 { hue_diff -= 360.0; }
    if hue_diff < -180.0 { hue_diff += 360.0; }
    HSLA { h: (from.h + hue_diff * t + 360.0) % 360.0, s: from.s + (to.s - from.s) * t, l: from.l + (to.l - from.l) * t, a: from.a + (to.a - from.a) * t }
}

pub fn lerp_hsla_clamped(from: &HSLA, to: &HSLA, t: f32) -> HSLA { lerp_hsla(from, to, t.clamp(0.0, 1.0)) }

pub fn hsla_to_rgba_string(hsla: &HSLA) -> String {
    let (r, g, b) = hsl_to_rgb(hsla.h, hsla.s, hsla.l);
    format!("rgba({},{},{},{:.2})", r, g, b, hsla.a)
}

pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    if s.abs() < 1e-6 { let v = (l * 255.0).round() as u8; return (v, v, v); }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let hue_to_rgb = |t: f32| -> f32 {
        let mut t = t;
        if t < 0.0 { t += 1.0; } if t > 1.0 { t -= 1.0; }
        if t < 1.0/6.0 { return p + (q-p)*6.0*t; } if t < 0.5 { return q; }
        if t < 2.0/3.0 { return p + (q-p)*(2.0/3.0 - t)*6.0; } p
    };
    let h_norm = h / 360.0;
    let r = (hue_to_rgb(h_norm + 1.0/3.0) * 255.0).round() as u8;
    let g = (hue_to_rgb(h_norm) * 255.0).round() as u8;
    let b = (hue_to_rgb(h_norm - 1.0/3.0) * 255.0).round() as u8;
    (r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_hex_three_digit() {
        let c = parse_color("#f00").expect("parses");
        let (r, g, b) = hsl_to_rgb(c.h, c.s, c.l);
        assert_eq!((r, g, b), (255, 0, 0));
    }
    #[test]
    fn lerp_clamps_above_one() {
        let from = HSLA { h: 0.0, s: 1.0, l: 0.5, a: 1.0 };
        let to = HSLA { h: 240.0, s: 1.0, l: 0.5, a: 1.0 };
        let result = lerp_hsla_clamped(&from, &to, 1.5);
        let expected = lerp_hsla(&from, &to, 1.0);
        assert!((result.l - expected.l).abs() < 1e-6);
    }
}
