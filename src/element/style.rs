use crate::style::{
    AlignItems, BackgroundFill, ComputedTextStyle, FlexDirection, JustifyContent, ObjectFit,
    Position, ShadowStyle, Transform,
};

#[derive(Clone, Debug)]
pub struct ComputedStyle {
    pub layout: ComputedLayoutStyle,
    pub visual: ComputedVisualStyle,
    pub text: ComputedTextStyle,
    pub id: String,
}

/// Only properties in this struct are allowed to flow from parent to child.
/// Everything else remains local to the node that declared it, or applies to the
/// rendered subtree at paint time.
#[derive(Clone, Debug, Default)]
pub struct InheritedStyle {
    pub text: ComputedTextStyle,
}

impl InheritedStyle {
    pub fn for_child(style: &ComputedStyle) -> Self {
        let mut text = style.text;
        let parent_has_inline_constraint =
            style.layout.width.is_some() || style.layout.width_full || style.text.wrap_text;
        let parent_stacks_text_vertically = !style.layout.is_flex
            || (style.layout.flex_direction == FlexDirection::Col
                && style.layout.align_items == AlignItems::Stretch);
        text.wrap_text = parent_has_inline_constraint && parent_stacks_text_vertically;
        Self { text }
    }
}

#[derive(Clone, Debug)]
pub struct ComputedLayoutStyle {
    pub position: Position,
    pub inset_left: Option<f32>,
    pub inset_top: Option<f32>,
    pub inset_right: Option<f32>,
    pub inset_bottom: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub width_full: bool,
    pub height_full: bool,
    pub padding_top: f32,
    pub padding_right: f32,
    pub padding_bottom: f32,
    pub padding_left: f32,
    pub margin_top: f32,
    pub margin_right: f32,
    pub margin_bottom: f32,
    pub margin_left: f32,
    pub is_flex: bool,
    pub auto_size: bool,
    pub flex_direction: FlexDirection,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub gap: f32,
    pub flex_grow: f32,
    pub flex_shrink: Option<f32>,
    pub z_index: i32,
}

#[derive(Clone, Debug)]
pub struct ComputedVisualStyle {
    pub opacity: f32,
    pub background: Option<BackgroundFill>,
    pub border_radius: f32,
    pub border_width: Option<f32>,
    pub border_color: Option<crate::style::ColorToken>,
    pub blur_sigma: Option<f32>,
    pub object_fit: ObjectFit,
    pub clip_contents: bool,
    pub transforms: Vec<Transform>,
    pub shadow: Option<ShadowStyle>,
}
