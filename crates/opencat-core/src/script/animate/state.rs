//! Animate utilities — easing parsing, stateless helpers.

use crate::scene::easing::{Easing, SpringConfig};

// AnimateEntry is now defined in `script::recorder`.
// Re-export for convenience.
pub use crate::script::recorder::AnimateEntry;

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
            } else {
                crate::scene::easing::easing_from_name(tag).unwrap_or(Easing::Linear)
            }
        }
        b if b.starts_with("bezier:") => {
            let parts: Vec<&str> = b[7..].split(',').collect();
            if parts.len() == 4 {
                Easing::CubicBezier(
                    parts[0].parse().unwrap_or(0.0),
                    parts[1].parse().unwrap_or(0.0),
                    parts[2].parse().unwrap_or(1.0),
                    parts[3].parse().unwrap_or(1.0),
                )
            } else {
                Easing::Linear
            }
        }
        _ => crate::scene::easing::easing_from_name(tag).unwrap_or(Easing::Linear),
    }
}

pub fn random_from_seed(seed: f32) -> f32 {
    let bits = if seed.is_finite() {
        seed.to_bits()
    } else {
        1u32
    };
    (xorshift32(bits) as f32) / (u32::MAX as f32)
}

fn xorshift32(seed: u32) -> u32 {
    let mut x = if seed == 0 { 0x9E3779B9 } else { seed };
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    x
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn random_seed_deterministic() {
        let a = random_from_seed(42.0);
        let b = random_from_seed(42.0);
        assert!((a - b).abs() < 1e-9);
    }
    #[test]
    fn parse_unknown_tag_falls_back_to_linear() {
        assert!(matches!(
            parse_easing_from_tag("totally-invalid"),
            Easing::Linear
        ));
    }
}
