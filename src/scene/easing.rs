#[derive(Clone, Copy, Debug)]
pub struct SpringConfig {
    pub stiffness: f32,
    pub damping: f32,
    pub mass: f32,
}

impl Default for SpringConfig {
    fn default() -> Self {
        Self {
            stiffness: 100.0,
            damping: 10.0,
            mass: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Easing {
    Linear,
    Ease,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier(f32, f32, f32, f32),
    Spring(SpringConfig),
}

impl Easing {
    pub fn apply(&self, t: f32) -> f32 {
        match self {
            Easing::Linear => t,
            Easing::Ease => cubic_bezier(t, 0.25, 0.1, 0.25, 1.0),
            Easing::EaseIn => cubic_bezier(t, 0.42, 0.0, 1.0, 1.0),
            Easing::EaseOut => cubic_bezier(t, 0.0, 0.0, 0.58, 1.0),
            Easing::EaseInOut => cubic_bezier(t, 0.42, 0.0, 0.58, 1.0),
            Easing::CubicBezier(x1, y1, x2, y2) => cubic_bezier(t, *x1, *y1, *x2, *y2),
            Easing::Spring(config) => spring_easing(t, config),
        }
    }

    pub fn default_duration(&self, fps: f32) -> Option<u32> {
        match self {
            Easing::Spring(config) => Some((settle_time(config) * fps).ceil() as u32),
            _ => None,
        }
    }

    pub fn is_spring(&self) -> bool {
        matches!(self, Easing::Spring(_))
    }
}

fn cubic_bezier(t: f32, x1: f32, _y1: f32, x2: f32, _y2: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }

    fn sample_x(s: f32, x1: f32, x2: f32) -> f32 {
        let a = 1.0 - s;
        3.0 * a * a * s * x1 + 3.0 * a * s * s * x2 + s * s * s
    }

    fn sample_y(s: f32, y1: f32, y2: f32) -> f32 {
        let a = 1.0 - s;
        3.0 * a * a * s * y1 + 3.0 * a * s * s * y2 + s * s * s
    }

    fn sample_dx(s: f32, x1: f32, x2: f32) -> f32 {
        let a = 1.0 - s;
        3.0 * a * a * x1 + 6.0 * a * s * (x2 - x1) + 3.0 * s * s * (1.0 - x2)
    }

    let mut s = t;
    for _ in 0..8 {
        let x = sample_x(s, x1, x2) - t;
        let dx = sample_dx(s, x1, x2);
        if dx.abs() < 1e-6 {
            break;
        }
        s -= x / dx;
        s = s.clamp(0.0, 1.0);
    }

    sample_y(s, _y1, _y2)
}

fn spring_easing(t: f32, config: &SpringConfig) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        let t_end = settle_time(config);
        return 1.0 - decay(t_end, config);
    }

    let t_real = t * settle_time(config);
    1.0 - decay(t_real, config)
}

pub(crate) fn decay(t: f32, config: &SpringConfig) -> f32 {
    let gamma = config.damping / (2.0 * config.mass);
    let omega0_sq = config.stiffness / config.mass;
    let gamma_sq = gamma * gamma;

    if omega0_sq > gamma_sq {
        let omega_d = (omega0_sq - gamma_sq).sqrt();
        (-gamma * t).exp() * ((omega_d * t).cos() + (gamma / omega_d) * (omega_d * t).sin())
    } else if omega0_sq < gamma_sq {
        let s = (gamma_sq - omega0_sq).sqrt();
        (-gamma * t).exp() * ((s * t).cosh() + (gamma / s) * (s * t).sinh())
    } else {
        (-gamma * t).exp() * (1.0 + gamma * t)
    }
}

pub(crate) fn settle_time(config: &SpringConfig) -> f32 {
    let gamma = config.damping / (2.0 * config.mass);
    if gamma <= 0.0 {
        return 5.0;
    }
    let threshold: f32 = 0.001;
    -threshold.ln() / gamma
}

pub fn animate_value(
    current_frame: u32,
    duration: u32,
    delay: u32,
    from: f32,
    to: f32,
    easing: &Easing,
    clamp: bool,
) -> f32 {
    if current_frame <= delay {
        return from;
    }
    let elapsed = (current_frame - delay) as f32;
    let t = (elapsed / duration as f32).clamp(0.0, 1.0);
    let progress = easing.apply(t);
    let p = if clamp {
        progress.clamp(0.0, 1.0)
    } else {
        progress
    };
    from + (to - from) * p
}

pub mod presets {
    use super::{Easing, SpringConfig};

