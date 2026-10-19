use std::hash::{Hash, Hasher};

use super::Rect;

#[derive(Clone, Debug, Default)]
pub struct PaintSpec {
    pub fill: FillSpec,
    pub style: PaintStyle,
    pub stroke: Option<StrokeSpec>,
    pub anti_alias: bool,
    pub blend_mode: BlendMode,
    pub image_filter: Option<ImageFilterSpec>,
    pub color_filter: Option<ColorFilterSpec>,
    pub mask_filter: Option<MaskFilterSpec>,
    pub path_effect: Option<PathEffectSpec>,
}

#[derive(Clone, Debug)]
pub enum FillSpec {
    Solid([f32; 4]),
    Shader(ShaderSpec),
}

impl Default for FillSpec {
    fn default() -> Self {
        FillSpec::Solid([0.0; 4])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PaintStyle {
    #[default]
    Fill,
    Stroke,
}

#[derive(Clone, Debug)]
pub struct StrokeSpec {
    pub width: f32,
    pub cap: StrokeCap,
    pub join: StrokeJoin,
    pub miter_limit: f32,
}

impl Default for StrokeSpec {
    fn default() -> Self {
        StrokeSpec {
            width: 1.0,
            cap: StrokeCap::Butt,
            join: StrokeJoin::Miter,
            miter_limit: 4.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StrokeCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StrokeJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BlendMode {
    Clear,
    Src,
    Dst,
    #[default]
    SrcOver,
    DstOver,
    SrcIn,
    DstIn,
    SrcOut,
    DstOut,
    SrcATop,
    DstATop,
    Xor,
    Plus,
    Modulate,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Multiply,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

#[derive(Clone, Debug)]
pub enum ImageFilterSpec {
    Blur {
        sigma_x: f32,
        sigma_y: f32,
        crop_rect: Option<Rect>,
        /// Tile the blur input transparently past its bounds (CSS
        /// `filter: blur()` semantics, where content outside the element
        /// is transparent). `false` = `Clamp` (backdrop blur, where the
        /// backdrop edge should extend past the snapshot).
        decal: bool,
    },
    DropShadow {
        dx: f32,
        dy: f32,
        sigma_x: f32,
        sigma_y: f32,
        color: [f32; 4],
        /// `true` = Skia `drop_shadow`（输出阴影 **并** 保留输入内容，
        /// 即 CSS `filter: drop-shadow()` 语义）；`false` = `drop_shadow_only`
        /// （仅阴影；配合外层单独绘制内容）。多个 CSS 阴影的链式语义由
        /// 渲染层用嵌套 `SaveLayer` 表达：内层输出（影+内容）成为外层
        /// filter 的输入。
        keep_content: bool,
    },
    ColorFilter(Box<ColorFilterSpec>),
    Compose(Box<ImageFilterSpec>, Box<ImageFilterSpec>),
    /// SVG `<filter>` 全链（P4）：primitive 扁平表 + result 索引，engine 侧
    /// 按 in/in2 引用递归展开成 skia filter 树（共享子链重复展开，语义
    /// 等价——各输入独立从层内容起算）。多元素为 `filter="url(#a) url(#b)"`
    /// 级联：按序求值，前链输出作为后链的 SourceGraphic。
    Chain(Vec<FilterChainSpec>),
}

/// SVG filter 的一个 primitive（feXxx）。
///
/// PartialEq 走派生（f32 均为解析自 SVG 文本的有限值，无 NaN/-0.0 分歧）。
#[derive(Clone, Debug, PartialEq)]
pub struct FilterChainSpec {
    pub primitives: Vec<FilterPrimSpec>,
    /// 最终输出 primitive 的下标。
    pub output: u16,
    /// filter region（`<filter x/y/width/height>`，usvg 归一后的 user space）。
    /// resvg 把 source 光栅化进 region 大小的 pixmap——SourceGraphic 先裁到
    /// region（region 外透明），blur/卷积的边界衰减由此而来。
    pub source_crop: Option<Rect>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FilterPrimSpec {
    /// `x/y/width/height`（primitive subregion，usvg 归一后的 user space）。
    pub crop: Option<Rect>,
    /// `color-interpolation-filters="sRGB"`。false = linearRGB（SVG 默认），
    /// engine 在空间切换处插 gamma color filter（resvg 同语义）。
    pub srgb: bool,
    pub kind: FilterPrimKind,
}

/// primitive 的输入引用（`in`/`in2`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterInput {
    SourceGraphic,
    SourceAlpha,
    /// primitives 表的下标（只允许向后引用）。
    Prim(u16),
}

#[derive(Clone, Debug, PartialEq)]
pub enum FilterPrimKind {
    Blend {
        mode: BlendMode,
        in1: FilterInput,
        in2: FilterInput,
    },
    ColorMatrix {
        row_major: [f32; 20],
        input: FilterInput,
    },
    /// feComponentTransfer：四通道函数统一展开成 256 查表
    /// （engine 走 color_filters::table_argb）。
    ComponentTransfer {
        fns: [TransferFn; 4],
        input: FilterInput,
    },
    /// op：0 over / 1 in / 2 out / 3 atop / 4 xor / 5 lighter / 6 arithmetic。
    Composite {
        op: u8,
        k: [f32; 4],
        in1: FilterInput,
        in2: FilterInput,
    },
    /// feConvolveMatrix：edge 0 none / 1 duplicate / 2 wrap。
    ConvolveMatrix {
        order: (u32, u32),
        kernel: Vec<f32>,
        divisor: f32,
        bias: f32,
        target: (i32, i32),
        kernel_unit_length: (f32, f32),
        edge: u8,
        preserve_alpha: bool,
        input: FilterInput,
    },
    DiffuseLighting {
        surface_scale: f32,
        diffuse_constant: f32,
        lighting_color: [f32; 4],
        light: LightSpec,
        input: FilterInput,
    },
    DisplacementMap {
        x_channel: u8,
        y_channel: u8,
        scale: f32,
        input: FilterInput,
        in2: FilterInput,
    },
    DropShadow {
        dx: f32,
        dy: f32,
        sigma_x: f32,
        sigma_y: f32,
        color: [f32; 4],
        input: FilterInput,
    },
    Flood {
        color: [f32; 4],
    },
    GaussianBlur {
        sigma_x: f32,
        sigma_y: f32,
        input: FilterInput,
    },
    Merge {
        inputs: Vec<FilterInput>,
    },
    Morphology {
        erode: bool,
        radius_x: f32,
        radius_y: f32,
        input: FilterInput,
    },
    Offset {
        dx: f32,
        dy: f32,
        input: FilterInput,
    },
    SpecularLighting {
        surface_scale: f32,
        specular_constant: f32,
        specular_exponent: f32,
        lighting_color: [f32; 4],
        light: LightSpec,
        input: FilterInput,
    },
    Tile {
        input: FilterInput,
    },
    /// feImage：引用子树独立光栅化后作为 prim 输出。`subtree` 由 emit 侧以
    /// 恒等 TS 录制（内容坐标 = 文档 user space）；engine 端把子树渲染进
    /// region 大小的图（平移 -region.xy），再原样放回 region 位置。resvg
    /// 的放置矩阵 scale 分量在无旋转组下与 CTM 相消，等价 1:1 平移放置。
    Image {
        subtree: u32,
    },
    /// feTurbulence：fractal=true 是 fractalNoise，false 是 turbulence。
    Turbulence {
        fractal: bool,
        base_frequency: [f32; 2],
        num_octaves: u32,
        seed: f32,
        stitch: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LightSpec {
    Distant { azimuth: f32, elevation: f32 },
    Point { x: f32, y: f32, z: f32 },
    Spot {
        x: f32,
        y: f32,
        z: f32,
        points_at_x: f32,
        points_at_y: f32,
        points_at_z: f32,
        specular_exponent: f32,
        limiting_cone_angle: f32,
    },
}

/// feComponentTransfer 的通道函数（feFuncR/G/B/A 的 type）。
#[derive(Clone, Debug, PartialEq)]
pub enum TransferFn {
    Identity,
    Table(Vec<f32>),
    Discrete(Vec<f32>),
    Linear { slope: f32, intercept: f32 },
    Gamma { amplitude: f32, exponent: f32, offset: f32 },
}

#[derive(Clone, Debug)]
pub enum ColorFilterSpec {
    Matrix([f32; 20]),
    BlendColor { color: [f32; 4], mode: BlendMode },
    LinearToSrgbGamma,
    SrgbToLinearGamma,
}

#[derive(Clone, Debug)]
pub enum ShaderSpec {
    LinearGradient {
        from: [f32; 2],
        to: [f32; 2],
        stops: Vec<f32>,
        colors: Vec<[f32; 4]>,
        tile_mode: TileMode,
        local_matrix: Option<[f32; 9]>,
        /// 停靠点插值空间。CSS Images 4（JSX 渐变）要求 premul；SVG/resvg
        /// 是 unpremul（stop-opacity 渐变两端 α 不等时中间色不同）。
        interpolate_premul: bool,
    },
    RadialGradient {
        center: [f32; 2],
        radius: f32,
        stops: Vec<f32>,
        colors: Vec<[f32; 4]>,
        tile_mode: TileMode,
        local_matrix: Option<[f32; 9]>,
        /// 同 [`ShaderSpec::LinearGradient::interpolate_premul`]。
        interpolate_premul: bool,
        /// SVG 焦点 `(fx, fy, fr)`（usvg 已归一化到 user space）。`None`
        /// 表示无焦点偏移，引擎走普通单点 radial；`Some` 走两点锥形
        /// （焦点圆 → 主圆，与 tiny-skia `RadialGradient` 同语义）。
        focal: Option<[f32; 3]>,
    },
    /// SVG `feTurbulence` fractal-noise 场。`tile` 为 `Some([w, h])` 时引擎先
    /// 烘一块瓦片再以 Repeat 平铺（CSS background 语义），瓦片锚定在 `origin`
    /// （节点盒原点）；`None` 时整个盒子直接铺未拼接的噪声场。
    Turbulence {
        base_frequency: [f32; 2],
        num_octaves: u32,
        seed: u32,
        tile: Option<[f32; 2]>,
        origin: [f32; 2],
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TileMode {
    Clamp,
    Repeat,
    Mirror,
    Decal,
}

#[derive(Clone, Debug)]
pub enum MaskFilterSpec {
    Blur {
        sigma: f32,
        style: BlurStyle,
        respect_ctm: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlurStyle {
    Normal,
    Inner,
    Solid,
    Outer,
}

#[derive(Clone, Debug)]
pub enum PathEffectSpec {
    Dash { intervals: Vec<f32>, phase: f32 },
}

impl PartialEq for PathEffectSpec {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                PathEffectSpec::Dash {
                    intervals: a_i,
                    phase: a_p,
                },
                PathEffectSpec::Dash {
                    intervals: b_i,
                    phase: b_p,
                },
            ) => {
                a_i.len() == b_i.len()
                    && a_i
                        .iter()
                        .zip(b_i.iter())
                        .all(|(a, b)| a.to_bits() == b.to_bits())
                    && a_p.to_bits() == b_p.to_bits()
            }
        }
    }
}
impl Eq for PathEffectSpec {}
impl Hash for PathEffectSpec {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            PathEffectSpec::Dash { intervals, phase } => {
                intervals.iter().for_each(|v| v.to_bits().hash(state));
                phase.to_bits().hash(state);
            }
        }
    }
}

impl PartialEq for MaskFilterSpec {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                MaskFilterSpec::Blur {
                    sigma: a_s,
                    style: a_st,
                    respect_ctm: a_rc,
                },
                MaskFilterSpec::Blur {
                    sigma: b_s,
                    style: b_st,
                    respect_ctm: b_rc,
                },
            ) => a_s.to_bits() == b_s.to_bits() && a_st == b_st && a_rc == b_rc,
        }
    }
}
impl Eq for MaskFilterSpec {}
impl Hash for MaskFilterSpec {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            MaskFilterSpec::Blur {
                sigma,
                style,
                respect_ctm,
            } => {
                sigma.to_bits().hash(state);
                style.hash(state);
                respect_ctm.hash(state);
            }
        }
    }
}

impl PartialEq for ColorFilterSpec {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ColorFilterSpec::Matrix(a), ColorFilterSpec::Matrix(b)) => a == b,
            (
                ColorFilterSpec::BlendColor {
                    color: ac,
                    mode: am,
                },
                ColorFilterSpec::BlendColor {
                    color: bc,
                    mode: bm,
                },
            ) => {
                ac.iter()
                    .zip(bc.iter())
                    .all(|(a, b)| a.to_bits() == b.to_bits())
                    && am == bm
            }
            (ColorFilterSpec::LinearToSrgbGamma, ColorFilterSpec::LinearToSrgbGamma) => true,
            (ColorFilterSpec::SrgbToLinearGamma, ColorFilterSpec::SrgbToLinearGamma) => true,
            _ => false,
        }
    }
}
impl Eq for ColorFilterSpec {}
impl Hash for ColorFilterSpec {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            ColorFilterSpec::Matrix(m) => m.iter().for_each(|v| v.to_bits().hash(state)),
            ColorFilterSpec::BlendColor { color, mode } => {
                color.iter().for_each(|v| v.to_bits().hash(state));
                mode.hash(state);
            }
            ColorFilterSpec::LinearToSrgbGamma | ColorFilterSpec::SrgbToLinearGamma => {}
        }
    }
}

