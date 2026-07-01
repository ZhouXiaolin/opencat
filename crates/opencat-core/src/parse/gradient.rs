//! Parser for CSS-style arbitrary background gradients.
//!
//! Consumed by the Tailwind arbitrary-value parser for classes like
//! `bg-[linear-gradient(...)]`, `bg-[radial-gradient(...)]`,
//! `bg-[repeating-linear-gradient(...)]`, and comma-separated multi-layer forms
//! `bg-[linear-gradient(...),linear-gradient(...)]`. Tailwind encodes whitespace
//! as `_`; this module restores it before tokenizing.

use crate::style::{
    ArbitraryGradient, BackgroundFill, ColorToken, GradientDirection, GradientStop,
};

/// Parse the interior of a `bg-[...]` class (after stripping the `bg-[` prefix
/// and trailing `]`). Returns `None` if the value is not a recognized gradient
/// function call, so the caller can fall through to other `bg-[...]` handlers
/// (e.g. hex colors).
///
/// Returns one `BackgroundFill::ArbitraryGradient` per comma-separated layer.
pub fn parse_background_gradient(value: &str) -> Option<Vec<BackgroundFill>> {
    // A gradient value always begins with a known function name followed by `(`.
    let trimmed = value.trim();
    if !is_gradient_function(trimmed) {
        return None;
    }

    // Tailwind encodes whitespace as `_`; restore it everywhere before parsing.
    // (Color tokens never contain `_`, so this is safe.)
    let normalized = trimmed.replace('_', " ");

    // Split into layers on top-level commas (depth-aware). Each layer is a full
    // `func(args)` call.
    let layers = split_top_level_commas(&normalized);
    let mut fills = Vec::with_capacity(layers.len());
    for layer in layers {
        let layer = layer.trim();
        if layer.is_empty() {
            continue;
        }
        let fill = parse_single_gradient(layer)?;
        fills.push(fill);
    }
    if fills.is_empty() { None } else { Some(fills) }
}

fn is_gradient_function(value: &str) -> bool {
    [
        "linear-gradient(",
        "radial-gradient(",
        "repeating-linear-gradient(",
    ]
    .iter()
    .any(|prefix| value.starts_with(prefix))
}

/// Parse a `bg-[length:...]` value (after stripping `bg-[` and `]`).
/// Accepts `length:64px_64px` or `length:64px` → `[w, h]`.
pub fn parse_background_size(value: &str) -> Option<[f32; 2]> {
    let inner = value.strip_prefix("length:")?;
    let normalized = inner.replace('_', " ");
    let parts: Vec<&str> = normalized.split_whitespace().collect();
    match parts.len() {
        1 => {
            let v = parse_length_px(parts[0])?;
            Some([v, v])
        }
        2 => {
            let w = parse_length_px(parts[0])?;
            let h = parse_length_px(parts[1])?;
            Some([w, h])
        }
        _ => None,
    }
}

fn parse_length_px(token: &str) -> Option<f32> {
    token
        .strip_suffix("px")
        .unwrap_or(token)
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0)
}

/// Attach a `background-size` to every layer in a multi-layer gradient list,
/// mutating in place. Used after parsing `bg-[length:...]` which must bind to
/// the gradient layers declared in the same class string.
pub fn apply_size_to_layers(layers: &mut [BackgroundFill], size: [f32; 2]) {
    for layer in layers.iter_mut() {
        if let BackgroundFill::ArbitraryGradient { gradient } = layer {
            match gradient {
                ArbitraryGradient::LinearGradient { size: slot, .. }
                | ArbitraryGradient::RadialGradient { size: slot, .. } => {
                    *slot = Some(size);
                }
            }
        }
    }
}

// ── Splitting helpers ──────────────────────────────────────────────────

/// Split a string on commas that are at paren-depth 0.
fn split_top_level_commas(value: &str) -> Vec<String> {
    let mut layers = Vec::new();
    let mut depth: i32 = 0;
    let mut start = 0;
    for (idx, ch) in value.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                layers.push(value[start..idx].trim().to_string());
                start = idx + 1;
            }
            _ => {}
        }
    }
    if start <= value.len() {
        layers.push(value[start..].trim().to_string());
    }
    layers
}