    pub const LINEAR: Easing = Easing::Linear;
    pub const EASE: Easing = Easing::Ease;
    pub const EASE_IN: Easing = Easing::EaseIn;
    pub const EASE_OUT: Easing = Easing::EaseOut;
    pub const EASE_IN_OUT: Easing = Easing::EaseInOut;

    pub const SPRING_DEFAULT: Easing = Easing::Spring(SpringConfig {
        stiffness: 100.0,
        damping: 10.0,
        mass: 1.0,
    });
    pub const SPRING_GENTLE: Easing = Easing::Spring(SpringConfig {
        stiffness: 60.0,
        damping: 8.0,
        mass: 0.8,
    });
    pub const SPRING_STIFF: Easing = Easing::Spring(SpringConfig {
        stiffness: 200.0,
        damping: 15.0,
        mass: 1.0,
    });
    pub const SPRING_SLOW: Easing = Easing::Spring(SpringConfig {
        stiffness: 80.0,
        damping: 12.0,
        mass: 1.5,
    });
    pub const SPRING_WOBBLY: Easing = Easing::Spring(SpringConfig {
        stiffness: 180.0,
        damping: 6.0,
        mass: 1.0,
    });
}

pub fn easing_from_name(name: &str) -> Option<Easing> {
    match name {
        "linear" => Some(Easing::Linear),
        "ease" => Some(Easing::Ease),
        "ease-in" => Some(Easing::EaseIn),
        "ease-out" => Some(Easing::EaseOut),
        "ease-in-out" => Some(Easing::EaseInOut),
        "spring-default" => Some(presets::SPRING_DEFAULT),
        "spring-gentle" => Some(presets::SPRING_GENTLE),
        "spring-stiff" => Some(presets::SPRING_STIFF),
        "spring-slow" => Some(presets::SPRING_SLOW),
        "spring-wobbly" => Some(presets::SPRING_WOBBLY),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_easing_is_identity() {
        assert!((Easing::Linear.apply(0.0) - 0.0).abs() < 1e-6);
        assert!((Easing::Linear.apply(0.5) - 0.5).abs() < 1e-6);
        assert!((Easing::Linear.apply(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cubic_bezier_boundary_values() {
        assert!((cubic_bezier(0.0, 0.25, 0.1, 0.25, 1.0) - 0.0).abs() < 1e-6);
        assert!((cubic_bezier(1.0, 0.25, 0.1, 0.25, 1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn decay_starts_at_one_and_converges_to_zero() {
        let config = SpringConfig {
            stiffness: 100.0,
            damping: 10.0,
            mass: 1.0,
        };
        assert!((decay(0.0, &config) - 1.0).abs() < 1e-6);
        assert!(decay(settle_time(&config), &config).abs() < 0.01);
    }

    #[test]
    fn spring_easing_starts_at_zero_ends_near_one() {
        let config = SpringConfig {
            stiffness: 100.0,
            damping: 10.0,
            mass: 1.0,
        };
        let easing = Easing::Spring(config);
        assert!((easing.apply(0.0) - 0.0).abs() < 1e-6);
        assert!((easing.apply(1.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn animate_value_before_delay_returns_from() {
        let result = animate_value(5, 20, 10, 0.0, 100.0, &Easing::Linear, false);
        assert!((result - 0.0).abs() < 1e-6);
    }

    #[test]
    fn animate_value_at_end_returns_to() {
        let result = animate_value(30, 20, 10, 0.0, 100.0, &Easing::Linear, false);
        assert!((result - 100.0).abs() < 1e-6);
    }

    #[test]
    fn easing_from_name_parses_all_presets() {
        assert!(easing_from_name("linear").is_some());
        assert!(easing_from_name("ease").is_some());
        assert!(easing_from_name("ease-in").is_some());
        assert!(easing_from_name("ease-out").is_some());
        assert!(easing_from_name("ease-in-out").is_some());
        assert!(easing_from_name("spring-default").is_some());
        assert!(easing_from_name("spring-gentle").is_some());
        assert!(easing_from_name("spring-stiff").is_some());
        assert!(easing_from_name("spring-slow").is_some());
        assert!(easing_from_name("spring-wobbly").is_some());
        assert!(easing_from_name("unknown").is_none());
    }

    #[test]
    fn default_duration_returns_none_for_non_spring() {
        assert!(Easing::Linear.default_duration(30.0).is_none());
        assert!(Easing::EaseOut.default_duration(30.0).is_none());
    }

    #[test]
    fn default_duration_returns_value_for_spring() {
        let config = SpringConfig {
            stiffness: 100.0,
            damping: 10.0,
            mass: 1.0,
        };
        let easing = Easing::Spring(config);
        let dur = easing.default_duration(30.0);
        assert!(dur.is_some());
        assert!(dur.unwrap() > 0);
    }
}
