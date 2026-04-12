use std::collections::HashMap;

use crate::frame_ctx::ScriptFrameCtx;
use crate::scene::easing::{animate_value, easing_from_name, Easing, SpringConfig};

pub(crate) struct AnimateResult {
    values: HashMap<String, ComputedValue>,
    settle_frame: u32,
    settled: bool,
    progress: f32,
}

#[derive(Clone)]
pub(crate) enum ComputedValue {
    Number(f32),
    Color(String),
}

impl AnimateResult {
    pub(crate) fn get(&self, key: &str) -> f32 {
        match self.values.get(key) {
            Some(ComputedValue::Number(v)) => *v,
            _ => 0.0,
        }
    }

    pub(crate) fn get_color(&self, key: &str) -> &str {
        match self.values.get(key) {
            Some(ComputedValue::Color(v)) => v,
            _ => "",
        }
    }

    pub(crate) fn settled(&self) -> bool {
        self.settled
    }

    pub(crate) fn settle_frame(&self) -> u32 {
        self.settle_frame
    }

    pub(crate) fn progress(&self) -> f32 {
        self.progress
    }
}

pub(crate) struct AnimateBuilder<'a> {
    ctx: &'a ScriptFrameCtx,
    from: Vec<(String, f32)>,
    to: Vec<(String, f32)>,
    from_color: Vec<(String, String)>,
    to_color: Vec<(String, String)>,
    duration: Option<u32>,
    delay: u32,
    easing: Easing,
    clamp: bool,
}

impl<'a> AnimateBuilder<'a> {
    pub(crate) fn new(ctx: &'a ScriptFrameCtx) -> Self {
        Self {
            ctx,
            from: Vec::new(),
            to: Vec::new(),
            from_color: Vec::new(),
            to_color: Vec::new(),
            duration: None,
            delay: 0,
            easing: Easing::Linear,
            clamp: false,
        }
    }

    pub(crate) fn from(mut self, values: Vec<(&str, f32)>) -> Self {
        self.from = values
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        self
    }

    pub(crate) fn to(mut self, values: Vec<(&str, f32)>) -> Self {
        self.to = values
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        self
    }

    pub(crate) fn from_color(mut self, values: Vec<(&str, &str)>) -> Self {
        self.from_color = values
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        self
    }

    pub(crate) fn to_color(mut self, values: Vec<(&str, &str)>) -> Self {
        self.to_color = values
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        self
    }

    pub(crate) fn duration(mut self, duration: u32) -> Self {
        self.duration = Some(duration);
        self
    }

    pub(crate) fn delay(mut self, delay: u32) -> Self {
        self.delay = delay;
        self
    }

    pub(crate) fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub(crate) fn easing_spring(mut self, config: SpringConfig) -> Self {
        self.easing = Easing::Spring(config);
        self
    }

    pub(crate) fn clamp(mut self, clamp: bool) -> Self {
        self.clamp = clamp;
        self
    }

    pub(crate) fn build(self) -> AnimateResult {
        let duration = self
            .duration
            .or_else(|| self.easing.default_duration(self.ctx.scene_frames as f32))
            .unwrap_or(1);

        let current_frame = self.ctx.current_frame;
        let t = if current_frame <= self.delay {
            0.0
        } else {
            ((current_frame - self.delay) as f32 / duration as f32).clamp(0.0, 1.0)
        };
        let progress = self.easing.apply(t);

        let settled = t >= 1.0;
        let settle_frame = self.delay.saturating_add(duration);

        let mut values = HashMap::new();

        for (key, from_val) in &self.from {
            let to_val = self
                .to
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| *v)
                .unwrap_or(*from_val);
            let val = animate_value(
                current_frame,
                duration,
                self.delay,
                *from_val,
                to_val,
                &self.easing,
                self.clamp,
            );
            values.insert(key.clone(), ComputedValue::Number(val));
        }

        for (key, from_color) in &self.from_color {
            let to_color = self
                .to_color
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
                .unwrap_or(from_color.as_str());
            let interpolated = interpolate_color(from_color, to_color, progress);
            values.insert(key.clone(), ComputedValue::Color(interpolated));
        }

        AnimateResult {
            values,
            settle_frame,
            settled,
            progress,
        }
    }
}

