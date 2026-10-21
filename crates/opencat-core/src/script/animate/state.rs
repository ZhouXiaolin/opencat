//! Animation utilities — stateless helpers.

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
}
