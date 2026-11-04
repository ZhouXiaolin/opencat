use crate::canvas::paint::{
    BlendMode, BlurStyle, FillSpec, ImageFilterSpec, MaskFilterSpec, PaintSpec, PaintStyle,
};
use crate::display::list::DisplayRect;
use crate::ir::draw_op::DrawOp;
use crate::ir::draw_types::PaintId;
use crate::render::builder::DrawOpBuilder;
use crate::style::{
    BorderRadius, BoxShadow, CssFilter,
    CssFilterKind, DropShadow, InsetShadow,
};

use super::RenderError;
use super::ctx::RenderCtx;


use super::items::{
    CanvasShadow, clip_bounds, color_token_to_rgba, effective_corner_radius, kurbo_rect,
    push_draw_rrect, rect_to_rect4, spread_radius,
};

fn color_matrix_for_filter_op(kind: CssFilterKind, value: f32) -> Option<[f32; 20]> {
    match kind {
        CssFilterKind::Blur | CssFilterKind::DropShadow => None,
        CssFilterKind::Brightness => Some([
            value, 0.0, 0.0, 0.0, 0.0, 0.0, value, 0.0, 0.0, 0.0, 0.0, 0.0, value, 0.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
        ]),
        CssFilterKind::Contrast => {
            let intercept = 0.5 * (1.0 - value);
            Some([
                value, 0.0, 0.0, 0.0, intercept, 0.0, value, 0.0, 0.0, intercept, 0.0, 0.0, value,
                0.0, intercept, 0.0, 0.0, 0.0, 1.0, 0.0,
            ])
        }
        CssFilterKind::Grayscale => {
            let a = value;
            let b = 1.0 - a;
            Some([
                b + a * 0.2126,
                a * 0.7152,
                a * 0.0722,
                0.0,
                0.0,
                a * 0.2126,
                b + a * 0.7152,
                a * 0.0722,
                0.0,
                0.0,
                a * 0.2126,
                a * 0.7152,
                b + a * 0.0722,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
            ])
        }
        CssFilterKind::HueRotate => {
            let radians = value * std::f32::consts::PI / 180.0;
            let cos = radians.cos();
            let sin = radians.sin();
            Some([
                0.2126 + cos * 0.7874 - sin * 0.2126,
                0.7152 - cos * 0.7152 - sin * 0.7152,
                0.0722 - cos * 0.0722 + sin * 0.9278,
                0.0,
                0.0,
                0.2126 - cos * 0.2126 + sin * 0.1437,
                0.7152 + cos * 0.2848 + sin * 0.1400,
                0.0722 - cos * 0.0722 - sin * 0.2837,
                0.0,
                0.0,
                0.2126 - cos * 0.2126 - sin * 0.7874,
                0.7152 - cos * 0.7152 + sin * 0.7152,
                0.0722 + cos * 0.9278 + sin * 0.0722,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
            ])
        }
        CssFilterKind::Invert => {
            let b = 1.0 - 2.0 * value;
            Some([
                b, 0.0, 0.0, 0.0, value, 0.0, b, 0.0, 0.0, value, 0.0, 0.0, b, 0.0, value, 0.0,
                0.0, 0.0, 1.0, 0.0,
            ])
        }
        CssFilterKind::Saturate => {
            let a = (1.0 - value) * 0.2126;
            let b = (1.0 - value) * 0.7152;
            let c = (1.0 - value) * 0.0722;
            Some([
                a + value,
                b,
                c,
                0.0,
                0.0,
                a,
                b + value,
                c,
                0.0,
                0.0,
                a,
                b,
                c + value,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
            ])
        }
        CssFilterKind::Sepia => {
            let a = value;
            let b = 1.0 - a;
            Some([
                b + a * 0.393,
                a * 0.769,
                a * 0.189,
                0.0,
                0.0,
                a * 0.349,
                b + a * 0.686,
                a * 0.168,
                0.0,
                0.0,
                a * 0.272,
                a * 0.534,
                b + a * 0.131,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
            ])
        }
    }
}