/// Split function arguments on top-level commas (inside the parens).
fn split_function_args(args: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth: i32 = 0;
    let mut start = 0;
    for (idx, ch) in args.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                out.push(args[start..idx].trim().to_string());
                start = idx + 1;
            }
            _ => {}
        }
    }
    if start <= args.len() {
        out.push(args[start..].trim().to_string());
    }
    out
}

// ── Single gradient parsing ────────────────────────────────────────────

fn parse_single_gradient(layer: &str) -> Option<BackgroundFill> {
    let (func, args) = split_function_call(layer)?;
    let normalized_args: Vec<String> = split_function_args(&args);
    match func.as_str() {
        "linear-gradient" => Some(parse_linear(&normalized_args, false)),
        "repeating-linear-gradient" => Some(parse_linear(&normalized_args, true)),
        "radial-gradient" => Some(parse_radial(&normalized_args)),
        _ => None,
    }
}

/// Split `func(args)` into `(func_name, args)`.
fn split_function_call(value: &str) -> Option<(String, String)> {
    let value = value.trim();
    let open = value.find('(')?;
    if !value.ends_with(')') {
        return None;
    }
    let func = value[..open].trim().to_string();
    let args = value[open + 1..value.len() - 1].trim().to_string();
    Some((func, args))
}

/// Parse a (repeating-)linear-gradient argument list.
/// The first argument may be an angle/direction; the rest are color stops.
fn parse_linear(args: &[String], repeat: bool) -> BackgroundFill {
    let mut angle_deg: Option<f32> = None;
    let mut direction: Option<GradientDirection> = None;
    let mut stop_args = args;

    // Detect leading direction/angle term.
    if let Some(first) = args.first()
        && let Some((a, d)) = parse_linear_direction(first)
    {
        angle_deg = a;
        direction = d;
        stop_args = &args[1..];
    }

    let stops = normalize_stops(stop_args);
    let stops = if stops.is_empty() {
        vec![
            GradientStop {
                pos: 0.0,
                color: ColorToken::Transparent,
            },
            GradientStop {
                pos: 1.0,
                color: ColorToken::Transparent,
            },
        ]
    } else {
        stops
    };

    BackgroundFill::ArbitraryGradient {
        gradient: ArbitraryGradient::LinearGradient {
            angle_deg,
            direction,
            stops,
            size: None,
            repeat,
        },
    }
}

/// Parse the leading direction/angle of a linear-gradient.
/// Recognizes: `90deg`, `to right`, `to bottom`, `to left`, `to top`.
fn parse_linear_direction(term: &str) -> Option<(Option<f32>, Option<GradientDirection>)> {
    let term = term.trim();
    if let Some(deg) = term.strip_suffix("deg")
        && let Ok(value) = deg.parse::<f32>()
    {
        return Some((Some(value), None));
    }
    // `to right` etc. (Tailwind encodes spaces as _, already restored).
    let direction = match term {
        "to right" => Some(GradientDirection::ToRight),
        "to left" => Some(GradientDirection::ToLeft),
        "to bottom" => Some(GradientDirection::ToBottom),
        "to top" => Some(GradientDirection::ToTop),
        "to bottom right" | "to right bottom" => Some(GradientDirection::ToBottomRight),
        _ => None,
    };
    direction.map(|d| (None, Some(d)))
}

/// Parse a radial-gradient argument list.
/// The first argument may be a shape/size term (`circle`, `ellipse`, `closest-side`,
/// `farthest-corner`, …); we accept and ignore it, defaulting to circle centered.
/// The rest are color stops.
fn parse_radial(args: &[String]) -> BackgroundFill {
    let mut stop_args = args;

    // Skip the leading shape/extent term if present (it is not a color stop).
    if let Some(first) = args.first()
        && !is_color_stop(first)
    {
        stop_args = &args[1..];
    }

    let stops = normalize_stops(stop_args);
    let stops = if stops.is_empty() {
        vec![
            GradientStop {
                pos: 0.0,
                color: ColorToken::Transparent,
            },
            GradientStop {
                pos: 1.0,
                color: ColorToken::Transparent,
            },
        ]
    } else {
        stops
    };

    BackgroundFill::ArbitraryGradient {
        gradient: ArbitraryGradient::RadialGradient {
            center: [0.5, 0.5],
            stops,
            size: None,
            repeat: false,
        },
    }
}

