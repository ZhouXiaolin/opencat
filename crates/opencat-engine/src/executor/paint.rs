use opencat_core::canvas::paint::{
    BlendMode, BlurStyle, ColorFilterSpec, FillSpec, FilterChainSpec, FilterInput, FilterPrimKind,
    ImageFilterSpec, LightSpec, MaskFilterSpec, PaintSpec, PaintStyle, PathEffectSpec, ShaderSpec,
    StrokeCap, StrokeJoin, TileMode, TransferFn,
};
use skia_safe::{
    AlphaType, BlendMode as SkBlendMode, BlurStyle as SkBlurStyle, Color, ColorChannel,
    ColorFilter, ColorType, Data, FilterMode, Image, ImageFilter, ImageInfo, Matrix, MaskFilter,
    MipmapMode, Paint, PaintStyle as SkPaintStyle, PathEffect, PictureRecorder, SamplingOptions,
    Shader, TileMode as SkTileMode, color_filters, gradient_shader, image_filters, scalar,
    shaders, surfaces,
};
use skia_safe::image::BitDepth;

/// feImage 的子树光栅图缓存：执行端在展开 filter 前预渲染填充，feImage
/// 分支取用。SaveLayer 处理是单线程递归的，thread_local 即可；成对
/// set/clear，泄漏面为零。
thread_local! {
    static FILTER_IMAGES: std::cell::RefCell<Option<std::collections::HashMap<u32, Image>>> =
        const { std::cell::RefCell::new(None) };
}

pub(crate) fn set_filter_images(images: std::collections::HashMap<u32, Image>) {
    FILTER_IMAGES.with(|c| *c.borrow_mut() = Some(images));
}

pub(crate) fn clear_filter_images() {
    FILTER_IMAGES.with(|c| *c.borrow_mut() = None);
}

fn filter_image(subtree: u32) -> Option<Image> {
    FILTER_IMAGES.with(|c| c.borrow().as_ref()?.get(&subtree).cloned())
}

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
        ImageFilterSpec::Chain(chains) => build_filter_chains(chains),
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

/// 多 filter 级联：`url(#a) url(#b)` 从左到右求值，compose(outer, inner) =
/// outer(inner(input))，故从右往左包 compose。每链 SourceGraphic 各自裁到
/// 自己的 source_crop（resvg 逐 filter 区域化求值同语义）。
fn build_filter_chains(chains: &[FilterChainSpec]) -> Option<ImageFilter> {
    let mut iter = chains.iter().rev();
    let mut acc = build_filter_chain(iter.next()?)?;
    for chain in iter {
        acc = image_filters::compose(acc, build_filter_chain(chain)?)?;
    }
    Some(acc)
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
                    resvg_cs_color_filter(true),
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
                    resvg_cs_color_filter(true),
                    f,
                    None::<image_filters::CropRect>,
                )
            }
        }
        FilterInput::Prim(i) => build_filter_prim(chain, *i as usize, want_srgb),
    }
}

/// feTile 的瓦片源窗口（resvg 语义）：input prim 的有效 region。feOffset
/// 继承 input 的 region（平移不扩域），故递归穿透；Source 系 = filter region。
fn tile_input_region(
    chain: &FilterChainSpec,
    input: &FilterInput,
) -> Option<opencat_core::canvas::Rect> {
    match input {
        FilterInput::Prim(i) => {
            let ip = chain.primitives.get(*i as usize)?;
            match &ip.kind {
                FilterPrimKind::Offset { input, .. } => tile_input_region(chain, input),
                _ => ip.crop,
            }
        }
        _ => chain.source_crop,
    }
}

/// prim subregion 收进 region 内（resvg 的 region pixmap 边界语义）。
/// 空交返回 None。
fn clamped_crop(
    chain: &FilterChainSpec,
    p: &opencat_core::canvas::paint::FilterPrimSpec,
) -> Option<opencat_core::canvas::Rect> {
    let mut c = p.crop?;
    if let Some(rc) = chain.source_crop {
        c.x0 = c.x0.max(rc.x0);
        c.y0 = c.y0.max(rc.y0);
        c.x1 = c.x1.min(rc.x1);
        c.y1 = c.y1.min(rc.y1);
    }
    if c.x1 <= c.x0 || c.y1 <= c.y0 {
        return None;
    }
    Some(c)
}

fn chain_crop_rect(
    chain: &FilterChainSpec,
    p: &opencat_core::canvas::paint::FilterPrimSpec,
) -> Option<image_filters::CropRect> {
    // resvg 的 prim 结果永远活在 region pixmap 里：subregion 超出 region 的
    // 部分被 Clear 掉（输出层 = region）。skia 树无 pixmap 边界，显式把
    // crop 收进 region 内。
    let c = clamped_crop(chain, p);
    // clip 后内容为空（subregion 与 region 不相交）→ 输出全透明
    // （resvg 的空内容也走正常链，元素不被裸画）。
    let rect = c
        .map(|c| skia_safe::Rect::new(c.x0 as f32, c.y0 as f32, c.x1 as f32, c.y1 as f32))
        .unwrap_or_else(|| skia_safe::Rect::new(0.0, 0.0, 0.0, 0.0));
    Some(image_filters::CropRect::from(rect))
}

