use skia_safe::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorToken {
    White,
    Black,
    Red,
    Green,
    Blue,
    Yellow,
    Orange,
    Purple,
    Pink,
    Gray,
}

impl ColorToken {
    pub fn to_skia(self) -> Color {
        match self {
            ColorToken::White => Color::WHITE,
            ColorToken::Black => Color::BLACK,
            ColorToken::Red => Color::RED,
            ColorToken::Green => Color::from_rgb(0x22, 0xc5, 0x5e), // Tailwind green-500
            ColorToken::Blue => Color::from_rgb(0x3b, 0x82, 0xf6),  // Tailwind blue-500
            ColorToken::Yellow => Color::from_rgb(0xea, 0xb3, 0x08), // Tailwind yellow-500
            ColorToken::Orange => Color::from_rgb(0xf9, 0x73, 0x16), // Tailwind orange-500
            ColorToken::Purple => Color::from_rgb(0xa8, 0x55, 0xf7), // Tailwind purple-500
            ColorToken::Pink => Color::from_rgb(0xec, 0x48, 0x99),  // Tailwind pink-500
            ColorToken::Gray => Color::from_rgb(0x6b, 0x72, 0x80),  // Tailwind gray-500
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
    pub transforms: Vec<Transform>,

    // Text
    pub text_color: Option<ColorToken>,
    pub text_px: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
pub struct ComputedTextStyle {
    pub color: ColorToken,
    pub text_px: f32,
}

impl Default for ComputedTextStyle {
    fn default() -> Self {
        Self {
            color: ColorToken::Black,
            text_px: 16.0,
        }
    }
}

pub fn resolve_text_style(parent: &ComputedTextStyle, style: &NodeStyle) -> ComputedTextStyle {
    ComputedTextStyle {
        color: style.text_color.unwrap_or(parent.color),
        text_px: style.text_px.unwrap_or(parent.text_px),
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
                self
            }

            pub fn h(mut self, value: f32) -> Self {
                self.style.height = Some(value);
                self
            }

            pub fn size(mut self, width: f32, height: f32) -> Self {
                self.style.width = Some(width);
                self.style.height = Some(height);
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
            pub fn border(mut self, width: f32) -> Self {
                self.style.border_width = Some(width);
                self
            }

            pub fn border_color(mut self, color: $crate::style::ColorToken) -> Self {
                self.style.border_color = Some(color);
                self
            }

            // === Visual: Background Colors ===
            pub fn bg(mut self, color: $crate::style::ColorToken) -> Self {
                self.style.bg_color = Some(color);
                self
            }

            pub fn bg_white(mut self) -> Self {
                self.bg($crate::style::ColorToken::White)
            }

            pub fn bg_black(mut self) -> Self {
                self.bg($crate::style::ColorToken::Black)
            }

            pub fn bg_red(mut self) -> Self {
                self.bg($crate::style::ColorToken::Red)
            }

            pub fn bg_green(mut self) -> Self {
                self.bg($crate::style::ColorToken::Green)
            }

            pub fn bg_blue(mut self) -> Self {
                self.bg($crate::style::ColorToken::Blue)
            }

            pub fn bg_yellow(mut self) -> Self {
                self.bg($crate::style::ColorToken::Yellow)
            }

            pub fn bg_orange(mut self) -> Self {
                self.bg($crate::style::ColorToken::Orange)
            }

            pub fn bg_purple(mut self) -> Self {
                self.bg($crate::style::ColorToken::Purple)
            }

            pub fn bg_pink(mut self) -> Self {
                self.bg($crate::style::ColorToken::Pink)
            }

            pub fn bg_gray(mut self) -> Self {
                self.bg($crate::style::ColorToken::Gray)
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

            pub fn text_white(mut self) -> Self {
                self.text_color($crate::style::ColorToken::White)
            }

            pub fn text_black(mut self) -> Self {
                self.text_color($crate::style::ColorToken::Black)
            }

            pub fn text_red(mut self) -> Self {
                self.text_color($crate::style::ColorToken::Red)
            }

            pub fn text_green(mut self) -> Self {
                self.text_color($crate::style::ColorToken::Green)
            }

            pub fn text_blue(mut self) -> Self {
                self.text_color($crate::style::ColorToken::Blue)
            }

            pub fn text_yellow(mut self) -> Self {
                self.text_color($crate::style::ColorToken::Yellow)
            }

            pub fn text_orange(mut self) -> Self {
                self.text_color($crate::style::ColorToken::Orange)
            }

            pub fn text_purple(mut self) -> Self {
                self.text_color($crate::style::ColorToken::Purple)
            }

            pub fn text_pink(mut self) -> Self {
                self.text_color($crate::style::ColorToken::Pink)
            }

            pub fn text_gray(mut self) -> Self {
                self.text_color($crate::style::ColorToken::Gray)
            }
        }
    };
}

pub(crate) use impl_node_style_api;