pub(crate) fn css_filter_image_filter(filter: &CssFilter) -> Option<ImageFilterSpec> {
    let mut image_filter = None;
    for op in &filter.ops {
        if op.is_identity() {
            continue;
        }
        let next = match op.kind {
            // CSS `filter: blur(R)` is Gaussian sigma = R (MDN: R "defines the
            // value of the standard deviation"; verified against Chrome on
            // pr-to-video-launch s02: a blur(2px) frame's Laplacian variance
            // sits within ~15-30% of a sigma=2 render, while sigma=1 is 16x
            // sharper — the half-radius rule that applies to box-shadow's
            // blur-radius does NOT apply to filter blur()). Skia takes the
            // sigma directly, so pass R through unchanged.
            CssFilterKind::Blur => ImageFilterSpec::Blur {
                sigma_x: op.value,
                sigma_y: op.value,
                crop_rect: None,
                // CSS blur: content outside the element is transparent, so the
                // falloff must not be filled with the edge colour (Clamp).
                decal: true,
            },
            // CSS `filter: drop-shadow(dx dy blur color)` keeps the element's
            // own pixels and adds a shadow of its alpha mask. `keep_content:
            // true` selects Skia's `drop_shadow` (shadow + input) instead of
            // `drop_shadow_only`.
            CssFilterKind::DropShadow => {
                let shadow = op.shadow?;
                let color = color_token_to_rgba(&shadow.color);
                ImageFilterSpec::DropShadow {
                    dx: shadow.offset_x,
                    dy: shadow.offset_y,
                    sigma_x: shadow.blur_sigma,
                    sigma_y: shadow.blur_sigma,
                    color,
                    keep_content: true,
                }
            }
            kind => {
                let matrix = color_matrix_for_filter_op(kind, op.value)?;
                ImageFilterSpec::ColorFilter(Box::new(
                    crate::canvas::paint::ColorFilterSpec::Matrix(matrix),
                ))
            }
        };
        image_filter = Some(match image_filter {
            Some(inner) => ImageFilterSpec::Compose(Box::new(next), Box::new(inner)),
            None => next,
        });
    }
    image_filter
}

#[cfg(feature = "profile")]
use tracing::{Level, event, span};

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
        keep_content: false,
    };
    (filter, color)
}

/// Paint for a `SaveLayer` that exists only to carry an image filter (shadow
/// layers). Nothing is ever drawn with this paint — but its **alpha modulates
/// the whole layer** when the executor composites it back
/// (`replay.rs` `DrawOp::SaveLayer`). It must therefore be fully opaque:
/// a transparent fill (`[0.0; 4]`, alpha 0) silently multiplies the filtered
/// content by zero and the shadow disappears.
pub(crate) fn filter_layer_paint(image_filter: ImageFilterSpec) -> PaintSpec {
    PaintSpec {
        fill: FillSpec::Solid([0.0, 0.0, 0.0, 1.0]),
        style: PaintStyle::Fill,
        stroke: None,
        anti_alias: true,
        blend_mode: BlendMode::SrcOver,
        image_filter: Some(image_filter),
        color_filter: None,
        mask_filter: None,
        path_effect: None,
    }
}

/// Emit `draw` wrapped in the canvas 2D shadow pass.
///
/// Canvas shadows are not a Skia op: `fill()`/`stroke()` under an active
/// shadow first paints the blurred silhouette of the same geometry, offset by
/// (`dx`, `dy`), and then the geometry itself. The standard vocabulary says
/// exactly that: a `SaveLayer` whose paint carries a `DropShadow` image filter
/// with `keep_content: false` composites back **only** the blurred silhouette
/// in the shadow colour (the paint of the inner draw is irrelevant — its
/// coverage is all that survives), and the real draw follows the layer.
/// Interns the `SaveLayer` paint that renders the group's alpha as a blurred
/// shadow. Chromium maps `shadowBlur` to a gaussian sigma of half the radius.
/// The offset is *user* space (canvas shadowOffsetX/Y ride the CTM), so it is
/// a `Translate` around the layer, not the filter's device-space dx/dy — same
/// reason `draw_box_shadow` offsets the rect it draws.
pub(crate) fn shadow_layer_paint_id(b: &mut DrawOpBuilder, sh: CanvasShadow) -> PaintId {
    let sigma = sh.blur * 0.5;
    let paint = filter_layer_paint(ImageFilterSpec::DropShadow {
        dx: 0.0,
        dy: 0.0,
        sigma_x: sigma,
        sigma_y: sigma,
        color: [
            sh.color.r as f32 / 255.0,
            sh.color.g as f32 / 255.0,
            sh.color.b as f32 / 255.0,
            sh.color.a as f32 / 255.0,
        ],
        keep_content: false,
    });
    b.intern_paint(paint)
}

pub(crate) fn push_shadowed(b: &mut DrawOpBuilder, shadow: Option<CanvasShadow>, draw: DrawOp) {
    let Some(sh) = shadow else {
        b.push(draw);
        return;
    };
    let paint_id = shadow_layer_paint_id(b, sh);
    b.push(DrawOp::Save);
    if sh.dx != 0.0 || sh.dy != 0.0 {
        b.push(DrawOp::Translate { x: sh.dx, y: sh.dy });
    }
    b.push(DrawOp::SaveLayer {
        bounds: None,
        paint: Some(paint_id),
        alpha: 1.0,
        backdrop: false,
    });
    b.push(draw.clone());
    b.push(DrawOp::Restore);
    b.push(DrawOp::Restore);
    b.push(draw);
}

