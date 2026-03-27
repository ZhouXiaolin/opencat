use skia_safe::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorToken {
    White,
    Black,
    Red,
}

impl ColorToken {
    pub fn to_skia(self) -> Color {
        match self {
            ColorToken::White => Color::WHITE,
            ColorToken::Black => Color::BLACK,
            ColorToken::Red => Color::RED,
        }
    }
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

/// Style context container - carries all possible style info for inheritance
#[derive(Debug, Clone, Copy, Default)]
pub struct NodeStyle {
    // Layout
    pub flex_direction: Option<FlexDirection>,
    pub justify_content: Option<JustifyContent>,
    pub align_items: Option<AlignItems>,
    pub gap: Option<f32>,
    pub flex_grow: Option<f32>,

    // Visual
    pub bg_color: Option<ColorToken>,
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
            // === Layout: Flex Direction ===
            pub fn flex_direction(
                mut self,
                direction: $crate::style::FlexDirection,
            ) -> Self {
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

            // === Visual: Text ===
            pub fn text_px(mut self, px: f32) -> Self {
                self.style.text_px = Some(px);
                self
            }

            pub fn text_black(mut self) -> Self {
                self.style.text_color = Some($crate::style::ColorToken::Black);
                self
            }

            pub fn text_red(mut self) -> Self {
                self.style.text_color = Some($crate::style::ColorToken::Red);
                self
            }

            pub fn text_white(mut self) -> Self {
                self.style.text_color = Some($crate::style::ColorToken::White);
                self
            }

            // === Visual: Background ===
            pub fn bg_white(mut self) -> Self {
                self.style.bg_color = Some($crate::style::ColorToken::White);
                self
            }

            pub fn bg_black(mut self) -> Self {
                self.style.bg_color = Some($crate::style::ColorToken::Black);
                self
            }

            pub fn bg_red(mut self) -> Self {
                self.style.bg_color = Some($crate::style::ColorToken::Red);
                self
            }
        }
    };
}

pub(crate) use impl_node_style_api;