impl PartialEq for ImageFilterSpec {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                ImageFilterSpec::Blur {
                    sigma_x: ax,
                    sigma_y: ay,
                    crop_rect: ar,
                    decal: adecal,
                },
                ImageFilterSpec::Blur {
                    sigma_x: bx,
                    sigma_y: by,
                    crop_rect: br,
                    decal: bdecal,
                },
            ) => {
                ax.to_bits() == bx.to_bits()
                    && ay.to_bits() == by.to_bits()
                    && adecal == bdecal
                    && match (ar, br) {
                        (Some(a), Some(b)) => {
                            a.x0.to_bits() == b.x0.to_bits()
                                && a.y0.to_bits() == b.y0.to_bits()
                                && a.x1.to_bits() == b.x1.to_bits()
                                && a.y1.to_bits() == b.y1.to_bits()
                        }
                        (None, None) => true,
                        _ => false,
                    }
            }
            (
                ImageFilterSpec::DropShadow {
                    dx: ax,
                    dy: ay,
                    sigma_x: asx,
                    sigma_y: asy,
                    color: ac,
                    keep_content: akc,
                },
                ImageFilterSpec::DropShadow {
                    dx: bx,
                    dy: by,
                    sigma_x: bsx,
                    sigma_y: bsy,
                    color: bc,
                    keep_content: bkc,
                },
            ) => {
                ax.to_bits() == bx.to_bits()
                    && ay.to_bits() == by.to_bits()
                    && asx.to_bits() == bsx.to_bits()
                    && asy.to_bits() == bsy.to_bits()
                    && ac
                        .iter()
                        .zip(bc.iter())
                        .all(|(a, b)| a.to_bits() == b.to_bits())
                    && akc == bkc
            }
            (ImageFilterSpec::ColorFilter(a), ImageFilterSpec::ColorFilter(b)) => a == b,
            (ImageFilterSpec::Compose(a1, a2), ImageFilterSpec::Compose(b1, b2)) => {
                a1 == b1 && a2 == b2
            }
            (ImageFilterSpec::Chain(a), ImageFilterSpec::Chain(b)) => a == b,
            _ => false,
        }
    }
}
impl Eq for ImageFilterSpec {}
impl Hash for ImageFilterSpec {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            ImageFilterSpec::Blur {
                sigma_x,
                sigma_y,
                crop_rect,
                decal,
            } => {
                sigma_x.to_bits().hash(state);
                sigma_y.to_bits().hash(state);
                decal.hash(state);
                if let Some(r) = crop_rect {
                    1u8.hash(state);
                    r.x0.to_bits().hash(state);
                    r.y0.to_bits().hash(state);
                    r.x1.to_bits().hash(state);
                    r.y1.to_bits().hash(state);
                } else {
                    0u8.hash(state);
                }
            }
            ImageFilterSpec::DropShadow {
                dx,
                dy,
                sigma_x,
                sigma_y,
                color,
                keep_content,
            } => {
                dx.to_bits().hash(state);
                dy.to_bits().hash(state);
                sigma_x.to_bits().hash(state);
                sigma_y.to_bits().hash(state);
                color.iter().for_each(|v| v.to_bits().hash(state));
                keep_content.hash(state);
            }
            ImageFilterSpec::ColorFilter(cf) => cf.hash(state),
            ImageFilterSpec::Compose(a, b) => {
                a.hash(state);
                b.hash(state);
            }
            // 链较长且 intern 重复率低，长度占位即可（eq→hash 契约仍成立，
            // 不同链同桶只多一次线性探测）。
            ImageFilterSpec::Chain(c) => c.len().hash(state),
        }
    }
}

