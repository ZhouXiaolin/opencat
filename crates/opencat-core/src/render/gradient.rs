use crate::canvas::Rect;
use crate::canvas::paint::{
    ShaderSpec, TileMode,
};
use crate::style::{
    ArbitraryGradient, GradientDirection, GradientLength, GradientStop, RadialExtent, RadialShape, StopUnit,
};



use super::items::color_token_to_rgba;

/// sqrt(2): the factor a corner-sized ellipse scales its side offsets by so the
/// shape passes through that corner (see `resolve_radii`).
const SQRT_2: f32 = std::f32::consts::SQRT_2;

/// Gradient box geometry: the box (in device px) a gradient is defined over,
/// plus its origin. Linear gradients are emitted directly in device coordinates
/// (endpoints offset by the box origin), so their direction does not depend on
/// the box aspect ratio; radial gradients keep a normalized shape mapped by an
/// explicit local matrix.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GradientBox {
    /// Box width/height in device px.
    w: f32,
    h: f32,
    /// Box origin in device coordinates.
    ox: f32,
    oy: f32,
}

impl GradientBox {
    /// Box covering the node rect.
    pub(crate) fn from_rect(rect: &Rect) -> Self {
        GradientBox {
            w: rect.width() as f32,
            h: rect.height() as f32,
            ox: rect.x0 as f32,
            oy: rect.y0 as f32,
        }
    }

    /// Box of an explicit `background-size` tile, anchored at the rect origin.
    pub(crate) fn with_size(rect: &Rect, size: [f32; 2]) -> Self {
        GradientBox {
            w: size[0],
            h: size[1],
            ox: rect.x0 as f32,
            oy: rect.y0 as f32,
        }
    }

    /// Absolute device-space point of a box-space coordinate.
    pub(crate) fn to_device(&self, px: f32, py: f32) -> [f32; 2] {
        [self.ox + px, self.oy + py]
    }
}

/// 图层经 `background-position` / `background-size` 定位后的绘制矩形（绝对
/// 坐标）。只有带定位信息的线性层返回 `Some`——此时该层只落在这一块里，不再
/// 铺满节点（`no-repeat` 语义）。
pub(crate) fn layer_tile_rect(fill: &crate::style::BackgroundFill, rect: Rect) -> Option<Rect> {
    let crate::style::BackgroundFill::ArbitraryGradient {
        gradient:
            ArbitraryGradient::LinearGradient {
                size,
                size_pct,
                position,
                position_px,
                ..
            },
    } = fill
    else {
        return None;
    };
    let pos = position.as_ref()?;
    let (bw, bh) = (rect.width() as f32, rect.height() as f32);
    let (w, h) = match (size_pct, size) {
        (Some([sx, sy]), _) => (bw * sx, bh * sy),
        (None, Some([w, h])) => (*w, *h),
        _ => (bw, bh),
    };
    let off = position_px.unwrap_or([0.0, 0.0]);
    let x0 = rect.x0 + f64::from((bw - w) * pos[0] + off[0]);
    let y0 = rect.y0 + f64::from((bh - h) * pos[1] + off[1]);
    Some(Rect::new(x0, y0, x0 + f64::from(w), y0 + f64::from(h)))
}

/// Radial gradient parameters (shape/size/position), independent of the box.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RadialParams {
    /// Center in normalized box coords (`[0,1]`).
    pub(crate) center: [f32; 2],
    pub(crate) shape: RadialShape,
    /// Explicit radii; `None` uses `extent`.
    pub(crate) radii: Option<[GradientLength; 2]>,
    pub(crate) extent: RadialExtent,
}

/// CSS gradient-line endpoints (in box-space px) for an angle or direction.
fn linear_endpoints(
    w: f32,
    h: f32,
    angle_deg: Option<f32>,
    direction: Option<GradientDirection>,
) -> ((f32, f32), (f32, f32)) {
    let cx = w / 2.0;
    let cy = h / 2.0;
    if let Some(angle) = angle_deg {
        let rad = angle.to_radians();
        // CSS: 0deg = up, 90deg = right (screen y grows downward).
        let dx = rad.sin();
        let dy = -rad.cos();
        // CSS gradient line length = |cos θ|·h + |sin θ|·w.
        let len = dx.abs() * w + dy.abs() * h;
        (
            (cx - dx * len / 2.0, cy - dy * len / 2.0),
            (cx + dx * len / 2.0, cy + dy * len / 2.0),
        )
    } else {
        match direction.unwrap_or(GradientDirection::ToRight) {
            GradientDirection::ToRight => ((0.0, cy), (w, cy)),
            GradientDirection::ToLeft => ((w, cy), (0.0, cy)),
            GradientDirection::ToBottom => ((cx, 0.0), (cx, h)),
            GradientDirection::ToTop => ((cx, h), (cx, 0.0)),
            // Corner keywords run exactly corner-to-corner for a rectangle.
            GradientDirection::ToBottomRight => ((0.0, 0.0), (w, h)),
        }
    }
}

