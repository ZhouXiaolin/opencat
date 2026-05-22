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
    Dash {
        intervals: Vec<f32>,
        phase: f32,
    },
}

impl PartialEq for PathEffectSpec {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PathEffectSpec::Dash { intervals: a_i, phase: a_p }, PathEffectSpec::Dash { intervals: b_i, phase: b_p }) => {
                a_i.len() == b_i.len()
                    && a_i.iter().zip(b_i.iter()).all(|(a, b)| a.to_bits() == b.to_bits())
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
            (MaskFilterSpec::Blur { sigma: a_s, style: a_st, respect_ctm: a_rc },
             MaskFilterSpec::Blur { sigma: b_s, style: b_st, respect_ctm: b_rc }) => {
                a_s.to_bits() == b_s.to_bits() && a_st == b_st && a_rc == b_rc
            }
        }
    }
}
impl Eq for MaskFilterSpec {}
impl Hash for MaskFilterSpec {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            MaskFilterSpec::Blur { sigma, style, respect_ctm } => {
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
            (ColorFilterSpec::BlendColor { color: ac, mode: am }, ColorFilterSpec::BlendColor { color: bc, mode: bm }) => {
                ac.iter().zip(bc.iter()).all(|(a, b)| a.to_bits() == b.to_bits()) && am == bm
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
            (ImageFilterSpec::Blur { sigma_x: ax, sigma_y: ay, crop_rect: ar },
             ImageFilterSpec::Blur { sigma_x: bx, sigma_y: by, crop_rect: br }) => {
                ax.to_bits() == bx.to_bits() && ay.to_bits() == by.to_bits()
                    && match (ar, br) {
                        (Some(a), Some(b)) => a.x0.to_bits() == b.x0.to_bits()
                            && a.y0.to_bits() == b.y0.to_bits()
                            && a.x1.to_bits() == b.x1.to_bits()
                            && a.y1.to_bits() == b.y1.to_bits(),
                        (None, None) => true,
                        _ => false,
                    }
            }
            (ImageFilterSpec::DropShadow { dx: ax, dy: ay, sigma_x: asx, sigma_y: asy, color: ac },
             ImageFilterSpec::DropShadow { dx: bx, dy: by, sigma_x: bsx, sigma_y: bsy, color: bc }) => {
                ax.to_bits() == bx.to_bits()
                    && ay.to_bits() == by.to_bits()
                    && asx.to_bits() == bsx.to_bits()
                    && asy.to_bits() == bsy.to_bits()
                    && ac.iter().zip(bc.iter()).all(|(a, b)| a.to_bits() == b.to_bits())
            }
            (ImageFilterSpec::ColorFilter(a), ImageFilterSpec::ColorFilter(b)) => a == b,
            (ImageFilterSpec::Compose(a1, a2), ImageFilterSpec::Compose(b1, b2)) => a1 == b1 && a2 == b2,
            _ => false,
        }
    }
}
impl Eq for ImageFilterSpec {}
impl Hash for ImageFilterSpec {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            ImageFilterSpec::Blur { sigma_x, sigma_y, crop_rect } => {
                sigma_x.to_bits().hash(state);
                sigma_y.to_bits().hash(state);
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
            ImageFilterSpec::DropShadow { dx, dy, sigma_x, sigma_y, color } => {
                dx.to_bits().hash(state);
                dy.to_bits().hash(state);
                sigma_x.to_bits().hash(state);
                sigma_y.to_bits().hash(state);
                color.iter().for_each(|v| v.to_bits().hash(state));
            }
            ImageFilterSpec::ColorFilter(cf) => cf.hash(state),
            ImageFilterSpec::Compose(a, b) => {
                a.hash(state);
                b.hash(state);
            }
        }
    }
}

impl PartialEq for ShaderSpec {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ShaderSpec::LinearGradient { from: af, to: at, stops: as_stops, colors: ac, tile_mode: atm },
             ShaderSpec::LinearGradient { from: bf, to: bt, stops: bs_stops, colors: bc, tile_mode: btm }) => {
                af[0].to_bits() == bf[0].to_bits()
                    && af[1].to_bits() == bf[1].to_bits()
                    && at[0].to_bits() == bt[0].to_bits()
                    && at[1].to_bits() == bt[1].to_bits()
                    && as_stops.len() == bs_stops.len()
                    && as_stops.iter().zip(bs_stops.iter()).all(|(a, b)| a.to_bits() == b.to_bits())
                    && ac.len() == bc.len()
                    && ac.iter().zip(bc.iter()).all(|(ca, cb)| {
                        ca.iter().zip(cb.iter()).all(|(a, b)| a.to_bits() == b.to_bits())
                    })
                    && atm == btm
            }
            (ShaderSpec::RadialGradient { center: ac, radius: ar, stops: as_stops, colors: acol, tile_mode: atm },
             ShaderSpec::RadialGradient { center: bc, radius: br, stops: bs_stops, colors: bcol, tile_mode: btm }) => {
                ac[0].to_bits() == bc[0].to_bits()
                    && ac[1].to_bits() == bc[1].to_bits()
                    && ar.to_bits() == br.to_bits()
                    && as_stops.len() == bs_stops.len()
                    && as_stops.iter().zip(bs_stops.iter()).all(|(a, b)| a.to_bits() == b.to_bits())
                    && acol.len() == bcol.len()
                    && acol.iter().zip(bcol.iter()).all(|(ca, cb)| {
                        ca.iter().zip(cb.iter()).all(|(a, b)| a.to_bits() == b.to_bits())
                    })
                    && atm == btm
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
            ShaderSpec::LinearGradient { from, to, stops, colors, tile_mode } => {
                from[0].to_bits().hash(state);
                from[1].to_bits().hash(state);
                to[0].to_bits().hash(state);
                to[1].to_bits().hash(state);
                stops.iter().for_each(|v| v.to_bits().hash(state));
                colors.iter().for_each(|c| c.iter().for_each(|v| v.to_bits().hash(state)));
                tile_mode.hash(state);
            }
            ShaderSpec::RadialGradient { center, radius, stops, colors, tile_mode } => {
                center[0].to_bits().hash(state);
                center[1].to_bits().hash(state);
                radius.to_bits().hash(state);
                stops.iter().for_each(|v| v.to_bits().hash(state));
                colors.iter().for_each(|c| c.iter().for_each(|v| v.to_bits().hash(state)));
                tile_mode.hash(state);
            }
        }
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
            (FillSpec::Solid(a), FillSpec::Solid(b)) => {
                a.iter().zip(b.iter()).all(|(x, y)| x.to_bits() == y.to_bits())
            }
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