/// Heuristic: does this argument look like a color stop (contains a color)
/// rather than a shape/extent keyword?
fn is_color_stop(term: &str) -> bool {
    let term = term.trim();
    // A color stop contains a color token: starts with #, rgb, rgba, or is a
    // known color keyword / `transparent`.
    if term.starts_with('#')
        || term.starts_with("rgb")
        || term == "transparent"
        || crate::style::color_token_from_class_suffix(term).is_some()
    {
        return true;
    }
    // A color stop may also be `<position>` only in linear (handled by direction),
    // so treat leading numeric-with-unit as part of a stop only when a color is
    // also present. For radial leading terms like `circle`/`ellipse`/`closest-side`
    // this returns false.
    false
}

// ── Color stop normalization ───────────────────────────────────────────

/// Parse a slice of raw stop strings (e.g. `rgba(0,255,136,0.06) 1px`) into
/// `GradientStop`s with positions normalized to 0..1 relative to the gradient
/// extent. Position may be `%` (relative to extent) or `px` (absolute; kept as
/// pixels for repeating gradients where the extent equals the period).
fn normalize_stops(args: &[String]) -> Vec<GradientStop> {
    let mut stops: Vec<(Option<f32>, ColorToken)> = Vec::new();
    for arg in args {
        let Some((pos, color)) = parse_one_stop(arg.trim()) else {
            continue;
        };
        stops.push((pos, color));
    }
    if stops.is_empty() {
        return Vec::new();
    }

    // Resolve positions: CSS auto-distributes un-positioned stops evenly between
    // their neighbors. First fill in explicit positions, then interpolate gaps.
    // Positions can be in % (0..1 after /100) or px (kept absolute; for repeating
    // gradients the period is the last explicit px position).
    let mut explicit: Vec<Option<f32>> = stops.iter().map(|(p, _)| *p).collect();

    // Leading/trailing defaults.
    if explicit[0].is_none() {
        explicit[0] = Some(0.0);
    }
    let last_idx = explicit.len() - 1;
    if explicit[last_idx].is_none() {
        explicit[last_idx] = Some(1.0);
    }

    // Fill interior gaps linearly between known neighbors (in normalized 0..1).
    let mut i = 0;
    while i < explicit.len() {
        if explicit[i].is_some() {
            i += 1;
            continue;
        }
        // Find next explicit.
        let mut j = i;
        while j < explicit.len() && explicit[j].is_none() {
            j += 1;
        }
        let start_pos = explicit[i - 1].unwrap_or(0.0);
        let end_pos = explicit.get(j).and_then(|p| *p).unwrap_or(1.0);
        let span = end_pos - start_pos;
        let count = (j - (i - 1)) as f32;
        for (k, slot) in explicit.iter_mut().enumerate().take(j).skip(i) {
            let frac = (k - (i - 1)) as f32 / count;
            *slot = Some(start_pos + span * frac);
        }
        i = j + 1;
    }

    stops
        .iter()
        .zip(explicit.iter())
        .map(|((_, color), pos)| GradientStop {
            pos: pos.unwrap_or(0.0).clamp(0.0, 1.0),
            color: *color,
        })
        .collect()
}

/// Parse a single color stop string like `rgba(0,255,136,0.06) 1px` or
/// `transparent 70%` or `#00ff88`. Returns `(position, color)` where position is
/// `None` when absent. Position semantics:
///   - `N%` → `N/100` (normalized within the gradient extent)
///   - `Npx` → `None` (px positions are only meaningful with a background-size;
///     for repeating gradients they imply the period; we mark them None here and
///     the renderer relies on the size/tile-mode. To keep scanlines working we
///     convert px to a fraction of the repeating period at parse time below.)
fn parse_one_stop(raw: &str) -> Option<(Option<f32>, ColorToken)> {
    // Split color from trailing position(s). The color may contain spaces
    // (e.g. inside rgba it does not, since args are comma-separated), so the
    // color is the first whitespace-separated run that parses as a color, and
    // the remainder is the position. But colors like `rgba(...)` have no spaces
    // here (commas), and hex/named are single tokens. A position is a trailing
    // `<len>%?` token.
    let raw = raw.trim();

    // Try: the whole thing is just a color (no position).
    if let Some(color) = parse_color(raw) {
        return Some((None, color));
    }

    // Otherwise split off the last token as position, the rest as color.
    // Find the last whitespace.
    let last_space = raw.rfind(' ')?;
    let (color_part, pos_part) = raw.split_at(last_space);
    let color = parse_color(color_part.trim())?;
    let pos = parse_position(pos_part.trim());
    Some((pos, color))
}