pub(crate) fn animate<'a>(ctx: &'a ScriptFrameCtx) -> AnimateBuilder<'a> {
    AnimateBuilder::new(ctx)
}

pub(crate) struct StaggerResult {
    items: Vec<AnimateResult>,
}

impl StaggerResult {
    pub(crate) fn get(&self, index: usize) -> &AnimateResult {
        &self.items[index]
    }

    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl std::ops::Index<usize> for StaggerResult {
    type Output = AnimateResult;

    fn index(&self, index: usize) -> &Self::Output {
        &self.items[index]
    }
}

pub(crate) fn stagger<'a>(ctx: &'a ScriptFrameCtx, count: usize) -> StaggerBuilder<'a> {
    StaggerBuilder::new(ctx, count)
}

pub(crate) struct StaggerBuilder<'a> {
    ctx: &'a ScriptFrameCtx,
    count: usize,
    from: Vec<(String, f32)>,
    to: Vec<(String, f32)>,
    duration: Option<u32>,
    delay: u32,
    gap: u32,
    easing: Easing,
    clamp: bool,
}

impl<'a> StaggerBuilder<'a> {
    pub(crate) fn new(ctx: &'a ScriptFrameCtx, count: usize) -> Self {
        Self {
            ctx,
            count,
            from: Vec::new(),
            to: Vec::new(),
            duration: None,
            delay: 0,
            gap: 0,
            easing: Easing::Linear,
            clamp: false,
        }
    }

    pub(crate) fn from(mut self, values: Vec<(&str, f32)>) -> Self {
        self.from = values
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        self
    }

    pub(crate) fn to(mut self, values: Vec<(&str, f32)>) -> Self {
        self.to = values
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        self
    }

    pub(crate) fn duration(mut self, duration: u32) -> Self {
        self.duration = Some(duration);
        self
    }

    pub(crate) fn delay(mut self, delay: u32) -> Self {
        self.delay = delay;
        self
    }

    pub(crate) fn gap(mut self, gap: u32) -> Self {
        self.gap = gap;
        self
    }

    pub(crate) fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub(crate) fn clamp(mut self, clamp: bool) -> Self {
        self.clamp = clamp;
        self
    }

    pub(crate) fn build(self) -> StaggerResult {
        let duration = self
            .duration
            .or_else(|| self.easing.default_duration(self.ctx.scene_frames as f32))
            .unwrap_or(1);

        let items: Vec<AnimateResult> = (0..self.count)
            .map(|i| {
                let item_delay = self
                    .delay
                    .saturating_add((i as u32).saturating_mul(self.gap));
                AnimateBuilder::new(self.ctx)
                    .from(self.from.iter().map(|(k, v)| (k.as_str(), *v)).collect())
                    .to(self.to.iter().map(|(k, v)| (k.as_str(), *v)).collect())
                    .duration(duration)
                    .delay(item_delay)
                    .easing(self.easing)
                    .clamp(self.clamp)
                    .build()
            })
            .collect();

        StaggerResult { items }
    }
}

fn interpolate_color(from: &str, to: &str, t: f32) -> String {
    let from_hsla = parse_color(from);
    let to_hsla = parse_color(to);

    match (from_hsla, to_hsla) {
        (Some(f), Some(t_c)) => {
            let result = lerp_hsla(&f, &t_c, t);
            hsla_to_rgba_string(&result)
        }
        _ => from.to_string(),
    }
}

#[derive(Clone, Copy, Debug)]
struct HSLA {
    h: f32,
    s: f32,
    l: f32,
    a: f32,
}

fn parse_color(input: &str) -> Option<HSLA> {
    let s = input.trim();

    if let Some(rest) = s.strip_prefix('#') {
        return parse_hex(rest);
    }

    if let Some(rest) = s.strip_prefix("hsl(").and_then(|r| r.strip_suffix(')')) {
        return parse_hsl_args(rest, 1.0);
    }

    if let Some(rest) = s.strip_prefix("hsla(").and_then(|r| r.strip_suffix(')')) {
        return parse_hsla_args(rest);
    }

    if let Some(rest) = s.strip_prefix("rgb(").and_then(|r| r.strip_suffix(')')) {
        return parse_rgb_args(rest, 1.0).map(|(r, g, b, _)| rgb_to_hsl(r, g, b, 1.0));
    }

    if let Some(rest) = s.strip_prefix("rgba(").and_then(|r| r.strip_suffix(')')) {
        if let Some((r, g, b, a)) = parse_rgba_args(rest) {
            return Some(rgb_to_hsl(r, g, b, a));
        }
    }

    None
}