impl PartialEq for ShaderSpec {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                ShaderSpec::LinearGradient {
                    from: af,
                    to: at,
                    stops: as_stops,
                    colors: ac,
                    tile_mode: atm,
                    local_matrix: alm,
                    interpolate_premul: aip,
                },
                ShaderSpec::LinearGradient {
                    from: bf,
                    to: bt,
                    stops: bs_stops,
                    colors: bc,
                    tile_mode: btm,
                    local_matrix: blm,
                    interpolate_premul: bip,
                },
            ) => {
                af[0].to_bits() == bf[0].to_bits()
                    && af[1].to_bits() == bf[1].to_bits()
                    && at[0].to_bits() == bt[0].to_bits()
                    && at[1].to_bits() == bt[1].to_bits()
                    && as_stops.len() == bs_stops.len()
                    && as_stops
                        .iter()
                        .zip(bs_stops.iter())
                        .all(|(a, b)| a.to_bits() == b.to_bits())
                    && ac.len() == bc.len()
                    && ac.iter().zip(bc.iter()).all(|(ca, cb)| {
                        ca.iter()
                            .zip(cb.iter())
                            .all(|(a, b)| a.to_bits() == b.to_bits())
                    })
                    && atm == btm
                    && aip == bip
                    && matrix_eq(alm, blm)
            }
            (
                ShaderSpec::RadialGradient {
                    center: ac,
                    radius: ar,
                    stops: as_stops,
                    colors: acol,
                    tile_mode: atm,
                    local_matrix: alm,
                    interpolate_premul: aip,
                    focal: afo,
                },
                ShaderSpec::RadialGradient {
                    center: bc,
                    radius: br,
                    stops: bs_stops,
                    colors: bcol,
                    tile_mode: btm,
                    local_matrix: blm,
                    interpolate_premul: bip,
                    focal: bfo,
                },
            ) => {
                ac[0].to_bits() == bc[0].to_bits()
                    && ac[1].to_bits() == bc[1].to_bits()
                    && ar.to_bits() == br.to_bits()
                    && as_stops.len() == bs_stops.len()
                    && as_stops
                        .iter()
                        .zip(bs_stops.iter())
                        .all(|(a, b)| a.to_bits() == b.to_bits())
                    && acol.len() == bcol.len()
                    && acol.iter().zip(bcol.iter()).all(|(ca, cb)| {
                        ca.iter()
                            .zip(cb.iter())
                            .all(|(a, b)| a.to_bits() == b.to_bits())
                    })
                    && atm == btm
                    && aip == bip
                    && focal_eq(afo, bfo)
                    && matrix_eq(alm, blm)
            }
            (
                ShaderSpec::Turbulence {
                    base_frequency: af,
                    num_octaves: ao,
                    seed: asd,
                    tile: at,
                    origin: aorig,
                },
                ShaderSpec::Turbulence {
                    base_frequency: bf,
                    num_octaves: bo,
                    seed: bsd,
                    tile: bt,
                    origin: borig,
                },
            ) => {
                af[0].to_bits() == bf[0].to_bits()
                    && af[1].to_bits() == bf[1].to_bits()
                    && ao == bo
                    && asd == bsd
                    && match (at, bt) {
                        (Some(a), Some(b)) => {
                            a[0].to_bits() == b[0].to_bits() && a[1].to_bits() == b[1].to_bits()
                        }
                        (None, None) => true,
                        _ => false,
                    }
                    && aorig[0].to_bits() == borig[0].to_bits()
                    && aorig[1].to_bits() == borig[1].to_bits()
            }
            _ => false,
        }
    }
}
impl Eq for ShaderSpec {}
impl Hash for ShaderSpec {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            ShaderSpec::LinearGradient {
                from,
                to,
                stops,
                colors,
                tile_mode,
                local_matrix,
                interpolate_premul,
            } => {
                from[0].to_bits().hash(state);
                from[1].to_bits().hash(state);
                to[0].to_bits().hash(state);
                to[1].to_bits().hash(state);
                stops.iter().for_each(|v| v.to_bits().hash(state));
                colors
                    .iter()
                    .for_each(|c| c.iter().for_each(|v| v.to_bits().hash(state)));
                tile_mode.hash(state);
                interpolate_premul.hash(state);
                matrix_hash(local_matrix, state);
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
                center[0].to_bits().hash(state);
                center[1].to_bits().hash(state);
                radius.to_bits().hash(state);
                stops.iter().for_each(|v| v.to_bits().hash(state));
                colors
                    .iter()
                    .for_each(|c| c.iter().for_each(|v| v.to_bits().hash(state)));
                tile_mode.hash(state);
                interpolate_premul.hash(state);
                focal.is_some().hash(state);
                if let Some(f) = focal {
                    f.iter().for_each(|v| v.to_bits().hash(state));
                }
                matrix_hash(local_matrix, state);
            }
            ShaderSpec::Turbulence {
                base_frequency,
                num_octaves,
                seed,
                tile,
                origin,
            } => {
                base_frequency
                    .iter()
                    .for_each(|v| v.to_bits().hash(state));
                num_octaves.hash(state);
                seed.hash(state);
                if let Some(t) = tile {
                    t[0].to_bits().hash(state);
                    t[1].to_bits().hash(state);
                }
                tile.is_some().hash(state);
                origin.iter().for_each(|v| v.to_bits().hash(state));
            }
        }
    }
}

