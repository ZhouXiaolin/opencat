use crate::style::{
    AlignItems, ColorToken, ComputedTextStyle, FlexDirection, JustifyContent, ObjectFit, Position,
    ShadowStyle, Transform,
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
        Self { text: style.text }
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
    pub padding_x: f32,
    pub padding_y: f32,
    pub margin_x: f32,
    pub margin_y: f32,
    pub flex_direction: FlexDirection,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub gap: f32,
    pub flex_grow: f32,
}

#[derive(Clone, Debug)]
pub struct ComputedVisualStyle {
    pub opacity: f32,
    pub background: Option<ColorToken>,
    pub border_radius: f32,
    pub border_width: Option<f32>,
    pub border_color: Option<ColorToken>,
    pub object_fit: ObjectFit,
    pub transforms: Vec<Transform>,
    pub shadow: Option<ShadowStyle>,
}
