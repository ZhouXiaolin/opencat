//! CSS 属性文本 → style 语义的单点换算（H2）。
//!
//! 两条写入通道（GSAP CSSPlugin 的字符串写、声明式 CSS 的 lightningcss
//! 解析）落在同一批 style 字段上；这里收纳与"属性文本怎么读"有关的纯
//! 函数，`script/recorder/store.rs` 的写路径只做 dispatch，不再携带
//! 解析逻辑。声明式路径的单位换算需要求值上下文，仍在
//! `css/to_node_style.rs`；本模块只收无上下文的纯换算。

use crate::style::Transform;

/// Parse a CSS `transform` function list into engine transforms.
///
/// GSAP's CSSPlugin is the only writer: it emits exactly `perspective()`,
/// `translate()/translate3d()`, `rotate()/rotateX()/rotateY()`, `skew()` and
/// `scale()` in a fixed order. Anything else (matrix(), rotateZ()…) is not
/// produced by the plugin suite and is ignored.
pub fn parse_css_transform(text: &str) -> Vec<Transform> {
    let mut out = Vec::new();
    // CSSPlugin emits `perspective()` first, then `rotateY()`, then `rotateX()`
    // — the three fold into one projective transform whose matrix is
    // P·Ry·Rx, exactly what `Transform::Rotate3` stores. The placeholder is
    // pushed where the first of the three appears so the list keeps the CSS
    // order (later entries are applied to points first).
    let mut rotate3: Option<usize> = None;
    let mut rest = text.trim();
    while let Some(open) = rest.find('(') {
        let name = rest[..open].trim().to_ascii_lowercase();
        let Some(close) = rest[open..].find(')') else {
            break;
        };
        let args: Vec<f32> = rest[open + 1..open + close]
            .split(',')
            .filter_map(|part| part.trim().trim_end_matches("px").trim_end_matches("deg").parse::<f32>().ok())
            .collect();
        let first = args.first().copied().unwrap_or(0.0);
        match name.as_str() {
            "translate" => out.push(Transform::Translate {
                x: first,
                y: args.get(1).copied().unwrap_or(0.0),
            }),
            "translate3d" => {
                out.push(Transform::TranslateX { value: first });
                out.push(Transform::TranslateY {
                    value: args.get(1).copied().unwrap_or(0.0),
                });
                let z = args.get(2).copied().unwrap_or(0.0);
                if z != 0.0 {
                    out.push(Transform::TranslateZ { value: z });
                }
            }
            "translatex" => out.push(Transform::TranslateX { value: first }),
            "translatey" => out.push(Transform::TranslateY { value: first }),
            "rotate" => out.push(Transform::RotateDeg { value: first }),
            "skew" => out.push(Transform::SkewDeg {
                x: first,
                y: args.get(1).copied().unwrap_or(0.0),
            }),
            "skewx" => out.push(Transform::SkewXDeg { value: first }),
            "skewy" => out.push(Transform::SkewYDeg { value: first }),
            // CSSPlugin's renderTransform emits `scale(sx)` for a uniform
            // scale and `scale(sx, sy)` for a non-uniform one — `scaleY: 0`
            // arrives as `scale(1, 0)`. Keeping only the first argument loses
            // the second axis, so the node never collapses.
            "scale" => match args.get(1).copied() {
                Some(y) => {
                    out.push(Transform::ScaleX { value: first });
                    out.push(Transform::ScaleY { value: y });
                }
                None => out.push(Transform::Scale { value: first }),
            },
            "scalex" => out.push(Transform::ScaleX { value: first }),
            "scaley" => out.push(Transform::ScaleY { value: first }),
            // perspective()/rotateX()/rotateY() fold into one Rotate3; the
            // writes land through the same path as the named properties.
            "perspective" => set_rotate3(&mut out, &mut rotate3, 2, first),
            "rotatex" => set_rotate3(&mut out, &mut rotate3, 0, first),
            "rotatey" => set_rotate3(&mut out, &mut rotate3, 1, first),
            _ => {}
        }
        rest = &rest[open + close + 1..];
    }
    out
}

/// Write one field of the folded `Rotate3`, pushing the transform the first
/// time any of `perspective()`/`rotateX()`/`rotateY()` shows up.
fn set_rotate3(out: &mut Vec<Transform>, slot: &mut Option<usize>, field: usize, value: f32) {
    let index = *slot.get_or_insert_with(|| {
        out.push(Transform::Rotate3 {
            rotation_x: 0.0,
            rotation_y: 0.0,
            perspective: 0.0,
        });
        out.len() - 1
    });
    if let Transform::Rotate3 {
        rotation_x,
        rotation_y,
        perspective,
    } = &mut out[index]
    {
        match field {
            0 => *rotation_x = value,
            1 => *rotation_y = value,
            _ => *perspective = value,
        }
    }
}

/// Resolve the pixel components of a gsap `transformOrigin` string into box
/// fractions (`'174px 0'` on an 820×1716 image → `'21.219513% 0%'`).
///
/// A browser resolves px origins against the used box; the write channel
/// has no layout, so it uses the declared box (`w-[820px] h-[1716px]`),
/// which is the used box for every fixed-size node. An axis without a
/// declared size keeps its token verbatim — `TransformOrigin::parse` then
/// either accepts it (percent/keyword/zero) or leaves the previous origin,
/// as before. `None` on both axes returns the input verbatim.
pub fn origin_px_to_fractions(value: &str, width: Option<f64>, height: Option<f64>) -> String {
    if width.is_none() && height.is_none() {
        return value.to_string();
    }
    // CSS accepts either `[x] [y]` or the y-first keyword order; a lone
    // token positions x (see `TransformOrigin::parse`).
    let tokens: Vec<&str> = value.split_whitespace().collect();
    let y_first = matches!(tokens.first().copied(), Some("top" | "bottom"));
    tokens
        .iter()
        .enumerate()
        .map(|(i, token)| {
            let Some(bare) = token.strip_suffix("px") else {
                return (*token).to_string();
            };
            let Ok(px) = bare.trim().parse::<f64>() else {
                return (*token).to_string();
            };
            let is_y = match tokens.len() {
                1 => false,
                _ => {
                    if y_first {
                        i == 0
                    } else {
                        i == 1
                    }
                }
            };
            match if is_y { height } else { width } {
                Some(size) => format!("{}%", px / size * 100.0),
                None => (*token).to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
