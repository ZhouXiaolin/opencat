use crate::style::{
    AlignItems, BackgroundFill, BoxShadow, ComputedTextStyle, DropShadow, FlexDirection, FlexWrap,
    GridAutoFlow, GridAutoRows, GridPlacement, InsetShadow, JustifyContent, LengthPercentageAuto,
    ObjectFit, Position, Transform,
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
        let container_has_definite_width = style.layout.width.is_some() || style.layout.width_full;
        // `position: absolute` with `width: auto` resolves via CSS shrink-to-fit —
        // its width comes from its own content, not from the containing block.
        // Inline-width constraints from ancestors must stop at such a boundary,
        // otherwise `wrap_text` leaks past it and breaks shrink-to-fit for text.
        let breaks_width_inheritance =
            style.layout.position == Position::Absolute && !container_has_definite_width;
        let parent_has_inline_constraint = if breaks_width_inheritance {
            false
        } else {
            container_has_definite_width || style.text.wrap_text
        };
        let parent_stacks_text_vertically = !style.layout.is_flex
            || ((style.layout.flex_direction == FlexDirection::Col
                || style.layout.flex_direction == FlexDirection::ColReverse)
                && style.layout.align_items == AlignItems::Stretch);
        text.wrap_text = parent_has_inline_constraint && parent_stacks_text_vertically;
        Self { text }
    }
}

#[derive(Clone, Debug)]
pub struct ComputedLayoutStyle {
    pub position: Position,
    pub inset_left: Option<LengthPercentageAuto>,
    pub inset_top: Option<LengthPercentageAuto>,
    pub inset_right: Option<LengthPercentageAuto>,
    pub inset_bottom: Option<LengthPercentageAuto>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub max_width: Option<f32>,
    pub width_full: bool,
    pub height_full: bool,
    pub padding_top: f32,
    pub padding_right: f32,
    pub padding_bottom: f32,
    pub padding_left: f32,
    pub margin_top: LengthPercentageAuto,
    pub margin_right: LengthPercentageAuto,
    pub margin_bottom: LengthPercentageAuto,
    pub margin_left: LengthPercentageAuto,
    pub min_height: Option<LengthPercentageAuto>,
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
    pub flex_direction: FlexDirection,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub flex_wrap: FlexWrap,
    pub align_content: Option<JustifyContent>,
    pub align_self: Option<AlignItems>,
    pub justify_items: Option<AlignItems>,
    pub justify_self: Option<AlignItems>,
    pub gap: f32,
    pub gap_x: Option<f32>,
    pub gap_y: Option<f32>,
    pub order: i32,
    pub aspect_ratio: Option<f32>,
    pub flex_basis: Option<LengthPercentageAuto>,
    pub flex_grow: f32,
    pub flex_shrink: Option<f32>,
    pub z_index: i32,
}

#[derive(Clone, Debug)]
pub struct ComputedVisualStyle {
    pub opacity: f32,
    pub background: Option<BackgroundFill>,
    pub fill: Option<BackgroundFill>,
    pub border_radius: crate::style::BorderRadius,
    pub border_width: Option<f32>,
    pub border_top_width: Option<f32>,
    pub border_right_width: Option<f32>,
    pub border_bottom_width: Option<f32>,
    pub border_left_width: Option<f32>,
    pub border_color: Option<crate::style::ColorToken>,
    pub stroke_color: Option<crate::style::ColorToken>,
    pub stroke_width: Option<f32>,
    pub border_style: Option<crate::style::BorderStyle>,
    pub blur_sigma: Option<f32>,
    pub backdrop_blur_sigma: Option<f32>,
    pub object_fit: ObjectFit,
    pub clip_contents: bool,
    pub transforms: Vec<Transform>,
    pub box_shadow: Option<BoxShadow>,
    pub inset_shadow: Option<InsetShadow>,
    pub drop_shadow: Option<DropShadow>,
}