fn focal_eq(a: &Option<[f32; 3]>, b: &Option<[f32; 3]>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a
            .iter()
            .zip(b.iter())
            .all(|(x, y)| x.to_bits() == y.to_bits()),
        _ => false,
    }
}

fn matrix_eq(a: &Option<[f32; 9]>, b: &Option<[f32; 9]>) -> bool {    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a
            .iter()
            .zip(b.iter())
            .all(|(x, y)| x.to_bits() == y.to_bits()),
        _ => false,
    }
}

fn matrix_hash<H: Hasher>(matrix: &Option<[f32; 9]>, state: &mut H) {
    matrix.is_some().hash(state);
    if let Some(m) = matrix {
        m.iter().for_each(|v| v.to_bits().hash(state));
    }
}

impl PartialEq for StrokeSpec {
    fn eq(&self, other: &Self) -> bool {
        self.width.to_bits() == other.width.to_bits()
            && self.cap == other.cap
            && self.join == other.join
            && self.miter_limit.to_bits() == other.miter_limit.to_bits()
    }
}
impl Eq for StrokeSpec {}
impl Hash for StrokeSpec {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.width.to_bits().hash(state);
        self.cap.hash(state);
        self.join.hash(state);
        self.miter_limit.to_bits().hash(state);
    }
}

