use skia_safe::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorToken {
    White,
    Black,
    Red,
    Green,
    Blue,
    Teal400,
    Teal500,
    Yellow,
    Orange,
    Purple,
    Pink,
    Gray,
    Slate50,
    Slate200,
    Slate300,
    Slate400,
    Slate500,
    Slate600,
    Slate700,
    Slate800,
    Slate900,
    Primary,
}

impl ColorToken {
    pub fn to_skia(self) -> Color {
        match self {
            ColorToken::White => Color::WHITE,
            ColorToken::Black => Color::BLACK,
            ColorToken::Red => Color::RED,
            ColorToken::Green => Color::from_rgb(0x22, 0xc5, 0x5e), // Tailwind green-500
            ColorToken::Blue => Color::from_rgb(0x3b, 0x82, 0xf6),  // Tailwind blue-500
            ColorToken::Teal400 => Color::from_rgb(0x2d, 0xd4, 0xbf),
            ColorToken::Teal500 => Color::from_rgb(0x14, 0xb8, 0xa6),
            ColorToken::Yellow => Color::from_rgb(0xea, 0xb3, 0x08), // Tailwind yellow-500
            ColorToken::Orange => Color::from_rgb(0xf9, 0x73, 0x16), // Tailwind orange-500
            ColorToken::Purple => Color::from_rgb(0xa8, 0x55, 0xf7), // Tailwind purple-500
            ColorToken::Pink => Color::from_rgb(0xec, 0x48, 0x99),   // Tailwind pink-500
            ColorToken::Gray => Color::from_rgb(0x6b, 0x72, 0x80),   // Tailwind gray-500
            ColorToken::Slate50 => Color::from_rgb(0xf8, 0xfa, 0xfc),
            ColorToken::Slate200 => Color::from_rgb(0xe2, 0xe8, 0xf0),
            ColorToken::Slate300 => Color::from_rgb(0xcb, 0xd5, 0xe1),
            ColorToken::Slate400 => Color::from_rgb(0x94, 0xa3, 0xb8),
            ColorToken::Slate500 => Color::from_rgb(0x64, 0x74, 0x8b),
            ColorToken::Slate600 => Color::from_rgb(0x47, 0x55, 0x69),
            ColorToken::Slate700 => Color::from_rgb(0x33, 0x41, 0x55),
            ColorToken::Slate800 => Color::from_rgb(0x1e, 0x29, 0x3b),
            ColorToken::Slate900 => Color::from_rgb(0x0f, 0x17, 0x2a),
            ColorToken::Primary => Color::from_rgb(0x3b, 0x82, 0xf6), // Same as blue-500
        }
    }
}

/// Position mode - Tailwind: relative, absolute
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Position {
    #[default]
    Relative,
    Absolute,
}

/// Flex direction - Tailwind: flex-row, flex-col
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    Col,
}

/// Main axis alignment - Tailwind: justify-*
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifyContent {
    #[default]
    Start,
    Center,
    End,
    Between,
    Around,
    Evenly,
}

