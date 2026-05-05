//! Animate state machine — keyframe + easing evaluation.

use std::collections::HashMap;
use crate::scene::easing::{Easing, SpringConfig};
use super::color::{hsla_to_rgba_string, lerp_hsla_clamped, parse_color};

pub struct AnimateEntry {
    pub progress: f32,
    pub settled: bool,
    pub settle_frame: u32,
    pub duration: u32,
    pub delay: u32,
    pub clamp: bool,
    pub easing: Easing,
    pub repeat: i32,
    pub yoyo: bool,
    pub repeat_delay: u32,
}

#[derive(Default)]
pub struct AnimateState {
    pub next_id: i32,
    pub entries: HashMap<i32, AnimateEntry>,
}

impl AnimateState {
    #[allow(clippy::too_many_arguments)]
    pub fn create(&mut self, current_frame: u32, duration: f32, delay: f32, clamp: bool,
                  easing_tag: &str, repeat: i32, yoyo: bool, repeat_delay: f32) -> i32 {
        let easing = parse_easing_from_tag(easing_tag);
        let fps = 30.0f32;
        let duration_u32 = if duration < 0.0 {
            easing.default_duration(fps).unwrap_or(1)
        } else {
            duration as u32
        };
        let delay_u32 = delay as u32;
        let repeat_delay_u32 = repeat_delay.max(0.0) as u32;
        let progress = crate::scene::easing::compute_progress(
            current_frame, duration_u32, delay_u32, &easing, clamp, repeat, yoyo, repeat_delay_u32);
        let total_frames = if repeat >= 0 {
            duration_u32.saturating_mul(repeat as u32 + 1)
                .saturating_add(repeat_delay_u32.saturating_mul(repeat as u32))
        } else { u32::MAX };
        let settled = repeat >= 0 && current_frame >= delay_u32.saturating_add(total_frames);
        let settle_frame = delay_u32.saturating_add(total_frames);
        let handle = self.next_id;
        self.next_id += 1;
        self.entries.insert(handle, AnimateEntry {
            progress, settled, settle_frame, duration: duration_u32, delay: delay_u32,
            clamp, easing, repeat, yoyo, repeat_delay: repeat_delay_u32,
        });
        handle
    }

    #[allow(clippy::too_many_arguments)]
    pub fn value(&self, current_frame: u32, handle: i32, from: f32, to: f32) -> f32 {
        if let Some(entry) = self.entries.get(&handle) {
            crate::scene::easing::animate_value(
                current_frame, entry.duration, entry.delay, from, to,
                &entry.easing, entry.clamp, entry.repeat, entry.yoyo, entry.repeat_delay)
        } else { from }
    }

    pub fn color(&self, handle: i32, from: &str, to: &str) -> String {
        let Some(entry) = self.entries.get(&handle) else { return from.to_string(); };
        match (parse_color(from), parse_color(to)) {
            (Some(f), Some(t)) => {
                let result = lerp_hsla_clamped(&f, &t, entry.progress);
                hsla_to_rgba_string(&result)
            }
            _ => from.to_string(),
        }
    }

    pub fn progress(&self, handle: i32) -> f32 { self.entries.get(&handle).map(|e| e.progress).unwrap_or(0.0) }
    pub fn settled(&self, handle: i32) -> bool { self.entries.get(&handle).map(|e| e.settled).unwrap_or(false) }
    pub fn settle_frame(&self, handle: i32) -> u32 { self.entries.get(&handle).map(|e| e.settle_frame).unwrap_or(0) }
}

pub fn parse_easing_from_tag(tag: &str) -> Easing {
    match tag {
        "linear" => Easing::Linear,
        "ease" => Easing::Ease,
        "ease-in" => Easing::EaseIn,
        "ease-out" => Easing::EaseOut,
        "ease-in-out" => Easing::EaseInOut,
        s if s.starts_with("spring:") => {
            let parts: Vec<&str> = s[7..].split(',').collect();
            if parts.len() == 3 {
                Easing::Spring(SpringConfig {
                    stiffness: parts[0].parse().unwrap_or(100.0),
                    damping: parts[1].parse().unwrap_or(10.0),
                    mass: parts[2].parse().unwrap_or(1.0),
                })
            } else { crate::scene::easing::easing_from_name(tag).unwrap_or(Easing::Linear) }
        }
        b if b.starts_with("bezier:") => {
            let parts: Vec<&str> = b[7..].split(',').collect();
            if parts.len() == 4 {
                Easing::CubicBezier(
                    parts[0].parse().unwrap_or(0.0), parts[1].parse().unwrap_or(0.0),
                    parts[2].parse().unwrap_or(1.0), parts[3].parse().unwrap_or(1.0))
            } else { Easing::Linear }
        }
        _ => crate::scene::easing::easing_from_name(tag).unwrap_or(Easing::Linear),
    }
}

pub fn random_from_seed(seed: f32) -> f32 {
    let bits = if seed.is_finite() { seed.to_bits() } else { 1u32 };
    (xorshift32(bits) as f32) / (u32::MAX as f32)
}

fn xorshift32(seed: u32) -> u32 {
    let mut x = if seed == 0 { 0x9E3779B9 } else { seed };
    x ^= x << 13; x ^= x >> 17; x ^= x << 5; x
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn random_seed_deterministic() {
        let a = random_from_seed(42.0); let b = random_from_seed(42.0);
        assert!((a - b).abs() < 1e-9);
    }
    #[test]
    fn parse_unknown_tag_falls_back_to_linear() {
        assert!(matches!(parse_easing_from_tag("totally-invalid"), Easing::Linear));
    }
    #[test]
    fn create_handle_increments() {
        let mut s = AnimateState::default();
        let h1 = s.create(0, 30.0, 0.0, true, "linear", -1, false, 0.0);
        let h2 = s.create(0, 30.0, 0.0, true, "linear", -1, false, 0.0);
        assert_eq!(h2, h1 + 1);
    }
}