impl PartialEq for FillSpec {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (FillSpec::Solid(a), FillSpec::Solid(b)) => a
                .iter()
                .zip(b.iter())
                .all(|(x, y)| x.to_bits() == y.to_bits()),
            (FillSpec::Shader(a), FillSpec::Shader(b)) => a == b,
            _ => false,
        }
    }
}
impl Eq for FillSpec {}
impl Hash for FillSpec {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            FillSpec::Solid(c) => {
                c.iter().for_each(|v| v.to_bits().hash(state));
            }
            FillSpec::Shader(s) => s.hash(state),
        }
    }
}

impl PartialEq for PaintSpec {
    fn eq(&self, other: &Self) -> bool {
        self.fill == other.fill
            && self.style == other.style
            && self.stroke == other.stroke
            && self.anti_alias == other.anti_alias
            && self.blend_mode == other.blend_mode
            && self.image_filter == other.image_filter
            && self.color_filter == other.color_filter
            && self.mask_filter == other.mask_filter
            && self.path_effect == other.path_effect
    }
}
impl Eq for PaintSpec {}
impl Hash for PaintSpec {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.fill.hash(state);
        self.style.hash(state);
        self.stroke.hash(state);
        self.anti_alias.hash(state);
        self.blend_mode.hash(state);
        self.image_filter.hash(state);
        self.color_filter.hash(state);
        self.mask_filter.hash(state);
        self.path_effect.hash(state);
    }
}
