use cosmic_text::Command;

use crate::canvas::paint::{BlendMode, FillSpec, ImageFilterSpec, PaintSpec, PaintStyle};
use crate::display::list::{DisplayRect, TextDisplayItem};
use crate::ir::GeneratedImageId;
use crate::ir::draw_op::{DrawOp, Rect4};
use crate::ir::draw_types::{EncodedPath, FillType, ImageRef, PathOp};
use crate::script::TextUnitOverride;
use crate::style::TextAlign;
use crate::text::{
    GlyphData, apply_text_transform, compute_line_x_shift, describe_text_unit_ranges,
    ranges_overlap, rasterize_glyphs,
};

use super::helpers::drop_shadow_to_image_filter;
use super::{RenderCtx, RenderError};

fn display_rect_to_rect4(r: DisplayRect) -> Rect4 {
    Rect4 {
        x: r.x,
        y: r.y,
        width: r.width,
        height: r.height,
    }
}

fn build_glyph_path(commands: &[Command], scale: f32) -> EncodedPath {
    let ops = commands
        .iter()
        .map(|cmd| match cmd {
            Command::MoveTo(p) => PathOp::MoveTo {
                x: p.x * scale,
                y: -p.y * scale,
            },
            Command::LineTo(p) => PathOp::LineTo {
                x: p.x * scale,
                y: -p.y * scale,
            },
            Command::QuadTo(c, p) => PathOp::QuadTo {
                cx: c.x * scale,
                cy: -c.y * scale,
                x: p.x * scale,
                y: -p.y * scale,
            },
            Command::CurveTo(c1, c2, p) => PathOp::CubicTo {
                c1x: c1.x * scale,
                c1y: -c1.y * scale,
                c2x: c2.x * scale,
                c2y: -c2.y * scale,
                x: p.x * scale,
                y: -p.y * scale,
            },
            Command::Close => PathOp::Close,
        })
        .collect();
    EncodedPath {
        fill_type: FillType::Winding,
        ops,
    }
}

pub fn render_text(ctx: &mut RenderCtx, item: &TextDisplayItem) -> Result<(), RenderError> {
    let raster = rasterize_glyphs(
        &item.text,
        &item.style,
        item.bounds.width,
        item.allow_wrap,
        item.truncate,
        ctx.font_db,
    );

    let rgba = super::helpers::color_token_to_rgba(&item.style.color);
    let paint = PaintSpec {
        fill: FillSpec::Solid(rgba),
        style: PaintStyle::Fill,
        stroke: None,
        anti_alias: true,
        blend_mode: BlendMode::SrcOver,
        image_filter: None,
        color_filter: None,
        mask_filter: None,
        path_effect: None,
    };

    let builder = &mut ctx.builder;
    let paint_id = builder.intern_paint(paint);

    for line in &raster.lines {
        let line_x_shift =
            compute_line_x_shift(line.width, item.bounds.width, item.style.text_align);
        for pos in &line.positions {
            let glyph_data = match raster.glyphs.get(&pos.cache_key) {
                Some(data) => data,
                None => continue,
            };
            let abs_x = item.bounds.x + line_x_shift + pos.x;
            let abs_y = item.bounds.y + pos.y;

            match glyph_data {
                GlyphData::Outline(commands, upem) => {
                    let draw_scale = item.style.text_px / *upem;
                    let unscale = *upem / item.style.text_px;
                    let encoded = build_glyph_path(commands, unscale);
                    let path_id = builder.intern_path(encoded);
                    builder.push(DrawOp::Save);
                    builder.push(DrawOp::Translate { x: abs_x, y: abs_y });
                    if (draw_scale - 1.0).abs() > f32::EPSILON {
                        builder.push(DrawOp::Scale {
                            x: draw_scale,
                            y: draw_scale,
                        });
                    }
                    builder.push(DrawOp::DrawPath {
                        path: path_id,
                        paint: paint_id,
                    });
                    builder.push(DrawOp::Restore);
                }
                GlyphData::ColorImage {
                    rgba,
                    width,
                    height,
                    placement_left,
                    placement_top,
                } => {
                    let id = GeneratedImageId(pos.cache_key);
                    ctx.generated_images
                        .insert(id, *width, *height, std::sync::Arc::from(rgba.as_slice()))
                        .map_err(|e| RenderError::Platform(e.to_string()))?;
                    let ix = abs_x + *placement_left as f32;
                    let iy = abs_y - *placement_top as f32;
                    builder.push(DrawOp::Image {
                        image: ImageRef::Generated { id },
                        x: ix,
                        y: iy,
                        paint: Some(paint_id),
                    });
                }
            }
        }
    }
    Ok(())
}