/// resvg 执行前的 region/subregion 整数化（tiny-skia to_int_rect：
/// floor(x)+max(1,ceil(w))，subregion 相位以 region 原点为基准）。SKSL
/// 路径的采样边界与裁剪都吃这个整数格。
fn resvg_crop_rect(
    chain: &FilterChainSpec,
    p: &opencat_core::canvas::paint::FilterPrimSpec,
) -> Option<skia_safe::Rect> {
    let c = clamped_crop(chain, p)?;
    let rc = chain.source_crop.unwrap_or(c);
    let rx0 = rc.x0.floor() as f32;
    let ry0 = rc.y0.floor() as f32;
    let x0 = (c.x0 as f32 - rx0).floor() + rx0;
    let y0 = (c.y0 as f32 - ry0).floor() + ry0;
    Some(skia_safe::Rect::new(
        x0,
        y0,
        x0 + (c.x1 - c.x0).ceil().max(1.0) as f32,
        y0 + (c.y1 - c.y0).ceil().max(1.0) as f32,
    ))
}

fn build_filter_prim(
    chain: &FilterChainSpec,
    idx: usize,
    out_srgb: bool,
) -> Option<ImageFilter> {
    let p = chain.primitives.get(idx)?;
    let crop = chain_crop_rect(chain, p);
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
        FilterPrimKind::ColorMatrix {
            row_major,
            input,
        } => {
            // resvg from_normalized 是 floor 截断（(clamp×255) as u8），skia
            // 矩阵 filter 输出 round——hueRotate 等大系数矩阵 G 通道系统性
            // 偏 +1。SKSL 矩阵（uniform 传参）+ floor 量化逐位对齐；失败
            // 回退内建。
            let inp = input_of(input);
            matrix_runtime_filter(row_major, inp.clone(), crop)
                .or_else(|| image_filters::color_filter(
                    color_filters::matrix_row_major(row_major, None),
                    inp,
                    crop,
                ))
        }
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
            // resvg 的卷积是 premul 域逐位公式（÷divisor 后 + bias·未clamp
            // new_a、通道 clamp 到 bounded_a）——skia matrix_convolution 的
            // bias 直接加 0–255 且不 clamp-to-alpha，半透明输入必差，SKSL
            // 精确复刻；编译失败回退内建（不透明输入两路等价）。
            let inp = input_of(input);
            let crop_rect = resvg_crop_rect(chain, p);
            convolve_runtime_filter(
                *order,
                kernel,
                *divisor,
                *bias,
                *target,
                *edge,
                *preserve_alpha,
                chain.source_crop,
                inp.clone(),
                crop_rect,
            )
            .or_else(|| {
                // edge 0 none / 1 duplicate / 2 wrap；unit length 由 usvg 忽略。
                let tile = match edge {
                    1 => SkTileMode::Clamp,
                    2 => SkTileMode::Repeat,
                    _ => SkTileMode::Decal,
                };
                // SVG bias∈[0,1] 按通道加 bias×255（skia bias 直接加在 0–255 域，
                // Chromium 同此换算）；gain=1/divisor。
                image_filters::matrix_convolution(
                    (order.0 as i32, order.1 as i32),
                    kernel,
                    1.0 / divisor,
                    *bias * 255.0,
                    (target.0, target.1),
                    tile,
                    !*preserve_alpha,
                    inp,
                    crop,
                )
            })
        }
        FilterPrimKind::DiffuseLighting {
            surface_scale,
            diffuse_constant,
            lighting_color,
            light,
            input,
        } => {
            // resvg 逐位公式走 SKSL（skia 内建拒负 kd 且锥边 smoothstep），
            // 编译失败回退内建。
            let inp = input_of(input);
            let crop_rect = resvg_crop_rect(chain, p);
            lighting_runtime_filter(
                true,
                *surface_scale,
                *diffuse_constant,
                1.0,
                *lighting_color,
                light,
                chain.source_crop,
                inp.clone(),
                crop_rect,
            )
            .or_else(|| {
                let c = float4_to_color(*lighting_color);
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
                        // SVG limitingConeAngle 单位为度；skia 内部自行 DegreesToRadians。
                        image_filters::spot_lit_diffuse(
                            (*x, *y, *z),
                            (*points_at_x, *points_at_y, *points_at_z),
                            *specular_exponent,
                            *limiting_cone_angle,
                            c,
                            *surface_scale,
                            *diffuse_constant,
                            inp,
                            crop,
                        )
                    }
                }
            })
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
            // 同 diffuse：resvg 逐位走 SKSL，失败回退内建。
            let inp = input_of(input);
            let crop_rect = resvg_crop_rect(chain, p);
            lighting_runtime_filter(
                false,
                *surface_scale,
                *specular_constant,
                *specular_exponent,
                *lighting_color,
                light,
                chain.source_crop,
                inp.clone(),
                crop_rect,
            )
            .or_else(|| {
                let c = float4_to_color(*lighting_color);
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
                        // 同 spot_lit_diffuse：cutoff 单位为度。
                        image_filters::spot_lit_specular(
                            (*x, *y, *z),
                            (*points_at_x, *points_at_y, *points_at_z),
                            *spot_exponent,
                            *limiting_cone_angle,
                            c,
                            *surface_scale,
                            *specular_constant,
                            *specular_exponent,
                            inp,
                            crop,
                        )
                    }
                }
            })
        }
        // feTile：瓦片 = input 结果在「input subregion 窗口」内的残片，以该
        // 窗口为周期 Repeat 充满 prim subregion（resvg apply_tile 就是
        // Pattern Repeat；feOffset 平移出窗口的部分被裁掉）。input 链绘制
        // 录成 Picture → lazy Image（skia 求值本 filter 时才光栅化），shader
        // localMatrix 把图像左上角锚回窗口原点——Repeat 网格即锚定源 rect。
        FilterPrimKind::Tile { input } => {
            let src = tile_input_region(chain, input)?;
            // resvg：prim subregion 与 filter region 不相交时整链求值失败、
            // 元素不渲染；这里等价为输出全透明。
            let region = chain.source_crop?;
            let touches = src.x0 < region.x1
                && region.x0 < src.x1
                && src.y0 < region.y1
                && region.y0 < src.y1;
            if !touches {
                return Some(image_filters::crop(
                    skia_safe::Rect::new(0.0, 0.0, 0.0, 0.0),
                    SkTileMode::Decal,
                    None::<ImageFilter>,
                )?);
            }
            let w = ((src.x1 - src.x0).ceil() as i32).max(1);
            let h = ((src.y1 - src.y0).ceil() as i32).max(1);
            let mut recorder = PictureRecorder::new();
            {
                // filter 树坐标 = user space，画布不能 translate（会偏移链内
                // crop rect）；平移交给 from_picture 的 matrix。
                let cull = skia_safe::Rect::new(src.x0 as f32, src.y0 as f32, src.x1 as f32, src.y1 as f32);
                let c = recorder.begin_recording(cull, false);
                let mut paint = Paint::default();
                paint.set_image_filter(input_of(input));
                c.draw_paint(&paint);
            }
            let picture = recorder.finish_recording_as_picture(None)?;
            let img = Image::from_picture(
                picture,
                (w, h),
                Some(&Matrix::translate((-(src.x0 as f32), -(src.y0 as f32)))),
                None::<&Paint>,
                BitDepth::U8,
                Some(skia_safe::ColorSpace::new_srgb()),
            )?;
            let matrix = Matrix::translate((src.x0 as f32, src.y0 as f32));
            let shader = img.to_shader(
                (SkTileMode::Repeat, SkTileMode::Repeat),
                SamplingOptions::new(FilterMode::Nearest, MipmapMode::None),
                Some(&matrix),
            )?;
            image_filters::shader(shader, crop)
        }
        FilterPrimKind::Image { subtree } => {
            // feImage：执行端已把子树光栅化进 region 大小的图（内容坐标
            // user space 平移 -region.xy），这里 1:1 放回 region 原点，再裁到
            // prim subregion（resvg 的 prim 后处理）。输出 = 原生 sRGB 像素。
            let region = chain.source_crop?;
            let img = filter_image(*subtree)?;
            native_srgb = true;
            let dst = skia_safe::Rect::from_xywh(
                region.x0 as f32,
                region.y0 as f32,
                (region.x1 - region.x0) as f32,
                (region.y1 - region.y0) as f32,
            );
            let f = image_filters::image(
                img,
                None::<&skia_safe::Rect>,
                Some(&dst),
                Some(SamplingOptions::new(FilterMode::Nearest, MipmapMode::None)),
            )?;
            // 裁到 prim subregion（resvg 的 prim 后处理；crop 与 dst 同矩形时
            // 恒等，小 subregion 时截断溢出）。
            match p.crop {
                Some(r) => image_filters::crop(
                    skia_safe::Rect::new(r.x0 as f32, r.y0 as f32, r.x1 as f32, r.y1 as f32),
                    SkTileMode::Decal,
                    f,
                ),
                None => Some(f),
            }
        }
        FilterPrimKind::Turbulence {
            fractal,
            base_frequency,
            num_octaves,
            seed,
            stitch,
        } => {
            // skia 与 resvg（SVG spec）同算法同 RNG，唯二差异：①skia 的噪声
            // 坐标 = seed_shader 的像素中心（x+0.5）再加 0.5 = (x+1)·bf，
            // spec/resvg 是 x·bf——LM=translate(1.0)（采样坐标 = LM⁻¹×设备
            // = 设备-1.0）补偿；②resvg 的 stitch wrap 判定因 tile_x 取当前
            // 像素而恒假（floor(x·bf) ≥ x·bf+w0 永假），实际只有频率调整
            // 生效——engine 侧按 spec 预调频率、tile_size 传 None 关掉 skia
            // 的 wrap。
            let region = chain.source_crop?;
            let (mut fx, mut fy) = (base_frequency[0] as f64, base_frequency[1] as f64);
            if *stitch {
                if fx != 0.0 {
                    let w = region.width();
                    let lo = (w * fx).floor() / w;
                    let hi = (w * fx).ceil() / w;
                    fx = if fx / lo < hi / fx { lo } else { hi };
                }
                if fy != 0.0 {
                    let h = region.height();
                    let lo = (h * fy).floor() / h;
                    let hi = (h * fy).ceil() / h;
                    fy = if fy / lo < hi / fy { lo } else { hi };
                }
            }
            let sh = if *fractal {
                shaders::fractal_noise(
                    (fx as scalar, fy as scalar),
                    *num_octaves as usize,
                    *seed,
                    None,
                )
            } else {
                shaders::turbulence(
                    (fx as scalar, fy as scalar),
                    *num_octaves as usize,
                    *seed,
                    None,
                )
            }?;
            let sh = sh.with_local_matrix(&Matrix::translate((1.0, 1.0)));
            image_filters::shader(sh, crop)
        }
    };
    let inner = inner?;
    // 输出域到消费方要求域的转换（同域零开销；转换层也挂同 crop，
    // 与 prim 的 crop 重复裁同矩形恒等）。
    if native_srgb == out_srgb {
        Some(inner)
    } else if out_srgb {
        image_filters::color_filter(resvg_cs_color_filter(false), inner, crop)
    } else {
        image_filters::color_filter(resvg_cs_color_filter(true), inner, crop)
    }
}