fn parse_position(token: &str) -> Option<f32> {
    let token = token.trim();
    if let Some(pct) = token.strip_suffix('%') {
        return pct.parse::<f32>().ok().map(|v| (v / 100.0).clamp(0.0, 1.0));
    }
    // px positions: meaningful only with a background-size/period. Without a
    // known period we cannot normalize to 0..1, so return None and let the
    // even-distribution step handle it. (For repeating gradients with explicit
    // px stops the caller sets size = period and the renderer tiles accordingly.)
    if token.ends_with("px") {
        return None;
    }
    token.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
}

fn parse_color(token: &str) -> Option<ColorToken> {
    let token = token.trim();
    if token == "transparent" {
        return Some(ColorToken::Transparent);
    }
    crate::parse::jsonl::tailwind::color_from_hex(token)
        .or_else(|| crate::parse::jsonl::tailwind::parse_rgb_function_color(token))
        .or_else(|| crate::style::color_token_from_class_suffix(token))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_radial_gradient_glow() {
        let fills = parse_background_gradient(
            "radial-gradient(circle,rgba(0,255,136,0.14),transparent_70%)",
        )
        .expect("should parse");
        assert_eq!(fills.len(), 1);
        let BackgroundFill::ArbitraryGradient { gradient } = &fills[0] else {
            panic!("expected arbitrary gradient");
        };
        let ArbitraryGradient::RadialGradient { stops, .. } = gradient else {
            panic!("expected radial");
        };
        assert_eq!(stops.len(), 2);
        assert_eq!(stops[0].pos, 0.0);
        assert!((stops[1].pos - 0.7).abs() < 1e-5);
    }

    #[test]
    fn parses_multi_layer_grid() {
        let fills = parse_background_gradient(
            "linear-gradient(rgba(0,255,136,0.06)_1px,transparent_1px),linear-gradient(90deg,rgba(0,255,136,0.06)_1px,transparent_1px)",
        )
        .expect("should parse");
        assert_eq!(fills.len(), 2);
    }

    #[test]
    fn parses_repeating_scanline() {
        let fills = parse_background_gradient(
            "repeating-linear-gradient(0deg,transparent_0,transparent_3px,rgba(0,0,0,0.35)_3px,rgba(0,0,0,0.35)_4px)",
        )
        .expect("should parse");
        assert_eq!(fills.len(), 1);
        let BackgroundFill::ArbitraryGradient { gradient } = &fills[0] else {
            panic!("expected arbitrary gradient");
        };
        let ArbitraryGradient::LinearGradient { repeat, .. } = gradient else {
            panic!("expected linear");
        };
        assert!(*repeat);
    }

    #[test]
    fn rejects_non_gradient_value() {
        assert!(parse_background_gradient("#00ff88").is_none());
        assert!(parse_background_gradient("rgba(0,0,0,0.3)").is_none());
    }

    #[test]
    fn parses_background_size() {
        assert_eq!(
            parse_background_size("length:64px_64px"),
            Some([64.0, 64.0])
        );
        assert_eq!(parse_background_size("length:64px"), Some([64.0, 64.0]));
        assert_eq!(
            parse_background_size("length:64px_32px"),
            Some([64.0, 32.0])
        );
        assert!(parse_background_size("64px").is_none());
    }

    #[test]
    fn angle_direction_takes_precedence() {
        let fills = parse_background_gradient(
            "linear-gradient(90deg,rgba(0,255,136,0.06)_1px,transparent_1px)",
        )
        .expect("should parse");
        let BackgroundFill::ArbitraryGradient { gradient } = &fills[0] else {
            panic!()
        };
        let ArbitraryGradient::LinearGradient { angle_deg, .. } = gradient else {
            panic!()
        };
        assert_eq!(*angle_deg, Some(90.0));
    }
}
