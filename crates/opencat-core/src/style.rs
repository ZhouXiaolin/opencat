use std::{hash::Hash, sync::Arc};

use crate::script::ScriptDriver;

include!(concat!(env!("OUT_DIR"), "/tailwind_color_items.rs"));

/// Position mode - Tailwind: relative, absolute
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum Position {
    #[default]
    Relative,
    Absolute,
}

/// Flex direction - Tailwind: flex-row, flex-col
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum FlexDirection {
    #[default]
    Row,
    Col,
    RowReverse,
    ColReverse,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

/// Main axis alignment - Tailwind: justify-*
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum JustifyContent {
    #[default]
    Start,
    Center,
    End,
    Between,
    Around,
    Evenly,
    Stretch,
}

/// Cross axis alignment - Tailwind: items-*
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum AlignItems {
    Start,
    Center,
    End,
    Baseline,
    #[default]
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LengthPercentage {
    Length(f32),
    Percent(f32),
}

impl LengthPercentage {
    pub const fn length(value: f32) -> Self {
        Self::Length(value)
    }

    pub const fn percent(value: f32) -> Self {
        Self::Percent(value)
    }
}

impl std::hash::Hash for LengthPercentage {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Length(value) => {
                0_u8.hash(state);
                value.to_bits().hash(state);
            }
            Self::Percent(value) => {
                1_u8.hash(state);
                value.to_bits().hash(state);
            }
        }
    }
}

impl serde::Serialize for LengthPercentage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format_length_percentage(*self))
    }
}

impl<'de> serde::Deserialize<'de> for LengthPercentage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = <String as serde::Deserialize>::deserialize(deserializer)?;
        parse_length_percentage_token(&raw)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid length percentage `{raw}`")))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClipPath {
    Inset(ClipInset),
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipInset {
    pub top: LengthPercentage,
    pub right: LengthPercentage,
    pub bottom: LengthPercentage,
    pub left: LengthPercentage,
}

impl std::hash::Hash for ClipPath {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Inset(inset) => {
                0_u8.hash(state);
                inset.hash(state);
            }
        }
    }
}

impl std::hash::Hash for ClipInset {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.top.hash(state);
        self.right.hash(state);
        self.bottom.hash(state);
        self.left.hash(state);
    }
}

impl ClipPath {
    pub fn parse_css(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.eq_ignore_ascii_case("none") {
            return None;
        }
        let inner = value
            .strip_prefix("inset(")
            .and_then(|value| value.strip_suffix(')'))?;
        let before_round = inner.split_once(" round ").map_or(inner, |(head, _)| head);
        let parts = before_round
            .split_whitespace()
            .map(parse_length_percentage_token)
            .collect::<Option<Vec<_>>>()?;
        let [top, right, bottom, left] = expand_box_shorthand(&parts)?;
        Some(Self::Inset(ClipInset {
            top,
            right,
            bottom,
            left,
        }))
    }

    pub fn to_css_string(self) -> String {
        match self {
            Self::Inset(inset) => format!(
                "inset({} {} {} {})",
                format_length_percentage(inset.top),
                format_length_percentage(inset.right),
                format_length_percentage(inset.bottom),
                format_length_percentage(inset.left)
            ),
        }
    }
}

fn expand_box_shorthand(parts: &[LengthPercentage]) -> Option<[LengthPercentage; 4]> {
    match parts {
        [all] => Some([*all, *all, *all, *all]),
        [vertical, horizontal] => Some([*vertical, *horizontal, *vertical, *horizontal]),
        [top, horizontal, bottom] => Some([*top, *horizontal, *bottom, *horizontal]),
        [top, right, bottom, left] => Some([*top, *right, *bottom, *left]),
        _ => None,
    }
}

fn parse_length_percentage_token(raw: &str) -> Option<LengthPercentage> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Some(percent) = raw.strip_suffix('%') {
        return percent
            .trim()
            .parse::<f32>()
            .ok()
            .map(|value| LengthPercentage::Percent(value / 100.0));
    }
    let px = raw.strip_suffix("px").unwrap_or(raw);
    px.trim().parse::<f32>().ok().map(LengthPercentage::Length)
}

fn format_length_percentage(value: LengthPercentage) -> String {
    match value {
        LengthPercentage::Length(value) => format_css_number_with_unit(value, "px"),
        LengthPercentage::Percent(value) => format_css_number_with_unit(value * 100.0, "%"),
    }
}