fn parse_hex(hex: &str) -> Option<HSLA> {
    let (r, g, b, a) = match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            (r, g, b, 1.0)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b, 1.0)
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
    if parts.len() != 3 {
        return None;
    }
    let h = parts[0].trim().trim_end_matches("deg").parse().ok()?;
    let s = parse_percentage(parts[1])?;
    let l = parse_percentage(parts[2])?;
    Some(HSLA {
        h,
        s: s / 100.0,
        l: l / 100.0,
        a: default_alpha,
    })
}

fn parse_hsla_args(args: &str) -> Option<HSLA> {
    let parts: Vec<&str> = args.split(',').collect();
    if parts.len() != 4 {
        return None;
    }
    let h = parts[0].trim().trim_end_matches("deg").parse().ok()?;
    let s = parse_percentage(parts[1])?;
    let l = parse_percentage(parts[2])?;
    let a = parts[3].trim().parse().ok()?;
    Some(HSLA {
        h,
        s: s / 100.0,
        l: l / 100.0,
        a,
    })
}

fn parse_rgb_args(args: &str, default_alpha: f32) -> Option<(u8, u8, u8, f32)> {
    let parts: Vec<&str> = args.split(',').collect();
    if parts.len() != 3 {
        return None;
    }
    let r = parts[0].trim().parse().ok()?;
    let g = parts[1].trim().parse().ok()?;
    let b = parts[2].trim().parse().ok()?;
    Some((r, g, b, default_alpha))
}

fn parse_rgba_args(args: &str) -> Option<(u8, u8, u8, f32)> {
    let parts: Vec<&str> = args.split(',').collect();
    if parts.len() != 4 {
        return None;
    }
    let r = parts[0].trim().parse().ok()?;
    let g = parts[1].trim().parse().ok()?;
    let b = parts[2].trim().parse().ok()?;
    let a = parts[3].trim().parse().ok()?;
    Some((r, g, b, a))
}

fn parse_percentage(s: &str) -> Option<f32> {
    s.trim().trim_end_matches('%').parse().ok()
}

