use opencat_core::canvas::paint::{
    BlendMode, BlurStyle, ColorFilterSpec, FillSpec, ImageFilterSpec, MaskFilterSpec, PaintSpec,
    PaintStyle, PathEffectSpec, ShaderSpec, StrokeCap, StrokeJoin, TileMode,
};
use skia_safe::{
    color_filters, gradient_shader, image_filters, BlendMode as SkBlendMode,
    BlurStyle as SkBlurStyle, Color, ColorFilter, ImageFilter, MaskFilter, Paint,
    PaintStyle as SkPaintStyle, PathEffect, Shader, TileMode as SkTileMode,
};

pub fn paint_from_spec(spec: &PaintSpec) -> Paint {
    let mut paint = Paint::default();
    paint.set_anti_alias(spec.anti_alias);
    paint.set_blend_mode(convert_blend_mode(spec.blend_mode));

    match spec.style {
        PaintStyle::Fill => {
            paint.set_style(SkPaintStyle::Fill);
            apply_fill(&mut paint, &spec.fill);
        }
        PaintStyle::Stroke => {
            paint.set_style(SkPaintStyle::Stroke);
            if let Some(ref stroke) = spec.stroke {
                paint.set_stroke_width(stroke.width);
                paint.set_stroke_cap(convert_stroke_cap(stroke.cap));
                paint.set_stroke_join(convert_stroke_join(stroke.join));
                paint.set_stroke_miter(stroke.miter_limit);
            }
            apply_fill(&mut paint, &spec.fill);
        }
    }

    if let Some(ref filter) = spec.image_filter {
        if let Some(imf) = build_skia_image_filter(filter) {
            paint.set_image_filter(Some(imf));
        }
    }
    if let Some(ref filter) = spec.color_filter {
        if let Some(cf) = build_color_filter(filter) {
            paint.set_color_filter(Some(cf));
        }
    }
    if let Some(ref mask) = spec.mask_filter {
        if let Some(mf) = build_mask_filter(mask) {
            paint.set_mask_filter(Some(mf));
        }
    }
    if let Some(ref effect) = spec.path_effect {
        if let Some(pe) = build_path_effect(effect) {
            paint.set_path_effect(Some(pe));
        }
    }

    paint
}

fn apply_fill(paint: &mut Paint, fill: &FillSpec) {
    match fill {
        FillSpec::Solid(color) => {
            paint.set_shader(None);
            paint.set_color(float4_to_color(*color));
        }
        FillSpec::Shader(shader_spec) => {
            if let Some(shader) = build_skia_shader(shader_spec) {
                paint.set_shader(shader);
            }
        }
    }
}

fn float4_to_color(c: [f32; 4]) -> Color {
    Color::from_argb(
        (c[3].clamp(0.0, 1.0) * 255.0) as u8,
        (c[0].clamp(0.0, 1.0) * 255.0) as u8,
        (c[1].clamp(0.0, 1.0) * 255.0) as u8,
        (c[2].clamp(0.0, 1.0) * 255.0) as u8,
    )
}

// ── Blend mode ────────────────────────────────────────────────────────

fn convert_blend_mode(m: BlendMode) -> SkBlendMode {
    match m {
        BlendMode::Clear => SkBlendMode::Clear,
        BlendMode::Src => SkBlendMode::Src,
        BlendMode::Dst => SkBlendMode::Dst,
        BlendMode::SrcOver => SkBlendMode::SrcOver,
        BlendMode::DstOver => SkBlendMode::DstOver,
        BlendMode::SrcIn => SkBlendMode::SrcIn,
        BlendMode::DstIn => SkBlendMode::DstIn,
        BlendMode::SrcOut => SkBlendMode::SrcOut,
        BlendMode::DstOut => SkBlendMode::DstOut,
        BlendMode::SrcATop => SkBlendMode::SrcATop,
        BlendMode::DstATop => SkBlendMode::DstATop,
        BlendMode::Xor => SkBlendMode::Xor,
        BlendMode::Plus => SkBlendMode::Plus,
        BlendMode::Modulate => SkBlendMode::Modulate,
        BlendMode::Screen => SkBlendMode::Screen,
        BlendMode::Overlay => SkBlendMode::Overlay,
        BlendMode::Darken => SkBlendMode::Darken,
        BlendMode::Lighten => SkBlendMode::Lighten,
        BlendMode::ColorDodge => SkBlendMode::ColorDodge,
        BlendMode::ColorBurn => SkBlendMode::ColorBurn,
        BlendMode::HardLight => SkBlendMode::HardLight,
        BlendMode::SoftLight => SkBlendMode::SoftLight,
        BlendMode::Difference => SkBlendMode::Difference,
        BlendMode::Exclusion => SkBlendMode::Exclusion,
        BlendMode::Multiply => SkBlendMode::Multiply,
        BlendMode::Hue => SkBlendMode::Hue,
        BlendMode::Saturation => SkBlendMode::Saturation,
        BlendMode::Color => SkBlendMode::Color,
        BlendMode::Luminosity => SkBlendMode::Luminosity,
    }
}

// ── Stroke helpers ────────────────────────────────────────────────────

fn convert_stroke_cap(c: StrokeCap) -> skia_safe::paint::Cap {
    match c {
        StrokeCap::Butt => skia_safe::paint::Cap::Butt,
        StrokeCap::Round => skia_safe::paint::Cap::Round,
        StrokeCap::Square => skia_safe::paint::Cap::Square,
    }
}

