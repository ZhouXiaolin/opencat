use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rquickjs::Function;

use crate::scene::easing::{Easing, SpringConfig};

use super::MutationStore;

pub(super) const ANIMATE_RUNTIME: &str = include_str!("runtime/animate_ctx.js");

struct AnimateEntry {
    progress: f32,
    settled: bool,
    settle_frame: u32,
    duration: u32,
    delay: u32,
    clamp: bool,
    easing: Easing,
}

#[derive(Default)]
pub(crate) struct AnimateState {
    next_id: i32,
    entries: HashMap<i32, AnimateEntry>,
}

pub(crate) fn install_animate_bindings<'js>(
    ctx: &rquickjs::Ctx<'js>,
    store: &MutationStore,
) -> anyhow::Result<()> {
    let globals = ctx.globals();

    let s = store.clone();
    globals.set(
        "__animate_create",
        Function::new(
            ctx.clone(),
            move |duration: f32, delay: f32, clamp_flag: i32, easing_tag: String| -> i32 {
                let clamp = clamp_flag != 0;

                let store = s.lock().unwrap();
                let current_frame = store.current_frame;
                let fps = 30.0f32;

                let easing = parse_easing_from_tag(&easing_tag);
                let duration_u32 = if duration < 0.0 {
                    easing.default_duration(fps).unwrap_or(1)
                } else {
                    duration as u32
                };
                let delay_u32 = delay as u32;

                let t = if current_frame <= delay_u32 {
                    0.0f32
                } else {
                    ((current_frame - delay_u32) as f32 / duration_u32 as f32).clamp(0.0, 1.0)
                };

                let progress = easing.apply(t);
                let settled = t >= 1.0;
                let settle_frame = delay_u32.saturating_add(duration_u32);

                let mut animate_state = store.animate_state.lock().unwrap();
                let handle = animate_state.next_id;
                animate_state.next_id += 1;
                animate_state.entries.insert(
                    handle,
                    AnimateEntry {
                        progress,
                        settled,
                        settle_frame,
                        duration: duration_u32,
                        delay: delay_u32,
                        clamp,
                        easing,
                    },
                );

                handle
            },
        )?,
    )?;

    let s = store.clone();
    globals.set(
        "__animate_value",
        Function::new(
            ctx.clone(),
            move |handle: i32, _key: String, from: f32, to: f32| -> f32 {
                let store = s.lock().unwrap();
                let current_frame = store.current_frame;

                let animate_state = store.animate_state.lock().unwrap();
                if let Some(entry) = animate_state.entries.get(&handle) {
                    crate::scene::easing::animate_value(
                        current_frame,
                        entry.duration,
                        entry.delay,
                        from,
                        to,
                        &entry.easing,
                        entry.clamp,
                    )
                } else {
                    from
                }
            },
        )?,
    )?;

    let s = store.clone();
    globals.set(
        "__animate_color",
        Function::new(
            ctx.clone(),
            move |handle: i32, _key: String, from: String, to: String| -> String {
                let store = s.lock().unwrap();
                let animate_state = store.animate_state.lock().unwrap();
                if let Some(entry) = animate_state.entries.get(&handle) {
                    let from_hsla = parse_color(&from);
                    let to_hsla = parse_color(&to);
                    match (from_hsla, to_hsla) {
                        (Some(f), Some(t)) => {
                            let result = lerp_hsla(&f, &t, entry.progress);
                            hsla_to_rgba_string(&result)
                        }
                        _ => from,
                    }
                } else {
                    from
                }
            },
        )?,
    )?;

    let s = store.clone();
    globals.set(
        "__animate_progress",
        Function::new(ctx.clone(), move |handle: i32| -> f32 {
            let store = s.lock().unwrap();
            let animate_state = store.animate_state.lock().unwrap();
            animate_state
                .entries
                .get(&handle)
                .map(|e| e.progress)
                .unwrap_or(0.0)
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__animate_settled",
        Function::new(ctx.clone(), move |handle: i32| -> bool {
            let store = s.lock().unwrap();
            let animate_state = store.animate_state.lock().unwrap();
            animate_state
                .entries
                .get(&handle)
                .map(|e| e.settled)
                .unwrap_or(false)
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__animate_settle_frame",
        Function::new(ctx.clone(), move |handle: i32| -> u32 {
            let store = s.lock().unwrap();
            let animate_state = store.animate_state.lock().unwrap();
            animate_state
                .entries
                .get(&handle)
                .map(|e| e.settle_frame)
                .unwrap_or(0)
        })?,
    )?;

    Ok(())
}

fn parse_easing_from_tag(tag: &str) -> Easing {
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