fn format_css_number_with_unit(value: f32, unit: &str) -> String {
    let number = if value.fract().abs() <= f32::EPSILON {
        format!("{}", value as i32)
    } else {
        format!("{value}")
    };
    format!("{number}{unit}")
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LengthPercentageAuto {
    Auto,
    Length(f32),
    Percent(f32),
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum GridAutoFlow {
    #[default]
    Row,
    Column,
    RowDense,
    ColumnDense,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GridAutoRows {
    Auto,
    Min,
    Max,
    Fr,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum GridPlacement {
    #[default]
    Auto,
    Line(i16),
    Span(u16),
}

impl std::hash::Hash for LengthPercentageAuto {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            LengthPercentageAuto::Auto => {}
            LengthPercentageAuto::Length(v) | LengthPercentageAuto::Percent(v) => {
                v.to_bits().hash(state);
            }
        }
    }
}

impl LengthPercentageAuto {
    pub const fn auto() -> Self {
        Self::Auto
    }

    pub const fn length(value: f32) -> Self {
        Self::Length(value)
    }

    pub const fn percent(value: f32) -> Self {
        Self::Percent(value)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum ObjectFit {
    #[default]
    Contain,
    Cover,
    Fill,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct FontWeight(pub u16);

impl FontWeight {
    pub const LIGHT: Self = Self(300);
    pub const NORMAL: Self = Self(400);
    pub const MEDIUM: Self = Self(500);
    pub const SEMIBOLD: Self = Self(600);
    pub const BOLD: Self = Self(700);
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum TextTransform {
    #[default]
    None,
    Uppercase,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BorderRadius {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl BorderRadius {
    pub const fn uniform(r: f32) -> Self {
        Self {
            top_left: r,
            top_right: r,
            bottom_right: r,
            bottom_left: r,
        }
    }
}

impl std::hash::Hash for BorderRadius {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.top_left.to_bits().hash(state);
        self.top_right.to_bits().hash(state);
        self.bottom_right.to_bits().hash(state);
        self.bottom_left.to_bits().hash(state);
    }
}

/// Border stroke style - Tailwind: border-solid / border-dashed / border-dotted
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum BorderStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoxShadowStyle {
    TwoXs,
    Xs,
    Sm,
    Base,
    Md,
    Lg,
    Xl,
    TwoXl,
    ThreeXl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DropShadowStyle {
    Xs,
    Sm,
    Base,
    Md,
    Lg,
    Xl,
    TwoXl,
    ThreeXl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InsetShadowStyle {
    TwoXs,
    Xs,
    Base,
    Sm,
    Md,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoxShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_sigma: f32,
    pub spread: f32,
    pub color: ColorToken,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsetShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_sigma: f32,
    pub spread: f32,
    pub color: ColorToken,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DropShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_sigma: f32,
    pub color: ColorToken,
}

impl std::hash::Hash for BoxShadow {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.offset_x.to_bits().hash(state);
        self.offset_y.to_bits().hash(state);
        self.blur_sigma.to_bits().hash(state);
        self.spread.to_bits().hash(state);
        self.color.hash(state);
    }
}

impl std::hash::Hash for InsetShadow {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.offset_x.to_bits().hash(state);
        self.offset_y.to_bits().hash(state);
        self.blur_sigma.to_bits().hash(state);
        self.spread.to_bits().hash(state);
        self.color.hash(state);
    }
}

impl std::hash::Hash for DropShadow {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.offset_x.to_bits().hash(state);
        self.offset_y.to_bits().hash(state);
        self.blur_sigma.to_bits().hash(state);
        self.color.hash(state);
    }
}

impl BoxShadow {
    pub fn from_style(style: BoxShadowStyle) -> Self {
        match style {
            BoxShadowStyle::TwoXs => Self::new(0.0, 1.0, 0.5 / 6.0, 0.0, 0.05),
            BoxShadowStyle::Xs => Self::new(0.0, 1.0, 2.0 / 6.0, 0.0, 0.05),
            BoxShadowStyle::Sm => Self::new(0.0, 1.0, 3.0 / 6.0, 0.0, 0.10),
            BoxShadowStyle::Base => Self::new(0.0, 1.0, 4.0 / 6.0, 0.0, 0.10),
            BoxShadowStyle::Md => Self::new(0.0, 4.0, 1.0, 0.0, 0.10),
            BoxShadowStyle::Lg => Self::new(0.0, 10.0, 15.0 / 6.0, 0.0, 0.10),
            BoxShadowStyle::Xl => Self::new(0.0, 20.0, 25.0 / 6.0, 0.0, 0.10),
            BoxShadowStyle::TwoXl => Self::new(0.0, 25.0, 50.0 / 6.0, 0.0, 0.18),
            BoxShadowStyle::ThreeXl => Self::new(0.0, 35.0, 60.0 / 6.0, 0.0, 0.22),
        }
    }

    pub const fn new(
        offset_x: f32,
        offset_y: f32,
        blur_sigma: f32,
        spread: f32,
        alpha: f32,
    ) -> Self {
        Self {
            offset_x,
            offset_y,
            blur_sigma,
            spread,
            color: shadow_color(alpha),
        }
    }

    pub const fn with_color(mut self, color: ColorToken) -> Self {
        self.color = color;
        self
    }

    pub fn outsets(self) -> (f32, f32, f32, f32) {
        shadow_outsets(
            self.blur_sigma,
            self.offset_x,
            self.offset_y,
            self.spread.max(0.0),
        )
    }
}

impl InsetShadow {
    pub fn from_style(style: InsetShadowStyle) -> Self {
        match style {
            InsetShadowStyle::TwoXs => Self::new(0.0, 1.0, 1.0 / 6.0, 0.0, 0.05),
            InsetShadowStyle::Xs => Self::new(0.0, 1.0, 2.0 / 6.0, 0.0, 0.08),
            InsetShadowStyle::Base => Self::new(0.0, 2.0, 4.0 / 6.0, 0.0, 0.10),
            InsetShadowStyle::Sm => Self::new(0.0, 2.0, 5.0 / 6.0, 0.0, 0.12),
            InsetShadowStyle::Md => Self::new(0.0, 3.0, 7.0 / 6.0, 1.0, 0.14),
        }
    }

    pub const fn new(
        offset_x: f32,
        offset_y: f32,
        blur_sigma: f32,
        spread: f32,
        alpha: f32,
    ) -> Self {
        Self {
            offset_x,
            offset_y,
            blur_sigma,
            spread,
            color: shadow_color(alpha),
        }
    }

    pub const fn with_color(mut self, color: ColorToken) -> Self {
        self.color = color;
        self
    }
}

impl DropShadow {
    pub fn from_style(style: DropShadowStyle) -> Self {
        match style {
            DropShadowStyle::Xs => Self::new(0.0, 1.0, 1.0 / 6.0, 0.05),
            DropShadowStyle::Sm => Self::new(0.0, 1.0, 2.0 / 6.0, 30.0 / 255.0),
            DropShadowStyle::Base => Self::new(0.0, 1.0, 2.0 / 6.0, 30.0 / 255.0),
            DropShadowStyle::Md => Self::new(0.0, 3.0, 4.0 / 6.0, 0.14),
            DropShadowStyle::Lg => Self::new(0.0, 6.0, 8.0 / 6.0, 0.16),
            DropShadowStyle::Xl => Self::new(0.0, 10.0, 14.0 / 6.0, 0.18),
            DropShadowStyle::TwoXl => Self::new(0.0, 16.0, 24.0 / 6.0, 0.20),
            DropShadowStyle::ThreeXl => Self::new(0.0, 24.0, 36.0 / 6.0, 0.22),
        }
    }

    pub const fn new(offset_x: f32, offset_y: f32, blur_sigma: f32, alpha: f32) -> Self {
        Self {
            offset_x,
            offset_y,
            blur_sigma,
            color: shadow_color(alpha),
        }
    }

    pub const fn with_color(mut self, color: ColorToken) -> Self {
        self.color = color;
        self
    }

    pub fn outsets(self) -> (f32, f32, f32, f32) {
        shadow_outsets(self.blur_sigma, self.offset_x, self.offset_y, 0.0)
    }
}

/// CSS `text-shadow` 阴影。与 `DropShadow` 结构一致（offset + blur + color，无 spread），
/// 但语义上是文本专属、支持多个并列（RGB-split / 多层辉光）。
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    /// 高斯模糊 σ（CSS blur-radius 经 /6 换算，与 box-shadow 一致）。
    pub blur_sigma: f32,
    pub color: ColorToken,
}

impl Eq for TextShadow {}

impl std::hash::Hash for TextShadow {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.offset_x.to_bits().hash(state);
        self.offset_y.to_bits().hash(state);
        self.blur_sigma.to_bits().hash(state);
        self.color.hash(state);
    }
}

impl TextShadow {
    pub fn outsets(self) -> (f32, f32, f32, f32) {
        shadow_outsets(self.blur_sigma, self.offset_x, self.offset_y, 0.0)
    }
}

const fn shadow_color(alpha: f32) -> ColorToken {
    ColorToken::Custom(0, 0, 0, (alpha * 255.0) as u8)
}

fn shadow_outsets(
    blur_sigma: f32,
    offset_x: f32,
    offset_y: f32,
    spread: f32,
) -> (f32, f32, f32, f32) {
    let extent = blur_sigma * 3.0;
    let left = (extent + spread - offset_x).max(0.0);
    let top = (extent + spread - offset_y).max(0.0);
    let right = (extent + spread + offset_x).max(0.0);
    let bottom = (extent + spread + offset_y).max(0.0);
    (left, top, right, bottom)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GradientDirection {
    ToRight,
    ToLeft,
    ToBottom,
    ToTop,
    ToBottomRight,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GradientStop {
    /// Stop position in 0..1 (relative to the gradient extent).
    #[serde(rename = "pos")]
    pub pos: f32,
    #[serde(rename = "color")]
    pub color: ColorToken,
}

// `GradientStop.pos` 含 `f32`，无法 derive `Eq`/`Hash`，按 `to_bits` 手动实现。
impl Eq for GradientStop {}

impl std::hash::Hash for GradientStop {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.pos.to_bits().hash(state);
        self.color.hash(state);
    }
}

/// CSS 渐变函数解析结果：支持任意色标位置与 background-size（local matrix）。
///
/// `size` 为 `Some([w, h])` 时，渲染层把渐变定义在 `[0,0]..[w,h]` 的像素空间内，
/// 并用一个把单位正方形缩放到节点 rect 的 local matrix 让该瓦片铺满节点。
/// `repeat` 为真时使用 `TileMode::Repeat`，否则 `TileMode::Clamp`。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ArbitraryGradient {
    #[serde(rename = "linear")]
    LinearGradient {
        /// CSS 角度（deg）。若提供则优先于 `direction`。
        #[serde(rename = "angleDeg")]
        angle_deg: Option<f32>,
        /// 预设方向（`to right` 等）。仅当 `angle_deg` 为 `None` 时使用。
        direction: Option<GradientDirection>,
        #[serde(rename = "stops")]
        stops: Vec<GradientStop>,
        #[serde(rename = "size")]
        size: Option<[f32; 2]>,
        #[serde(rename = "repeat")]
        repeat: bool,
    },
    #[serde(rename = "radial")]
    RadialGradient {
        /// 单位正方形内圆心 `[x, y]`。
        #[serde(rename = "center")]
        center: [f32; 2],
        #[serde(rename = "stops")]
        stops: Vec<GradientStop>,
        #[serde(rename = "size")]
        size: Option<[f32; 2]>,
        #[serde(rename = "repeat")]
        repeat: bool,
    },
}

impl std::hash::Hash for ArbitraryGradient {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            ArbitraryGradient::LinearGradient {
                angle_deg,
                direction,
                stops,
                size,
                repeat,
            } => {
                if let Some(a) = angle_deg {
                    a.to_bits().hash(state);
                }
                direction.hash(state);
                for stop in stops {
                    stop.pos.to_bits().hash(state);
                    stop.color.hash(state);
                }
                if let Some(s) = size {
                    s[0].to_bits().hash(state);
                    s[1].to_bits().hash(state);
                }
                repeat.hash(state);
            }
            ArbitraryGradient::RadialGradient {
                center,
                stops,
                size,
                repeat,
            } => {
                center[0].to_bits().hash(state);
                center[1].to_bits().hash(state);
                for stop in stops {
                    stop.pos.to_bits().hash(state);
                    stop.color.hash(state);
                }
                if let Some(s) = size {
                    s[0].to_bits().hash(state);
                    s[1].to_bits().hash(state);
                }
                repeat.hash(state);
            }
        }
    }
}

impl ArbitraryGradient {
    pub fn stops(&self) -> &[GradientStop] {
        match self {
            ArbitraryGradient::LinearGradient { stops, .. }
            | ArbitraryGradient::RadialGradient { stops, .. } => stops,
        }
    }

    pub fn repeat(&self) -> bool {
        match self {
            ArbitraryGradient::LinearGradient { repeat, .. }
            | ArbitraryGradient::RadialGradient { repeat, .. } => *repeat,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BackgroundFill {
    #[serde(rename = "solid")]
    Solid {
        #[serde(rename = "color")]
        color: ColorToken,
    },
    LinearGradient {
        direction: GradientDirection,
        stops: Vec<GradientStop>,
    },
    /// 径向渐变。`center` 为单位正方形内的圆心坐标 `[0,1]`，
    /// 半径在渲染层取圆心到最远角的距离（`farthest-corner`）。
    RadialGradient {
        center: [f32; 2],
        stops: Vec<GradientStop>,
    },
    /// 从 CSS 任意值语法（`bg-[linear-gradient(...)]` 等）解析得到的渐变。
    /// 携带任意色标位置、background-size 与平铺模式。
    #[serde(rename = "arbitrary")]
    ArbitraryGradient {
        #[serde(rename = "gradient")]
        gradient: ArbitraryGradient,
    },
}

impl std::hash::Hash for BackgroundFill {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            BackgroundFill::Solid { color } => color.hash(state),
            BackgroundFill::LinearGradient { direction, stops } => {
                direction.hash(state);
                for stop in stops {
                    stop.pos.to_bits().hash(state);
                    stop.color.hash(state);
                }
            }
            BackgroundFill::RadialGradient { center, stops } => {
                center[0].to_bits().hash(state);
                center[1].to_bits().hash(state);
                for stop in stops {
                    stop.pos.to_bits().hash(state);
                    stop.color.hash(state);
                }
            }
            BackgroundFill::ArbitraryGradient { gradient } => gradient.hash(state),
        }
    }
}

impl BackgroundFill {
    /// 由 `from`/`via`/`to` 构造一个固定色标（0 / 0.5 / 1）的线性渐变。
    pub fn linear_from_via_to(
        direction: GradientDirection,
        from: ColorToken,
        via: Option<ColorToken>,
        to: ColorToken,
    ) -> Self {
        let stops = match via {
            Some(mid) => vec![
                GradientStop {
                    pos: 0.0,
                    color: from,
                },
                GradientStop {
                    pos: 0.5,
                    color: mid,
                },
                GradientStop {
                    pos: 1.0,
                    color: to,
                },
            ],
            None => vec![
                GradientStop {
                    pos: 0.0,
                    color: from,
                },
                GradientStop {
                    pos: 1.0,
                    color: to,
                },
            ],
        };
        BackgroundFill::LinearGradient { direction, stops }
    }

    /// 由 `from`/`via`/`to` 构造一个固定色标（0 / 0.5 / 1）的径向渐变。
    pub fn radial_from_via_to(
        center: [f32; 2],
        from: ColorToken,
        via: Option<ColorToken>,
        to: ColorToken,
    ) -> Self {
        let stops = match via {
            Some(mid) => vec![
                GradientStop {
                    pos: 0.0,
                    color: from,
                },
                GradientStop {
                    pos: 0.5,
                    color: mid,
                },
                GradientStop {
                    pos: 1.0,
                    color: to,
                },
            ],
            None => vec![
                GradientStop {
                    pos: 0.0,
                    color: from,
                },
                GradientStop {
                    pos: 1.0,
                    color: to,
                },
            ],
        };
        BackgroundFill::RadialGradient { center, stops }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Transform {
    #[serde(rename = "translateX")]
    TranslateX {
        #[serde(rename = "value")]
        value: f32,
    },
    #[serde(rename = "translateY")]
    TranslateY {
        #[serde(rename = "value")]
        value: f32,
    },
    #[serde(rename = "translate")]
    Translate {
        #[serde(rename = "x")]
        x: f32,
        #[serde(rename = "y")]
        y: f32,
    },
    #[serde(rename = "scale")]
    Scale {
        #[serde(rename = "value")]
        value: f32,
    },
    #[serde(rename = "scaleX")]
    ScaleX {
        #[serde(rename = "value")]
        value: f32,
    },
    #[serde(rename = "scaleY")]
    ScaleY {
        #[serde(rename = "value")]
        value: f32,
    },
    #[serde(rename = "rotate")]
    RotateDeg {
        #[serde(rename = "value")]
        value: f32,
    },
    #[serde(rename = "skewX")]
    SkewXDeg {
        #[serde(rename = "value")]
        value: f32,
    },
    #[serde(rename = "skewY")]
    SkewYDeg {
        #[serde(rename = "value")]
        value: f32,
    },
    #[serde(rename = "skew")]
    SkewDeg {
        #[serde(rename = "x")]
        x: f32,
        #[serde(rename = "y")]
        y: f32,
    },
}

impl std::hash::Hash for Transform {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match *self {
            Transform::TranslateX { value } => {
                0_u8.hash(state);
                value.to_bits().hash(state);
            }
            Transform::TranslateY { value } => {
                1_u8.hash(state);
                value.to_bits().hash(state);
            }
            Transform::Translate { x, y } => {
                2_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
            }
            Transform::Scale { value } => {
                3_u8.hash(state);
                value.to_bits().hash(state);
            }
            Transform::ScaleX { value } => {
                4_u8.hash(state);
                value.to_bits().hash(state);
            }
            Transform::ScaleY { value } => {
                5_u8.hash(state);
                value.to_bits().hash(state);
            }
            Transform::RotateDeg { value } => {
                6_u8.hash(state);
                value.to_bits().hash(state);
            }
            Transform::SkewXDeg { value } => {
                7_u8.hash(state);
                value.to_bits().hash(state);
            }
            Transform::SkewYDeg { value } => {
                8_u8.hash(state);
                value.to_bits().hash(state);
            }
            Transform::SkewDeg { x, y } => {
                9_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CssFilterKind {
    Blur,
    Brightness,
    Contrast,
    Grayscale,
    HueRotate,
    Invert,
    Saturate,
    Sepia,
}

impl CssFilterKind {
    pub fn from_property(name: &str) -> Option<Self> {
        match name {
            "blur" | "blurSigma" => Some(Self::Blur),
            "brightness" => Some(Self::Brightness),
            "contrast" => Some(Self::Contrast),
            "grayscale" => Some(Self::Grayscale),
            "hue-rotate" | "hueRotate" => Some(Self::HueRotate),
            "invert" => Some(Self::Invert),
            "saturate" => Some(Self::Saturate),
            "sepia" => Some(Self::Sepia),
            _ => None,
        }
    }

    pub fn css_name(self) -> &'static str {
        match self {
            Self::Blur => "blur",
            Self::Brightness => "brightness",
            Self::Contrast => "contrast",
            Self::Grayscale => "grayscale",
            Self::HueRotate => "hue-rotate",
            Self::Invert => "invert",
            Self::Saturate => "saturate",
            Self::Sepia => "sepia",
        }
    }

    pub fn property_name(self) -> &'static str {
        match self {
            Self::Blur => "blur",
            Self::Brightness => "brightness",
            Self::Contrast => "contrast",
            Self::Grayscale => "grayscale",
            Self::HueRotate => "hueRotate",
            Self::Invert => "invert",
            Self::Saturate => "saturate",
            Self::Sepia => "sepia",
        }
    }

    fn identity_value(self) -> f32 {
        match self {
            Self::Brightness | Self::Contrast | Self::Saturate => 1.0,
            Self::Blur | Self::Grayscale | Self::HueRotate | Self::Invert | Self::Sepia => 0.0,
        }
    }

    fn clamp_value(self, value: f32) -> f32 {
        match self {
            Self::Blur | Self::Brightness | Self::Contrast | Self::Saturate => value.max(0.0),
            Self::Grayscale | Self::Invert | Self::Sepia => value.clamp(0.0, 1.0),
            Self::HueRotate => value,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CssFilterOp {
    pub kind: CssFilterKind,
    pub value: f32,
}

impl CssFilterOp {
    pub fn new(kind: CssFilterKind, value: f32) -> Self {
        Self {
            kind,
            value: kind.clamp_value(value),
        }
    }

    pub fn is_identity(self) -> bool {
        match self.kind {
            CssFilterKind::Blur => self.value <= 0.0,
            _ => (self.value - self.kind.identity_value()).abs() <= f32::EPSILON,
        }
    }
}

impl std::hash::Hash for CssFilterOp {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
        self.value.to_bits().hash(state);
    }
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CssFilter {
    pub ops: Vec<CssFilterOp>,
}

impl CssFilter {
    pub fn is_identity(&self) -> bool {
        self.ops.iter().all(|op| op.is_identity())
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn merge_from(&mut self, other: &Self) {
        self.ops.extend(other.ops.iter().copied());
    }

    pub fn push(&mut self, kind: CssFilterKind, value: f32) {
        self.ops.push(CssFilterOp::new(kind, value));
    }

    pub fn set_property(&mut self, property: &str, value: f32) -> bool {
        let Some(kind) = CssFilterKind::from_property(property) else {
            return false;
        };
        let value = kind.clamp_value(value);
        if let Some(op) = self.ops.iter_mut().rev().find(|op| op.kind == kind) {
            op.value = value;
        } else {
            self.ops.push(CssFilterOp { kind, value });
        }
        true
    }

    pub fn value(&self, property: &str) -> Option<f32> {
        let kind = CssFilterKind::from_property(property)?;
        self.ops
            .iter()
            .rev()
            .find(|op| op.kind == kind)
            .map(|op| op.value)
    }

    pub fn to_css_string(&self) -> String {
        self.ops
            .iter()
            .map(|op| {
                format!(
                    "{}({})",
                    op.kind.css_name(),
                    format_css_filter_value(op.kind, op.value)
                )
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn parse_css_filter_string(value: &str) -> Self {
        let mut filter = Self::default();
        let mut rest = value.trim();
        while let Some(open) = rest.find('(') {
            let name = rest[..open].trim();
            let after_open = &rest[open + 1..];
            let Some(close) = after_open.find(')') else {
                break;
            };
            let raw_value = after_open[..close].trim();
            if let Some(kind) = CssFilterKind::from_property(name)
                && let Some(parsed) = parse_css_filter_number(raw_value, kind)
            {
                filter.push(kind, parsed);
            }
            rest = after_open[close + 1..].trim_start();
        }
        filter
    }
}

impl std::hash::Hash for CssFilter {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ops.hash(state);
    }
}

fn parse_css_filter_number(raw: &str, kind: CssFilterKind) -> Option<f32> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let value = if let Some(percent) = raw.strip_suffix('%') {
        percent.trim().parse::<f32>().ok()? / 100.0
    } else if kind == CssFilterKind::HueRotate {
        raw.strip_suffix("deg")
            .unwrap_or(raw)
            .trim()
            .parse::<f32>()
            .ok()?
    } else if kind == CssFilterKind::Blur {
        raw.strip_suffix("px")
            .unwrap_or(raw)
            .trim()
            .parse::<f32>()
            .ok()?
    } else {
        raw.parse::<f32>().ok()?
    };
    Some(kind.clamp_value(value))
}

fn format_css_filter_value(kind: CssFilterKind, value: f32) -> String {
    let unit = match kind {
        CssFilterKind::Blur => "px",
        CssFilterKind::HueRotate => "deg",
        _ => "",
    };
    let number = if value.fract().abs() <= f32::EPSILON {
        format!("{}", value as i32)
    } else {
        format!("{value}")
    };
    format!("{number}{unit}")
}

/// Style context container - carries all possible style info for inheritance
#[derive(Debug, Clone, Default)]
pub struct NodeStyle {
    // Positioning
    pub position: Option<Position>,
    pub inset_left: Option<LengthPercentageAuto>,
    pub inset_top: Option<LengthPercentageAuto>,
    pub inset_right: Option<LengthPercentageAuto>,
    pub inset_bottom: Option<LengthPercentageAuto>,

    // Size
    pub width: Option<f32>,
    /// 任意百分比宽度，对应 Tailwind 的 `w-[N%]`。
    pub width_percent: Option<f32>,
    /// 任意百分比高度，对应 Tailwind 的 `h-[N%]`。
    pub height_percent: Option<f32>,
    pub height: Option<f32>,
    pub max_width: Option<f32>,
    pub width_full: bool,
    pub height_full: bool,

    // Spacing
    pub padding: Option<f32>,
    pub padding_x: Option<f32>,
    pub padding_y: Option<f32>,
    pub padding_top: Option<f32>,
    pub padding_right: Option<f32>,
    pub padding_bottom: Option<f32>,
    pub padding_left: Option<f32>,
    pub margin: Option<LengthPercentageAuto>,
    pub margin_x: Option<LengthPercentageAuto>,
    pub margin_y: Option<LengthPercentageAuto>,
    pub margin_top: Option<LengthPercentageAuto>,
    pub margin_right: Option<LengthPercentageAuto>,
    pub margin_bottom: Option<LengthPercentageAuto>,
    pub margin_left: Option<LengthPercentageAuto>,

    // Layout
    pub flex_direction: Option<FlexDirection>,
    pub justify_content: Option<JustifyContent>,
    pub align_items: Option<AlignItems>,
    pub is_flex: bool,
    pub is_grid: bool,
    pub grid_template_columns: Option<u16>,
    pub grid_template_rows: Option<u16>,
    pub grid_auto_flow: Option<GridAutoFlow>,
    pub grid_auto_rows: Option<GridAutoRows>,
    pub col_start: Option<GridPlacement>,
    pub col_end: Option<GridPlacement>,
    pub row_start: Option<GridPlacement>,
    pub row_end: Option<GridPlacement>,
    pub auto_size: bool,
    pub gap: Option<f32>,
    pub gap_x: Option<f32>,
    pub gap_y: Option<f32>,
    pub order: Option<i32>,
    pub aspect_ratio: Option<f32>,
    pub min_height: Option<LengthPercentageAuto>,
    pub flex_wrap: Option<FlexWrap>,
    pub align_content: Option<JustifyContent>,
    pub align_self: Option<AlignItems>,
    pub justify_items: Option<AlignItems>,
    pub justify_self: Option<AlignItems>,
    pub flex_basis: Option<LengthPercentageAuto>,
    pub flex_grow: Option<f32>,
    pub flex_shrink: Option<f32>,
    pub z_index: Option<i32>,

    // Visual
    pub opacity: Option<f32>,
    pub bg_color: Option<ColorToken>,
    pub bg_gradient_from: Option<ColorToken>,
    pub bg_gradient_via: Option<ColorToken>,
    pub bg_gradient_to: Option<ColorToken>,
    pub bg_gradient_direction: Option<GradientDirection>,
    /// `Some` 表示当前为径向渐变，值为单位正方形内的圆心 `[x, y]`。
    /// 与 `bg_gradient_direction` 互斥：解析时设置一方会清除另一方。
    pub bg_gradient_radial_center: Option<[f32; 2]>,
    /// 任意值语法（`bg-[linear-gradient(...)]` 等）解析得到的背景层。
    /// 多层时按声明顺序叠加（第一层在最底）。与 `bg_color` 互斥：有任意层时忽略 `bg_color`。
    pub background_layers: Vec<BackgroundFill>,
    /// `bg-[length:Wpx_Hpx]`，绑定到最近添加的背景层（grid 等瓦片背景）。
    pub bg_size: Option<[f32; 2]>,
    pub border_radius: Option<BorderRadius>,
    pub border_width: Option<f32>,
    pub border_top_width: Option<f32>,
    pub border_right_width: Option<f32>,
    pub border_bottom_width: Option<f32>,
    pub border_left_width: Option<f32>,
    pub border_color: Option<ColorToken>,
    pub border_style: Option<BorderStyle>,
    pub css_filter: CssFilter,
    pub backdrop_blur_sigma: Option<f32>,
    pub object_fit: Option<ObjectFit>,
    pub overflow_hidden: bool,
    pub clip_path: Option<ClipPath>,
    pub truncate: bool,
    pub transforms: Vec<Transform>,

    // SVG / Path (Tailwind align: fill-*, stroke-*)
    pub fill_color: Option<ColorToken>,
    pub stroke_color: Option<ColorToken>,
    pub stroke_width: Option<f32>,
    pub stroke_dasharray: Option<f32>,
    pub stroke_dashoffset: Option<f32>,
    pub svg_path: Option<String>,

    // Text
    pub text_color: Option<ColorToken>,
    pub text_px: Option<f32>,
    /// Resolved `font-family` name for cosmic-text (from `<fonts>` + `font-[id]`).
    pub font_family: Option<String>,
    /// Reference to `<font id="...">` before manifest resolution.
    pub font_face_id: Option<String>,
    /// Tailwind `font-sans` → document default face from `<fonts default="...">`.
    pub use_document_default_font: bool,
    pub font_weight: Option<FontWeight>,
    pub letter_spacing: Option<f32>,
    pub letter_spacing_em: Option<f32>,
    pub text_align: Option<TextAlign>,
    pub line_height: Option<f32>,
    pub line_height_px: Option<f32>,
    pub text_transform: Option<TextTransform>,
    pub line_through: bool,

    // Shadow
    pub box_shadow: Vec<BoxShadow>,
    pub box_shadow_color: Option<ColorToken>,
    pub inset_shadow: Vec<InsetShadow>,
    pub inset_shadow_color: Option<ColorToken>,
    pub drop_shadow: Vec<DropShadow>,
    pub drop_shadow_color: Option<ColorToken>,

    // Text shadow（CSS text-shadow，文本专属，支持多个并列）
    pub text_shadows: Vec<TextShadow>,

    // Identity (for JS animation targeting and stable scene updates)
    pub id: String,

    // Node-local animation script scoped to this subtree.
    pub script_driver: Option<Arc<ScriptDriver>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputedTextStyle {
    pub color: ColorToken,
    pub text_px: f32,
    /// Shaping family name; `None` uses generic `sans-serif` (fontdb mapping).
    pub font_family: Option<String>,
    pub font_weight: FontWeight,
    pub letter_spacing: f32,
    pub text_align: TextAlign,
    pub line_height: f32,
    pub line_height_px: Option<f32>,
    pub text_transform: TextTransform,
    pub wrap_text: bool,
    pub line_through: bool,
}

impl std::hash::Hash for ComputedTextStyle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.color.hash(state);
        self.text_px.to_bits().hash(state);
        self.font_family.hash(state);
        self.font_weight.hash(state);
        self.letter_spacing.to_bits().hash(state);
        self.text_align.hash(state);
        self.line_height.to_bits().hash(state);
        self.line_height_px.map(f32::to_bits).hash(state);
        self.text_transform.hash(state);
        self.wrap_text.hash(state);
        self.line_through.hash(state);
    }
}

impl Default for ComputedTextStyle {
    fn default() -> Self {
        Self {
            color: ColorToken::Black,
            text_px: 16.0,
            font_family: None,
            font_weight: FontWeight::NORMAL,
            letter_spacing: 0.0,
            text_align: TextAlign::Left,
            line_height: 1.5,
            line_height_px: None,
            text_transform: TextTransform::None,
            wrap_text: false,
            line_through: false,
        }
    }
}

pub fn resolve_text_style(parent: &ComputedTextStyle, style: &NodeStyle) -> ComputedTextStyle {
    let text_px = style.text_px.unwrap_or(parent.text_px);
    let letter_spacing = style
        .letter_spacing
        .or_else(|| style.letter_spacing_em.map(|em| em * text_px))
        .unwrap_or(parent.letter_spacing);
    let (line_height, line_height_px) = if let Some(px) = style.line_height_px {
        (parent.line_height, Some(px))
    } else if let Some(scale) = style.line_height {
        (scale, None)
    } else {
        (parent.line_height, parent.line_height_px)
    };

    ComputedTextStyle {
        color: style.text_color.unwrap_or(parent.color),
        text_px,
        font_family: style
            .font_family
            .clone()
            .or_else(|| parent.font_family.clone()),
        font_weight: style.font_weight.unwrap_or(parent.font_weight),
        letter_spacing,
        text_align: style.text_align.unwrap_or(parent.text_align),
        line_height,
        line_height_px,
        text_transform: style.text_transform.unwrap_or(parent.text_transform),
        wrap_text: parent.wrap_text,
        line_through: style.line_through,
    }
}

impl ComputedTextStyle {
    pub fn resolved_line_height_px(&self) -> f32 {
        self.line_height_px
            .unwrap_or(self.text_px * self.line_height)
            .max(1.0)
    }
}

macro_rules! impl_node_style_api {
    ($ty:ty) => {
        impl $ty {
            // === Positioning ===
            pub fn position(mut self, position: $crate::style::Position) -> Self {
                self.style.position = Some(position);
                self
            }

            pub fn absolute(self) -> Self {
                self.position($crate::style::Position::Absolute)
            }

            pub fn relative(self) -> Self {
                self.position($crate::style::Position::Relative)
            }

            pub fn left(mut self, value: f32) -> Self {
                self.style.inset_left = Some($crate::style::LengthPercentageAuto::length(value));
                self
            }

            pub fn top(mut self, value: f32) -> Self {
                self.style.inset_top = Some($crate::style::LengthPercentageAuto::length(value));
                self
            }

            pub fn right(mut self, value: f32) -> Self {
                self.style.inset_right = Some($crate::style::LengthPercentageAuto::length(value));
                self
            }

            pub fn bottom(mut self, value: f32) -> Self {
                self.style.inset_bottom = Some($crate::style::LengthPercentageAuto::length(value));
                self
            }

            pub fn inset(mut self, value: f32) -> Self {
                let value = $crate::style::LengthPercentageAuto::length(value);
                self.style.inset_left = Some(value);
                self.style.inset_top = Some(value);
                self.style.inset_right = Some(value);
                self.style.inset_bottom = Some(value);
                self
            }

            // === Size ===
            pub fn w(mut self, value: f32) -> Self {
                self.style.width = Some(value);
                self.style.width_full = false;
                self
            }

            pub fn h(mut self, value: f32) -> Self {
                self.style.height = Some(value);
                self.style.height_full = false;
                self
            }

            pub fn size(mut self, width: f32, height: f32) -> Self {
                self.style.width = Some(width);
                self.style.height = Some(height);
                self.style.width_full = false;
                self.style.height_full = false;
                self
            }

            // === Padding ===
            pub fn p(mut self, value: f32) -> Self {
                self.style.padding = Some(value);
                self
            }

            pub fn px(mut self, value: f32) -> Self {
                self.style.padding_x = Some(value);
                self
            }

            pub fn py(mut self, value: f32) -> Self {
                self.style.padding_y = Some(value);
                self
            }

            pub fn pt(mut self, value: f32) -> Self {
                self.style.padding_top = Some(value);
                self
            }

            pub fn pb(mut self, value: f32) -> Self {
                self.style.padding_bottom = Some(value);
                self
            }

            pub fn pl(mut self, value: f32) -> Self {
                self.style.padding_left = Some(value);
                self
            }

            pub fn pr(mut self, value: f32) -> Self {
                self.style.padding_right = Some(value);
                self
            }

            // === Margin ===
            pub fn m(mut self, value: f32) -> Self {
                self.style.margin = Some($crate::style::LengthPercentageAuto::Length(value));
                self
            }

            pub fn mx(mut self, value: f32) -> Self {
                self.style.margin_x = Some($crate::style::LengthPercentageAuto::Length(value));
                self
            }

            pub fn my(mut self, value: f32) -> Self {
                self.style.margin_y = Some($crate::style::LengthPercentageAuto::Length(value));
                self
            }

            pub fn mt(mut self, value: f32) -> Self {
                self.style.margin_top = Some($crate::style::LengthPercentageAuto::Length(value));
                self
            }

            pub fn mb(mut self, value: f32) -> Self {
                self.style.margin_bottom = Some($crate::style::LengthPercentageAuto::Length(value));
                self
            }

            pub fn ml(mut self, value: f32) -> Self {
                self.style.margin_left = Some($crate::style::LengthPercentageAuto::Length(value));
                self
            }

            pub fn mr(mut self, value: f32) -> Self {
                self.style.margin_right = Some($crate::style::LengthPercentageAuto::Length(value));
                self
            }

            pub fn mx_auto(mut self) -> Self {
                self.style.margin_x = Some($crate::style::LengthPercentageAuto::Auto);
                self
            }

            pub fn my_auto(mut self) -> Self {
                self.style.margin_y = Some($crate::style::LengthPercentageAuto::Auto);
                self
            }

            pub fn m_auto(mut self) -> Self {
                self.style.margin = Some($crate::style::LengthPercentageAuto::Auto);
                self
            }

            pub fn ml_auto(mut self) -> Self {
                self.style.margin_left = Some($crate::style::LengthPercentageAuto::Auto);
                self
            }

            pub fn mr_auto(mut self) -> Self {
                self.style.margin_right = Some($crate::style::LengthPercentageAuto::Auto);
                self
            }

            pub fn mt_auto(mut self) -> Self {
                self.style.margin_top = Some($crate::style::LengthPercentageAuto::Auto);
                self
            }

            pub fn mb_auto(mut self) -> Self {
                self.style.margin_bottom = Some($crate::style::LengthPercentageAuto::Auto);
                self
            }

            // === Layout: Flex Direction ===
            pub fn flex_direction(mut self, direction: $crate::style::FlexDirection) -> Self {
                self.style.is_flex = true;
                self.style.flex_direction = Some(direction);
                self
            }

            pub fn flex_row(self) -> Self {
                self.flex_direction($crate::style::FlexDirection::Row)
            }

            pub fn flex_col(self) -> Self {
                self.flex_direction($crate::style::FlexDirection::Col)
            }

            pub fn flex_row_reverse(self) -> Self {
                self.flex_direction($crate::style::FlexDirection::RowReverse)
            }

            pub fn flex_col_reverse(self) -> Self {
                self.flex_direction($crate::style::FlexDirection::ColReverse)
            }

            pub fn flex(self) -> Self {
                self.flex_row()
            }

            pub fn grid(mut self) -> Self {
                self.style.is_grid = true;
                self
            }

            pub fn grid_cols(mut self, cols: u16) -> Self {
                self.style.is_grid = true;
                self.style.grid_template_columns = Some(cols);
                self
            }

            pub fn grid_rows(mut self, rows: u16) -> Self {
                self.style.is_grid = true;
                self.style.grid_template_rows = Some(rows);
                self
            }

            pub fn grid_auto_flow(mut self, flow: $crate::style::GridAutoFlow) -> Self {
                self.style.grid_auto_flow = Some(flow);
                self
            }

            pub fn col_start(mut self, line: i16) -> Self {
                self.style.col_start = Some($crate::style::GridPlacement::Line(line));
                self
            }

            pub fn col_end(mut self, line: i16) -> Self {
                self.style.col_end = Some($crate::style::GridPlacement::Line(line));
                self
            }

            pub fn row_start(mut self, line: i16) -> Self {
                self.style.row_start = Some($crate::style::GridPlacement::Line(line));
                self
            }

            pub fn row_end(mut self, line: i16) -> Self {
                self.style.row_end = Some($crate::style::GridPlacement::Line(line));
                self
            }

            pub fn w_full(mut self) -> Self {
                self.style.width = None;
                self.style.width_full = true;
                self
            }

            pub fn max_w(mut self, value: f32) -> Self {
                self.style.max_width = Some(value);
                self
            }

            pub fn h_full(mut self) -> Self {
                self.style.height = None;
                self.style.height_full = true;
                self
            }

            pub fn min_h_full(self) -> Self {
                self.h_full()
            }

            pub fn max_w_full(self) -> Self {
                self.w_full()
            }

            // === Layout: Justify Content (main axis) ===
            pub fn justify_content(
                mut self,
                justify_content: $crate::style::JustifyContent,
            ) -> Self {
                self.style.justify_content = Some(justify_content);
                self
            }

            pub fn justify_start(self) -> Self {
                self.justify_content($crate::style::JustifyContent::Start)
            }

            pub fn justify_center(self) -> Self {
                self.justify_content($crate::style::JustifyContent::Center)
            }

            pub fn justify_end(self) -> Self {
                self.justify_content($crate::style::JustifyContent::End)
            }

            pub fn justify_between(self) -> Self {
                self.justify_content($crate::style::JustifyContent::Between)
            }

            pub fn justify_around(self) -> Self {
                self.justify_content($crate::style::JustifyContent::Around)
            }

            pub fn justify_evenly(self) -> Self {
                self.justify_content($crate::style::JustifyContent::Evenly)
            }

            pub fn justify_stretch(self) -> Self {
                self.justify_content($crate::style::JustifyContent::Stretch)
            }

            pub fn align_content(mut self, align_content: $crate::style::JustifyContent) -> Self {
                self.style.align_content = Some(align_content);
                self
            }

            pub fn content_start(self) -> Self {
                self.align_content($crate::style::JustifyContent::Start)
            }

            pub fn content_center(self) -> Self {
                self.align_content($crate::style::JustifyContent::Center)
            }

            pub fn content_end(self) -> Self {
                self.align_content($crate::style::JustifyContent::End)
            }

            pub fn content_between(self) -> Self {
                self.align_content($crate::style::JustifyContent::Between)
            }

            pub fn content_around(self) -> Self {
                self.align_content($crate::style::JustifyContent::Around)
            }

            pub fn content_evenly(self) -> Self {
                self.align_content($crate::style::JustifyContent::Evenly)
            }

            pub fn content_stretch(self) -> Self {
                self.align_content($crate::style::JustifyContent::Stretch)
            }

            // === Layout: Align Items (cross axis) ===
            pub fn align_items(mut self, align_items: $crate::style::AlignItems) -> Self {
                self.style.align_items = Some(align_items);
                self
            }

            pub fn items_start(self) -> Self {
                self.align_items($crate::style::AlignItems::Start)
            }

            pub fn items_center(self) -> Self {
                self.align_items($crate::style::AlignItems::Center)
            }

            pub fn items_end(self) -> Self {
                self.align_items($crate::style::AlignItems::End)
            }

            pub fn items_baseline(self) -> Self {
                self.align_items($crate::style::AlignItems::Baseline)
            }

            pub fn items_stretch(self) -> Self {
                self.align_items($crate::style::AlignItems::Stretch)
            }

            pub fn align_self(mut self, align_self: $crate::style::AlignItems) -> Self {
                self.style.align_self = Some(align_self);
                self
            }

            pub fn self_start(self) -> Self {
                self.align_self($crate::style::AlignItems::Start)
            }

            pub fn self_center(self) -> Self {
                self.align_self($crate::style::AlignItems::Center)
            }

            pub fn self_end(self) -> Self {
                self.align_self($crate::style::AlignItems::End)
            }

            pub fn self_baseline(self) -> Self {
                self.align_self($crate::style::AlignItems::Baseline)
            }

            pub fn self_stretch(self) -> Self {
                self.align_self($crate::style::AlignItems::Stretch)
            }

            pub fn flex_wrap(mut self, flex_wrap: $crate::style::FlexWrap) -> Self {
                self.style.flex_wrap = Some(flex_wrap);
                self
            }

            pub fn wrap(self) -> Self {
                self.flex_wrap($crate::style::FlexWrap::Wrap)
            }

            pub fn wrap_reverse(self) -> Self {
                self.flex_wrap($crate::style::FlexWrap::WrapReverse)
            }

            pub fn nowrap(self) -> Self {
                self.flex_wrap($crate::style::FlexWrap::NoWrap)
            }

            // === Layout: Gap ===
            pub fn gap(mut self, gap: f32) -> Self {
                self.style.gap = Some(gap);
                self
            }

            pub fn flex_basis(mut self, basis: f32) -> Self {
                self.style.flex_basis = Some($crate::style::LengthPercentageAuto::length(basis));
                self
            }

            // === Layout: Flex Grow ===
            pub fn flex_grow(mut self, grow: f32) -> Self {
                self.style.flex_grow = Some(grow);
                self
            }

            pub fn flex_shrink(mut self, shrink: f32) -> Self {
                self.style.flex_shrink = Some(shrink);
                self
            }

            pub fn flex_1(self) -> Self {
                self.flex_grow(1.0)
            }

            // === Visual: Border Radius ===
            pub fn opacity(mut self, opacity: f32) -> Self {
                self.style.opacity = Some(opacity.clamp(0.0, 1.0));
                self
            }

            pub fn filter(mut self, value: impl AsRef<str>) -> Self {
                self.style.css_filter =
                    $crate::style::CssFilter::parse_css_filter_string(value.as_ref());
                self
            }

            pub fn filter_blur(mut self, value: f32) -> Self {
                self.style
                    .css_filter
                    .push($crate::style::CssFilterKind::Blur, value);
                self
            }

            pub fn filter_brightness(mut self, value: f32) -> Self {
                self.style
                    .css_filter
                    .push($crate::style::CssFilterKind::Brightness, value);
                self
            }

            pub fn filter_contrast(mut self, value: f32) -> Self {
                self.style
                    .css_filter
                    .push($crate::style::CssFilterKind::Contrast, value);
                self
            }

            pub fn filter_grayscale(mut self, value: f32) -> Self {
                self.style
                    .css_filter
                    .push($crate::style::CssFilterKind::Grayscale, value);
                self
            }

            pub fn filter_hue_rotate(mut self, degrees: f32) -> Self {
                self.style
                    .css_filter
                    .push($crate::style::CssFilterKind::HueRotate, degrees);
                self
            }

            pub fn filter_invert(mut self, value: f32) -> Self {
                self.style
                    .css_filter
                    .push($crate::style::CssFilterKind::Invert, value);
                self
            }

            pub fn filter_saturate(mut self, value: f32) -> Self {
                self.style
                    .css_filter
                    .push($crate::style::CssFilterKind::Saturate, value);
                self
            }

            pub fn filter_sepia(mut self, value: f32) -> Self {
                self.style
                    .css_filter
                    .push($crate::style::CssFilterKind::Sepia, value);
                self
            }

            pub fn object_fit(mut self, fit: $crate::style::ObjectFit) -> Self {
                self.style.object_fit = Some(fit);
                self
            }

            pub fn contain(self) -> Self {
                self.object_fit($crate::style::ObjectFit::Contain)
            }

            pub fn cover(self) -> Self {
                self.object_fit($crate::style::ObjectFit::Cover)
            }

            pub fn fill(self) -> Self {
                self.object_fit($crate::style::ObjectFit::Fill)
            }

            pub fn transform(mut self, transform: $crate::style::Transform) -> Self {
                self.style.transforms.push(transform);
                self
            }

            pub fn translate_x(self, value: f32) -> Self {
                self.transform($crate::style::Transform::TranslateX { value })
            }

            pub fn translate_y(self, value: f32) -> Self {
                self.transform($crate::style::Transform::TranslateY { value })
            }

            pub fn translate(self, x: f32, y: f32) -> Self {
                self.transform($crate::style::Transform::Translate { x, y })
            }

            pub fn scale(self, value: f32) -> Self {
                self.transform($crate::style::Transform::Scale { value })
            }

            pub fn scale_x(self, value: f32) -> Self {
                self.transform($crate::style::Transform::ScaleX { value })
            }

            pub fn scale_y(self, value: f32) -> Self {
                self.transform($crate::style::Transform::ScaleY { value })
            }

            pub fn rotate_deg(self, value: f32) -> Self {
                self.transform($crate::style::Transform::RotateDeg { value })
            }

            pub fn skew_x_deg(self, value: f32) -> Self {
                self.transform($crate::style::Transform::SkewXDeg { value })
            }

            pub fn skew_y_deg(self, value: f32) -> Self {
                self.transform($crate::style::Transform::SkewYDeg { value })
            }

            pub fn skew_deg(self, x_deg: f32, y_deg: f32) -> Self {
                self.transform($crate::style::Transform::SkewDeg { x: x_deg, y: y_deg })
            }

            pub fn rounded(mut self, radius: f32) -> Self {
                self.style.border_radius = Some($crate::style::BorderRadius::uniform(radius));
                self
            }

            pub fn rounded_none(self) -> Self {
                self.rounded(0.0)
            }

            pub fn rounded_sm(self) -> Self {
                self.rounded(4.0)
            }

            pub fn rounded_md(self) -> Self {
                self.rounded(8.0)
            }

            pub fn rounded_lg(self) -> Self {
                self.rounded(16.0)
            }

            pub fn rounded_xl(self) -> Self {
                self.rounded(24.0)
            }

            pub fn rounded_2xl(self) -> Self {
                self.rounded(32.0)
            }

            pub fn rounded_full(self) -> Self {
                self.rounded(9999.0)
            }

            // === Visual: Border ===
            pub fn border(mut self) -> Self {
                self.style.border_width = Some(1.0);
                self
            }

            pub fn border_w(mut self, width: f32) -> Self {
                self.style.border_width = Some(width);
                self
            }

            pub fn border_color(mut self, color: $crate::style::ColorToken) -> Self {
                self.style.border_color = Some(color);
                self
            }

            pub fn border_top_w(mut self, width: f32) -> Self {
                self.style.border_top_width = Some(width);
                self
            }

            pub fn border_right_w(mut self, width: f32) -> Self {
                self.style.border_right_width = Some(width);
                self
            }

            pub fn border_bottom_w(mut self, width: f32) -> Self {
                self.style.border_bottom_width = Some(width);
                self
            }

            pub fn border_left_w(mut self, width: f32) -> Self {
                self.style.border_left_width = Some(width);
                self
            }

            pub fn border_style(mut self, style: $crate::style::BorderStyle) -> Self {
                self.style.border_style = Some(style);
                self
            }

            pub fn stroke_width(mut self, width: f32) -> Self {
                self.style.stroke_width = Some(width.max(0.0));
                self
            }

            pub fn stroke_color(mut self, color: $crate::style::ColorToken) -> Self {
                self.style.stroke_color = Some(color);
                self
            }

            pub fn fill_color(mut self, color: $crate::style::ColorToken) -> Self {
                self.style.fill_color = Some(color);
                self
            }

            // === Visual: Background Colors ===
            pub fn bg(mut self, color: $crate::style::ColorToken) -> Self {
                self.style.bg_color = Some(color);
                self.style.bg_gradient_from = None;
                self.style.bg_gradient_via = None;
                self.style.bg_gradient_to = None;
                self.style.bg_gradient_direction = None;
                self.style.bg_gradient_radial_center = None;
                self
            }

            pub fn bg_primary(self) -> Self {
                self.bg($crate::style::ColorToken::Primary)
            }

            pub fn overflow_hidden(mut self) -> Self {
                self.style.overflow_hidden = true;
                self
            }

            // === Visual: Text Colors ===
            pub fn text_color(mut self, color: $crate::style::ColorToken) -> Self {
                self.style.text_color = Some(color);
                self
            }

            pub fn text_px(mut self, px: f32) -> Self {
                self.style.text_px = Some(px);
                self
            }

            pub fn text_primary(self) -> Self {
                self.text_color($crate::style::ColorToken::Primary)
            }

            // === Font Weight ===
            pub fn font_weight(mut self, weight: $crate::style::FontWeight) -> Self {
                self.style.font_weight = Some(weight);
                self
            }

            pub fn font_normal(self) -> Self {
                self.font_weight($crate::style::FontWeight::NORMAL)
            }

            pub fn font_light(self) -> Self {
                self.font_weight($crate::style::FontWeight::LIGHT)
            }

            pub fn font_medium(self) -> Self {
                self.font_weight($crate::style::FontWeight::MEDIUM)
            }

            pub fn font_semibold(self) -> Self {
                self.font_weight($crate::style::FontWeight::SEMIBOLD)
            }

            pub fn font_bold(self) -> Self {
                self.font_weight($crate::style::FontWeight::BOLD)
            }

            pub fn text_transform(mut self, transform: $crate::style::TextTransform) -> Self {
                self.style.text_transform = Some(transform);
                self
            }

            pub fn uppercase(self) -> Self {
                self.text_transform($crate::style::TextTransform::Uppercase)
            }

            // === Shadow ===
            pub fn shadow(mut self, style: $crate::style::BoxShadowStyle) -> Self {
                self.style
                    .box_shadow
                    .push($crate::style::BoxShadow::from_style(style));
                self
            }

            pub fn shadow_sm(self) -> Self {
                self.shadow($crate::style::BoxShadowStyle::Sm)
            }

            pub fn shadow_md(self) -> Self {
                self.shadow($crate::style::BoxShadowStyle::Md)
            }

            pub fn shadow_lg(self) -> Self {
                self.shadow($crate::style::BoxShadowStyle::Lg)
            }

            pub fn shadow_xl(self) -> Self {
                self.shadow($crate::style::BoxShadowStyle::Xl)
            }

            pub fn shadow_color(mut self, color: $crate::style::ColorToken) -> Self {
                self.style.box_shadow_color = Some(color);
                self
            }

            pub fn inset_shadow(mut self, style: $crate::style::InsetShadowStyle) -> Self {
                self.style
                    .inset_shadow
                    .push($crate::style::InsetShadow::from_style(style));
                self
            }

            pub fn inset_shadow_sm(self) -> Self {
                self.inset_shadow($crate::style::InsetShadowStyle::Sm)
            }

            pub fn inset_shadow_md(self) -> Self {
                self.inset_shadow($crate::style::InsetShadowStyle::Md)
            }

            pub fn inset_shadow_color(mut self, color: $crate::style::ColorToken) -> Self {
                self.style.inset_shadow_color = Some(color);
                self
            }

            pub fn drop_shadow(mut self, style: $crate::style::DropShadowStyle) -> Self {
                self.style
                    .drop_shadow
                    .push($crate::style::DropShadow::from_style(style));
                self
            }

            pub fn drop_shadow_sm(self) -> Self {
                self.drop_shadow($crate::style::DropShadowStyle::Sm)
            }

            pub fn drop_shadow_md(self) -> Self {
                self.drop_shadow($crate::style::DropShadowStyle::Md)
            }

            pub fn drop_shadow_lg(self) -> Self {
                self.drop_shadow($crate::style::DropShadowStyle::Lg)
            }

            pub fn drop_shadow_xl(self) -> Self {
                self.drop_shadow($crate::style::DropShadowStyle::Xl)
            }

            pub fn drop_shadow_color(mut self, color: $crate::style::ColorToken) -> Self {
                self.style.drop_shadow_color = Some(color);
                self
            }

            // === Letter Spacing ===
            pub fn letter_spacing(mut self, value: f32) -> Self {
                self.style.letter_spacing = Some(value);
                self
            }

            pub fn tracking_normal(self) -> Self {
                self.letter_spacing(0.0)
            }

            pub fn tracking_wide(self) -> Self {
                self.letter_spacing(0.5)
            }

            pub fn tracking_wider(self) -> Self {
                self.letter_spacing(1.0)
            }

            // === Text Alignment ===
            pub fn text_align(mut self, align: $crate::style::TextAlign) -> Self {
                self.style.text_align = Some(align);
                self
            }

            pub fn text_left(self) -> Self {
                self.text_align($crate::style::TextAlign::Left)
            }

            pub fn text_center(self) -> Self {
                self.text_align($crate::style::TextAlign::Center)
            }

            pub fn text_right(self) -> Self {
                self.text_align($crate::style::TextAlign::Right)
            }

            // === Line Height ===
            pub fn line_height(mut self, value: f32) -> Self {
                self.style.line_height = Some(value);
                self
            }

            pub fn leading(mut self, value: f32) -> Self {
                self.style.line_height = Some(value);
                self
            }

            // === Identity ===
            pub fn id(mut self, id: &str) -> Self {
                self.style.id = id.to_string();
                self
            }

            pub fn script_driver(mut self, driver: $crate::ScriptDriver) -> Self {
                self.style.script_driver = Some(std::sync::Arc::new(driver));
                self
            }

            pub fn script_source(self, source: &str) -> anyhow::Result<Self> {
                let driver = $crate::ScriptDriver::from_source(source)?;
                Ok(self.script_driver(driver))
            }
        }
    };
}

pub(crate) use impl_node_style_api;

include!(concat!(
    env!("OUT_DIR"),
    "/tailwind_color_inherent_impls.rs"
));

/// Parse a color from a script-facing string (named color, hex, hsla, etc.)
/// into a `ColorToken`. Used by both engine and web script bridges.
pub fn color_token_from_script_string(name: &str) -> Option<ColorToken> {
    if let Some(c) = color_token_from_script_name(name) {
        return Some(c);
    }
    let hsla = crate::script::animate::parse_color(name)?;
    let (r, g, b) = crate::script::animate::hsl_to_rgb(hsla.h, hsla.s, hsla.l);
    let a = (hsla.a.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some(ColorToken::Custom(r, g, b, a))
}

#[cfg(test)]
mod tests {
    use super::{ColorToken, color_token_from_class_suffix, color_token_from_script_name};

    #[test]
    fn generated_tailwind_palette_supports_numbered_classes() {
        assert_eq!(
            color_token_from_class_suffix("slate-950"),
            Some(ColorToken::Slate950)
        );
        assert_eq!(
            color_token_from_class_suffix("emerald-300"),
            Some(ColorToken::Emerald300)
        );
    }

    #[test]
    fn generated_tailwind_palette_keeps_family_aliases_and_script_names() {
        assert_eq!(
            color_token_from_class_suffix("blue"),
            Some(ColorToken::Blue)
        );
        assert_eq!(
            color_token_from_script_name("blue500"),
            Some(ColorToken::Blue500)
        );
        assert_eq!(
            color_token_from_script_name("slate_700"),
            Some(ColorToken::Slate700)
        );
        assert_eq!(
            color_token_from_script_name("primary"),
            Some(ColorToken::Primary)
        );
        assert_eq!(
            color_token_from_class_suffix("transparent"),
            Some(ColorToken::Transparent)
        );
        assert_eq!(
            color_token_from_script_name("transparent"),
            Some(ColorToken::Transparent)
        );
    }

    #[test]
    fn color_token_serializes_as_rgba_for_wire() {
        let json = serde_json::to_string(&ColorToken::Red500).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("r").is_some(), "expected `r` field, got {json}");
        assert!(parsed.get("g").is_some(), "expected `g` field");
        assert!(parsed.get("b").is_some(), "expected `b` field");
        assert!(parsed.get("a").is_some(), "expected `a` field");
    }

    #[test]
    fn color_token_deserializes_from_camelcase_variant_name() {
        let token: ColorToken = serde_json::from_str("\"red500\"").unwrap();
        assert_eq!(token, ColorToken::Red500);

        let token: ColorToken = serde_json::from_str("\"transparent\"").unwrap();
        assert_eq!(token, ColorToken::Transparent);
    }

    #[test]
    fn color_token_custom_variant_serializes_as_rgba() {
        let token = ColorToken::Custom(10, 20, 30, 40);
        let json = serde_json::to_string(&token).unwrap();
        assert_eq!(json, r#"{"r":10,"g":20,"b":30,"a":40}"#);
    }
}
