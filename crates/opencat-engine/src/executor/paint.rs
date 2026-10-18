use opencat_core::canvas::paint::{
    BlendMode, BlurStyle, ColorFilterSpec, FillSpec, FilterChainSpec, FilterInput, FilterPrimKind,
    ImageFilterSpec, LightSpec, MaskFilterSpec, PaintSpec, PaintStyle, PathEffectSpec, ShaderSpec,
    StrokeCap, StrokeJoin, TileMode, TransferFn,
};
use skia_safe::{
    BlendMode as SkBlendMode, BlurStyle as SkBlurStyle, Color, ColorChannel, ColorFilter,
    FilterMode, ImageFilter, Matrix, MaskFilter, MipmapMode, Paint, PaintStyle as SkPaintStyle,
    PathEffect, SamplingOptions, Shader, TileMode as SkTileMode, color_filters, gradient_shader,
    image_filters, scalar, shaders, surfaces,
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

/// 非预乘 [r,g,b,a]（0..1）→ Color4f（drop shadow 颜色不过 8bit 量化）。
fn float4_to_color4f(c: [f32; 4]) -> skia_safe::Color4f {
    skia_safe::Color4f::new(
        c[0].clamp(0.0, 1.0),
        c[1].clamp(0.0, 1.0),
        c[2].clamp(0.0, 1.0),
        c[3].clamp(0.0, 1.0),
    )
}

// ── Blend mode ────────────────────────────────────────────────────────

pub(crate) fn convert_blend_mode(m: BlendMode) -> SkBlendMode {
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
    // 插值空间由 spec 声明：CSS Images 4（JSX 渐变）要求 premultiplied；
    // SVG/resvg 是 unpremultiplied（Skia 默认，stop-opacity 渐变两端 α
    // 不等时中间色不同）。Skia 默认 unpremul 插值会把 transparent → color
    // 的 RGB 耦合成 color·t²，CSS 侧必须显式开 premul。
    match spec {
        ShaderSpec::LinearGradient {
            from,
            to,
            stops,
            colors,
            tile_mode,
            local_matrix,
            interpolate_premul,
        } => {
            let skia_colors: Vec<Color> = colors.iter().map(|c| float4_to_color(*c)).collect();
            let matrix = local_matrix.as_ref().map(skia_matrix_from_row_major);
            let grad_flags = if *interpolate_premul {
                Some(gradient_shader::Flags::INTERPOLATE_COLORS_IN_PREMUL)
            } else {
                None
            };
            gradient_shader::linear(
                ((from[0], from[1]), (to[0], to[1])),
                skia_colors.as_slice(),
                Some(stops.as_slice()),
                convert_tile_mode(*tile_mode),
                grad_flags,
                matrix.as_ref(),
            )
        }
        ShaderSpec::RadialGradient {
            center,
            radius,
            stops,
            colors,
            tile_mode,
            local_matrix,
            interpolate_premul,
            focal,
        } => {
            let skia_colors: Vec<Color> = colors.iter().map(|c| float4_to_color(*c)).collect();
            let matrix = local_matrix.as_ref().map(skia_matrix_from_row_major);
            let grad_flags = if *interpolate_premul {
                Some(gradient_shader::Flags::INTERPOLATE_COLORS_IN_PREMUL)
            } else {
                None
            };
            // SVG 焦点：两点锥形（焦点圆 → 主圆），与 tiny-skia
            // RadialGradient 同语义（resvg 同构）。
            if let Some([fx, fy, fr]) = focal {
                return gradient_shader::two_point_conical(
                    (*fx, *fy),
                    *fr,
                    (center[0], center[1]),
                    *radius,
                    skia_colors.as_slice(),
                    Some(stops.as_slice()),
                    convert_tile_mode(*tile_mode),
                    grad_flags,
                    matrix.as_ref(),
                );
            }
            gradient_shader::radial(
                (center[0], center[1]),
                *radius,
                skia_colors.as_slice(),
                Some(stops.as_slice()),
                convert_tile_mode(*tile_mode),
                grad_flags,
                matrix.as_ref(),
            )
        }
        ShaderSpec::Turbulence {
            base_frequency,
            num_octaves,
            seed,
            tile,
            origin,
        } => {
            // SVG feTurbulence 是用户空间的确定性噪声；Chrome 光栅化该滤镜用
            // 的就是同一个 Skia 实现，同参数逐位一致（alpha 通道实测逐位相
            // 同）。Skia 噪声输出为线性空间预乘值，SVG 滤镜输出前做
            // linear→sRGB，用同一 gamma LUT 对齐。之后按 CSS background 语义
            // 烘瓦片 Repeat 平铺，锚定在节点盒原点。
            let noise = shaders::fractal_noise(
                (base_frequency[0], base_frequency[1]),
                *num_octaves as usize,
                *seed as scalar,
                None,
            )?;
            let Some([tw, th]) = tile.map(|t| [t[0].round() as i32, t[1].round() as i32]) else {
                return None;
            };
            if tw <= 0 || th <= 0 {
                return None;
            }
            let mut tile_surface = surfaces::raster_n32_premul((tw, th))?;
            let mut fill = Paint::default();
            fill.set_shader(noise);
            fill.set_color_filter(color_filters::linear_to_srgb_gamma());
            tile_surface.canvas().draw_paint(&fill);
            let image = tile_surface.image_snapshot();
            let matrix = Matrix::translate((origin[0], origin[1]));
            image.to_shader(
                (SkTileMode::Repeat, SkTileMode::Repeat),
                SamplingOptions::new(FilterMode::Nearest, MipmapMode::None),
                &matrix,
            )
        }
    }
}

fn skia_matrix_from_row_major(m: &[f32; 9]) -> skia_safe::Matrix {
    skia_safe::Matrix::new_all(m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8])
}

