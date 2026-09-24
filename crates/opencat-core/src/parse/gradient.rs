//! Parser for CSS-style arbitrary background gradients.
//!
//! Consumed by the Tailwind arbitrary-value parser for classes like
//! `bg-[linear-gradient(...)]`, `bg-[radial-gradient(...)]`,
//! `bg-[repeating-linear-gradient(...)]`, and comma-separated multi-layer forms
//! `bg-[linear-gradient(...),linear-gradient(...)]`. Tailwind encodes whitespace
//! as `_`; this module restores it before tokenizing.

use crate::style::{
    ArbitraryGradient, BackgroundFill, ColorToken, GradientDirection, GradientLength,
    GradientStop, RadialExtent, RadialShape, StopUnit,
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

    let stops = normalize_stops(stop_args, repeat);
    let stops = if stops.is_empty() {
        vec![
            GradientStop {
                pos: 0.0,
                unit: StopUnit::Fraction,
                color: ColorToken::Transparent,
            },
            GradientStop {
                pos: 1.0,
                unit: StopUnit::Fraction,
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

/// A radial-gradient shape/size/position descriptor (the argument before the
/// color stops), e.g. `circle`, `ellipse 64% 70% at 50% 50%`, `farthest-corner`.
#[derive(Default)]
struct RadialDescriptor {
    center: [f32; 2],
    shape: RadialShape,
    radii: Option<[GradientLength; 2]>,
    extent: RadialExtent,
    recognized: bool,
}

/// Parse a radial-gradient argument list.
/// The first argument may be a shape/size/position descriptor
/// (`circle`, `ellipse 64% 70% at 50% 50%`, `closest-side`, …).
/// The rest are color stops.
fn parse_radial(args: &[String]) -> BackgroundFill {
    let mut stop_args = args;

    // Skip the leading shape/size/position term if present (it is not a color stop).
    if let Some(first) = args.first()
        && split_color_positions(first).is_none()
    {
        let desc = parse_radial_descriptor(first);
        if desc.recognized {
            stop_args = &args[1..];
            return BackgroundFill::ArbitraryGradient {
                gradient: ArbitraryGradient::RadialGradient {
                    center: desc.center,
                    stops: radial_stops(stop_args),
                    size: None,
                    repeat: false,
                    shape: desc.shape,
                    radii: desc.radii,
                    extent: desc.extent,
                },
            };
        }
    }

    BackgroundFill::ArbitraryGradient {
        gradient: ArbitraryGradient::RadialGradient {
            center: [0.5, 0.5],
            stops: radial_stops(stop_args),
            size: None,
            repeat: false,
            shape: RadialShape::Ellipse,
            radii: None,
            extent: RadialExtent::FarthestCorner,
        },
    }
}

fn radial_stops(stop_args: &[String]) -> Vec<GradientStop> {
    let stops = normalize_stops(stop_args, false);
    if stops.is_empty() {
        vec![
            GradientStop {
                pos: 0.0,
                unit: StopUnit::Fraction,
                color: ColorToken::Transparent,
            },
            GradientStop {
                pos: 1.0,
                unit: StopUnit::Fraction,
                color: ColorToken::Transparent,
            },
        ]
    } else {
        stops
    }
}

/// Parse `circle` / `ellipse <rx> <ry>` / `at <x> <y>` / extent keywords.
fn parse_radial_descriptor(term: &str) -> RadialDescriptor {
    let mut desc = RadialDescriptor {
        center: [0.5, 0.5],
        ..Default::default()
    };

    // Split off a trailing `at <pos>` clause.
    let (shape_part, at_part) = match term.find(" at ") {
        Some(idx) => (&term[..idx], Some(&term[idx + 4..])),
        None => (term, None),
    };

    if let Some(at) = at_part
        && let Some(center) = parse_radial_position(at)
    {
        desc.center = center;
        desc.recognized = true;
    }

    let mut lengths: Vec<GradientLength> = Vec::new();
    for tok in shape_part.split_whitespace() {
        match tok {
            "circle" => {
                desc.shape = RadialShape::Circle;
                desc.recognized = true;
            }
            "ellipse" => {
                desc.shape = RadialShape::Ellipse;
                desc.recognized = true;
            }
            "closest-side" => {
                desc.extent = RadialExtent::ClosestSide;
                desc.recognized = true;
            }
            "closest-corner" => {
                desc.extent = RadialExtent::ClosestCorner;
                desc.recognized = true;
            }
            "farthest-side" => {
                desc.extent = RadialExtent::FarthestSide;
                desc.recognized = true;
            }
            "farthest-corner" => {
                desc.extent = RadialExtent::FarthestCorner;
                desc.recognized = true;
            }
            other => {
                if let Some(len) = parse_gradient_length(other) {
                    lengths.push(len);
                    desc.recognized = true;
                }
            }
        }
    }

    match lengths.len() {
        1 => desc.radii = Some([lengths[0], lengths[0]]),
        2 => desc.radii = Some([lengths[0], lengths[1]]),
        _ => {}
    }

    desc
}

/// Parse a radial center position (relative to the box as unit fractions):
/// `50% 50%`, `center`, `left top`, `20% center`, `at 30% 10%`, …
fn parse_radial_position(term: &str) -> Option<[f32; 2]> {
    let toks: Vec<&str> = term.split_whitespace().collect();
    let resolve = |tok: &str, axis_len_ok: bool| -> Option<f32> {
        let _ = axis_len_ok;
        match tok {
            "center" => Some(0.5),
            "left" | "top" => Some(0.0),
            "right" | "bottom" => Some(1.0),
            other => other
                .strip_suffix('%')
                .and_then(|p| p.parse::<f32>().ok())
                .map(|v| (v / 100.0).clamp(0.0, 1.0)),
        }
    };
    match toks.as_slice() {
        [x] => Some([resolve(x, true)?, 0.5]),
        [x, y] => Some([resolve(x, true)?, resolve(y, false)?]),
        _ => None,
    }
}

fn parse_gradient_length(tok: &str) -> Option<GradientLength> {
    if let Some(pct) = tok.strip_suffix('%') {
        return pct
            .parse::<f32>()
            .ok()
            .map(|value| GradientLength::Percent { value });
    }
    if let Some(px) = tok.strip_suffix("px") {
        return px
            .parse::<f32>()
            .ok()
            .map(|value| GradientLength::Px { value });
    }
    None
}

// ── Color stop normalization ───────────────────────────────────────────

/// Parse a slice of raw stop strings (e.g. `rgba(0,255,136,0.06) 1px`,
/// `#ff0000 0%`, or a two-position stop like `rgba(0,0,0,0.5) 0 1px`) into
/// `GradientStop`s. Position units are preserved (`%` → fraction, `px` → px) so
/// the renderer can resolve px against the real gradient-line length. When
/// `repeat` is true the positions are normalized against the repeating period
/// so a single period maps to `0..1` (tiled by `TileMode::Repeat`).
fn normalize_stops(args: &[String], repeat: bool) -> Vec<GradientStop> {
    let mut entries: Vec<(Option<(f32, StopUnit)>, ColorToken)> = Vec::new();
    for arg in args {
        let Some((color, positions)) = split_color_positions(arg.trim()) else {
            continue;
        };
        if positions.is_empty() {
            entries.push((None, color));
        } else {
            for p in positions {
                entries.push((Some(p), color));
            }
        }
    }
    if entries.is_empty() {
        return Vec::new();
    }

    // Reference used to compare px and fraction positions while interpolating
    // auto-distributed stops: the largest explicit px position (fallback 1).
    let ref_px = entries
        .iter()
        .filter_map(|(p, _)| match p {
            Some((v, StopUnit::Px)) => Some(*v),
            _ => None,
        })
        .fold(0.0_f32, f32::max)
        .max(1.0);

    // Leading/trailing defaults.
    if entries[0].0.is_none() {
        entries[0].0 = Some((0.0, StopUnit::Fraction));
    }
    let last_idx = entries.len() - 1;
    if entries[last_idx].0.is_none() {
        let default = if repeat {
            // The last stop defines the repeating period; anchor it at the
            // largest explicit position (px) or at 1.0 for fraction stops.
            entries
                .iter()
                .filter_map(|(p, _)| match p {
                    Some((v, StopUnit::Px)) => Some((*v, StopUnit::Px)),
                    _ => None,
                })
                .next_back()
                .unwrap_or((1.0, StopUnit::Fraction))
        } else {
            (1.0, StopUnit::Fraction)
        };
        entries[last_idx].0 = Some(default);
    }

    // Convert a position to a comparable scalar for gap interpolation.
    let to_scalar = |p: (f32, StopUnit)| match p.1 {
        StopUnit::Px => p.0,
        StopUnit::Fraction => p.0 * ref_px,
    };
    let from_scalar = |v: f32, unit: StopUnit| match unit {
        StopUnit::Px => (v, StopUnit::Px),
        StopUnit::Fraction => (v / ref_px, StopUnit::Fraction),
    };

    // Fill interior gaps linearly between known neighbors.
    let mut i = 0;
    while i < entries.len() {
        if entries[i].0.is_some() {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < entries.len() && entries[j].0.is_none() {
            j += 1;
        }
        let start = entries[i - 1].0.map(to_scalar).unwrap_or(0.0);
        let end = entries.get(j).and_then(|e| e.0).map(to_scalar).unwrap_or(ref_px);
        let unit = entries[i - 1].0.map(|p| p.1).unwrap_or(StopUnit::Fraction);
        let span = end - start;
        let count = (j - (i - 1)) as f32;
        for k in i..j {
            let frac = (k - (i - 1)) as f32 / count;
            entries[k].0 = Some(from_scalar(start + span * frac, unit));
        }
        i = j + 1;
    }

    entries
        .into_iter()
        .map(|(p, color)| {
            let (v, unit) = p.unwrap_or((0.0, StopUnit::Fraction));
            GradientStop {
                pos: if matches!(unit, StopUnit::Fraction) {
                    v.clamp(0.0, 1.0)
                } else {
                    v.max(0.0)
                },
                unit,
                color,
            }
        })
        .collect()
}

/// Split a raw color-stop argument like `rgba(0,255,136,0.06) 1px` or
/// `#ff0000 0%` or `transparent 70%` into its color and (0..2) trailing position
/// tokens. Returns `None` when the argument is not a color stop (e.g. a shape
/// keyword such as `circle`).
fn split_color_positions(term: &str) -> Option<(ColorToken, Vec<(f32, StopUnit)>)> {
    let toks: Vec<&str> = term.split_whitespace().collect();
    if toks.is_empty() {
        return None;
    }
    let mut n_pos = 0usize;
    let mut positions: Vec<(f32, StopUnit)> = Vec::new();
    for tok in toks.iter().rev() {
        match parse_stop_position(tok) {
            Some(p) => {
                positions.push(p);
                n_pos += 1;
            }
            None => break,
        }
    }
    positions.reverse();
    let color_part = toks[..toks.len() - n_pos].join(" ");
    let color = parse_color(&color_part)?;
    Some((color, positions))
}

fn parse_stop_position(token: &str) -> Option<(f32, StopUnit)> {
    let token = token.trim();
    if let Some(pct) = token.strip_suffix('%') {
        return pct
            .parse::<f32>()
            .ok()
            .map(|v| ((v / 100.0).clamp(0.0, 1.0), StopUnit::Fraction));
    }
    if let Some(px) = token.strip_suffix("px") {
        return px.parse::<f32>().ok().map(|v| (v, StopUnit::Px));
    }
    token.parse::<f32>().ok().map(|v| (v, StopUnit::Fraction))
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
