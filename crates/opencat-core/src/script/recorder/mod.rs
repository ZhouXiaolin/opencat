//! Mutation recorder trait + default `MutationStore` implementation.
//!
//! This is the unified interface for accumulating per-frame style/canvas
//! mutations from a script driver. Both the engine (quickjs bindings) and
//! the web target (wasm-bindgen wrappers) implement / consume this trait.

mod store;

pub use store::MutationStore;

use crate::scene::script::mutations::{
    CanvasCommand, StyleMutations, TextUnitGranularity,
};
use crate::scene::script::ScriptTextSource;
use crate::style::{
    AlignItems, BorderStyle, BoxShadow, ColorToken, DropShadow, FlexDirection, FontWeight,
    InsetShadow, JustifyContent, ObjectFit, Position, TextAlign, Transform,
};

/// Per-text-unit override values.
#[derive(Debug, Default, Clone, Copy)]
pub struct TextUnitValues {
    pub opacity: Option<f32>,
    pub translate_x: Option<f32>,
    pub translate_y: Option<f32>,
    pub scale: Option<f32>,
    pub rotation_deg: Option<f32>,
    pub color: Option<ColorToken>,
}

/// Recorder receives mutation calls from the JS bridge layer.
///
/// The default implementation [`MutationStore`] accumulates everything in
/// HashMaps; platform-specific impls (e.g. wasm-bindgen wrapper) typically
/// delegate to the default impl after argument conversion.
pub trait MutationRecorder {
    fn record_opacity(&mut self, id: &str, v: f32);
    fn record_translate(&mut self, id: &str, x: f32, y: f32);
    fn record_translate_x(&mut self, id: &str, v: f32);
    fn record_translate_y(&mut self, id: &str, v: f32);
    fn record_scale(&mut self, id: &str, v: f32);
    fn record_scale_x(&mut self, id: &str, v: f32);
    fn record_scale_y(&mut self, id: &str, v: f32);
    fn record_rotate(&mut self, id: &str, deg: f32);
    fn record_skew_x(&mut self, id: &str, deg: f32);
    fn record_skew_y(&mut self, id: &str, deg: f32);
    fn record_skew(&mut self, id: &str, x_deg: f32, y_deg: f32);
    fn record_left(&mut self, id: &str, v: f32);
    fn record_top(&mut self, id: &str, v: f32);
    fn record_right(&mut self, id: &str, v: f32);
    fn record_bottom(&mut self, id: &str, v: f32);
    fn record_width(&mut self, id: &str, v: f32);
    fn record_height(&mut self, id: &str, v: f32);
    fn record_padding(&mut self, id: &str, v: f32);
    fn record_padding_x(&mut self, id: &str, v: f32);
    fn record_padding_y(&mut self, id: &str, v: f32);
    fn record_margin(&mut self, id: &str, v: f32);
    fn record_margin_x(&mut self, id: &str, v: f32);
    fn record_margin_y(&mut self, id: &str, v: f32);
    fn record_gap(&mut self, id: &str, v: f32);
    fn record_flex_grow(&mut self, id: &str, v: f32);
    fn record_border_radius(&mut self, id: &str, v: f32);
    fn record_border_width(&mut self, id: &str, v: f32);
    fn record_border_top_width(&mut self, id: &str, v: f32);
    fn record_border_right_width(&mut self, id: &str, v: f32);
    fn record_border_bottom_width(&mut self, id: &str, v: f32);
    fn record_border_left_width(&mut self, id: &str, v: f32);
    fn record_stroke_width(&mut self, id: &str, v: f32);
    fn record_stroke_dasharray(&mut self, id: &str, v: f32);
    fn record_stroke_dashoffset(&mut self, id: &str, v: f32);
    fn record_text_size(&mut self, id: &str, v: f32);
    fn record_letter_spacing(&mut self, id: &str, v: f32);
    fn record_line_height(&mut self, id: &str, v: f32);

    fn record_position(&mut self, id: &str, pos: Position);
    fn record_flex_direction(&mut self, id: &str, fd: FlexDirection);
    fn record_justify_content(&mut self, id: &str, jc: JustifyContent);
    fn record_align_items(&mut self, id: &str, ai: AlignItems);
    fn record_object_fit(&mut self, id: &str, of: ObjectFit);
    fn record_text_align(&mut self, id: &str, ta: TextAlign);
    fn record_border_style(&mut self, id: &str, bs: BorderStyle);
    fn record_font_weight(&mut self, id: &str, w: FontWeight);
    fn record_box_shadow(&mut self, id: &str, sh: BoxShadow);
    fn record_inset_shadow(&mut self, id: &str, sh: InsetShadow);
    fn record_drop_shadow(&mut self, id: &str, sh: DropShadow);

    fn record_bg_color(&mut self, id: &str, color: ColorToken);
    fn record_fill_color(&mut self, id: &str, color: ColorToken);
    fn record_stroke_color(&mut self, id: &str, color: ColorToken);
    fn record_border_color(&mut self, id: &str, color: ColorToken);
    fn record_text_color(&mut self, id: &str, color: ColorToken);
    fn record_box_shadow_color(&mut self, id: &str, color: ColorToken);
    fn record_inset_shadow_color(&mut self, id: &str, color: ColorToken);
    fn record_drop_shadow_color(&mut self, id: &str, color: ColorToken);

    fn record_transform(&mut self, id: &str, t: Transform);
    fn record_text_content(&mut self, id: &str, text: String);
    fn record_text_unit_override(
        &mut self,
        id: &str,
        granularity: TextUnitGranularity,
        index: usize,
        values: TextUnitValues,
    );
    fn record_svg_path(&mut self, id: &str, data: String);

    fn record_canvas_command(&mut self, id: &str, cmd: CanvasCommand);

    fn reset_for_frame(&mut self, current_frame: u32);
    fn snapshot_mutations(&self) -> StyleMutations;

    fn register_text_source(&mut self, id: &str, source: ScriptTextSource);
    fn clear_text_sources(&mut self);
    fn get_text_source(&self, id: &str) -> Option<&ScriptTextSource>;
    fn current_frame(&self) -> u32;
}