fn convert_stroke_join(j: StrokeJoin) -> skia_safe::paint::Join {
    match j {
        StrokeJoin::Miter => skia_safe::paint::Join::Miter,
        StrokeJoin::Round => skia_safe::paint::Join::Round,
        StrokeJoin::Bevel => skia_safe::paint::Join::Bevel,
    }
}

// ── Tile mode ─────────────────────────────────────────────────────────

fn convert_tile_mode(t: TileMode) -> SkTileMode {
    match t {
        TileMode::Clamp => SkTileMode::Clamp,
        TileMode::Repeat => SkTileMode::Repeat,
        TileMode::Mirror => SkTileMode::Mirror,
        TileMode::Decal => SkTileMode::Decal,
    }
}

// ── Blur style ────────────────────────────────────────────────────────

fn convert_blur_style(s: BlurStyle) -> SkBlurStyle {
    match s {
        BlurStyle::Normal => SkBlurStyle::Normal,
        BlurStyle::Inner => SkBlurStyle::Inner,
        BlurStyle::Solid => SkBlurStyle::Solid,
        BlurStyle::Outer => SkBlurStyle::Outer,
    }
}

// ── Shader ────────────────────────────────────────────────────────────

fn build_skia_shader(spec: &ShaderSpec) -> Option<Shader> {
    match spec {
        ShaderSpec::LinearGradient {
            from,
            to,
            stops,
            colors,
            tile_mode,
        } => {
            let skia_colors: Vec<Color> =
                colors.iter().map(|c| float4_to_color(*c)).collect();
            gradient_shader::linear(
                ((from[0], from[1]), (to[0], to[1])),
                skia_colors.as_slice(),
                Some(stops.as_slice()),
                convert_tile_mode(*tile_mode),
                None,
                None,
            )
        }
        ShaderSpec::RadialGradient {
            center,
            radius,
            stops,
            colors,
            tile_mode,
        } => {
            let skia_colors: Vec<Color> =
                colors.iter().map(|c| float4_to_color(*c)).collect();
            gradient_shader::radial(
                (center[0], center[1]),
                *radius,
                skia_colors.as_slice(),
                Some(stops.as_slice()),
                convert_tile_mode(*tile_mode),
                None,
                None,
            )
        }
    }
}

// ── Image filter ──────────────────────────────────────────────────────

fn build_skia_image_filter(spec: &ImageFilterSpec) -> Option<ImageFilter> {
    match spec {
        ImageFilterSpec::Blur {
            sigma_x,
            sigma_y,
            crop_rect,
        } => {
            let crop = crop_rect.as_ref().map(|r| skia_safe::Rect::from_xywh(
                r.x0 as f32, r.y0 as f32, r.width() as f32, r.height() as f32,
            ));
            image_filters::blur(
                (*sigma_x, *sigma_y),
                SkTileMode::Clamp,
                None,
                crop.map(image_filters::CropRect::from),
            )
        }
        ImageFilterSpec::DropShadow {
            dx,
            dy,
            sigma_x,
            sigma_y,
            color,
        } => {
            let c = float4_to_color(*color);
            image_filters::drop_shadow_only(
                (*dx, *dy),
                (*sigma_x, *sigma_y),
                c,
                None::<skia_safe::ColorSpace>,
                None::<ImageFilter>,
                None::<image_filters::CropRect>,
            )
        }
        ImageFilterSpec::ColorFilter(cf) => {
            if let Some(skia_cf) = build_color_filter(cf) {
                image_filters::color_filter(
                    skia_cf,
                    None::<ImageFilter>,
                    None::<image_filters::CropRect>,
                )
            } else {
                None
            }
        }
        ImageFilterSpec::Compose(outer, inner) => {
            let out = build_skia_image_filter(outer)?;
            let inn = build_skia_image_filter(inner)?;
            image_filters::compose(out, inn)
        }
    }
}

// ── Color filter ──────────────────────────────────────────────────────

fn build_color_filter(spec: &ColorFilterSpec) -> Option<ColorFilter> {
    match spec {
        ColorFilterSpec::Matrix(matrix) => {
            Some(color_filters::matrix_row_major(matrix, None))
        }
        ColorFilterSpec::BlendColor { color, mode } => {
            color_filters::blend(float4_to_color(*color), convert_blend_mode(*mode))
        }
        ColorFilterSpec::LinearToSrgbGamma => {
            Some(color_filters::linear_to_srgb_gamma())
        }
        ColorFilterSpec::SrgbToLinearGamma => {
            Some(color_filters::srgb_to_linear_gamma())
        }
    }
}

// ── Mask filter ───────────────────────────────────────────────────────

fn build_mask_filter(spec: &MaskFilterSpec) -> Option<MaskFilter> {
    match spec {
        MaskFilterSpec::Blur {
            sigma,
            style,
            respect_ctm,
        } => MaskFilter::blur(convert_blur_style(*style), *sigma, *respect_ctm),
    }
}

// ── Path effect ───────────────────────────────────────────────────────

fn build_path_effect(spec: &PathEffectSpec) -> Option<PathEffect> {
    match spec {
        PathEffectSpec::Dash { intervals, phase } => PathEffect::dash(intervals, *phase),
    }
}
