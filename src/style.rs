use std::{hash::Hash, sync::Arc};

use crate::scene::script::ScriptDriver;

include!(concat!(env!("OUT_DIR"), "/tailwind_color_items.rs"));

/// Position mode - Tailwind: relative, absolute
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Position {
    #[default]
    Relative,
    Absolute,
}

/// Flex direction - Tailwind: flex-row, flex-col
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum FlexDirection {
    #[default]
    Row,
    Col,
    RowReverse,
    ColReverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

/// Main axis alignment - Tailwind: justify-*
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LengthPercentageAuto {
    Auto,
    Length(f32),
    Percent(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GridAutoFlow {
    #[default]
    Row,
    Column,
    RowDense,
    ColumnDense,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GridAutoRows {
    Auto,
    Min,
    Max,
    Fr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum ObjectFit {
    #[default]
    Contain,
    Cover,
    Fill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum FontWeight {
    Light,
    #[default]
    Normal,
    Medium,
    SemiBold,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum TextTransform {
    #[default]
    None,
    Uppercase,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_sigma: f32,
    pub spread: f32,
    pub color: ColorToken,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InsetShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_sigma: f32,
    pub spread: f32,
    pub color: ColorToken,
}

#[derive(Debug, Clone, Copy, PartialEq)]
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
            BoxShadowStyle::Md => Self::new(0.0, 4.0, 6.0 / 6.0, 0.0, 0.10),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GradientDirection {
    ToRight,
    ToLeft,
    ToBottom,
    ToTop,
    ToBottomRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackgroundFill {
    Solid(ColorToken),
    LinearGradient {
        direction: GradientDirection,
        from: ColorToken,
        via: Option<ColorToken>,
        to: ColorToken,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Transform {
    TranslateX(f32),
    TranslateY(f32),
    Translate(f32, f32),
    Scale(f32),
    ScaleX(f32),
    ScaleY(f32),
    RotateDeg(f32),
    SkewXDeg(f32),
    SkewYDeg(f32),
    SkewDeg(f32, f32),
}

impl std::hash::Hash for Transform {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match *self {
            Transform::TranslateX(x) => {
                0_u8.hash(state);
                x.to_bits().hash(state);
            }
            Transform::TranslateY(y) => {
                1_u8.hash(state);
                y.to_bits().hash(state);
            }
            Transform::Translate(x, y) => {
                2_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
            }
            Transform::Scale(value) => {
                3_u8.hash(state);
                value.to_bits().hash(state);
            }
            Transform::ScaleX(value) => {
                4_u8.hash(state);
                value.to_bits().hash(state);
            }
            Transform::ScaleY(value) => {
                5_u8.hash(state);
                value.to_bits().hash(state);
            }
            Transform::RotateDeg(value) => {
                6_u8.hash(state);
                value.to_bits().hash(state);
            }
            Transform::SkewXDeg(value) => {
                7_u8.hash(state);
                value.to_bits().hash(state);
            }
            Transform::SkewYDeg(value) => {
                8_u8.hash(state);
                value.to_bits().hash(state);
            }
            Transform::SkewDeg(x, y) => {
                9_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
            }
        }
    }
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
    /// 注意：当前仅实现了 width 维度，`height_percent` 暂未引入——
    /// 容器高度通常由 content 或 `h-full` 决定，按需求驱动添加。
    pub width_percent: Option<f32>,
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
    pub border_radius: Option<BorderRadius>,
    pub border_width: Option<f32>,
    pub border_top_width: Option<f32>,
    pub border_right_width: Option<f32>,
    pub border_bottom_width: Option<f32>,
    pub border_left_width: Option<f32>,
    pub border_color: Option<ColorToken>,
    pub border_style: Option<BorderStyle>,
    pub blur_sigma: Option<f32>,
    pub backdrop_blur_sigma: Option<f32>,
    pub object_fit: Option<ObjectFit>,
    pub overflow_hidden: bool,
    pub truncate: bool,
    pub transforms: Vec<Transform>,

    // SVG / Path (Tailwind align: fill-*, stroke-*)
    pub fill_color: Option<ColorToken>,
    pub stroke_color: Option<ColorToken>,
    pub stroke_width: Option<f32>,
    pub svg_path: Option<String>,

    // Text
    pub text_color: Option<ColorToken>,
    pub text_px: Option<f32>,
    pub font_weight: Option<FontWeight>,
    pub letter_spacing: Option<f32>,
    pub letter_spacing_em: Option<f32>,
    pub text_align: Option<TextAlign>,
    pub line_height: Option<f32>,
    pub line_height_px: Option<f32>,
    pub text_transform: Option<TextTransform>,
    pub line_through: bool,

    // Shadow
    pub box_shadow: Option<BoxShadow>,
    pub box_shadow_color: Option<ColorToken>,
    pub inset_shadow: Option<InsetShadow>,
    pub inset_shadow_color: Option<ColorToken>,
    pub drop_shadow: Option<DropShadow>,
    pub drop_shadow_color: Option<ColorToken>,

    // Identity (for JS animation targeting and stable scene updates)
    pub id: String,

    // Node-local animation script scoped to this subtree.
    pub script_driver: Option<Arc<ScriptDriver>>,
}

#[derive(Debug, Clone, Copy)]
pub struct ComputedTextStyle {
    pub color: ColorToken,
    pub text_px: f32,
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
            font_weight: FontWeight::Normal,
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
                self.transform($crate::style::Transform::TranslateX(value))
            }

            pub fn translate_y(self, value: f32) -> Self {
                self.transform($crate::style::Transform::TranslateY(value))
            }

            pub fn translate(self, x: f32, y: f32) -> Self {
                self.transform($crate::style::Transform::Translate(x, y))
            }

            pub fn scale(self, value: f32) -> Self {
                self.transform($crate::style::Transform::Scale(value))
            }

            pub fn scale_x(self, value: f32) -> Self {
                self.transform($crate::style::Transform::ScaleX(value))
            }

            pub fn scale_y(self, value: f32) -> Self {
                self.transform($crate::style::Transform::ScaleY(value))
            }

            pub fn rotate_deg(self, value: f32) -> Self {
                self.transform($crate::style::Transform::RotateDeg(value))
            }

            pub fn skew_x_deg(self, value: f32) -> Self {
                self.transform($crate::style::Transform::SkewXDeg(value))
            }

            pub fn skew_y_deg(self, value: f32) -> Self {
                self.transform($crate::style::Transform::SkewYDeg(value))
            }

            pub fn skew_deg(self, x_deg: f32, y_deg: f32) -> Self {
                self.transform($crate::style::Transform::SkewDeg(x_deg, y_deg))
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
                self.font_weight($crate::style::FontWeight::Normal)
            }

            pub fn font_light(self) -> Self {
                self.font_weight($crate::style::FontWeight::Light)
            }

            pub fn font_medium(self) -> Self {
                self.font_weight($crate::style::FontWeight::Medium)
            }

            pub fn font_semibold(self) -> Self {
                self.font_weight($crate::style::FontWeight::SemiBold)
            }

            pub fn font_bold(self) -> Self {
                self.font_weight($crate::style::FontWeight::Bold)
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
                self.style.box_shadow = Some($crate::style::BoxShadow::from_style(style));
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
                self.style.inset_shadow = Some($crate::style::InsetShadow::from_style(style));
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
                self.style.drop_shadow = Some($crate::style::DropShadow::from_style(style));
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
}
