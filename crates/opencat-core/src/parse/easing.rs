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
    // GSAP back easings carry the overshoot amount `s` (e.g. `back.out(2.5)`);
    // the bare CSS-style `back-out` uses GSAP's default 1.70158.
    BackIn(f32),
    BackOut(f32),
    BackInOut(f32),
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
            Easing::BackIn(s) => back_in(t, *s),
            Easing::BackOut(s) => back_out(t, *s),
            Easing::BackInOut(s) => back_in_out(t, *s),
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
                } else if t >= 1.0 {
                    1.0
                } else {
                    // GSAP SteppedEase (gsap@3.12 Ease.js `steps` config, de-minified):
                    // ratio(p) = (floor((N+1) * clamp(p, 0, 0.99999999)) + 0) / N.
                    // steps(1) therefore jumps at the MIDDLE of the tween (floor(2p)),
                    // not at the end — verified against the reference renders.
                    // (The p>=1 case is handled above; JS doubles keep 0.99999999
                    // distinct from 1.0 but f32 does not, so don't rely on the clamp.)
                    let nf = *n as f32;
                    ((nf + 1.0) * t.max(0.0)).floor() / nf
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

/// Time-based variant of [`compute_progress`] (all quantities in **seconds**).
///
/// GSAP advances tweens by elapsed wall-clock time, so this is the faithful
/// model: `duration`/`delay`/`repeat_delay` stay fractional and progress is a
/// function of `current_time`. The frame-based [`compute_progress`] quantises to
/// whole frames, which drifts up to half a frame on short tweens.
#[allow(clippy::too_many_arguments)]
pub fn compute_progress_secs(
    current_time: f32,
    duration: f32,
    delay: f32,
    easing: &Easing,
    clamp: bool,
    repeat: i32,
    yoyo: bool,
    repeat_delay: f32,
) -> f32 {
    if current_time <= delay {
        return 0.0;
    }
    if duration <= 0.0 {
        return 1.0;
    }
    let elapsed = current_time - delay;
    let cycle_len = duration + repeat_delay.max(0.0);
    let cycle_idx = (elapsed / cycle_len).floor() as i32;

    if repeat >= 0 && cycle_idx > repeat {
        return if yoyo && (repeat % 2 == 1) { 0.0 } else { 1.0 };
    }

    let in_cycle = elapsed - (cycle_idx as f32) * cycle_len;
    if in_cycle >= duration {
        return if !yoyo || cycle_idx % 2 == 0 {
            1.0
        } else {
            0.0
        };
    }

    let mut t = (in_cycle / duration).clamp(0.0, 1.0);
    if yoyo && (cycle_idx % 2 == 1) {
        t = 1.0 - t;
    }
    let p = easing.apply(t);
    if clamp { p.clamp(0.0, 1.0) } else { p }
}

/// Time-based variant of [`animate_value`] (all quantities in **seconds**).
#[allow(clippy::too_many_arguments)]
pub fn animate_value_secs(
    current_time: f32,
    duration: f32,
    delay: f32,
    from: f32,
    to: f32,
    easing: &Easing,
    clamp: bool,
    repeat: i32,
    yoyo: bool,
    repeat_delay: f32,
) -> f32 {
    if current_time <= delay {
        return from;
    }
    let p = compute_progress_secs(
        current_time,
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
        "back-in" | "back_in" => Some(Easing::BackIn(1.70158)),
        "back-out" | "back_out" => Some(Easing::BackOut(1.70158)),
        "back-in-out" | "back_in_out" => Some(Easing::BackInOut(1.70158)),
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
            parse_gsap_back(&s[8..s.len() - 1]).map(Easing::BackIn)
        }
        s if s.starts_with("back.out(") && s.ends_with(')') => {
            parse_gsap_back(&s[9..s.len() - 1]).map(Easing::BackOut)
        }
        s if s.starts_with("back.inOut(") && s.ends_with(')') => {
            parse_gsap_back(&s[11..s.len() - 1]).map(Easing::BackInOut)
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

/// Parses the GSAP `back.<dir>(<overshoot>)` argument. The overshoot defaults
/// to 1.70158 when omitted (GSAP's own default), and is otherwise forwarded
/// verbatim as the back-easing strength `s`.
fn parse_gsap_back(input: &str) -> Option<f32> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Some(1.70158);
    }
    trimmed.parse::<f32>().ok()
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


