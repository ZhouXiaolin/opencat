use crate::canvas::glyph::FontEdging;
use crate::canvas::paint::{FillSpec, StrokeCap, StrokeJoin};
use crate::canvas::PointMode;
use crate::scene::script::mutations::{
    ScriptColor, ScriptFontEdging, ScriptLineCap, ScriptLineJoin, ScriptPointMode,
};

/// Convert `ScriptColor` to `[f32; 4]` with channels in 0.0–1.0.
pub fn script_color_to_rgba(c: ScriptColor) -> [f32; 4] {
    [
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
        c.a as f32 / 255.0,
    ]
}

/// Apply a global alpha multiplier to a `ScriptColor`, returning `[f32; 4]`.
pub fn script_color_with_alpha(c: ScriptColor, global_alpha: f32) -> [f32; 4] {
    let mut rgba = script_color_to_rgba(c);
    rgba[3] *= global_alpha;
    rgba
}

/// Convert `ScriptColor` + global alpha into a `FillSpec::Solid`.
pub fn to_fill_spec(c: ScriptColor, global_alpha: f32) -> FillSpec {
    FillSpec::Solid(script_color_with_alpha(c, global_alpha))
}

/// Convert `ScriptLineCap` to canvas `StrokeCap`.
pub fn script_line_cap(c: ScriptLineCap) -> StrokeCap {
    match c {
        ScriptLineCap::Butt => StrokeCap::Butt,
        ScriptLineCap::Round => StrokeCap::Round,
        ScriptLineCap::Square => StrokeCap::Square,
    }
}

/// Convert `ScriptLineJoin` to canvas `StrokeJoin`.
pub fn script_line_join(j: ScriptLineJoin) -> StrokeJoin {
    match j {
        ScriptLineJoin::Miter => StrokeJoin::Miter,
        ScriptLineJoin::Round => StrokeJoin::Round,
        ScriptLineJoin::Bevel => StrokeJoin::Bevel,
    }
}

/// Convert `ScriptPointMode` to canvas `PointMode`.
pub fn script_point_mode(m: ScriptPointMode) -> PointMode {
    match m {
        ScriptPointMode::Points => PointMode::Points,
        ScriptPointMode::Lines => PointMode::Lines,
        ScriptPointMode::Polygon => PointMode::Polygon,
    }
}

/// Convert `ScriptFontEdging` to canvas `FontEdging`.
pub fn script_font_edging(e: ScriptFontEdging) -> FontEdging {
    match e {
        ScriptFontEdging::Alias => FontEdging::Alias,
        ScriptFontEdging::AntiAlias => FontEdging::AntiAlias,
        ScriptFontEdging::SubpixelAntiAlias => FontEdging::SubpixelAntiAlias,
    }
}