fn render_text_with_unit_overrides(
    ctx: &mut RenderCtx,
    item: &TextDisplayItem,
) -> Result<(), RenderError> {
    let batch = item.text_unit_overrides.as_ref().unwrap();
    let rendered = apply_text_transform(&item.text, item.style.text_transform);
    let raster = rasterize_glyphs(
        &item.text,
        &item.style,
        item.bounds.width,
        item.allow_wrap,
        false,
        ctx.font_db,
    );

    let base_rgba = super::helpers::color_token_to_rgba(&item.style.color);
    let text_align = if item.allow_wrap {
        item.style.text_align
    } else {
        TextAlign::Left
    };
    let units = describe_text_unit_ranges(&rendered, batch.granularity);

    for (index, unit_range) in units.into_iter().enumerate() {
        let ov = batch
            .overrides
            .get(index)
            .cloned()
            .unwrap_or_else(TextUnitOverride::default);
        let opacity = ov.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
        if opacity <= 0.0 {
            continue;
        }

        let unit_rgba = ov
            .color
            .as_ref()
            .map(super::helpers::color_token_to_rgba)
            .unwrap_or(base_rgba);
        let unit_paint = PaintSpec {
            fill: FillSpec::Solid(unit_rgba),
            style: PaintStyle::Fill,
            stroke: None,
            anti_alias: true,
            blend_mode: BlendMode::SrcOver,
            image_filter: None,
            color_filter: None,
            mask_filter: None,
            path_effect: None,
        };

        let mut min_x: f32 = f32::INFINITY;
        let mut min_y: f32 = f32::INFINITY;
        let mut max_x: f32 = f32::NEG_INFINITY;
        let mut max_y: f32 = f32::NEG_INFINITY;

        struct GlyphEntry {
            encoded: Option<EncodedPath>,
            cache_key: u64,
            abs_x: f32,
            abs_y: f32,
            scale: f32,
            ix: f32,
            iy: f32,
        }

        let mut entries: Vec<GlyphEntry> = Vec::new();

        for line in &raster.lines {
            let line_x_shift = compute_line_x_shift(line.width, item.bounds.width, text_align);
            for pos in &line.positions {
                if !ranges_overlap(pos.byte_range.clone(), unit_range.clone()) {
                    continue;
                }
                let glyph_data = match raster.glyphs.get(&pos.cache_key) {
                    Some(d) => d,
                    None => continue,
                };
                let abs_x = item.bounds.x + line_x_shift + pos.x;
                let abs_y = item.bounds.y + pos.y;

                match glyph_data {
                    GlyphData::Outline(commands, upem) => {
                        let draw_scale = item.style.text_px / *upem;
                        let unscale = *upem / item.style.text_px;
                        let encoded = build_glyph_path(commands, unscale);
                        let (bx, by, bw, bh) = outline_bounds(commands, unscale);
                        let gx = abs_x + bx * draw_scale;
                        let gy = abs_y + by * draw_scale;
                        if bw > 0.0 && bh > 0.0 {
                            min_x = min_x.min(gx);
                            min_y = min_y.min(gy);
                            max_x = max_x.max(gx + bw * draw_scale);
                            max_y = max_y.max(gy + bh * draw_scale);
                        }
                        entries.push(GlyphEntry {
                            encoded: Some(encoded),
                            cache_key: pos.cache_key,
                            abs_x,
                            abs_y,
                            scale: draw_scale,
                            ix: 0.0,
                            iy: 0.0,
                        });
                    }
                    GlyphData::ColorImage {
                        rgba,
                        width: im_w,
                        height: im_h,
                        placement_left,
                        placement_top,
                    } => {
                        let id = GeneratedImageId(pos.cache_key);
                        ctx.generated_images
                            .insert(id, *im_w, *im_h, std::sync::Arc::from(rgba.as_slice()))
                            .map_err(|e| RenderError::Platform(e.to_string()))?;
                        let ix = abs_x + *placement_left as f32;
                        let iy = abs_y - *placement_top as f32;
                        let iw = *im_w as f32;
                        let ih = *im_h as f32;
                        min_x = min_x.min(ix);
                        min_y = min_y.min(iy);
                        max_x = max_x.max(ix + iw);
                        max_y = max_y.max(iy + ih);

                        entries.push(GlyphEntry {
                            encoded: None,
                            cache_key: pos.cache_key,
                            abs_x,
                            abs_y,
                            scale: 1.0,
                            ix,
                            iy,
                        });
                    }
                }
            }
        }

        if entries.is_empty() || min_x.is_infinite() || min_y.is_infinite() {
            continue;
        }

        let translate_x = ov.translate_x.unwrap_or(0.0);
        let translate_y = ov.translate_y.unwrap_or(0.0);
        let scale = ov.scale.unwrap_or(1.0);
        let rotation_deg = ov.rotation_deg.unwrap_or(0.0);
        let pivot_x = (min_x + max_x) * 0.5;
        let pivot_y = (min_y + max_y) * 0.5;

        let builder = &mut ctx.builder;
        let unit_paint_id = builder.intern_paint(unit_paint);

        builder.push(DrawOp::Save);
        builder.push(DrawOp::Translate {
            x: pivot_x + translate_x,
            y: pivot_y + translate_y,
        });
        if rotation_deg != 0.0 {
            builder.push(DrawOp::Rotate {
                degrees: rotation_deg,
                cx: 0.0,
                cy: 0.0,
            });
        }
        if (scale - 1.0).abs() > f32::EPSILON {
            builder.push(DrawOp::Scale { x: scale, y: scale });
        }
        builder.push(DrawOp::Translate {
            x: -pivot_x,
            y: -pivot_y,
        });

        if opacity < 1.0 {
            let alpha_paint = PaintSpec {
                fill: FillSpec::Solid([1.0, 1.0, 1.0, opacity]),
                style: PaintStyle::Fill,
                stroke: None,
                anti_alias: true,
                blend_mode: BlendMode::SrcOver,
                image_filter: None,
                color_filter: None,
                mask_filter: None,
                path_effect: None,
            };
            let alpha_id = builder.intern_paint(alpha_paint);
            let bw = (max_x - min_x).ceil().max(1.0);
            let bh = (max_y - min_y).ceil().max(1.0);
            builder.push(DrawOp::SaveLayer {
                bounds: Some(Rect4 {
                    x: min_x,
                    y: min_y,
                    width: bw,
                    height: bh,
                }),
                paint: Some(alpha_id),
                alpha: 1.0,
            });
        }

        for entry in &entries {
            if let Some(ref encoded) = entry.encoded {
                let path_id = builder.intern_path(encoded.clone());
                builder.push(DrawOp::Save);
                builder.push(DrawOp::Translate {
                    x: entry.abs_x,
                    y: entry.abs_y,
                });
                if (entry.scale - 1.0).abs() > f32::EPSILON {
                    builder.push(DrawOp::Scale {
                        x: entry.scale,
                        y: entry.scale,
                    });
                }
                builder.push(DrawOp::DrawPath {
                    path: path_id,
                    paint: unit_paint_id,
                });
                builder.push(DrawOp::Restore);
            } else {
                let id = GeneratedImageId(entry.cache_key);
                builder.push(DrawOp::Image {
                    image: ImageRef::Generated { id },
                    x: entry.ix,
                    y: entry.iy,
                    paint: Some(unit_paint_id),
                });
            }
        }

        if opacity < 1.0 {
            builder.push(DrawOp::Restore);
        }
        builder.push(DrawOp::Restore);
    }

    Ok(())
}

