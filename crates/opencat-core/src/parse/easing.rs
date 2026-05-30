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
    BackIn,
    BackOut,
    BackInOut,
    ElasticIn,
    ElasticOut,
    ElasticInOut,
    BounceIn,
    BounceOut,
    BounceInOut,
    Steps(u32),
    CubicBezier(f32, f32, f32, f32),
    Spring(SpringConfig),
    // GSAP-style power easings
    Power1In,
    Power1Out,
    Power1InOut,
    Power2In,
    Power2Out,
    Power2InOut,
    Power3In,
    Power3Out,
    Power3InOut,
    Power4In,
    Power4Out,
    Power4InOut,
    // GSAP-style math easings
    CircIn,
    CircOut,
    CircInOut,
    ExpoIn,
    ExpoOut,
    ExpoInOut,
    SineIn,
    SineOut,
    SineInOut,
}

impl Easing {
    pub fn apply(&self, t: f32) -> f32 {
        match self {
            Easing::Linear => t,
            Easing::Ease => cubic_bezier(t, 0.25, 0.1, 0.25, 1.0),
            Easing::EaseIn => cubic_bezier(t, 0.42, 0.0, 1.0, 1.0),
            Easing::EaseOut => cubic_bezier(t, 0.0, 0.0, 0.58, 1.0),
            Easing::EaseInOut => cubic_bezier(t, 0.42, 0.0, 0.58, 1.0),
            Easing::BackIn => back_in(t, 1.70158),
            Easing::BackOut => back_out(t, 1.70158),
            Easing::BackInOut => back_in_out(t, 1.70158),
            Easing::ElasticIn => elastic_in(t),
            Easing::ElasticOut => elastic_out(t),
            Easing::ElasticInOut => elastic_in_out(t),
            Easing::BounceIn => 1.0 - bounce_out(1.0 - t),
            Easing::BounceOut => bounce_out(t),
            Easing::BounceInOut => {
                if t < 0.5 {
                    (1.0 - bounce_out(1.0 - 2.0 * t)) / 2.0
                } else {
                    (1.0 + bounce_out(2.0 * t - 1.0)) / 2.0
                }
            }
            Easing::Steps(n) => {
                if *n == 0 {
                    t
                } else {
                    let nf = *n as f32;
                    if t >= 1.0 { 1.0 } else { (t * nf).floor() / nf }
                }
            }
            Easing::CubicBezier(x1, y1, x2, y2) => cubic_bezier(t, *x1, *y1, *x2, *y2),
            Easing::Spring(config) => spring_easing(t, config),
            // GSAP power easings (power1=quad, power2=cubic, power3=quart, power4=quint)
            Easing::Power1In => t * t,
            Easing::Power1Out => 1.0 - (1.0 - t) * (1.0 - t),
            Easing::Power1InOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Easing::Power2In => t * t * t,
            Easing::Power2Out => 1.0 - (1.0 - t).powi(3),
            Easing::Power2InOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Easing::Power3In => t.powi(4),
            Easing::Power3Out => 1.0 - (1.0 - t).powi(4),
            Easing::Power3InOut => {
                if t < 0.5 {
                    8.0 * t.powi(4)
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(4) / 2.0
                }
            }
            Easing::Power4In => t.powi(5),
            Easing::Power4Out => 1.0 - (1.0 - t).powi(5),
            Easing::Power4InOut => {
                if t < 0.5 {
                    16.0 * t.powi(5)
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(5) / 2.0
                }
            }
            // GSAP math easings
            Easing::CircIn => 1.0 - (1.0 - t * t).sqrt(),
            Easing::CircOut => (1.0 - (t - 1.0) * (t - 1.0)).sqrt(),
            Easing::CircInOut => {
                if t < 0.5 {
                    (1.0 - (1.0 - (2.0 * t).powi(2)).sqrt()) / 2.0
                } else {
                    ((1.0 - (-2.0 * t + 2.0).powi(2)).sqrt() + 1.0) / 2.0
                }
            }
            Easing::ExpoIn => {
                if t <= 0.0 {
                    0.0
                } else {
                    2.0_f32.powf(10.0 * t - 10.0)
                }
            }
            Easing::ExpoOut => {
                if t >= 1.0 {
                    1.0
                } else {
                    1.0 - 2.0_f32.powf(-10.0 * t)
                }
            }
            Easing::ExpoInOut => {
                if t <= 0.0 {
                    0.0
                } else if t >= 1.0 {
                    1.0
                } else if t < 0.5 {
                    2.0_f32.powf(20.0 * t - 10.0) / 2.0
                } else {
                    (2.0 - 2.0_f32.powf(-20.0 * t + 10.0)) / 2.0
                }
            }
            Easing::SineIn => 1.0 - (t * std::f32::consts::PI / 2.0).cos(),
            Easing::SineOut => (t * std::f32::consts::PI / 2.0).sin(),
            Easing::SineInOut => -((std::f32::consts::PI * t).cos() - 1.0) / 2.0,
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

fn back_in(t: f32, s: f32) -> f32 {
    t * t * ((s + 1.0) * t - s)
}

fn back_out(t: f32, s: f32) -> f32 {
    let u = t - 1.0;
    u * u * ((s + 1.0) * u + s) + 1.0
}

fn back_in_out(t: f32, s: f32) -> f32 {
    let c = s * 1.525;
    if t < 0.5 {
        let u = 2.0 * t;
        0.5 * (u * u * ((c + 1.0) * u - c))
    } else {
        let u = 2.0 * t - 2.0;
        0.5 * (u * u * ((c + 1.0) * u + c) + 2.0)
    }
}

fn elastic_in(t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    let c = (2.0 * std::f32::consts::PI) / 3.0;
    -(2.0_f32.powf(10.0 * t - 10.0)) * ((t * 10.0 - 10.75) * c).sin()
}

fn elastic_out(t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    let c = (2.0 * std::f32::consts::PI) / 3.0;
    2.0_f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c).sin() + 1.0
}

fn elastic_in_out(t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    let c = (2.0 * std::f32::consts::PI) / 4.5;
    if t < 0.5 {
        -(2.0_f32.powf(20.0 * t - 10.0)) * ((20.0 * t - 11.125) * c).sin() / 2.0
    } else {
        (2.0_f32.powf(-20.0 * t + 10.0)) * ((20.0 * t - 11.125) * c).sin() / 2.0 + 1.0
    }
}

fn bounce_out(t: f32) -> f32 {
    let n1 = 7.5625;
    let d1 = 2.75;
    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let t = t - 1.5 / d1;
        n1 * t * t + 0.75
    } else if t < 2.5 / d1 {
        let t = t - 2.25 / d1;
        n1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / d1;
        n1 * t * t + 0.984375
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

#[allow(clippy::too_many_arguments)]
pub fn compute_progress(
    current_frame: u32,
    duration: u32,
    delay: u32,
    easing: &Easing,
    clamp: bool,
    repeat: i32,
    yoyo: bool,
    repeat_delay: u32,
) -> f32 {
    if current_frame <= delay {
        return 0.0;
    }
    if duration == 0 {
        return 1.0;
    }
    let elapsed = (current_frame - delay) as f32;
    let cycle_len = duration as f32 + repeat_delay as f32;
    let cycle_idx = (elapsed / cycle_len).floor() as i32;

    if repeat >= 0 && cycle_idx > repeat {
        return if yoyo && (repeat % 2 == 1) { 0.0 } else { 1.0 };
    }

    let in_cycle = elapsed - (cycle_idx as f32) * cycle_len;
    if in_cycle >= duration as f32 {
        return if !yoyo || cycle_idx % 2 == 0 {
            1.0
        } else {
            0.0
        };
    }

    let mut t = (in_cycle / duration as f32).clamp(0.0, 1.0);
    if yoyo && (cycle_idx % 2 == 1) {
        t = 1.0 - t;
    }
    let p = easing.apply(t);
    if clamp { p.clamp(0.0, 1.0) } else { p }
}

#[allow(clippy::too_many_arguments)]
pub fn animate_value(
    current_frame: u32,
    duration: u32,
    delay: u32,
    from: f32,
    to: f32,
    easing: &Easing,
    clamp: bool,
    repeat: i32,
    yoyo: bool,
    repeat_delay: u32,
) -> f32 {
    if current_frame <= delay {
        return from;
    }
    let p = compute_progress(
        current_frame,
        duration,
        delay,
        easing,
        clamp,
        repeat,
        yoyo,
        repeat_delay,
    );
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
        "linear" | "none" => Some(Easing::Linear),
        "ease" => Some(Easing::Ease),
        "ease-in" | "ease_in" => Some(Easing::EaseIn),
        "ease-out" | "ease_out" => Some(Easing::EaseOut),
        "ease-in-out" | "ease_in_out" => Some(Easing::EaseInOut),
        "back-in" | "back_in" => Some(Easing::BackIn),
        "back-out" | "back_out" => Some(Easing::BackOut),
        "back-in-out" | "back_in_out" => Some(Easing::BackInOut),
        "elastic-in" | "elastic_in" => Some(Easing::ElasticIn),
        "elastic-out" | "elastic_out" => Some(Easing::ElasticOut),
        "elastic-in-out" | "elastic_in_out" => Some(Easing::ElasticInOut),
        "bounce-in" | "bounce_in" => Some(Easing::BounceIn),
        "bounce-out" | "bounce_out" => Some(Easing::BounceOut),
        "bounce-in-out" | "bounce_in_out" => Some(Easing::BounceInOut),
        s if s.starts_with("steps(") && s.ends_with(')') => {
            let inner = &s[6..s.len() - 1];
            inner.parse::<u32>().ok().map(Easing::Steps)
        }
        "spring-default" | "spring_default" => Some(presets::SPRING_DEFAULT),
        "spring-gentle" | "spring_gentle" => Some(presets::SPRING_GENTLE),
        "spring-stiff" | "spring_stiff" => Some(presets::SPRING_STIFF),
        "spring-slow" | "spring_slow" => Some(presets::SPRING_SLOW),
        "spring-wobbly" | "spring_wobbly" => Some(presets::SPRING_WOBBLY),
        s if s.starts_with("bezier:") => parse_bezier(&s[7..]),
        // GSAP-style power easings
        "power1.in" => Some(Easing::Power1In),
        "power1.out" => Some(Easing::Power1Out),
        "power1.inOut" | "power1.inout" => Some(Easing::Power1InOut),
        "power2.in" => Some(Easing::Power2In),
        "power2.out" => Some(Easing::Power2Out),
        "power2.inOut" | "power2.inout" => Some(Easing::Power2InOut),
        "power3.in" => Some(Easing::Power3In),
        "power3.out" => Some(Easing::Power3Out),
        "power3.inOut" | "power3.inout" => Some(Easing::Power3InOut),
        "power4.in" => Some(Easing::Power4In),
        "power4.out" => Some(Easing::Power4Out),
        "power4.inOut" | "power4.inout" => Some(Easing::Power4InOut),
        // GSAP-style math easings
        "circ.in" => Some(Easing::CircIn),
        "circ.out" => Some(Easing::CircOut),
        "circ.inOut" | "circ.inout" => Some(Easing::CircInOut),
        "expo.in" => Some(Easing::ExpoIn),
        "expo.out" => Some(Easing::ExpoOut),
        "expo.inOut" | "expo.inout" => Some(Easing::ExpoInOut),
        "sine.in" => Some(Easing::SineIn),
        "sine.out" => Some(Easing::SineOut),
        "sine.inOut" | "sine.inout" => Some(Easing::SineInOut),
        // GSAP-style back easings with parameter
        s if s.starts_with("back.in(") && s.ends_with(')') => {
            parse_gsap_back(&s[8..s.len() - 1], Easing::BackIn)
        }
        s if s.starts_with("back.out(") && s.ends_with(')') => {
            parse_gsap_back(&s[9..s.len() - 1], Easing::BackOut)
        }
        s if s.starts_with("back.inOut(") && s.ends_with(')') => {
            parse_gsap_back(&s[11..s.len() - 1], Easing::BackInOut)
        }
        // GSAP-style elastic easings with parameters
        s if s.starts_with("elastic.in(") && s.ends_with(')') => {
            parse_gsap_elastic(&s[11..s.len() - 1], true)
        }
        s if s.starts_with("elastic.out(") && s.ends_with(')') => {
            parse_gsap_elastic(&s[12..s.len() - 1], false)
        }
        _ => None,
    }
}

fn parse_bezier(input: &str) -> Option<Easing> {
    let parts: Vec<&str> = input.split(',').collect();
    if parts.len() != 4 {
        return None;
    }
    let x1 = parts[0].parse().ok()?;
    let y1 = parts[1].parse().ok()?;
    let x2 = parts[2].parse().ok()?;
    let y2 = parts[3].parse().ok()?;
    Some(Easing::CubicBezier(x1, y1, x2, y2))
}

fn parse_gsap_back(input: &str, _default: Easing) -> Option<Easing> {
    // GSAP back easing parameter is the overshoot amount
    // We ignore the parameter for now and use the default back easing
    let _overshoot: f32 = input.parse().ok()?;
    Some(_default)
}

fn parse_gsap_elastic(input: &str, is_in: bool) -> Option<Easing> {
    // GSAP elastic easing parameters: amplitude, period
    // We ignore the parameters for now and use the default elastic easing
    let parts: Vec<&str> = input.split(',').collect();
    if parts.len() >= 1 {
        let _amplitude: f32 = parts[0].parse().ok()?;
    }
    if is_in {
        Some(Easing::ElasticIn)
    } else {
        Some(Easing::ElasticOut)
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
        let result = animate_value(5, 20, 10, 0.0, 100.0, &Easing::Linear, false, 0, false, 0);
        assert!((result - 0.0).abs() < 1e-6);
    }

    #[test]
    fn animate_value_at_end_returns_to() {
        let result = animate_value(30, 20, 10, 0.0, 100.0, &Easing::Linear, false, 0, false, 0);
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
    fn easing_from_name_parses_bezier() {
        let easing = easing_from_name("bezier:0.4,0,0.2,1").unwrap();
        assert!(matches!(easing, Easing::CubicBezier(0.4, 0.0, 0.2, 1.0)));
        assert!(easing_from_name("bezier:0.4").is_none());
        assert!(easing_from_name("bezier:a,b,c,d").is_none());
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

    #[test]
    fn back_easings_have_overshoot() {
        let v = Easing::BackOut.apply(0.5);
        assert!(v > 0.5, "BackOut at 0.5 should be > 0.5, got {v}");
        assert!((Easing::BackOut.apply(0.0) - 0.0).abs() < 1e-4);
        assert!((Easing::BackOut.apply(1.0) - 1.0).abs() < 1e-4);
        let v = Easing::BackIn.apply(0.5);
        assert!(v < 0.5, "BackIn at 0.5 should be < 0.5, got {v}");
    }

    #[test]
    fn back_easings_via_name() {
        assert!(matches!(easing_from_name("back-in"), Some(Easing::BackIn)));
        assert!(matches!(
            easing_from_name("back-out"),
            Some(Easing::BackOut)
        ));
        assert!(matches!(
            easing_from_name("back-in-out"),
            Some(Easing::BackInOut)
        ));
    }

    #[test]
    fn elastic_easings_boundaries() {
        for e in &[Easing::ElasticIn, Easing::ElasticOut, Easing::ElasticInOut] {
            assert!(
                (e.apply(0.0) - 0.0).abs() < 1e-4,
                "{:?} at 0 should be 0",
                e
            );
            assert!(
                (e.apply(1.0) - 1.0).abs() < 1e-4,
                "{:?} at 1 should be 1",
                e
            );
        }
    }

    #[test]
    fn elastic_easings_via_name() {
        assert!(matches!(
            easing_from_name("elastic-in"),
            Some(Easing::ElasticIn)
        ));
        assert!(matches!(
            easing_from_name("elastic-out"),
            Some(Easing::ElasticOut)
        ));
        assert!(matches!(
            easing_from_name("elastic-in-out"),
            Some(Easing::ElasticInOut)
        ));
    }

    #[test]
    fn bounce_easings_boundaries() {
        for e in &[Easing::BounceIn, Easing::BounceOut, Easing::BounceInOut] {
            assert!((e.apply(0.0) - 0.0).abs() < 1e-4);
            assert!((e.apply(1.0) - 1.0).abs() < 1e-4);
            for i in 1..10 {
                let t = i as f32 / 10.0;
                let v = e.apply(t);
                assert!(
                    (0.0..=1.0).contains(&v),
                    "{:?} at {t} = {v} out of range",
                    e
                );
            }
        }
    }

    #[test]
    fn bounce_easings_via_name() {
        assert!(matches!(
            easing_from_name("bounce-in"),
            Some(Easing::BounceIn)
        ));
        assert!(matches!(
            easing_from_name("bounce-out"),
            Some(Easing::BounceOut)
        ));
        assert!(matches!(
            easing_from_name("bounce-in-out"),
            Some(Easing::BounceInOut)
        ));
    }

    #[test]
    fn steps_easing_quantizes() {
        let e = Easing::Steps(4);
        assert!((e.apply(0.0) - 0.0).abs() < 1e-6);
        assert!((e.apply(0.24) - 0.0).abs() < 1e-6);
        assert!((e.apply(0.26) - 0.25).abs() < 1e-6);
        assert!((e.apply(0.5) - 0.5).abs() < 1e-6);
        assert!((e.apply(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn steps_easing_zero_falls_back_to_linear() {
        let e = Easing::Steps(0);
        assert!((e.apply(0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn steps_easing_via_name() {
        assert!(matches!(
            easing_from_name("steps(4)"),
            Some(Easing::Steps(4))
        ));
        assert!(matches!(
            easing_from_name("steps(1)"),
            Some(Easing::Steps(1))
        ));
        assert!(easing_from_name("steps()").is_none());
        assert!(easing_from_name("steps(abc)").is_none());
    }

    #[test]
    fn animate_value_repeat_loops_back() {
        let v0 = animate_value(0, 10, 0, 0.0, 100.0, &Easing::Linear, false, 2, false, 0);
        let v5 = animate_value(5, 10, 0, 0.0, 100.0, &Easing::Linear, false, 2, false, 0);
        let v15 = animate_value(15, 10, 0, 0.0, 100.0, &Easing::Linear, false, 2, false, 0);
        let v25 = animate_value(25, 10, 0, 0.0, 100.0, &Easing::Linear, false, 2, false, 0);
        let v40 = animate_value(40, 10, 0, 0.0, 100.0, &Easing::Linear, false, 2, false, 0);
        assert!((v0 - 0.0).abs() < 1e-3);
        assert!((v5 - 50.0).abs() < 1e-3);
        assert!((v15 - 50.0).abs() < 1e-3);
        assert!((v25 - 50.0).abs() < 1e-3);
        assert!(
            (v40 - 100.0).abs() < 1e-3,
            "frame 40 past repeat=2, expect 100.0, got {v40}"
        );
    }

    #[test]
    fn animate_value_yoyo_reverses_alternate_cycles() {
        let v5 = animate_value(5, 10, 0, 0.0, 100.0, &Easing::Linear, false, -1, true, 0);
        let v15 = animate_value(15, 10, 0, 0.0, 100.0, &Easing::Linear, false, -1, true, 0);
        let v25 = animate_value(25, 10, 0, 0.0, 100.0, &Easing::Linear, false, -1, true, 0);
        assert!((v5 - 50.0).abs() < 1e-3);
        assert!((v15 - 50.0).abs() < 1e-3, "cycle 1 reverse at half = 50");
        assert!((v25 - 50.0).abs() < 1e-3, "cycle 2 forward at half = 50");
    }

    #[test]
    fn animate_value_repeat_delay_holds_to() {
        let v = animate_value(12, 10, 0, 0.0, 100.0, &Easing::Linear, false, -1, false, 5);
        assert!(
            (v - 100.0).abs() < 1e-3,
            "in repeat_delay should hold `to`, got {v}"
        );
    }

    #[test]
    fn animate_value_no_repeat_matches_old_behavior() {
        let v = animate_value(15, 10, 5, 0.0, 100.0, &Easing::Linear, false, 0, false, 0);
        assert!((v - 100.0).abs() < 1e-3);
    }

    // GSAP-style easing tests
    #[test]
    fn gsap_power_easings_parse() {
        assert!(matches!(
            easing_from_name("power1.in"),
            Some(Easing::Power1In)
        ));
        assert!(matches!(
            easing_from_name("power1.out"),
            Some(Easing::Power1Out)
        ));
        assert!(matches!(
            easing_from_name("power1.inOut"),
            Some(Easing::Power1InOut)
        ));
        assert!(matches!(
            easing_from_name("power2.in"),
            Some(Easing::Power2In)
        ));
        assert!(matches!(
            easing_from_name("power2.out"),
            Some(Easing::Power2Out)
        ));
        assert!(matches!(
            easing_from_name("power2.inOut"),
            Some(Easing::Power2InOut)
        ));
        assert!(matches!(
            easing_from_name("power3.in"),
            Some(Easing::Power3In)
        ));
        assert!(matches!(
            easing_from_name("power3.out"),
            Some(Easing::Power3Out)
        ));
        assert!(matches!(
            easing_from_name("power3.inOut"),
            Some(Easing::Power3InOut)
        ));
        assert!(matches!(
            easing_from_name("power4.in"),
            Some(Easing::Power4In)
        ));
        assert!(matches!(
            easing_from_name("power4.out"),
            Some(Easing::Power4Out)
        ));
        assert!(matches!(
            easing_from_name("power4.inOut"),
            Some(Easing::Power4InOut)
        ));
    }

    #[test]
    fn gsap_math_easings_parse() {
        assert!(matches!(easing_from_name("circ.in"), Some(Easing::CircIn)));
        assert!(matches!(
            easing_from_name("circ.out"),
            Some(Easing::CircOut)
        ));
        assert!(matches!(
            easing_from_name("circ.inOut"),
            Some(Easing::CircInOut)
        ));
        assert!(matches!(easing_from_name("expo.in"), Some(Easing::ExpoIn)));
        assert!(matches!(
            easing_from_name("expo.out"),
            Some(Easing::ExpoOut)
        ));
        assert!(matches!(
            easing_from_name("expo.inOut"),
            Some(Easing::ExpoInOut)
        ));
        assert!(matches!(easing_from_name("sine.in"), Some(Easing::SineIn)));
        assert!(matches!(
            easing_from_name("sine.out"),
            Some(Easing::SineOut)
        ));
        assert!(matches!(
            easing_from_name("sine.inOut"),
            Some(Easing::SineInOut)
        ));
    }

    #[test]
    fn gsap_power_easings_boundaries() {
        for e in &[
            Easing::Power1In,
            Easing::Power1Out,
            Easing::Power1InOut,
            Easing::Power2In,
            Easing::Power2Out,
            Easing::Power2InOut,
            Easing::Power3In,
            Easing::Power3Out,
            Easing::Power3InOut,
            Easing::Power4In,
            Easing::Power4Out,
            Easing::Power4InOut,
        ] {
            assert!(
                (e.apply(0.0) - 0.0).abs() < 1e-4,
                "{:?} at 0 should be 0",
                e
            );
            assert!(
                (e.apply(1.0) - 1.0).abs() < 1e-4,
                "{:?} at 1 should be 1",
                e
            );
        }
    }

    #[test]
    fn gsap_math_easings_boundaries() {
        for e in &[
            Easing::CircIn,
            Easing::CircOut,
            Easing::CircInOut,
            Easing::ExpoIn,
            Easing::ExpoOut,
            Easing::ExpoInOut,
            Easing::SineIn,
            Easing::SineOut,
            Easing::SineInOut,
        ] {
            assert!(
                (e.apply(0.0) - 0.0).abs() < 1e-4,
                "{:?} at 0 should be 0",
                e
            );
            assert!(
                (e.apply(1.0) - 1.0).abs() < 1e-4,
                "{:?} at 1 should be 1",
                e
            );
        }
    }

    #[test]
    fn gsap_power_easings_monotonic() {
        // Power easings should be monotonic (increasing)
        for e in &[
            Easing::Power1In,
            Easing::Power2In,
            Easing::Power3In,
            Easing::Power4In,
        ] {
            let mut prev = 0.0;
            for i in 1..=10 {
                let t = i as f32 / 10.0;
                let v = e.apply(t);
                assert!(v >= prev, "{:?} at {t} = {v} should be >= {prev}", e);
                prev = v;
            }
        }
    }

    #[test]
    fn gsap_back_easings_with_parameter() {
        assert!(matches!(
            easing_from_name("back.in(1.7)"),
            Some(Easing::BackIn)
        ));
        assert!(matches!(
            easing_from_name("back.out(1.7)"),
            Some(Easing::BackOut)
        ));
        assert!(matches!(
            easing_from_name("back.inOut(1.7)"),
            Some(Easing::BackInOut)
        ));
    }

    #[test]
    fn gsap_elastic_easings_with_parameter() {
        assert!(matches!(
            easing_from_name("elastic.in(1, 0.3)"),
            Some(Easing::ElasticIn)
        ));
        assert!(matches!(
            easing_from_name("elastic.out(1, 0.3)"),
            Some(Easing::ElasticOut)
        ));
    }
}
