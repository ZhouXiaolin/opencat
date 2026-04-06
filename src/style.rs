use std::sync::Arc;

use skia_safe::Color;

use crate::script::ScriptDriver;

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
}

/// Cross axis alignment - Tailwind: items-*
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum AlignItems {
    #[default]
    Start,
    Center,
    End,
    Stretch,
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
    #[default]
    Normal,
    Medium,
    SemiBold,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShadowStyle {
    SM,
    MD,
    LG,
    XL,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GradientDirection {
    ToRight,
    ToLeft,
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
    pub padding_top: Option<f32>,
    pub padding_right: Option<f32>,
    pub padding_bottom: Option<f32>,
    pub padding_left: Option<f32>,
    pub margin: Option<f32>,
    pub margin_x: Option<f32>,
    pub margin_y: Option<f32>,
    pub margin_top: Option<f32>,
    pub margin_right: Option<f32>,
    pub margin_bottom: Option<f32>,
    pub margin_left: Option<f32>,

    // Layout
    pub flex_direction: Option<FlexDirection>,
    pub justify_content: Option<JustifyContent>,
    pub align_items: Option<AlignItems>,
    pub is_flex: bool,
    pub auto_size: bool,
    pub gap: Option<f32>,
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
    pub border_radius: Option<f32>,
    pub border_width: Option<f32>,
    pub border_color: Option<ColorToken>,
    pub object_fit: Option<ObjectFit>,
    pub overflow_hidden: bool,
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
                self.style.margin_top = Some(value);
                self
            }

            pub fn mb(mut self, value: f32) -> Self {
                self.style.margin_bottom = Some(value);
                self
            }

            pub fn ml(mut self, value: f32) -> Self {
                self.style.margin_left = Some(value);
                self
            }

            pub fn mr(mut self, value: f32) -> Self {
                self.style.margin_right = Some(value);
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

            pub fn stroke_width(mut self, width: f32) -> Self {
                self.style.border_width = Some(width.max(0.0));
                self
            }

            pub fn stroke_color(mut self, color: $crate::style::ColorToken) -> Self {
                self.style.border_color = Some(color);
                self
            }

            pub fn fill_color(mut self, color: $crate::style::ColorToken) -> Self {
                self.style.bg_color = Some(color);
                self.style.bg_gradient_from = None;
                self.style.bg_gradient_via = None;
                self.style.bg_gradient_to = None;
                self.style.bg_gradient_direction = None;
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

            // === Identity ===
            pub fn id(mut self, id: &str) -> Self {
                self.style.id = id.to_string();
                self
            }

            pub fn script_driver(mut self, driver: $crate::script::ScriptDriver) -> Self {
                self.style.script_driver = Some(std::sync::Arc::new(driver));
                self
            }

            pub fn script_source(self, source: &str) -> anyhow::Result<Self> {
                let driver = $crate::script::ScriptDriver::from_source(source)?;
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
