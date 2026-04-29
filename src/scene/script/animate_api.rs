use std::collections::HashMap;

use rquickjs::{Array, Function};

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
    repeat: i32,
    yoyo: bool,
    repeat_delay: u32,
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
            move |duration: f32,
                  delay: f32,
                  clamp_flag: i32,
                  easing_tag: String,
                  repeat: i32,
                  yoyo_flag: i32,
                  repeat_delay: f32|
                  -> i32 {
                let clamp = clamp_flag != 0;
                let yoyo = yoyo_flag != 0;

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
                let repeat_delay_u32 = repeat_delay.max(0.0) as u32;

                let progress = crate::scene::easing::compute_progress(
                    current_frame,
                    duration_u32,
                    delay_u32,
                    &easing,
                    clamp,
                    repeat,
                    yoyo,
                    repeat_delay_u32,
                );

                let total_frames = if repeat >= 0 {
                    duration_u32
                        .saturating_mul(repeat as u32 + 1)
                        .saturating_add(repeat_delay_u32.saturating_mul(repeat as u32))
                } else {
                    u32::MAX
                };
                let settled =
                    repeat >= 0 && current_frame >= delay_u32.saturating_add(total_frames);
                let settle_frame = delay_u32.saturating_add(total_frames);

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
                        repeat,
                        yoyo,
                        repeat_delay: repeat_delay_u32,
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
                        entry.repeat,
                        entry.yoyo,
                        entry.repeat_delay,
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
                            let result = lerp_hsla_clamped(&f, &t, entry.progress);
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

    globals.set(
        "__util_random_seeded",
        Function::new(ctx.clone(), |seed: f32| -> f32 { random_from_seed(seed) })?,
    )?;

    globals.set(
        "__text_graphemes",
        Function::new(
            ctx.clone(),
            |ctx_inner: rquickjs::Ctx<'js>, text: String| -> Result<Array<'js>, rquickjs::Error> {
                let result = Array::new(ctx_inner)?;
                for (index, grapheme) in unicode_segmentation::UnicodeSegmentation::graphemes(
                    text.as_str(),
                    true,
                )
                .enumerate()
                {
                    result.set(index, grapheme.to_string())?;
                }
                Ok(result)
            },
        )?,
    )?;

    globals.set(
        "__easing_apply",
        Function::new(ctx.clone(), |tag: String, t: f32| -> f32 {
            let easing = parse_easing_from_tag(&tag);
            easing.apply(t)
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__along_path_create",
        Function::new(
            ctx.clone(),
            move |svg: String| -> Result<i32, rquickjs::Error> {
                let path = skia_safe::Path::from_svg(&svg).ok_or_else(|| {
                    rquickjs::Error::new_from_js_message(
                        "alongPath",
                        "create",
                        format!("invalid SVG path: `{svg}`"),
                    )
                })?;
                let mut iter = skia_safe::ContourMeasureIter::new(&path, false, None);
                let contour = iter.next().ok_or_else(|| {
                    rquickjs::Error::new_from_js_message(
                        "alongPath",
                        "create",
                        "SVG path has no measurable contours".to_string(),
                    )
                })?;
                let store = s.lock().unwrap();
                let mut state = store.path_measure_state.lock().unwrap();
                let handle = state.next_id;
                state.next_id += 1;
                state.entries.insert(handle, PathMeasureEntry { contour });
                Ok(handle)
            },
        )?,
    )?;

    let s = store.clone();
    globals.set(
        "__along_path_length",
        Function::new(ctx.clone(), move |handle: i32| -> f32 {
            let store = s.lock().unwrap();
            let state = store.path_measure_state.lock().unwrap();
            state
                .entries
                .get(&handle)
                .map(|e| e.contour.length())
                .unwrap_or(0.0)
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__along_path_at",
        Function::new(ctx.clone(), move |handle: i32, t: f32| -> Vec<f32> {
            let store = s.lock().unwrap();
            let state = store.path_measure_state.lock().unwrap();
            let Some(entry) = state.entries.get(&handle) else {
                return vec![0.0, 0.0, 0.0];
            };
            let len = entry.contour.length();
            let clamped = t.clamp(0.0, 1.0);
            match entry.contour.pos_tan(clamped * len) {
                Some((p, v)) => {
                    let angle_deg = v.y.atan2(v.x).to_degrees();
                    vec![p.x, p.y, angle_deg]
                }
                None => vec![0.0, 0.0, 0.0],
            }
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__along_path_dispose",
        Function::new(ctx.clone(), move |handle: i32| {
            let store = s.lock().unwrap();
            let mut state = store.path_measure_state.lock().unwrap();
            state.entries.remove(&handle);
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
pub(super) struct HSLA {
    pub h: f32,
    pub s: f32,
    pub l: f32,
    pub a: f32,
}

pub(super) fn parse_color(input: &str) -> Option<HSLA> {
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

fn lerp_hsla_clamped(from: &HSLA, to: &HSLA, t: f32) -> HSLA {
    lerp_hsla(from, to, t.clamp(0.0, 1.0))
}

fn hsla_to_rgba_string(hsla: &HSLA) -> String {
    let (r, g, b) = hsl_to_rgb(hsla.h, hsla.s, hsla.l);
    format!("rgba({},{},{},{:.2})", r, g, b, hsla.a)
}

pub(super) fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
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

fn xorshift32(seed: u32) -> u32 {
    let mut x = if seed == 0 { 0x9E3779B9 } else { seed };
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    x
}

pub(crate) fn random_from_seed(seed: f32) -> f32 {
    let bits = if seed.is_finite() {
        seed.to_bits()
    } else {
        1u32
    };
    let r = xorshift32(bits);
    (r as f32) / (u32::MAX as f32)
}

pub(crate) struct PathMeasureEntry {
    pub contour: skia_safe::ContourMeasure,
}

#[derive(Default)]
pub(crate) struct PathMeasureState {
    pub next_id: i32,
    pub entries: std::collections::HashMap<i32, PathMeasureEntry>,
}

#[cfg(test)]
mod tests {
    use super::{HSLA, lerp_hsla, lerp_hsla_clamped, parse_easing_from_tag, random_from_seed};

    #[test]
    fn random_from_seed_is_deterministic() {
        let a = random_from_seed(42.0);
        let b = random_from_seed(42.0);
        assert!((a - b).abs() < 1e-9);
    }

    #[test]
    fn random_from_seed_is_in_unit_range() {
        for s in [0.5_f32, 1.0, -3.14, 999.999] {
            let r = random_from_seed(s);
            assert!((0.0..=1.0).contains(&r), "seed {s} -> {r}");
        }
    }

    #[test]
    fn random_from_seed_distributes_across_seeds() {
        let r1 = random_from_seed(1.0);
        let r2 = random_from_seed(2.0);
        let r3 = random_from_seed(3.0);
        assert!(
            r1 != r2 && r2 != r3,
            "expected distinct outputs for distinct seeds"
        );
    }

    #[test]
    fn lerp_hsla_clamps_progress_for_color_path() {
        let from = HSLA {
            h: 0.0,
            s: 1.0,
            l: 0.5,
            a: 1.0,
        };
        let to = HSLA {
            h: 240.0,
            s: 1.0,
            l: 0.5,
            a: 1.0,
        };
        let result = lerp_hsla_clamped(&from, &to, 1.5);
        let expected = lerp_hsla(&from, &to, 1.0);
        assert!((result.l - expected.l).abs() < 1e-6);
        assert!((result.s - expected.s).abs() < 1e-6);
    }

    #[test]
    fn parse_easing_from_tag_handles_extended() {
        use crate::scene::easing::Easing;
        assert!(matches!(parse_easing_from_tag("back-out"), Easing::BackOut));
        assert!(matches!(
            parse_easing_from_tag("steps(8)"),
            Easing::Steps(8)
        ));
        assert!(matches!(parse_easing_from_tag("unknown"), Easing::Linear));
    }

    #[test]
    fn skia_can_parse_svg_path_and_measure() {
        let path = skia_safe::Path::from_svg("M100 360 C400 80 880 640 1180 360")
            .expect("Skia should parse the SVG path");
        let mut iter = skia_safe::ContourMeasureIter::new(&path, false, None);
        let contour = iter.next().expect("path should have at least one contour");
        let len = contour.length();
        assert!(len > 1000.0, "expected length > 1000 (rough), got {len}");
        let (start, _) = contour.pos_tan(0.0).expect("pos_tan at 0 should exist");
        assert!((start.x - 100.0).abs() < 1.0, "start.x = {}", start.x);
        assert!((start.y - 360.0).abs() < 1.0, "start.y = {}", start.y);
        let (end, _) = contour.pos_tan(len).expect("pos_tan at len should exist");
        assert!((end.x - 1180.0).abs() < 1.0, "end.x = {}", end.x);
        assert!((end.y - 360.0).abs() < 1.0, "end.y = {}", end.y);
    }
}
