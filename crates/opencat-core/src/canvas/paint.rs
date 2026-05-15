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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrokeCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrokeJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Clear,
    Src,
    Dst,
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

impl Default for BlendMode {
    fn default() -> Self {
        BlendMode::SrcOver
    }
}

#[derive(Clone, Debug)]
pub enum ImageFilterSpec {
    Blur {
        sigma_x: f32,
        sigma_y: f32,
        crop_rect: Option<Rect>,
    },
    DropShadow {
        dx: f32,
        dy: f32,
        sigma_x: f32,
        sigma_y: f32,
        color: [f32; 4],
    },
    ColorFilter(Box<ColorFilterSpec>),
    Compose(Box<ImageFilterSpec>, Box<ImageFilterSpec>),
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
    },
    RadialGradient {
        center: [f32; 2],
        radius: f32,
        stops: Vec<f32>,
        colors: Vec<[f32; 4]>,
        tile_mode: TileMode,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlurStyle {
    Normal,
    Inner,
    Solid,
    Outer,
}

#[derive(Clone, Debug)]
pub enum PathEffectSpec {
    Dash {
        intervals: Vec<f32>,
        phase: f32,
    },
}