fn rgb_to_hsl(r: u8, g: u8, b: u8, a: f32) -> HSLA {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < 1e-6 {
        return HSLA {
            h: 0.0,
            s: 0.0,
            l,
            a,
        };
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let h = if (max - r).abs() < 1e-6 {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < 1e-6 {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };

    HSLA {
        h: h * 60.0,
        s,
        l,
        a,
    }
}

fn lerp_hsla(from: &HSLA, to: &HSLA, t: f32) -> HSLA {
    let mut hue_diff = to.h - from.h;
    if hue_diff > 180.0 {
        hue_diff -= 360.0;
    }
    if hue_diff < -180.0 {
        hue_diff += 360.0;
    }

    HSLA {
        h: (from.h + hue_diff * t + 360.0) % 360.0,
        s: from.s + (to.s - from.s) * t,
        l: from.l + (to.l - from.l) * t,
        a: from.a + (to.a - from.a) * t,
    }
}

fn hsla_to_rgba_string(hsla: &HSLA) -> String {
    let (r, g, b) = hsl_to_rgb(hsla.h, hsla.s, hsla.l);
    format!("rgba({},{},{},{:.2})", r, g, b, hsla.a)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    if s.abs() < 1e-6 {
        let v = (l * 255.0).round() as u8;
        return (v, v, v);
    }

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;

    let hue_to_rgb = |t: f32| -> f32 {
        let mut t = t;
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            return p + (q - p) * 6.0 * t;
        }
        if t < 0.5 {
            return q;
        }
        if t < 2.0 / 3.0 {
            return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
        }
        p
    };

    let h_norm = h / 360.0;
    let r = (hue_to_rgb(h_norm + 1.0 / 3.0) * 255.0).round() as u8;
    let g = (hue_to_rgb(h_norm) * 255.0).round() as u8;
    let b = (hue_to_rgb(h_norm - 1.0 / 3.0) * 255.0).round() as u8;

    (r, g, b)
}

pub(crate) fn parse_easing_from_js(value: &str) -> Option<Easing> {
    easing_from_name(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_ctx::ScriptFrameCtx;

    fn ctx(frame: u32, scene_frames: u32) -> ScriptFrameCtx {
        ScriptFrameCtx {
            frame,
            total_frames: scene_frames,
            current_frame: frame,
            scene_frames,
        }
    }

    #[test]
    fn animate_linear_opacity() {
        let c = ctx(10, 30);
        let result = animate(&c)
            .from(vec![("opacity", 0.0)])
            .to(vec![("opacity", 1.0)])
            .duration(20)
            .easing(Easing::Linear)
            .build();
        assert!((result.get("opacity") - 0.5).abs() < 1e-4);
    }

    #[test]
    fn animate_before_delay_returns_from() {
        let c = ctx(5, 30);
        let result = animate(&c)
            .from(vec![("translateX", 0.0)])
            .to(vec![("translateX", 100.0)])
            .duration(20)
            .delay(10)
            .easing(Easing::Linear)
            .build();
        assert!((result.get("translateX") - 0.0).abs() < 1e-4);
    }

    #[test]
    fn animate_after_end_returns_to() {
        let c = ctx(40, 50);
        let result = animate(&c)
            .from(vec![("opacity", 0.0)])
            .to(vec![("opacity", 1.0)])
            .duration(20)
            .delay(10)
            .easing(Easing::Linear)
            .build();
        assert!((result.get("opacity") - 1.0).abs() < 1e-4);
        assert!(result.settled());
        assert_eq!(result.settle_frame(), 30);
    }

    #[test]
    fn stagger_produces_correct_count() {
        let c = ctx(5, 30);
        let result = stagger(&c, 5)
            .from(vec![("opacity", 0.0)])
            .to(vec![("opacity", 1.0)])
            .duration(15)
            .gap(3)
            .easing(Easing::EaseOut)
            .build();
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn stagger_items_have_increasing_delays() {
        let c = ctx(0, 60);
        let result = stagger(&c, 3)
            .from(vec![("opacity", 0.0)])
            .to(vec![("opacity", 1.0)])
            .duration(20)
            .gap(5)
            .easing(Easing::Linear)
            .build();
        assert!((result[0].get("opacity") - 0.0).abs() < 1e-4);
        assert!(result[1].get("opacity") - 0.0 < 1e-4);
    }

    #[test]
    fn spring_auto_duration() {
        let c = ctx(0, 120);
        let result = animate(&c)
            .from(vec![("opacity", 0.0)])
            .to(vec![("opacity", 1.0)])
            .easing_spring(SpringConfig {
                stiffness: 100.0,
                damping: 10.0,
                mass: 1.0,
            })
            .build();
        assert!((result.get("opacity") - 0.0).abs() < 1e-4);
    }

    #[test]
    fn rgb_to_hsl_and_back() {
        let hsla = rgb_to_hsl(59, 130, 246, 1.0);
        let (r, g, b) = hsl_to_rgb(hsla.h, hsla.s, hsla.l);
        assert!((r as i32 - 59).abs() <= 1);
        assert!((g as i32 - 130).abs() <= 1);
        assert!((b as i32 - 246).abs() <= 1);
    }

    #[test]
    fn color_interpolation_between_hex_colors() {
        let result = interpolate_color("#3b82f6", "#8b5cf6", 0.5);
        assert!(result.starts_with("rgba("));
    }

    #[test]
    fn parse_hex_3_digit() {
        let hsla = parse_color("#f00").unwrap();
        assert!((hsla.h - 0.0).abs() < 1e-2 || (hsla.h - 360.0).abs() < 1e-2);
        assert!(hsla.s > 0.9);
    }

    #[test]
    fn parse_hex_6_digit() {
        let hsla = parse_color("#3b82f6").unwrap();
        assert!(hsla.h > 200.0);
        assert!(hsla.s > 0.8);
    }

    #[test]
    fn hsl_hue_interpolation_wraps_shortest_path() {
        let from = HSLA {
            h: 350.0,
            s: 1.0,
            l: 0.5,
            a: 1.0,
        };
        let to = HSLA {
            h: 10.0,
            s: 1.0,
            l: 0.5,
            a: 1.0,
        };
        let mid = lerp_hsla(&from, &to, 0.5);
        assert!((mid.h - 0.0).abs() < 1e-2 || (mid.h - 360.0).abs() < 1e-2);
    }
}