pub fn draw_box_shadow(
    builder: &mut DrawOpBuilder,
    bounds: DisplayRect,
    border_radius: &BorderRadius,
    shadow: &BoxShadow,
) {
    let shadow_bounds = if shadow.spread != 0.0 {
        bounds.outset(shadow.spread, shadow.spread, shadow.spread, shadow.spread)
    } else {
        bounds
    };
    let rect = kurbo_rect(shadow_bounds.translate(shadow.offset_x, shadow.offset_y));
    let sr = spread_radius(border_radius, shadow.spread);
    let radii = effective_corner_radius(&rect, &sr);

    let (mask_filter, color) = box_shadow_to_mask_filter(shadow);
    let paint = PaintSpec {
        fill: FillSpec::Solid(color),
        style: PaintStyle::Fill,
        stroke: None,
        anti_alias: true,
        blend_mode: BlendMode::SrcOver,
        image_filter: None,
        color_filter: None,
        mask_filter: Some(mask_filter),
        path_effect: None,
    };
    let paint_id = builder.intern_paint(paint);

    // CSS css-backgrounds-3: an *outer* box-shadow is clipped to outside the
    // border-box ("the shadow is not painted inside the border-box"). The blur
    // fills the whole shadow shape, so without the clip the shadow's interior
    // bleeds through semi-transparent element backgrounds (glass cards) and
    // dims them. Draw the blurred shape into its own layer, erase the
    // border-box with BlendMode::Clear, then composite the layer. For opaque
    // element backgrounds the result is pixel-identical to before; for
    // transparent ones it now matches the browser.
    let blur_extent = (shadow.blur_sigma * 3.0).ceil();
    let layer_bounds = shadow_bounds
        .translate(shadow.offset_x, shadow.offset_y)
        .outset(blur_extent, blur_extent, blur_extent, blur_extent);
    builder.push(DrawOp::SaveLayer {
        bounds: Some(rect_to_rect4(kurbo_rect(layer_bounds))),
        paint: None,
        alpha: 1.0,
        backdrop: false,
    });

    if radii.iter().any(|&r| r > 0.0) {
        push_draw_rrect(builder, rect, radii, paint_id);
    } else {
        builder.push(DrawOp::Rect {
            rect: rect_to_rect4(rect),
            paint: paint_id,
        });
    }

    let clear_paint = builder.intern_paint(PaintSpec {
        fill: FillSpec::Solid([0.0, 0.0, 0.0, 1.0]),
        style: PaintStyle::Fill,
        stroke: None,
        anti_alias: true,
        blend_mode: BlendMode::Clear,
        image_filter: None,
        color_filter: None,
        mask_filter: None,
        path_effect: None,
    });
    let box_rect = kurbo_rect(bounds);
    let box_radii = effective_corner_radius(&box_rect, border_radius);
    if box_radii.iter().any(|&r| r > 0.0) {
        push_draw_rrect(builder, box_rect, box_radii, clear_paint);
    } else {
        builder.push(DrawOp::Rect {
            rect: rect_to_rect4(box_rect),
            paint: clear_paint,
        });
    }
    builder.push(DrawOp::Restore);
}

pub fn draw_inset_shadow(
    builder: &mut DrawOpBuilder,
    bounds: DisplayRect,
    border_radius: &BorderRadius,
    shadow: &InsetShadow,
) {
    let shadow_bounds = if shadow.spread != 0.0 {
        bounds.outset(shadow.spread, shadow.spread, shadow.spread, shadow.spread)
    } else {
        bounds
    };
    let rect = kurbo_rect(shadow_bounds.translate(shadow.offset_x, shadow.offset_y));
    let sr = spread_radius(border_radius, shadow.spread);
    let radii = effective_corner_radius(&rect, &sr);

    let (mask_filter, color) = inset_shadow_to_mask_filter(shadow);
    let paint = PaintSpec {
        fill: FillSpec::Solid(color),
        style: PaintStyle::Fill,
        stroke: None,
        anti_alias: true,
        blend_mode: BlendMode::SrcOver,
        image_filter: None,
        color_filter: None,
        mask_filter: Some(mask_filter),
        path_effect: None,
    };
    let paint_id = builder.intern_paint(paint);

    builder.push(DrawOp::Save);
    clip_bounds(builder, bounds, border_radius);
    if radii.iter().any(|&r| r > 0.0) {
        push_draw_rrect(builder, rect, radii, paint_id);
    } else {
        builder.push(DrawOp::Rect {
            rect: rect_to_rect4(rect),
            paint: paint_id,
        });
    }
    builder.push(DrawOp::Restore);
}

pub fn draw_item_drop_shadow(
    ctx: &mut RenderCtx,
    bounds: DisplayRect,
    shadow: &DropShadow,
    draw: impl FnOnce(&mut RenderCtx) -> Result<(), RenderError>,
) -> Result<(), RenderError> {
    let (left, top, right, bottom) = shadow.outsets();
    let shadow_bounds = kurbo_rect(bounds.outset(left, top, right, bottom));

    let (image_filter, _color) = drop_shadow_to_image_filter(shadow);
    let paint_id = ctx.builder.intern_paint(filter_layer_paint(image_filter));
    ctx.builder.push(DrawOp::SaveLayer {
        bounds: Some(rect_to_rect4(shadow_bounds)),
        paint: Some(paint_id),
        alpha: 1.0,
        backdrop: false,
    });

    let result = draw(ctx);

    ctx.builder.push(DrawOp::Restore);
    result
}