fn outline_bounds(commands: &[Command], scale: f32) -> (f32, f32, f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    let mut track = |x: f32, y: f32| {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    };
    for cmd in commands {
        match cmd {
            Command::MoveTo(p) | Command::LineTo(p) => track(p.x * scale, -p.y * scale),
            Command::QuadTo(c, p) => {
                track(c.x * scale, -c.y * scale);
                track(p.x * scale, -p.y * scale);
            }
            Command::CurveTo(c1, c2, p) => {
                track(c1.x * scale, -c1.y * scale);
                track(c2.x * scale, -c2.y * scale);
                track(p.x * scale, -p.y * scale);
            }
            Command::Close => {}
        }
    }
    if min_x.is_infinite() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    (min_x, min_y, max_x - min_x, max_y - min_y)
}

pub fn render_text_with_shadows(
    ctx: &mut RenderCtx,
    item: &TextDisplayItem,
) -> Result<(), RenderError> {
    for shadow in &item.drop_shadow {
        let (left, top, right, bottom) = shadow.outsets();
        let shadow_bounds = item.bounds.outset(left, top, right, bottom);
        let (image_filter, _color) = drop_shadow_to_image_filter(shadow);
        let paint_id = ctx
            .builder
            .intern_paint(super::helpers::filter_layer_paint(image_filter));
        ctx.builder.push(DrawOp::SaveLayer {
            bounds: Some(display_rect_to_rect4(shadow_bounds)),
            paint: Some(paint_id),
            alpha: 1.0,
        });
        render_text_body(ctx, item)?;
        ctx.builder.push(DrawOp::Restore);
    }

    // CSS text-shadow: zero-blur shadows are hard offset copies. Rendering those
    // through a drop-shadow image filter can disappear on some backends when
    // sigma is exactly 0, which loses the glitch RGB edge.
    for shadow in &item.text_shadows {
        if shadow.blur_sigma.abs() <= f32::EPSILON {
            let mut shadow_item = item.clone();
            shadow_item.bounds = item.bounds.translate(shadow.offset_x, shadow.offset_y);
            shadow_item.style.color = shadow.color;
            shadow_item.drop_shadow.clear();
            shadow_item.text_shadows.clear();
            if let Some(batch) = &mut shadow_item.text_unit_overrides {
                for unit in &mut batch.overrides {
                    unit.color = Some(shadow.color);
                }
            }
            render_text_body(ctx, &shadow_item)?;
        }
    }

    // Blurred text-shadows compound with CSS `filter` list semantics: each later
    // shadow takes the previous shadow's output (shadow + content) as its input.
    // Nested `SaveLayer`s express the chain — the inner `keep_content` shadow
    // layer's output becomes the input of the next outer shadow filter. The
    // innermost body draw is retained by every level, so no extra final body
    // draw is needed.
    let blurred: Vec<&crate::style::TextShadow> = item
        .text_shadows
        .iter()
        .filter(|s| s.blur_sigma > f32::EPSILON)
        .collect();
    if !blurred.is_empty() {
        // Per-level SaveLayer bounds must contain every deeper shadow's spread
        // too (each level's output includes the deeper shadows), so use the
        // suffix-max of the outsets.
        let suffix = suffix_max_outsets(&blurred);
        render_blurred_shadow_chain(ctx, item, &blurred, &suffix, 0)?;
    } else {
        render_text_body(ctx, item)?;
    }
    Ok(())
}