// ── Image filter ──────────────────────────────────────────────────────

pub(crate) fn build_skia_image_filter(spec: &ImageFilterSpec) -> Option<ImageFilter> {
    match spec {
        ImageFilterSpec::Blur {
            sigma_x,
            sigma_y,
            crop_rect,
            decal,
        } => {
            let crop = crop_rect.as_ref().map(|r| {
                skia_safe::Rect::from_xywh(
                    r.x0 as f32,
                    r.y0 as f32,
                    r.width() as f32,
                    r.height() as f32,
                )
            });
            // `decal` = CSS filter semantics: the input is transparent past its
            // bounds so the gaussian falloff decays instead of smearing the
            // edge colour (Clamp) across the outset layer bounds.
            let tile = if *decal {
                SkTileMode::Decal
            } else {
                SkTileMode::Clamp
            };
            image_filters::blur(
                (*sigma_x, *sigma_y),
                tile,
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
            keep_content,
        } => {
            let c = float4_to_color(*color);
            if *keep_content {
                // CSS `filter: drop-shadow()` 语义：阴影 + 原内容。
                image_filters::drop_shadow(
                    (*dx, *dy),
                    (*sigma_x, *sigma_y),
                    c,
                    None::<skia_safe::ColorSpace>,
                    None::<ImageFilter>,
                    None::<image_filters::CropRect>,
                )
            } else {
                image_filters::drop_shadow_only(
                    (*dx, *dy),
                    (*sigma_x, *sigma_y),
                    c,
                    None::<skia_safe::ColorSpace>,
                    None::<ImageFilter>,
                    None::<image_filters::CropRect>,
                )
            }
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
        ImageFilterSpec::Chain(chain) => build_filter_chain(chain),
    }
}

// ── SVG filter chain（P4）────────────────────────────────────────────

/// SourceAlpha：输出 (0,0,0,a)（黑体 + alpha，resvg 同语义）。
const SOURCE_ALPHA_MATRIX: [f32; 20] = [
    0.0, 0.0, 0.0, 0.0, 0.0, //
    0.0, 0.0, 0.0, 0.0, 0.0, //
    0.0, 0.0, 0.0, 0.0, 0.0, //
    0.0, 0.0, 0.0, 1.0, 0.0,
];

/// FilterChainSpec → skia filter 树：从 output primitive 递归展开。`Prim`
/// 引用按各消费点重复展开（DAG 共享子链独立求值，语义等价）。
/// SourceGraphic = None（skia 空 input 即层快照）。链输出恒 sRGB（层合成域）。
fn build_filter_chain(chain: &FilterChainSpec) -> Option<ImageFilter> {
    build_filter_prim(chain, chain.output as usize, true)
}

/// 段式色彩空间（与 resvg 的 per-primitive `into_color_space` 同构）：
/// `want_srgb` 是消费方要求的空间；只有相邻 primitive 空间不同才插 gamma
/// color filter，连续同空间原语零往返。crop 挂链段最外层，避免双重裁剪。
/// SourceGraphic/SourceAlpha 先裁到 filter region（resvg 的 source 光栅化
/// 天然只含 region 内容，region 外透明——blur/卷积的边界衰减依赖它）。
fn build_filter_input(
    chain: &FilterChainSpec,
    input: &FilterInput,
    want_srgb: bool,
) -> Option<ImageFilter> {
    let source_crop = || -> Option<ImageFilter> {
        chain.source_crop.map(|r| {
            image_filters::crop(
                skia_safe::Rect::new(r.x0 as f32, r.y0 as f32, r.x1 as f32, r.y1 as f32),
                SkTileMode::Decal,
                None::<ImageFilter>,
            )
        })?
    };
    match input {
        FilterInput::SourceGraphic => {
            let f = source_crop();
            if want_srgb {
                f
            } else {
                image_filters::color_filter(
                    color_filters::srgb_to_linear_gamma(),
                    f,
                    None::<image_filters::CropRect>,
                )
            }
        }
        FilterInput::SourceAlpha => {
            // SourceAlpha 只保留 α，色彩空间无关，按 sRGB 计。
            let f = image_filters::color_filter(
                color_filters::matrix_row_major(&SOURCE_ALPHA_MATRIX, None),
                source_crop(),
                None::<image_filters::CropRect>,
            )
            .unwrap();
            if want_srgb {
                Some(f)
            } else {
                image_filters::color_filter(
                    color_filters::srgb_to_linear_gamma(),
                    f,
                    None::<image_filters::CropRect>,
                )
            }
        }
        FilterInput::Prim(i) => build_filter_prim(chain, *i as usize, want_srgb),
    }
}

fn chain_crop_rect(p: &opencat_core::canvas::paint::FilterPrimSpec) -> Option<image_filters::CropRect> {
    p.crop.map(|r| {
        image_filters::CropRect::from(skia_safe::Rect::new(
            r.x0 as f32,
            r.y0 as f32,
            r.x1 as f32,
            r.y1 as f32,
        ))
    })
}

fn build_filter_prim(
    chain: &FilterChainSpec,
    idx: usize,
    out_srgb: bool,
) -> Option<ImageFilter> {
    let p = chain.primitives.get(idx)?;
    let crop = chain_crop_rect(p);
    let input_of = |i: &FilterInput| -> Option<ImageFilter> {
        build_filter_input(chain, i, p.srgb)
    };
    // 各臂输出的原生域：消费 source 的 prim = primitive 的 cs；
    // Flood 的 shader 输出恒 sRGB 数值，输出转换按此选方向，避免对
    // 已是 sRGB 的值再 γ⁻¹（feBlend+feFlood 簇全图变亮的根因）。
    let mut native_srgb = p.srgb;
    let inner: Option<ImageFilter> = match &p.kind {
        FilterPrimKind::Blend { mode, in1, in2 } => image_filters::blend(
            convert_blend_mode(*mode),
            input_of(in2),
            input_of(in1),
            crop,
        ),
        FilterPrimKind::ColorMatrix { row_major, input } => image_filters::color_filter(
            color_filters::matrix_row_major(row_major, None),
            input_of(input),
            crop,
        ),
        FilterPrimKind::ComponentTransfer { fns, input } => {
            // 恒等通道传 None 跳过量化和域变换。
            let conv = |f: &TransferFn| -> Option<[u8; 256]> {
                match f {
                    TransferFn::Identity => None,
                    other => Some(transfer_table(other)),
                }
            };
            let cf = color_filters::table_argb(
                conv(&fns[3]).as_ref(),
                conv(&fns[0]).as_ref(),
                conv(&fns[1]).as_ref(),
                conv(&fns[2]).as_ref(),
            )?;
            image_filters::color_filter(cf, input_of(input), crop)
        }
        FilterPrimKind::Composite { op, k, in1, in2 } => {
            let bg = input_of(in2);
            let fg = input_of(in1);
            match op {
                0 => image_filters::blend(SkBlendMode::SrcOver, bg, fg, crop),
                1 => image_filters::blend(SkBlendMode::SrcIn, bg, fg, crop),
                2 => image_filters::blend(SkBlendMode::SrcOut, bg, fg, crop),
                3 => image_filters::blend(SkBlendMode::SrcATop, bg, fg, crop),
                4 => image_filters::blend(SkBlendMode::Xor, bg, fg, crop),
                5 => image_filters::blend(SkBlendMode::Plus, bg, fg, crop),
                6 => image_filters::arithmetic(k[0], k[1], k[2], k[3], true, bg, fg, crop),
                _ => return None,
            }
        }
        FilterPrimKind::ConvolveMatrix {
            order,
            kernel,
            divisor,
            bias,
            target,
            kernel_unit_length: _,
            edge,
            preserve_alpha,
            input,
        } => {
            // edge 0 none / 1 duplicate / 2 wrap；unit length 由 usvg 忽略。
            let tile = match edge {
                1 => SkTileMode::Clamp,
                2 => SkTileMode::Repeat,
                _ => SkTileMode::Decal,
            };
            image_filters::matrix_convolution(
                (order.0 as i32, order.1 as i32),
                kernel,
                1.0 / divisor,
                *bias,
                (target.0, target.1),
                tile,
                !*preserve_alpha,
                input_of(input),
                crop,
            )
        }
        FilterPrimKind::DiffuseLighting {
            surface_scale,
            diffuse_constant,
            lighting_color,
            light,
            input,
        } => {
            let c = float4_to_color(*lighting_color);
            let inp = input_of(input);
            match light {
                LightSpec::Distant { azimuth, elevation } => image_filters::distant_lit_diffuse(
                    light_direction(*azimuth, *elevation),
                    c,
                    *surface_scale,
                    *diffuse_constant,
                    inp,
                    crop,
                ),
                LightSpec::Point { x, y, z } => image_filters::point_lit_diffuse(
                    (*x, *y, *z),
                    c,
                    *surface_scale,
                    *diffuse_constant,
                    inp,
                    crop,
                ),
                LightSpec::Spot { x, y, z, points_at_x, points_at_y, points_at_z, specular_exponent, limiting_cone_angle } => {
                    image_filters::spot_lit_diffuse(
                        (*x, *y, *z),
                        (*points_at_x, *points_at_y, *points_at_z),
                        *specular_exponent,
                        limiting_cone_angle.to_radians(),
                        c,
                        *surface_scale,
                        *diffuse_constant,
                        inp,
                        crop,
                    )
                }
            }
        }
        FilterPrimKind::DisplacementMap {
            x_channel,
            y_channel,
            scale,
            input,
            in2,
        } => image_filters::displacement_map(
            (channel_selector(*x_channel), channel_selector(*y_channel)),
            *scale,
            input_of(in2),
            input_of(input),
            crop,
        ),
        FilterPrimKind::DropShadow {
            dx,
            dy,
            sigma_x,
            sigma_y,
            color,
            input,
        } => image_filters::drop_shadow(
            (*dx, *dy),
            (*sigma_x, *sigma_y),
            float4_to_color4f(*color),
            None::<skia_safe::ColorSpace>,
            input_of(input),
            crop,
        ),
        FilterPrimKind::Flood { color } => {
            native_srgb = true;
            image_filters::shader(shaders::color(float4_to_color(*color)), crop)
        }
        FilterPrimKind::GaussianBlur {
            sigma_x,
            sigma_y,
            input,
        } => image_filters::blur(
            (*sigma_x, *sigma_y),
            SkTileMode::Decal,
            input_of(input),
            crop,
        ),
        FilterPrimKind::Merge { inputs } => image_filters::merge(
            inputs.iter().map(|i| build_filter_input(chain, i, p.srgb)),
            crop,
        ),
        FilterPrimKind::Morphology {
            erode,
            radius_x,
            radius_y,
            input,
        } => {
            let f = if *erode {
                image_filters::erode((*radius_x, *radius_y), input_of(input), crop)
            } else {
                image_filters::dilate((*radius_x, *radius_y), input_of(input), crop)
            };
            f
        }
        FilterPrimKind::Offset { dx, dy, input } => {
            image_filters::offset((*dx, *dy), input_of(input), crop)
        }
        FilterPrimKind::SpecularLighting {
            surface_scale,
            specular_constant,
            specular_exponent,
            lighting_color,
            light,
            input,
        } => {
            let c = float4_to_color(*lighting_color);
            let inp = input_of(input);
            match light {
                LightSpec::Distant { azimuth, elevation } => image_filters::distant_lit_specular(
                    light_direction(*azimuth, *elevation),
                    c,
                    *surface_scale,
                    *specular_constant,
                    *specular_exponent,
                    inp,
                    crop,
                ),
                LightSpec::Point { x, y, z } => image_filters::point_lit_specular(
                    (*x, *y, *z),
                    c,
                    *surface_scale,
                    *specular_constant,
                    *specular_exponent,
                    inp,
                    crop,
                ),
                LightSpec::Spot { x, y, z, points_at_x, points_at_y, points_at_z, specular_exponent: spot_exponent, limiting_cone_angle } => {
                    image_filters::spot_lit_specular(
                        (*x, *y, *z),
                        (*points_at_x, *points_at_y, *points_at_z),
                        *spot_exponent,
                        limiting_cone_angle.to_radians(),
                        c,
                        *surface_scale,
                        *specular_constant,
                        *specular_exponent,
                        inp,
                        crop,
                    )
                }
            }
        }
        // feTile 首版直通（语料占比小，tile 源/目标区域语义待对齐）。
        FilterPrimKind::Tile { input } => build_filter_input(chain, input, p.srgb).or(None),
        FilterPrimKind::Turbulence {
            fractal,
            base_frequency,
            num_octaves,
            seed,
            stitch: _,
        } => {
            // stitchTiles 首版忽略（skia 需要 tile_size 取整频率，语料占比小）。
            let sh = if *fractal {
                shaders::fractal_noise(
                    (base_frequency[0], base_frequency[1]),
                    *num_octaves as usize,
                    *seed,
                    None,
                )
            } else {
                shaders::turbulence(
                    (base_frequency[0], base_frequency[1]),
                    *num_octaves as usize,
                    *seed,
                    None,
                )
            }?;
            image_filters::shader(sh, crop)
        }
    };
    let inner = inner?;
    // 输出域到消费方要求域的转换（同域零开销；转换层也挂同 crop，
    // 与 prim 的 crop 重复裁同矩形恒等）。
    if native_srgb == out_srgb {
        Some(inner)
    } else if out_srgb {
        image_filters::color_filter(color_filters::linear_to_srgb_gamma(), inner, crop)
    } else {
        image_filters::color_filter(color_filters::srgb_to_linear_gamma(), inner, crop)
    }
}

/// SVG azimuth/elevation（度）→ 光方向向量（skia/resvg 同构）。
fn light_direction(azimuth: f32, elevation: f32) -> (scalar, scalar, scalar) {
    let (az, el) = (azimuth.to_radians(), elevation.to_radians());
    (az.cos() * el.cos(), az.sin() * el.cos(), el.sin())
}

fn channel_selector(c: u8) -> ColorChannel {
    match c {
        0 => ColorChannel::R,
        1 => ColorChannel::G,
        2 => ColorChannel::B,
        _ => ColorChannel::A,
    }
}

/// TransferFn → 256 项 unpremul 查表（engine 侧展开，SVG 规范插值）。
fn transfer_table(f: &TransferFn) -> [u8; 256] {
    let mut t = [0u8; 256];
    for (i, out) in t.iter_mut().enumerate() {
        let x = i as f32 / 255.0;
        let v = match f {
            TransferFn::Identity => x,
            TransferFn::Table(v) => {
                if v.len() < 2 {
                    v.first().copied().unwrap_or(x)
                } else {
                    let k = (x * (v.len() - 1) as f32).floor() as usize;
                    let k = k.min(v.len() - 2);
                    let frac = x * (v.len() - 1) as f32 - k as f32;
                    v[k] + (v[k + 1] - v[k]) * frac
                }
            }
            TransferFn::Discrete(v) => {
                if v.is_empty() {
                    x
                } else {
                    let k = (x * v.len() as f32).floor() as usize;
                    v[k.min(v.len() - 1)]
                }
            }
            TransferFn::Linear { slope, intercept } => slope * x + intercept,
            TransferFn::Gamma { amplitude, exponent, offset } => {
                amplitude * x.powf(*exponent) + offset
            }
        };
        *out = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    t
}

// ── Color filter ──────────────────────────────────────────────────────

fn build_color_filter(spec: &ColorFilterSpec) -> Option<ColorFilter> {
    match spec {
        ColorFilterSpec::Matrix(matrix) => Some(color_filters::matrix_row_major(matrix, None)),
        ColorFilterSpec::BlendColor { color, mode } => {
            color_filters::blend(float4_to_color(*color), convert_blend_mode(*mode))
        }
        ColorFilterSpec::LinearToSrgbGamma => Some(color_filters::linear_to_srgb_gamma()),
        ColorFilterSpec::SrgbToLinearGamma => Some(color_filters::srgb_to_linear_gamma()),
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

// ── TEMP: Skia backdrop-blur 行为探针（CPU 光栅面）───────────────────────
#[cfg(test)]
mod backdrop_probe {
    use super::*;
    use skia_safe::{canvas::SaveLayerRec, image_filters, surfaces, BlendMode, Canvas, Color, Paint, Rect, Surface};

    fn noise(c: &Canvas) {
        let mut s: u64 = 12345;
        let mut nxt = || { s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (s >> 33) as i32 };
        let mut p = Paint::default();
        for _ in 0..6000 {
            let v = (nxt() % 256) as u8;
            p.set_color(Color::from_argb(255, v, v, v));
            let x = nxt().rem_euclid(400) as f32;
            let y = nxt().rem_euclid(300) as f32;
            c.draw_rect(Rect::from_xywh(x, y, 2.0, 2.0), &p);
        }
    }

    fn center_stddev(surface: &mut Surface) -> f64 {
        let info = surface.image_info();
        let mut buf = vec![0u8; info.min_row_bytes() * surface.height() as usize];
        surface.read_pixels(&info, &mut buf, info.min_row_bytes(), (0, 0));
        let w = surface.width() as usize;
        let mut vals = vec![];
        for y in 100..200usize {
            for x in 150..250usize {
                vals.push(buf[(y * w + x) * 4] as f64);
            }
        }
        let m = vals.iter().sum::<f64>() / vals.len() as f64;
        (vals.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / vals.len() as f64).sqrt()
    }

    #[test]
    fn backdrop_blur_reads_parent_layer() {
        // EXP-F2: 噪点画在 backdrop 层的直接父层内（真实管线拓扑：bg 视频与
        // gi-frame 之间无层边界），看嵌套时 backdrop 是否读得到。
        for depth in [0usize, 1, 2] {
            let mut surface = surfaces::raster_n32_premul((400, 300)).unwrap();
            {
                let c = surface.canvas();
                c.clear(Color::WHITE);
                for _ in 0..depth {
                    c.save_layer(&SaveLayerRec::default());
                }
                noise(c); // 与 backdrop 同层（parent device）
                let blur = image_filters::blur((14.0, 14.0), SkTileMode::Clamp, None, None).unwrap();
                let rec = SaveLayerRec::default().backdrop(&blur);
                c.save_layer(&rec);
                let mut p = Paint::default();
                p.set_color(Color::from_argb(19, 255, 255, 255));
                c.draw_rect(Rect::from_xywh(0.0, 0.0, 400.0, 300.0), &p);
                c.restore();
                for _ in 0..depth {
                    c.restore();
                }
            }
            let after = center_stddev(&mut surface);
            println!("EXP-F2 depth={depth} 环区stddev={after:.2}");
        }
    }

    #[test]
    fn backdrop_blur_survives_nesting() {
        // EXP-F: 深层嵌套 saveLayer 下 backdrop 是否失效
        for depth in [0usize, 1, 2, 3, 6, 12] {
            let mut surface = surfaces::raster_n32_premul((400, 300)).unwrap();
            {
                let c = surface.canvas();
                c.clear(Color::WHITE);
                noise(c);
                for i in 0..depth {
                    c.save_layer(&SaveLayerRec::default());
                    // 每层画个不透明小块，模拟真实层内容
                    let mut p = Paint::default();
                    p.set_color(Color::from_argb(255, 240, 240, 240));
                    c.draw_rect(Rect::from_xywh(i as f32, i as f32, 1.0, 1.0), &p);
                }
                // backdrop 层
                let blur = image_filters::blur((14.0, 14.0), SkTileMode::Clamp, None, None).unwrap();
                let rec = SaveLayerRec::default().backdrop(&blur);
                c.save_layer(&rec);
                let mut p = Paint::default();
                p.set_color(Color::from_argb(19, 255, 255, 255));
                c.draw_rect(Rect::from_xywh(0.0, 0.0, 400.0, 300.0), &p);
                c.restore();
                for _ in 0..depth {
                    c.restore();
                }
            }
            let after = center_stddev(&mut surface);
            println!("EXP-F depth={depth} 环区方差stddev={after:.2}");
        }
    }

    #[test]
    fn backdrop_blur_reduces_noise_on_raster() {
        let mut surface = surfaces::raster_n32_premul((400, 300)).unwrap();
        {
            let c = surface.canvas();
            c.clear(Color::WHITE);
            noise(c);
        }
        let before = center_stddev(&mut surface);
        {
            let c = surface.canvas();
            // 复刻真实管线：CTM = translate+scale（zoom wrapper），底下是 drawImage 的
            // 噪点图，ClipPath 矩形，saveLayer(paint+backdrop, 本地系 bounds)。
            let img_info = skia_safe::ImageInfo::new((400, 300), skia_safe::ColorType::RGBA8888, skia_safe::AlphaType::Premul, None);
            let mut px = vec![255u8; 400 * 300 * 4];
            let mut s: u64 = 999u64;
            for v in px.iter_mut() { s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); *v = (s >> 33) as u8; }
            let data = unsafe { skia_safe::Data::new_bytes(&px) };
            let img = skia_safe::images::raster_from_data(&img_info, data, 400 * 4).expect("img");
            c.save();
            c.scale((0.8168, 0.8168));
            c.translate((30.0, 20.0));
            let origin = skia_safe::Point { x: 0.0, y: 0.0 };
            c.draw_image(&img, origin, None);
            let local = Rect::new(-5.0, -4.0, 405.0, 304.0); // 本地系 bounds（含外扩）
            c.save();
            let rpath = skia_safe::Path::rect(local, None);
            c.clip_path(&rpath, skia_safe::ClipOp::Intersect, false);
            let blur = image_filters::blur((14.0, 14.0), SkTileMode::Clamp, None, None)
                .expect("blur filter builds");
            let mut lp = Paint::default();
            lp.set_color(Color::from_argb(255, 255, 255, 255));
            lp.set_anti_alias(true);
            let rec = SaveLayerRec::default()
                .paint(&lp)
                .backdrop(&blur)
                .bounds(&local);
            c.save_layer(&rec);
            let mut p = Paint::default();
            p.set_color(Color::from_argb(19, 255, 255, 255));
            p.set_blend_mode(BlendMode::SrcOver);
            c.draw_rect(Rect::from_xywh(0.0, 0.0, 400.0, 300.0), &p);
            c.restore();
            c.restore();
        }
        let after = center_stddev(&mut surface);
        println!("backdrop probe: stddev before={before:.2} after={after:.2}");
        assert!(after < before * 0.5, "backdrop blur 应显著抹平噪点 before={before:.2} after={after:.2}");
    }
}