/// Absolute device-space positions of stops along the gradient line: px stays
/// as px, fraction is scaled by the full line length `line_len`.
fn stop_device_positions(stops: &[GradientStop], line_len: f32) -> Vec<f32> {
    stops
        .iter()
        .map(|s| match s.unit {
            StopUnit::Fraction => s.pos * line_len,
            StopUnit::Px => s.pos,
        })
        .collect()
}

/// Build a `ShaderSpec::LinearGradient` for `box_`.
///
/// `cycle` is set for `repeating-*` gradients: the shader line is shrunk to one
/// period (the last stop position, in device px) so `TileMode::Repeat` tiles it.
/// `tile_mode` is the tiling used for the shader (Repeat for repeating gradients
/// and for `background-size` tiles, Clamp otherwise).
pub(crate) fn build_linear_gradient(
    angle_deg: Option<f32>,
    direction: Option<GradientDirection>,
    stops: &[GradientStop],
    cycle: bool,
    tile_mode: TileMode,
    box_: GradientBox,
) -> ShaderSpec {
    let (from_box, to_box) = linear_endpoints(box_.w, box_.h, angle_deg, direction);
    let full_len =
        ((to_box.0 - from_box.0).powi(2) + (to_box.1 - from_box.1).powi(2)).sqrt();

    let dev = stop_device_positions(stops, full_len);

    // Resolve the extent the stops are normalized against: for repeating
    // gradients that is the period (last stop position), else the whole line.
    let extent = if cycle {
        let period = dev.iter().cloned().fold(0.0_f32, f32::max);
        if period > 0.0 { period } else { full_len }
    } else {
        full_len
    };
    let positions: Vec<f32> = dev
        .iter()
        .map(|d| if extent > 0.0 { d / extent } else { 0.0 })
        .collect();

    // For repeating gradients shrink the shader line to exactly one period.
    let to_box = if cycle && full_len > 0.0 {
        let k = extent / full_len;
        (
            from_box.0 + (to_box.0 - from_box.0) * k,
            from_box.1 + (to_box.1 - from_box.1) * k,
        )
    } else {
        to_box
    };

    let colors = stops
        .iter()
        .map(|s| color_token_to_rgba(&s.color))
        .collect();

    // Linear gradient endpoints are kept in box (device) space with no local
    // matrix. Normalizing them into the unit box and scaling back with an
    // anisotropic `local_matrix = scale(w, h)` would re-interpret the angle in
    // non-uniform coordinates: a gradient direction `(dx, dy)` in device space
    // becomes `(dx/w², dy/h²)` after the round trip, so the perceived angle
    // skews with the box aspect ratio (a 95deg gradient in a 400×100 box
    // rendered at ~54deg). CSS evaluates the angle in the element's own
    // coordinate system, so the raw box-space endpoints are correct.
    ShaderSpec::LinearGradient {
        from: box_.to_device(from_box.0, from_box.1),
        to: box_.to_device(to_box.0, to_box.1),
        stops: positions,
        colors,
        tile_mode,
        local_matrix: None,
        interpolate_premul: true,
    }
}

/// Distance from `(cx, cy)` to the farthest corner of a `w × h` box.
fn farthest_corner(cx: f32, cy: f32, w: f32, h: f32) -> f32 {
    let d = |x: f32, y: f32| ((cx - x).powi(2) + (cy - y).powi(2)).sqrt();
    d(0.0, 0.0).max(d(w, 0.0)).max(d(0.0, h)).max(d(w, h))
}

/// Distance from `(cx, cy)` to the closest corner of a `w × h` box.
fn closest_corner(cx: f32, cy: f32, w: f32, h: f32) -> f32 {
    let d = |x: f32, y: f32| ((cx - x).powi(2) + (cy - y).powi(2)).sqrt();
    d(0.0, 0.0).min(d(w, 0.0)).min(d(0.0, h)).min(d(w, h))
}

/// CSS reference for `circle` percentage radii: half the box diagonal.
fn circle_percent_reference(w: f32, h: f32) -> f32 {
    (w * w + h * h).sqrt() / 2.0
}