/// resvg apply_color_matrix 的 SKSL 复刻：demultiply（u8 round）→ unpremul
/// 域矩阵（clamp 后 **floor** 量化，from_normalized 的 `(c*255) as u8` 截断
/// 语义，与 skia 内建的 round 不同）→ premultiply（u8 round）。矩阵走
/// uniform，源码恒定。
fn matrix_runtime_filter(
    row_major: &[f32; 20],
    input: Option<ImageFilter>,
    crop: Option<image_filters::CropRect>,
) -> Option<ImageFilter> {
    // resvg apply_color_matrix（filter/mod.rs:916-929）语义：demultiply
    // （u8 round）→ unpremul 域矩阵（clamp 后 floor 量化）→ premultiply
    // （u8 round）。color_matrix.rs 的 to/from_normalized 在 RGBA8 上做，
    // 输入是 demul 后的 unpremul 像素。a==0 短路（Rust `as u8` 对 inf 的
    // 饱和结果即 0）；demul 的 u8 饱和用 min 255 复刻。
    let sksl = "uniform float4 m0;\nuniform float4 m1;\nuniform float4 m2;\nuniform float4 m3;\nuniform float4 m4;\nhalf4 main(half4 p) {\n    float a = p.a;\n    if (a <= 0.0) { return half4(0.0); }\n    float3 u = p.rgb;\n    if (a < 1.0) { u = min(floor(u / a * 255.0 + 0.5), 255.0) / 255.0; }\n    float4 c = float4(u, a);\n    float nr = clamp(dot(c, m0) + m4.r, 0.0, 1.0);\n    float ng = clamp(dot(c, m1) + m4.g, 0.0, 1.0);\n    float nb = clamp(dot(c, m2) + m4.b, 0.0, 1.0);\n    float na = clamp(dot(c, m3) + m4.a, 0.0, 1.0);\n    nr = floor(nr * 255.0) / 255.0;\n    ng = floor(ng * 255.0) / 255.0;\n    nb = floor(nb * 255.0) / 255.0;\n    na = floor(na * 255.0) / 255.0;\n    if (na < 1.0) {\n        nr = floor(nr * na * 255.0 + 0.5) / 255.0;\n        ng = floor(ng * na * 255.0 + 0.5) / 255.0;\n        nb = floor(nb * na * 255.0 + 0.5) / 255.0;\n    }\n    return half4(nr, ng, nb, na);\n}";
    let effect = match skia_safe::RuntimeEffect::make_for_color_filter(sksl, None) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("[sksl-err-matrix] {e:?}");
            return None;
        }
    };
    // uniform 布局按声明序打包：4 个 float4（矩阵行）+ 1 个 float4（offset 列）。
    let mut bytes = Vec::with_capacity(80);
    for i in 0..4 {
        for v in &row_major[i * 5..i * 5 + 4] {
            bytes.extend_from_slice(&v.to_ne_bytes());
        }
    }
    for v in row_major.iter().skip(4).step_by(5) {
        bytes.extend_from_slice(&v.to_ne_bytes());
    }
    let cf = effect.make_color_filter(
        // SAFETY: bytes 本地构造；new_copy 在调用瞬间拷贝走，无生命周期外泄。
        unsafe { skia_safe::Data::new_copy(&bytes) },
        None,
    )?;
    image_filters::color_filter(cf, input, crop)
}

