use crate::canvas::paint::*;
use crate::style::{
    BackgroundFill, BoxShadow, ColorToken, DropShadow, GradientDirection, InsetShadow,
};

/// Convert `ColorToken` to `[f32; 4]` with channels in 0.0–1.0.
pub fn color_token_to_rgba(ct: &ColorToken) -> [f32; 4] {
    let (r, g, b, a) = ct.rgba();
    [
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    ]
}

/// Convert `BackgroundFill` to `PaintSpec` (fill-only, no stroke).
pub fn background_fill_to_paint_spec(fill: &BackgroundFill) -> PaintSpec {
    PaintSpec {
        fill: background_fill_to_fill_spec(fill),
        style: PaintStyle::Fill,
        stroke: None,
        anti_alias: true,
        blend_mode: BlendMode::SrcOver,
        image_filter: None,
        color_filter: None,
        mask_filter: None,
        path_effect: None,
    }
}

/// Convert `BackgroundFill` to `FillSpec`.
pub fn background_fill_to_fill_spec(fill: &BackgroundFill) -> FillSpec {
    match fill {
        BackgroundFill::Solid(color) => FillSpec::Solid(color_token_to_rgba(color)),
        BackgroundFill::LinearGradient {
            direction,
            from,
            via,
            to,
        } => {
            let shader = gradient_to_shader_spec(*direction, from, via.as_ref(), to);
            FillSpec::Shader(shader)
        }
    }
}

/// Build a `MaskFilterSpec::Blur` (Normal style) from a `BoxShadow`, plus the shadow color.
pub fn box_shadow_to_mask_filter(shadow: &BoxShadow) -> (MaskFilterSpec, [f32; 4]) {
    let color = color_token_to_rgba(&shadow.color);
    let filter = MaskFilterSpec::Blur {
        sigma: shadow.blur_sigma,
        style: BlurStyle::Normal,
        respect_ctm: true,
    };
    (filter, color)
}

/// Build a `MaskFilterSpec::Blur` (Inner style) from an `InsetShadow`, plus the shadow color.
pub fn inset_shadow_to_mask_filter(shadow: &InsetShadow) -> (MaskFilterSpec, [f32; 4]) {
    let color = color_token_to_rgba(&shadow.color);
    let filter = MaskFilterSpec::Blur {
        sigma: shadow.blur_sigma,
        style: BlurStyle::Inner,
        respect_ctm: true,
    };
    (filter, color)
}

/// Build an `ImageFilterSpec::DropShadow` from a `DropShadow`, plus the shadow color.
pub fn drop_shadow_to_image_filter(shadow: &DropShadow) -> (ImageFilterSpec, [f32; 4]) {
    let color = color_token_to_rgba(&shadow.color);
    let filter = ImageFilterSpec::DropShadow {
        dx: shadow.offset_x,
        dy: shadow.offset_y,
        sigma_x: shadow.blur_sigma,
        sigma_y: shadow.blur_sigma,
        color,
    };
    (filter, color)
}

/// Convert a linear gradient definition to a `ShaderSpec::LinearGradient`.
///
/// `via` is the optional middle stop. The gradient is always horizontal/vertical
/// based on direction, with `width`/`height` supplied by the caller as the rect extent
/// and the shader origin assumed to be (0, 0).
fn gradient_to_shader_spec(
    direction: GradientDirection,
    from: &ColorToken,
    via: Option<&ColorToken>,
    to: &ColorToken,
) -> ShaderSpec {
    let (from_pt, to_pt) = direction_endpoints(&direction);
    let from_color = color_token_to_rgba(from);
    let to_color = color_token_to_rgba(to);

    let (stops, colors) = match via {
        Some(mid) => {
            let mid_color = color_token_to_rgba(mid);
            (vec![0.0, 0.5, 1.0], vec![from_color, mid_color, to_color])
        }
        None => (vec![0.0, 1.0], vec![from_color, to_color]),
    };

    ShaderSpec::LinearGradient {
        from: from_pt,
        to: to_pt,
        stops,
        colors,
        tile_mode: TileMode::Clamp,
    }
}

/// Return (from, to) unit-square endpoints for a `GradientDirection`.
fn direction_endpoints(dir: &GradientDirection) -> ([f32; 2], [f32; 2]) {
    match dir {
        GradientDirection::ToRight => ([0.0, 0.0], [1.0, 0.0]),
        GradientDirection::ToLeft => ([1.0, 0.0], [0.0, 0.0]),
        GradientDirection::ToBottom => ([0.0, 0.0], [0.0, 1.0]),
        GradientDirection::ToTop => ([0.0, 1.0], [0.0, 0.0]),
        GradientDirection::ToBottomRight => ([0.0, 0.0], [1.0, 1.0]),
    }
}
