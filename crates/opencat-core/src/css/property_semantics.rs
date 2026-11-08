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
/// One parsed `transform()` argument. GSAP's CSSPlugin emits `translate(x%,
/// y%)` for xPercent/yPercent (percent of the element's own box) and px/deg
/// everywhere else; the percent components land in the percent variants and
/// resolve against the element bounds at display build.
#[derive(Clone, Copy)]
enum ArgVal {
    Px(f32),
    Pct(f32),
    Deg(f32),
}

fn parse_arg(part: &str) -> Option<ArgVal> {
    let s = part.trim();
    let (bare, val) = if let Some(p) = s.strip_suffix('%') {
        (s, p.parse::<f32>().ok().map(ArgVal::Pct))
    } else if let Some(p) = s.strip_suffix("deg") {
        (s, p.parse::<f32>().ok().map(ArgVal::Deg))
    } else if let Some(p) = s.strip_suffix("px") {
        (s, p.parse::<f32>().ok().map(ArgVal::Px))
    } else {
        (s, s.parse::<f32>().ok().map(ArgVal::Px))
    };
    let _ = bare;
    val
}

/// A percent arg as a `translate` component (px keeps its value; percent
/// keeps the percent).
fn translate_component(v: Option<ArgVal>) -> (Option<f32>, Option<f32>) {
    match v {
        Some(ArgVal::Px(px)) => (Some(px), None),
        Some(ArgVal::Pct(pct)) => (None, Some(pct)),
        _ => (None, None),
    }
}

pub fn parse_css_transform(text: &str) -> Vec<Transform> {
    let mut out = Vec::new();
    // CSSPlugin emits `perspective()` first, then `rotateY()`, then `rotateX()`
    // — the three fold into one projective transform whose matrix is
    // P·Ry·Rx, exactly what `Transform::Rotate3` stores. The placeholder is
    // pushed where the first of the three appears so the list keeps the CSS
    // order (later entries are applied to points first).
    let mut rotate3: Option<usize> = None;
    // 没有 `perspective()` 时是普通 CSS 3D 变换串（脚本拼的 `rotateY(a)
    // translateZ(r) rotateY(90deg)` 环形排布）：各函数按序独立入列，折叠会让
    // 后一个 rotateY 覆盖前一个。
    let fold = text.contains("perspective(");
    let mut rest = text.trim();
    while let Some(open) = rest.find('(') {
        let name = rest[..open].trim().to_ascii_lowercase();
        let Some(close) = rest[open..].find(')') else {
            break;
        };
        let args: Vec<ArgVal> = rest[open + 1..open + close]
            .split(',')
            .filter_map(parse_arg)
            .collect();
        let first = args.first().copied();
        let px_or = |v: Option<ArgVal>| -> f32 {
            match v {
                Some(ArgVal::Px(px)) => px,
                _ => 0.0,
            }
        };
        let deg_or = |v: Option<ArgVal>| -> f32 {
            match v {
                Some(ArgVal::Deg(d)) => d,
                Some(ArgVal::Px(px)) => px,
                _ => 0.0,
            }
        };
        match name.as_str() {
            // px 分量与百分比分量混排：各自落到对应变体，display build 统一折算。
            "translate" => {
                let (x_px, x_pct) = translate_component(first);
                let (y_px, y_pct) = translate_component(args.get(1).copied());
                match (x_px, x_pct, y_px, y_pct) {
                    (Some(x), _, Some(y), _) => out.push(Transform::Translate { x, y }),
                    (Some(x), _, _, pct_y @ Some(_)) => {
                        out.push(Transform::TranslateX { value: x });
                        out.push(Transform::TranslateYPercent { value: pct_y.unwrap() });
                    }
                    (_, pct_x @ Some(_), Some(y), _) => {
                        out.push(Transform::TranslateXPercent { value: pct_x.unwrap() });
                        out.push(Transform::TranslateY { value: y });
                    }
                    (_, pct_x @ Some(_), _, pct_y @ Some(_)) => {
                        out.push(Transform::TranslateXPercent { value: pct_x.unwrap() });
                        out.push(Transform::TranslateYPercent { value: pct_y.unwrap() });
                    }
                    _ => {}
                }
            }
            "translate3d" => {
                let (x_px, x_pct) = translate_component(first);
                let (y_px, y_pct) = translate_component(args.get(1).copied());
                match (x_px, x_pct) {
                    (Some(x), _) => out.push(Transform::TranslateX { value: x }),
                    (_, Some(p)) => out.push(Transform::TranslateXPercent { value: p }),
                    _ => {}
                }
                match (y_px, y_pct) {
                    (Some(y), _) => out.push(Transform::TranslateY { value: y }),
                    (_, Some(p)) => out.push(Transform::TranslateYPercent { value: p }),
                    _ => {}
                }
                let z = px_or(args.get(2).copied());
                if z != 0.0 {
                    out.push(Transform::TranslateZ { value: z });
                }
            }
            "translatex" => match translate_component(first) {
                (Some(v), _) => out.push(Transform::TranslateX { value: v }),
                (_, Some(p)) => out.push(Transform::TranslateXPercent { value: p }),
                _ => {}
            },
            "translatey" => match translate_component(first) {
                (Some(v), _) => out.push(Transform::TranslateY { value: v }),
                (_, Some(p)) => out.push(Transform::TranslateYPercent { value: p }),
                _ => {}
            },
            "rotate" => out.push(Transform::RotateDeg {
                value: deg_or(first),
            }),
            "skew" => out.push(Transform::SkewDeg {
                x: deg_or(first),
                y: deg_or(args.get(1).copied()),
            }),
            "skewx" => out.push(Transform::SkewXDeg {
                value: deg_or(first),
            }),
            "skewy" => out.push(Transform::SkewYDeg {
                value: deg_or(first),
            }),
            // CSSPlugin's renderTransform emits `scale(sx)` for a uniform
            // scale and `scale(sx, sy)` for a non-uniform one — `scaleY: 0`
            // arrives as `scale(1, 0)`. Keeping only the first argument loses
            // the second axis, so the node never collapses.
            "scale" => {
                let sx = px_or(first);
                // gsap 对 `scale(sx, sy)` 才发双参；单参 `scale(sx)` 无第二
                // 个 arg。0 是合法 scaleY（压扁），不能用 0 区分缺省。
                match args.get(1) {
                    Some(_) => {
                        out.push(Transform::ScaleX { value: sx });
                        out.push(Transform::ScaleY {
                            value: px_or(args.get(1).copied()),
                        });
                    }
                    None => out.push(Transform::Scale { value: sx }),
                }
            }
            "scalex" => out.push(Transform::ScaleX {
                value: px_or(first),
            }),
            "scaley" => out.push(Transform::ScaleY {
                value: px_or(first),
            }),
            // perspective()/rotateX()/rotateY() fold into one Rotate3; the
            // writes land through the same path as the named properties.
            "perspective" => set_rotate3(&mut out, &mut rotate3, 2, px_or(first)),
            "rotatex" if fold => set_rotate3(&mut out, &mut rotate3, 0, deg_or(first)),
            "rotatey" if fold => set_rotate3(&mut out, &mut rotate3, 1, deg_or(first)),
            "rotatex" => out.push(Transform::RotateXDeg { value: deg_or(first) }),
            "rotatey" => out.push(Transform::RotateYDeg { value: deg_or(first) }),
            "rotatez" => out.push(Transform::RotateDeg { value: deg_or(first) }),
            "translatez" => out.push(Transform::TranslateZ { value: px_or(first) }),
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