/// resvg filter/mod.rs 的色彩空间转换 SKSL 复刻：demultiply（u8 round）→
/// γ 公式→ u8 round → multiply（u8 round）。skia 内建 gamma 是 float 精确
/// 且在 premul 域直过——缺 resvg 的三步 u8 量化（feColorMatrix 等链差
/// ±1~2/255 的来源）与半透明域差。runtime color filter 无 premul 包裹，
/// 域完全自控。to_linear=true: sRGB→linearRGB。
fn resvg_cs_color_filter(to_linear: bool) -> ColorFilter {
    thread_local! {
        static CACHE: std::cell::RefCell<[Option<ColorFilter>; 2]> =
            const { std::cell::RefCell::new([None, None]) };
    }
    CACHE.with(|c| {
        if let Some(f) = c.borrow()[to_linear as usize].clone() {
            return f;
        }
        // q = resvg 的 (x*255+0.5) as u8 回 0..1 网格。
        let gamma = if to_linear {
            "float lin(float x) { return x <= 0.04045 ? x / 12.92 : pow((x + 0.055) / 1.055, 2.4); }"
        } else {
            "float lin(float x) { return x <= 0.0031308 ? x * 12.92 : 1.055 * pow(x, 1.0 / 2.4) - 0.055; }"
        };
        let sksl = format!(
            "{gamma}\nhalf4 main(half4 p) {{\n    float a = p.a;\n    if (a <= 0.0) {{ return half4(0.0); }}\n    float3 u = p.rgb;\n    if (a < 1.0) {{\n        u = floor(u / a * 255.0 + 0.5) / 255.0;\n    }}\n    float3 g = float3(lin(u.r), lin(u.g), lin(u.b));\n    float3 o = floor(g * 255.0 + 0.5) / 255.0;\n    if (a < 1.0) {{\n        o = floor(o * a * 255.0 + 0.5) / 255.0;\n    }}\n    return half4(o, a);\n}}"
        );
        let f = match skia_safe::RuntimeEffect::make_for_color_filter(&sksl, None) {
            Ok(e) => e.make_color_filter(skia_safe::Data::new_empty(), None),
            Err(e) => {
                eprintln!("[sksl-err-cs] {e:?}\n----\n{sksl}");
                None
            }
        }
        .unwrap_or_else(|| color_filters::srgb_to_linear_gamma());
        c.borrow_mut()[to_linear as usize] = Some(f.clone());
        f
    })
}