/// Resolve the horizontal/vertical radii (device px) for a radial gradient.
fn resolve_radii(box_: GradientBox, cx: f32, cy: f32, params: &RadialParams) -> (f32, f32) {
    let w = box_.w;
    let h = box_.h;
    if let Some([a, b]) = params.radii {
        return match params.shape {
            RadialShape::Circle => {
                let r_ref = circle_percent_reference(w, h);
                (a.resolve(r_ref), b.resolve(r_ref))
            }
            // For `ellipse` the `%` resolves against width (horizontal radius)
            // and height (vertical radius) respectively.
            RadialShape::Ellipse => (a.resolve(w), b.resolve(h)),
        };
    }

    let dcx = cx.max(w - cx);
    let dcy = cy.max(h - cy);
    let scx = cx.min(w - cx);
    let scy = cy.min(h - cy);
    match params.shape {
        RadialShape::Circle => {
            let r = match params.extent {
                RadialExtent::FarthestCorner => farthest_corner(cx, cy, w, h),
                RadialExtent::ClosestCorner => closest_corner(cx, cy, w, h),
                RadialExtent::FarthestSide => dcx.max(dcy),
                RadialExtent::ClosestSide => scx.min(scy),
            };
            (r, r)
        }
        RadialShape::Ellipse => match params.extent {
            // For a corner-sized ellipse, CSS keeps the ratio of the two radii
            // equal to the ratio of the center-to-corner offsets, then scales
            // the shape so it exactly touches that corner at (dcx, dcy) from
            // the center. Scaling the offsets by sqrt(2) lands the corner on
            // the ellipse: (dcx/(√2·dcx))² + (dcy/(√2·dcy))² = 1. Blink and
            // WebKit resolve `farthest-corner`/`closest-corner` the same way;
            // the `*-side` keywords use the raw side distances.
            RadialExtent::FarthestCorner => (dcx * SQRT_2, dcy * SQRT_2),
            RadialExtent::ClosestCorner => (scx * SQRT_2, scy * SQRT_2),
            RadialExtent::FarthestSide => (dcx, dcy),
            RadialExtent::ClosestSide => (scx, scy),
        },
    }
}

/// Build a `ShaderSpec::RadialGradient` for `box_`. A unit circle in shader-local
/// space is mapped to the (possibly elliptical) gradient shape with `local_matrix`.
pub(crate) fn build_radial_gradient(
    box_: GradientBox,
    params: &RadialParams,
    stops: &[GradientStop],
    repeat: bool,
) -> ShaderSpec {
    let cx = params.center[0] * box_.w;
    let cy = params.center[1] * box_.h;
    let (rx, ry) = resolve_radii(box_, cx, cy, params);
    let ref_len = if rx > 0.0 { rx } else { ry };

    let dev = stop_device_positions(stops, ref_len);
    let tile_mode = if repeat {
        TileMode::Repeat
    } else {
        TileMode::Clamp
    };

    let extent = if repeat {
        let period = dev.iter().cloned().fold(0.0_f32, f32::max);
        if period > 0.0 { period } else { ref_len }
    } else {
        ref_len
    };
    let positions: Vec<f32> = dev
        .iter()
        .map(|d| if extent > 0.0 { d / extent } else { 0.0 })
        .collect();
    let colors = stops
        .iter()
        .map(|s| color_token_to_rgba(&s.color))
        .collect();

    // Map the shader's unit circle (center (0,0), radius 1) onto the ellipse of
    // radii (rx, ry) centered at the box-space center.
    let m = [
        rx,
        0.0,
        box_.ox + cx,
        0.0,
        ry,
        box_.oy + cy,
        0.0,
        0.0,
        1.0,
    ];

    ShaderSpec::RadialGradient {
        center: [0.0, 0.0],
        radius: 1.0,
        stops: positions,
        colors,
        tile_mode,
        local_matrix: Some(m),
        interpolate_premul: true,
        focal: None,
    }
}