/// Cross axis alignment - Tailwind: items-*
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignItems {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ObjectFit {
    #[default]
    Contain,
    Cover,
    Fill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontWeight {
    #[default]
    Normal,
    Medium,
    SemiBold,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowStyle {
    SM,
    MD,
    LG,
    XL,
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

/// Style context container - carries all possible style info for inheritance
#[derive(Debug, Clone, Default)]
pub struct NodeStyle {
    // Positioning
    pub position: Option<Position>,
    pub inset_left: Option<f32>,
    pub inset_top: Option<f32>,
    pub inset_right: Option<f32>,
    pub inset_bottom: Option<f32>,

    // Size
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub width_full: bool,
    pub height_full: bool,

    // Spacing
    pub padding: Option<f32>,
    pub padding_x: Option<f32>,
    pub padding_y: Option<f32>,
    pub margin: Option<f32>,
    pub margin_x: Option<f32>,
    pub margin_y: Option<f32>,

    // Layout
    pub flex_direction: Option<FlexDirection>,
    pub justify_content: Option<JustifyContent>,
    pub align_items: Option<AlignItems>,
    pub gap: Option<f32>,
    pub flex_grow: Option<f32>,

    // Visual
    pub opacity: Option<f32>,
    pub bg_color: Option<ColorToken>,
    pub border_radius: Option<f32>,
    pub border_width: Option<f32>,
    pub border_color: Option<ColorToken>,
    pub object_fit: Option<ObjectFit>,
    pub transforms: Vec<Transform>,

    // Text
    pub text_color: Option<ColorToken>,
    pub text_px: Option<f32>,
    pub font_weight: Option<FontWeight>,
    pub letter_spacing: Option<f32>,
    pub text_align: Option<TextAlign>,
    pub line_height: Option<f32>,

    // Shadow
    pub shadow: Option<ShadowStyle>,

    // Identity (for JS animation targeting)
    pub data_id: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct ComputedTextStyle {
    pub color: ColorToken,
    pub text_px: f32,
    pub font_weight: FontWeight,
    pub letter_spacing: f32,
    pub text_align: TextAlign,
    pub line_height: f32,
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
        }
    }
}

pub fn resolve_text_style(parent: &ComputedTextStyle, style: &NodeStyle) -> ComputedTextStyle {
    ComputedTextStyle {
        color: style.text_color.unwrap_or(parent.color),
        text_px: style.text_px.unwrap_or(parent.text_px),
        font_weight: style.font_weight.unwrap_or(parent.font_weight),
        letter_spacing: style.letter_spacing.unwrap_or(parent.letter_spacing),
        text_align: style.text_align.unwrap_or(parent.text_align),
        line_height: style.line_height.unwrap_or(parent.line_height),
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
                self.style.inset_left = Some(value);
                self
            }

            pub fn top(mut self, value: f32) -> Self {
                self.style.inset_top = Some(value);
                self
            }

            pub fn right(mut self, value: f32) -> Self {
                self.style.inset_right = Some(value);
                self
            }

            pub fn bottom(mut self, value: f32) -> Self {
                self.style.inset_bottom = Some(value);
                self
            }

            pub fn inset(mut self, value: f32) -> Self {
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
                self.style.padding_y = Some(value);
                self
            }

            pub fn pb(mut self, value: f32) -> Self {
                self.style.padding_y = Some(value);
                self
            }

            pub fn pl(mut self, value: f32) -> Self {
                self.style.padding_x = Some(value);
                self
            }

            pub fn pr(mut self, value: f32) -> Self {
                self.style.padding_x = Some(value);
                self
            }

            // === Margin ===
            pub fn m(mut self, value: f32) -> Self {
                self.style.margin = Some(value);
                self
            }

            pub fn mx(mut self, value: f32) -> Self {
                self.style.margin_x = Some(value);
                self
            }

            pub fn my(mut self, value: f32) -> Self {
                self.style.margin_y = Some(value);
                self
            }

            pub fn mt(mut self, value: f32) -> Self {
                self.style.margin_y = Some(value);
                self
            }

            pub fn mb(mut self, value: f32) -> Self {
                self.style.margin_y = Some(value);
                self
            }

            pub fn ml(mut self, value: f32) -> Self {
                self.style.margin_x = Some(value);
                self
            }

            pub fn mr(mut self, value: f32) -> Self {
                self.style.margin_x = Some(value);
                self
            }

            // === Layout: Flex Direction ===
            pub fn flex_direction(mut self, direction: $crate::style::FlexDirection) -> Self {
                self.style.flex_direction = Some(direction);
                self
            }

            pub fn flex_row(self) -> Self {
                self.flex_direction($crate::style::FlexDirection::Row)
            }

            pub fn flex_col(self) -> Self {
                self.flex_direction($crate::style::FlexDirection::Col)
            }

            pub fn flex(self) -> Self {
                self.flex_row()
            }

            pub fn w_full(mut self) -> Self {
                self.style.width = None;
                self.style.width_full = true;
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

            pub fn items_stretch(self) -> Self {
                self.align_items($crate::style::AlignItems::Stretch)
            }

            // === Layout: Gap ===
            pub fn gap(mut self, gap: f32) -> Self {
                self.style.gap = Some(gap);
                self
            }

            // === Layout: Flex Grow ===
            pub fn flex_grow(mut self, grow: f32) -> Self {
                self.style.flex_grow = Some(grow);
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
                self.style.border_radius = Some(radius);
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

            pub fn border_slate_200(self) -> Self {
                self.border_color($crate::style::ColorToken::Slate200)
            }

            pub fn border_slate_300(self) -> Self {
                self.border_color($crate::style::ColorToken::Slate300)
            }

            pub fn border_slate_700(self) -> Self {
                self.border_color($crate::style::ColorToken::Slate700)
            }

            // === Visual: Background Colors ===
            pub fn bg(mut self, color: $crate::style::ColorToken) -> Self {
                self.style.bg_color = Some(color);
                self
            }

            pub fn bg_white(self) -> Self {
                self.bg($crate::style::ColorToken::White)
            }

            pub fn bg_black(self) -> Self {
                self.bg($crate::style::ColorToken::Black)
            }

            pub fn bg_red(self) -> Self {
                self.bg($crate::style::ColorToken::Red)
            }

            pub fn bg_green(self) -> Self {
                self.bg($crate::style::ColorToken::Green)
            }

            pub fn bg_blue(self) -> Self {
                self.bg($crate::style::ColorToken::Blue)
            }

            pub fn bg_yellow(self) -> Self {
                self.bg($crate::style::ColorToken::Yellow)
            }

            pub fn bg_teal_400(self) -> Self {
                self.bg($crate::style::ColorToken::Teal400)
            }

            pub fn bg_teal_500(self) -> Self {
                self.bg($crate::style::ColorToken::Teal500)
            }

            pub fn bg_orange(self) -> Self {
                self.bg($crate::style::ColorToken::Orange)
            }

            pub fn bg_purple(self) -> Self {
                self.bg($crate::style::ColorToken::Purple)
            }

            pub fn bg_pink(self) -> Self {
                self.bg($crate::style::ColorToken::Pink)
            }

            pub fn bg_gray(self) -> Self {
                self.bg($crate::style::ColorToken::Gray)
            }

            pub fn bg_slate_50(self) -> Self {
                self.bg($crate::style::ColorToken::Slate50)
            }

            pub fn bg_slate_200(self) -> Self {
                self.bg($crate::style::ColorToken::Slate200)
            }

            pub fn bg_slate_300(self) -> Self {
                self.bg($crate::style::ColorToken::Slate300)
            }

            pub fn bg_slate_400(self) -> Self {
                self.bg($crate::style::ColorToken::Slate400)
            }

            pub fn bg_slate_500(self) -> Self {
                self.bg($crate::style::ColorToken::Slate500)
            }

            pub fn bg_slate_600(self) -> Self {
                self.bg($crate::style::ColorToken::Slate600)
            }

            pub fn bg_slate_700(self) -> Self {
                self.bg($crate::style::ColorToken::Slate700)
            }

            pub fn bg_slate_800(self) -> Self {
                self.bg($crate::style::ColorToken::Slate800)
            }

            pub fn bg_slate_900(self) -> Self {
                self.bg($crate::style::ColorToken::Slate900)
            }

            pub fn bg_primary(self) -> Self {
                self.bg($crate::style::ColorToken::Primary)
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

            pub fn text_white(self) -> Self {
                self.text_color($crate::style::ColorToken::White)
            }

            pub fn text_black(self) -> Self {
                self.text_color($crate::style::ColorToken::Black)
            }

            pub fn text_red(self) -> Self {
                self.text_color($crate::style::ColorToken::Red)
            }

            pub fn text_green(self) -> Self {
                self.text_color($crate::style::ColorToken::Green)
            }

            pub fn text_blue(self) -> Self {
                self.text_color($crate::style::ColorToken::Blue)
            }

            pub fn text_yellow(self) -> Self {
                self.text_color($crate::style::ColorToken::Yellow)
            }

            pub fn text_teal_400(self) -> Self {
                self.text_color($crate::style::ColorToken::Teal400)
            }

            pub fn text_teal_500(self) -> Self {
                self.text_color($crate::style::ColorToken::Teal500)
            }

            pub fn text_orange(self) -> Self {
                self.text_color($crate::style::ColorToken::Orange)
            }

            pub fn text_purple(self) -> Self {
                self.text_color($crate::style::ColorToken::Purple)
            }

            pub fn text_pink(self) -> Self {
                self.text_color($crate::style::ColorToken::Pink)
            }

            pub fn text_gray(self) -> Self {
                self.text_color($crate::style::ColorToken::Gray)
            }

            pub fn text_slate_400(self) -> Self {
                self.text_color($crate::style::ColorToken::Slate400)
            }

            pub fn text_slate_500(self) -> Self {
                self.text_color($crate::style::ColorToken::Slate500)
            }

            pub fn text_slate_600(self) -> Self {
                self.text_color($crate::style::ColorToken::Slate600)
            }

            pub fn text_slate_700(self) -> Self {
                self.text_color($crate::style::ColorToken::Slate700)
            }

            pub fn text_slate_800(self) -> Self {
                self.text_color($crate::style::ColorToken::Slate800)
            }

            pub fn text_slate_900(self) -> Self {
                self.text_color($crate::style::ColorToken::Slate900)
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

            pub fn font_medium(self) -> Self {
                self.font_weight($crate::style::FontWeight::Medium)
            }

            pub fn font_semibold(self) -> Self {
                self.font_weight($crate::style::FontWeight::SemiBold)
            }

            pub fn font_bold(self) -> Self {
                self.font_weight($crate::style::FontWeight::Bold)
            }

            // === Shadow ===
            pub fn shadow(mut self, style: $crate::style::ShadowStyle) -> Self {
                self.style.shadow = Some(style);
                self
            }

            pub fn shadow_sm(self) -> Self {
                self.shadow($crate::style::ShadowStyle::SM)
            }

            pub fn shadow_md(self) -> Self {
                self.shadow($crate::style::ShadowStyle::MD)
            }

            pub fn shadow_lg(self) -> Self {
                self.shadow($crate::style::ShadowStyle::LG)
            }

            pub fn shadow_xl(self) -> Self {
                self.shadow($crate::style::ShadowStyle::XL)
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

            // === Data Attributes (for JS animation targeting) ===
            pub fn data_id(mut self, id: &str) -> Self {
                self.style.data_id = Some(id.to_string());
                self
            }
        }
    };
}

pub(crate) use impl_node_style_api;