/// SVG azimuth/elevation（度）→ 光方向向量（skia/resvg 同构）。
fn light_direction(azimuth: f32, elevation: f32) -> (scalar, scalar, scalar) {
    let (az, el) = (azimuth.to_radians(), elevation.to_radians());
    (az.cos() * el.cos(), az.sin() * el.cos(), el.sin())
}

/// resvg lighting.rs 的 SKSL 逐位复刻。与 skia 内建 lighting 的系统性差异：
/// ①skia 拒绝负 kd/ks（filter 创建失败直通原图），resvg 的 factor=k·kd 在
/// clamp 前相乘，负系数背面发光；②skia 的 spot 锥边 smoothstep 过渡，
/// resvg 硬截断；③法线是 resvg 的 9 位置 Sobel 模板（角/边系数 2/3、1/3、
/// 1/2，非标准 Sobel/4）；④光色与输出都做 u8 量化 round。常量全部折叠
/// 进源码。
#[allow(clippy::too_many_arguments)]
fn lighting_runtime_filter(
    diffuse: bool,
    surface_scale: f32,
    constant: f32,
    exponent: f32,
    lighting_color: [f32; 4],
    light: &LightSpec,
    region: Option<opencat_core::canvas::Rect>,
    input: Option<ImageFilter>,
    crop_rect: Option<skia_safe::Rect>,
) -> Option<ImageFilter> {
    let region = region?;
    let f = |v: f32| {
        if v == v.trunc() && v.abs() < 1e9 {
            format!("{v:.1}")
        } else {
            format!("{v}")
        }
    };
    // resvg to_int_rect 语义（floor+ceil）整数化，同 convolve。
    let rx0 = region.x0.floor() as f32;
    let ry0 = region.y0.floor() as f32;
    let rx1 = rx0 + (region.x1 - region.x0).ceil().max(1.0) as f32;
    let ry1 = ry0 + (region.y1 - region.y0).ceil().max(1.0) as f32;
    let (r0x, r0y, r1x, r1y) = (f(rx0), f(ry0), f(rx1 - 1.0), f(ry1 - 1.0));

    // light vector 与光色（resvg apply/light_color 逐位）。
    let light_code = match light {
        LightSpec::Distant { azimuth, elevation } => {
            let (az, el) = ((*azimuth as f64).to_radians(), (*elevation as f64).to_radians());
            let (lx, ly, lz) = (
                az.cos() * el.cos(),
                az.sin() * el.cos(),
                el.sin(),
            );
            format!(
                "float3 lv = float3({}, {}, {});\n    float3 lc = u8c;",
                f(lx as f32),
                f(ly as f32),
                f(lz as f32)
            )
        }
        LightSpec::Point { x, y, z } => format!(
            "float nz = c * {};\n    float3 v = float3({}, {}, {}) - float3(x, y, nz);\n    float vlen = length(v);\n    float3 lv = vlen > 0.0 ? v / vlen : v;\n    float3 lc = u8c;",
            f(surface_scale),
            f(*x),
            f(*y),
            f(*z)
        ),
        LightSpec::Spot { x, y, z, points_at_x, points_at_y, points_at_z, specular_exponent, limiting_cone_angle } => {
            let (dx, dy, dz) = (
                points_at_x - x,
                points_at_y - y,
                points_at_z - z,
            );
            let cos_cone = (*limiting_cone_angle as f64).to_radians().cos();
            format!(
                "float nz = c * {ss};\n    float3 v = float3({x}, {y}, {z}) - float3(x, y, nz);\n    float vlen = length(v);\n    float3 lv = vlen > 0.0 ? v / vlen : v;\n    float3 dir = float3({dx}, {dy}, {dz});\n    float dl = length(dir);\n    dir = dl > 0.0 ? dir / dl : dir;\n    float mls = -dot(lv, dir);\n    float3 lc = float3(0.0);\n    if (mls > 0.0 && mls >= {cc}) {{\n        float sf = spow(mls, {se});\n        lc = float3(floor(clamp(u8c.r * sf, 0.0, 255.0) + 0.5), floor(clamp(u8c.g * sf, 0.0, 255.0) + 0.5), floor(clamp(u8c.b * sf, 0.0, 255.0) + 0.5));\n    }}",
                ss = f(surface_scale),
                x = f(*x),
                y = f(*y),
                z = f(*z),
                dx = f(dx),
                dy = f(dy),
                dz = f(dz),
                cc = f(cos_cone as f32),
                se = f(*specular_exponent),
            )
        }
    };

    let (u8r, u8g, u8b) = (
        f(lighting_color[0] * 255.0),
        f(lighting_color[1] * 255.0),
        f(lighting_color[2] * 255.0),
    );
    let k_code = if diffuse {
        // k = kd·(N·L/|N|)；平坦区 = L.z。输出 α=255。
        format!(
            "float k;\n    if (nxs == 0.0 && nys == 0.0) {{ k = lv.z; }} else {{ float3 N = float3(nx, ny, 1.0); k = dot(N, lv) / length(N); }}\n    float factor = {kc} * k;\n    float rr = floor(clamp(lc.r * factor, 0.0, 255.0) + 0.5) / 255.0;\n    float gg = floor(clamp(lc.g * factor, 0.0, 255.0) + 0.5) / 255.0;\n    float bb = floor(clamp(lc.b * factor, 0.0, 255.0) + 0.5) / 255.0;\n    return half4(rr, gg, bb, 1.0);",
            kc = f(constant)
        )
    } else {
        // Blinn：h=L+(0,0,1)；负 ndh 经 clamp/as-u8 恒 0，直接短路。
        format!(
            "float3 h = lv + float3(0.0, 0.0, 1.0);\n    float hl = length(h);\n    float k = 0.0;\n    if (hl > 0.0) {{\n        float ndh;\n        if (nxs == 0.0 && nys == 0.0) {{ ndh = h.z / hl; }} else {{ float3 N = float3(nx, ny, 1.0); ndh = dot(N, h) / (length(N) * hl); }}\n        k = ndh <= 0.0 ? 0.0 : ({ex} == 1.0 ? ndh : pow(ndh, {ex}));\n    }}\n    float factor = {kc} * k;\n    float rr = floor(clamp(lc.r * factor, 0.0, 255.0) + 0.5) / 255.0;\n    float gg = floor(clamp(lc.g * factor, 0.0, 255.0) + 0.5) / 255.0;\n    float bb = floor(clamp(lc.b * factor, 0.0, 255.0) + 0.5) / 255.0;\n    return half4(rr, gg, bb, max(rr, max(gg, bb)));",
            kc = f(constant),
            ex = f(exponent)
        )
    };

    let s = format!(
        "uniform shader child;\nfloat spow(float v, float e) {{ return v <= 0.0 ? 0.0 : pow(v, e); }}\nfloat aa(float px, float py) {{\n    if (px < {r0x} || px > {r1x} || py < {r0y} || py > {r1y}) {{ return 0.0; }}\n    return child.eval(float2(px + 0.5, py + 0.5)).a;\n}}\nhalf4 main(float2 coord) {{\n    float x = floor(coord.x);\n    float y = floor(coord.y);\n    float c = aa(x, y);\n    float t = aa(x, y - 1.0);\n    float b = aa(x, y + 1.0);\n    float l = aa(x - 1.0, y);\n    float r = aa(x + 1.0, y);\n    float tl = aa(x - 1.0, y - 1.0);\n    float tr = aa(x + 1.0, y - 1.0);\n    float bl = aa(x - 1.0, y + 1.0);\n    float br = aa(x + 1.0, y + 1.0);\n    float nxs; float nys; float fx; float fy;\n    if (x <= {r0x}) {{\n        if (y <= {r0y}) {{ nxs = -2.0*c + 2.0*r - b + br; nys = -2.0*c - r + 2.0*b + br; fx = 0.6666667; fy = 0.6666667; }}\n        else if (y >= {r1y}) {{ nxs = -t + tr - 2.0*c + 2.0*r; nys = -2.0*t - tr + 2.0*c + r; fx = 0.6666667; fy = 0.6666667; }}\n        else {{ nxs = -t + tr - 2.0*c + 2.0*r - b + br; nys = -2.0*t - tr + 2.0*b + br; fx = 0.5; fy = 0.33333334; }}\n    }} else if (x >= {r1x}) {{\n        if (y <= {r0y}) {{ nxs = -2.0*l + 2.0*c - bl + b; nys = -l - 2.0*c + bl + 2.0*b; fx = 0.6666667; fy = 0.6666667; }}\n        else if (y >= {r1y}) {{ nxs = -tl + t - 2.0*l + 2.0*c; nys = -tl - 2.0*t + l + 2.0*c; fx = 0.6666667; fy = 0.6666667; }}\n        else {{ nxs = -tl + t - 2.0*l + 2.0*c - bl + b; nys = -tl - 2.0*t + bl + 2.0*b; fx = 0.5; fy = 0.33333334; }}\n    }} else {{\n        if (y <= {r0y}) {{ nxs = -2.0*l + 2.0*r - bl + br; nys = -l - 2.0*c - r + bl + 2.0*b + br; fx = 0.33333334; fy = 0.5; }}\n        else if (y >= {r1y}) {{ nxs = -tl + tr - 2.0*l + 2.0*r; nys = -tl - 2.0*t - tr + l + 2.0*c + r; fx = 0.33333334; fy = 0.5; }}\n        else {{ nxs = -tl + tr - 2.0*l + 2.0*r - bl + br; nys = -tl - 2.0*t - tr + bl + 2.0*b + br; fx = 0.25; fy = 0.25; }}\n    }}\n    float nx = -nxs * {ss} * fx;\n    float ny = -nys * {ss} * fy;\n    float3 u8c = float3({u8r}, {u8g}, {u8b});\n    {light_code}\n    {k_code}\n}}",
        r0x = r0x,
        r0y = r0y,
        r1x = r1x,
        r1y = r1y,
        ss = f(surface_scale),
        u8r = u8r,
        u8g = u8g,
        u8b = u8b,
        light_code = light_code,
        k_code = k_code,
    );

    let effect = match skia_safe::RuntimeEffect::make_for_shader(&s, None) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("[sksl-err] {e:?}\n----\n{s}");
            return None;
        }
    };
    let builder = skia_safe::runtime_effect::RuntimeShaderBuilder::new(effect);
    let rf = image_filters::runtime_shader(&builder, "child", input)?;
    match crop_rect {
        Some(r) => image_filters::crop(r, SkTileMode::Decal, rf),
        None => Some(rf),
    }
}