/// Build a `ShaderSpec::SweepGradient` (CSS `conic-gradient`) for `box_`.
///
/// CSS 角度语义（`from θ`，0 = 12 点方向、顺时针）在此换算成 Skia sweep
/// 角度（0 = +x 轴；y 轴向下，顺时针为正）：12 点方向 = −90°，故 α = θ − 90。
/// stop 位置是绕心比例（0..1 映射一整圈 360°）；`repeat`（repeating-conic）
/// 把 shader 收缩到最后一个 stop 的周期并配 `TileMode::Repeat`，与 linear
/// 的收缩逻辑同构。
fn build_sweep_gradient(
    angle_deg: Option<f32>,
    center_ratio: [f32; 2],
    stops: &[GradientStop],
    repeat: bool,
    tile_mode: TileMode,
    box_: GradientBox,
) -> ShaderSpec {
    let start = angle_deg.unwrap_or(0.0) - 90.0;
    // 比例直接就是绕心位置；Px 形态的 stop 在 conic 语法里不合法，解析层
    // 只产生 Fraction——这里沿用同一换算兜底（px 数值按比例处理）。
    let dev = stop_device_positions(stops, 1.0);
    let extent = if repeat {
        let period = dev.iter().cloned().fold(0.0_f32, f32::max);
        if period > 0.0 { period } else { 1.0 }
    } else {
        1.0
    };
    let positions: Vec<f32> = dev
        .iter()
        .map(|d| if extent > 0.0 { d / extent } else { 0.0 })
        .collect();
    let colors = stops
        .iter()
        .map(|s| color_token_to_rgba(&s.color))
        .collect();
    ShaderSpec::SweepGradient {
        center: box_.to_device(center_ratio[0] * box_.w, center_ratio[1] * box_.h),
        start_angle_deg: start,
        end_angle_deg: start + 360.0 * extent,
        stops: positions,
        colors,
        tile_mode,
        interpolate_premul: true,
    }
}

/// Convert an arbitrary CSS-syntax gradient into a canvas `ShaderSpec` mapped
/// onto the node `rect` (or its `background-size` tile).
pub(crate) fn arbitrary_gradient_to_shader_spec(gradient: &ArbitraryGradient, rect: Rect) -> ShaderSpec {
    match gradient {
        ArbitraryGradient::LinearGradient {
            angle_deg,
            direction,
            stops,
            size,
            repeat,
            size_pct,
            position,
            ..
        } => {
            // `position` 非空的层已经过 `layer_tile_rect` 定位，传入的 rect
            // 就是折算完尺寸的 tile 本体——再套一次 size 会把渐变线拉长
            // size 倍（窗口永远停在首段）。只有未定位的层（节点盒 + 瓦片
            // 尺寸 Repeat 平铺）才在这里套。
            let size = if position.is_some() {
                None
            } else {
                match (size_pct, size) {
                    (Some([sx, sy]), _) => {
                        Some([rect.width() as f32 * sx, rect.height() as f32 * sy])
                    }
                    (None, Some(s)) => Some(*s),
                    _ => None,
                }
            };
            let box_ = match size {
                Some(s) => GradientBox::with_size(&rect, s),
                None => GradientBox::from_rect(&rect),
            };
            // `repeating-*` gradients tile their (shrunk) one-period line; a
            // `background-size` tile also repeats to fill the node, but keeps the
            // full line so its stops stay laid out over the whole (larger) line.
            let tile_mode = if *repeat || size.is_some() {
                TileMode::Repeat
            } else {
                TileMode::Clamp
            };
            build_linear_gradient(*angle_deg, *direction, stops, *repeat, tile_mode, box_)
        }
        ArbitraryGradient::RadialGradient {
            center,
            stops,
            size,
            repeat,
            shape,
            radii,
            extent,
        } => {
            let box_ = match size {
                Some(s) => GradientBox::with_size(&rect, *s),
                None => GradientBox::from_rect(&rect),
            };
            let params = RadialParams {
                center: *center,
                shape: *shape,
                radii: *radii,
                extent: *extent,
            };
            build_radial_gradient(box_, &params, stops, *repeat)
        }
        ArbitraryGradient::ConicGradient {
            angle_deg,
            center,
            stops,
            size,
            repeat,
        } => {
            let box_ = match size {
                Some(s) => GradientBox::with_size(&rect, *s),
                None => GradientBox::from_rect(&rect),
            };
            let tile_mode = if *repeat || size.is_some() {
                TileMode::Repeat
            } else {
                TileMode::Clamp
            };
            build_sweep_gradient(*angle_deg, center.unwrap_or([0.5, 0.5]), stops, *repeat, tile_mode, box_)
        }
        ArbitraryGradient::Turbulence {
            base_frequency,
            num_octaves,
            seed,
            size,
        } => {
            // 无显式 background-size 时瓦片取节点盒：Repeat 永远不会在盒内
            // 回绕，与“整盒一个未拼接噪声场”严格等价。
            let tile = size.unwrap_or([rect.width() as f32, rect.height() as f32]);
            ShaderSpec::Turbulence {
                base_frequency: [*base_frequency, *base_frequency],
                num_octaves: *num_octaves,
                seed: *seed,
                tile: Some(tile),
                origin: [rect.x0 as f32, rect.y0 as f32],
            }
        }
    }
}

// ── Script conversion ────────────────────────────────────────────────