/// Per-level SaveLayer bounds outsets for a chained (nested) shadow stack: the
/// outsets at index `i` are the component-wise max over `shadows[i..]`, because
/// level `i`'s layer output contains every deeper shadow as well.
fn suffix_max_outsets(
    shadows: &[&crate::style::TextShadow],
) -> Vec<(f32, f32, f32, f32)> {
    let mut suffix: Vec<(f32, f32, f32, f32)> = Vec::with_capacity(shadows.len());
    let mut acc: Option<(f32, f32, f32, f32)> = None;
    for shadow in shadows.iter().rev() {
        let (l, t, r, b) = shadow.outsets();
        acc = Some(match acc {
            Some((al, at, ar, ab)) => (al.max(l), at.max(t), ar.max(r), ab.max(b)),
            None => (l, t, r, b),
        });
        suffix.push(acc.unwrap());
    }
    suffix.reverse();
    suffix
}

/// Recursively emit nested `SaveLayer`s for the blurred text-shadow chain.
/// `level` indexes into `blurred`/`suffix_max`; the innermost level draws the
/// text body itself.
fn render_blurred_shadow_chain(
    ctx: &mut RenderCtx,
    item: &TextDisplayItem,
    blurred: &[&crate::style::TextShadow],
    suffix_max: &[(f32, f32, f32, f32)],
    level: usize,
) -> Result<(), RenderError> {
    let shadow = blurred[level];
    let (left, top, right, bottom) = suffix_max[level];
    let shadow_bounds = item.bounds.outset(left, top, right, bottom);
    let image_filter = ImageFilterSpec::DropShadow {
        dx: shadow.offset_x,
        dy: shadow.offset_y,
        sigma_x: shadow.blur_sigma,
        sigma_y: shadow.blur_sigma,
        color: super::helpers::color_token_to_rgba(&shadow.color),
        keep_content: true,
    };
    let paint_id = ctx
        .builder
        .intern_paint(super::helpers::filter_layer_paint(image_filter));
    ctx.builder.push(DrawOp::SaveLayer {
        bounds: Some(display_rect_to_rect4(shadow_bounds)),
        paint: Some(paint_id),
        alpha: 1.0,
    });
    let result = if level + 1 < blurred.len() {
        render_blurred_shadow_chain(ctx, item, blurred, suffix_max, level + 1)
    } else {
        render_text_body(ctx, item)
    };
    ctx.builder.push(DrawOp::Restore);
    result
}

