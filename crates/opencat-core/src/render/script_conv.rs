use crate::canvas::glyph::FontEdging;
use crate::canvas::paint::{FillSpec, StrokeCap, StrokeJoin};
use crate::ir::draw_op::{ColorU8, LineCap, LineJoin, PointMode as DrawPointMode};

/// Convert `ColorU8` to `[f32; 4]` with channels in 0.0–1.0.
pub fn script_color_to_rgba(c: ColorU8) -> [f32; 4] {
    [
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
        c.a as f32 / 255.0,
    ]
}

/// Apply a global alpha multiplier to a `ColorU8`, returning `[f32; 4]`.
pub fn script_color_with_alpha(c: ColorU8, global_alpha: f32) -> [f32; 4] {
    let mut rgba = script_color_to_rgba(c);
    rgba[3] *= global_alpha;
    rgba
}

/// Convert `ColorU8` + global alpha into a `FillSpec::Solid`.
pub fn to_fill_spec(c: ColorU8, global_alpha: f32) -> FillSpec {
    FillSpec::Solid(script_color_with_alpha(c, global_alpha))
}

/// Convert `LineCap` to canvas `StrokeCap`.
pub fn script_line_cap(c: LineCap) -> StrokeCap {
    match c {
        LineCap::Butt => StrokeCap::Butt,
        LineCap::Round => StrokeCap::Round,
        LineCap::Square => StrokeCap::Square,
    }
}

/// Convert `LineJoin` to canvas `StrokeJoin`.
pub fn script_line_join(j: LineJoin) -> StrokeJoin {
    match j {
        LineJoin::Miter => StrokeJoin::Miter,
        LineJoin::Round => StrokeJoin::Round,
        LineJoin::Bevel => StrokeJoin::Bevel,
    }
}

/// Convert draw `PointMode` to canvas `PointMode`.
pub fn script_point_mode(m: DrawPointMode) -> crate::canvas::PointMode {
    match m {
        DrawPointMode::Points => crate::canvas::PointMode::Points,
        DrawPointMode::Lines => crate::canvas::PointMode::Lines,
        DrawPointMode::Polygon => crate::canvas::PointMode::Polygon,
    }
}

/// Parse font edging from string.
pub fn script_font_edging(name: &str) -> FontEdging {
    match name {
        "alias" => FontEdging::Alias,
        "antiAlias" => FontEdging::AntiAlias,
        "subpixelAntiAlias" => FontEdging::SubpixelAntiAlias,
        _ => FontEdging::AntiAlias,
    }
}