/// resvg convolve_matrix.rs 的 SKSL 逐位复刻：premul 像素 ÷255 归一卷积
/// （核按转置镜像索引），new_a = Σa·k/divisor + bias（preserve 时 = 原 a），
/// 通道 = Σc·k/divisor + bias·new_a，clamp 到 bounded_a（非 1）。edge 三
/// 模式对 region pixmap 边界（resvg 的 prim 求值域）显式处理，出界与
/// resvg 同义：none=透明、duplicate=clamp、wrap=正模。divisor/bias/核
/// 全部常量折叠进源码，零 uniform。
#[allow(clippy::too_many_arguments)]
fn convolve_runtime_filter(
    order: (u32, u32),
    kernel: &[f32],
    divisor: f32,
    bias: f32,
    target: (i32, i32),
    edge: u8,
    preserve_alpha: bool,
    region: Option<opencat_core::canvas::Rect>,
    input: Option<ImageFilter>,
    crop_rect: Option<skia_safe::Rect>,
) -> Option<ImageFilter> {
    let region = region?;
    let (cols, rows) = (order.0 as usize, order.1 as usize);
    if kernel.len() != cols * rows || cols == 0 || rows == 0 || divisor == 0.0 {
        return None;
    }
    // SkSL 无隐式 int→float，字面量恒保小数点。
    let f = |v: f32| {
        if v == v.trunc() && v.abs() < 1e9 {
            format!("{v:.1}")
        } else {
            format!("{v}")
        }
    };
    // resvg 执行前把 region/subregion 整数化（tiny-skia to_int_rect：
    // floor(x)+max(1,ceil(w))，subregion 相位以 region 原点为基准）。wrap
    // 模数与 edge 边界吃的是这个整数格，直接用 f32 值会差半列。
    let irx0 = region.x0.floor() as f32;
    let iry0 = region.y0.floor() as f32;
    let irw = (region.x1 - region.x0).ceil().max(1.0) as f32;
    let irh = (region.y1 - region.y0).ceil().max(1.0) as f32;
    let (rx0, ry0) = (irx0, iry0);
    let (rx1, ry1) = (irx0 + irw, iry0 + irh);
    let mut s = String::from(
        "uniform shader child;\nhalf4 main(float2 coord) {\n    float3 acc = float3(0.0);\n    float acca = 0.0;\n",
    );
    for oy in 0..rows {
        for ox in 0..cols {
            let k = f(kernel[(rows - oy - 1) * cols + (cols - ox - 1)]);
            let dx = f((ox as i32 - target.0) as f32);
            let dy = f((oy as i32 - target.1) as f32);
            // t = 采样坐标（region 系整数格）；eval 半像素对齐像素中心。
            // preserve_alpha 时 resvg 先 demultiply 再卷积（unpremul 域）。
            let unpre = if preserve_alpha {
                "float3 u = c.a > 0.0 ? c.rgb / c.a : float3(0.0);\n        "
            } else {
                ""
            };
            let chan = if preserve_alpha { "u" } else { "c.rgb" };
            let t = match edge {
                1 => format!(
                    "float2 t = clamp(floor(coord) + float2({dx}, {dy}), float2({}, {}), float2({}, {}));",
                    f(rx0),
                    f(ry0),
                    f(rx1 - 1.0),
                    f(ry1 - 1.0)
                ),
                2 => format!(
                    "float2 t = mod(floor(coord) + float2({dx}, {dy}) - float2({}, {}), float2({}, {})) + float2({}, {});",
                    f(rx0),
                    f(ry0),
                    f(rx1 - rx0),
                    f(ry1 - ry0),
                    f(rx0),
                    f(ry0)
                ),
                _ => format!(
                    "float2 t = floor(coord) + float2({dx}, {dy});\n        if (t.x >= {} && t.x <= {} && t.y >= {} && t.y <= {})",
                    f(rx0),
                    f(rx1 - 1.0),
                    f(ry0),
                    f(ry1 - 1.0)
                ),
            };
            s.push_str(&format!(
                "    {{ {t} {{\n        float4 c = child.eval(t + float2(0.5));\n        {unpre}acc += {chan} * {k};\n        acca += c.a * {k};\n    }} }}\n"
            ));
        }
    }
    if preserve_alpha {
        s.push_str("    float na = child.eval(coord).a;\n");
    } else {
        s.push_str("    float na = acca / ");
        s.push_str(&f(divisor));
        s.push_str(" + ");
        s.push_str(&f(bias));
        s.push_str(";\n");
    }
    s.push_str("    float ba = clamp(na, 0.0, 1.0);\n    float3 rgb = acc / ");
    s.push_str(&f(divisor));
    s.push_str(" + ");
    s.push_str(&f(bias));
    s.push_str(" * na;\n");
    if preserve_alpha {
        s.push_str("    rgb = clamp(rgb, 0.0, 1.0) * ba;\n");
    } else {
        s.push_str("    rgb = clamp(rgb, 0.0, ba);\n");
    }
    s.push_str("    return half4(rgb, ba);\n}");

    let effect = match skia_safe::RuntimeEffect::make_for_shader(&s, None) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("[sksl-err] {e:?}
----
{s}");
            return None;
        }
    };
    let builder = skia_safe::runtime_effect::RuntimeShaderBuilder::new(effect);
    let rf = image_filters::runtime_shader(&builder, "child", input)?;
    match crop_rect {
        Some(r) => image_filters::crop(r, SkTileMode::Decal, rf),
        None => Some(rf),
    }
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