fn render_text_body(ctx: &mut RenderCtx, item: &TextDisplayItem) -> Result<(), RenderError> {
    if item.text_unit_overrides.is_some() {
        render_text_with_unit_overrides(ctx, item)
    } else {
        render_text(ctx, item)
    }
}

#[cfg(test)]
mod chained_shadow_tests {
    use super::suffix_max_outsets;
    use crate::style::{ColorToken, TextShadow};

    fn shadow(dx: f32, dy: f32, sigma: f32) -> TextShadow {
        TextShadow {
            offset_x: dx,
            offset_y: dy,
            blur_sigma: sigma,
            color: ColorToken::Custom(0, 0, 0, 255),
        }
    }

    /// Chained (nested) shadow layers need per-level bounds that also contain
    /// every deeper shadow's spread: level i's output includes shadows i+1..,
    /// so its outsets are the suffix max, not just its own.
    #[test]
    fn suffix_max_outsets_takes_component_wise_suffix_max() {
        let shadows = vec![shadow(100.0, 0.0, 6.0), shadow(0.0, 60.0, 3.0)];
        let refs: Vec<&TextShadow> = shadows.iter().collect();
        let suffix = suffix_max_outsets(&refs);
        // shadow_outsets: (max(3s−dx,0), max(3s−dy,0), 3s+dx, 3s+dy).
        // shadow(100,0,6) → (0,18,118,18); shadow(0,60,3) → (9,0,9,69).
        assert_eq!(suffix.len(), 2);
        assert_eq!(suffix[0], (9.0, 18.0, 118.0, 69.0));
        assert_eq!(suffix[1], (9.0, 0.0, 9.0, 69.0));
    }

    #[test]
    fn suffix_max_outsets_single_shadow_is_identity() {
        let shadows = vec![shadow(5.0, 7.0, 2.0)];
        let refs: Vec<&TextShadow> = shadows.iter().collect();
        let suffix = suffix_max_outsets(&refs);
        assert_eq!(suffix.len(), 1);
        assert_eq!(suffix[0], (1.0, 0.0, 11.0, 13.0));
    }
}
