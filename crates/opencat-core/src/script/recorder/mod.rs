//! Mutation recorder trait + default `MutationStore` implementation.
//!
//! This is the unified interface for accumulating per-frame style/canvas
//! mutations from a script driver. Both the engine (V8 bindings) and
//! the web target (wasm-bindgen wrappers) implement / consume this trait.

mod store;

pub(crate) use store::css_filter_to_value;
pub use store::MutationStore;

use crate::ir::draw_op::DrawOp;
use crate::script::ScriptTextSource;
use crate::script::mutations::StyleMutations;
use crate::style::{
    AlignItems, BorderStyle, BoxShadow, ColorToken, DropShadow, FlexDirection, FontWeight,
    InsetShadow, JustifyContent, ObjectFit, Position, TextAlign, Transform, TransformOrigin,
};

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
    /// GSAP `rotationX`/`rotationY`/`transformPerspective` folded into one
    /// projective transform (see `Transform::Rotate3`).
    fn record_rotate3(&mut self, id: &str, rotation_x: f32, rotation_y: f32, perspective: f32);
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
    fn record_stroke_dasharray(&mut self, id: &str, v: crate::style::StrokeDash);
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
    fn record_transform_origin(&mut self, id: &str, origin: TransformOrigin);
    fn record_text_content(&mut self, id: &str, text: String);
    fn record_svg_path(&mut self, id: &str, data: String);

    fn record_draw_op(&mut self, id: &str, cmd: DrawOp);

    /// G2 chart cache: replay a cached build (false = miss, run a real build).
    fn chart_cache_fetch(&mut self, node_id: &str) -> bool { false }
    /// Snapshot the build that just ran; second call at a different frame
    /// time decides static (`Some(false)`) vs dynamic (`Some(true)`).
    fn chart_cache_commit(&mut self, node_id: &str) -> Option<bool> { let _ = node_id; None }
    /// Drop a node's cached chart.
    fn chart_cache_drop(&mut self, node_id: &str) { let _ = node_id; }

    /// three-like shim 每帧整树提交一次（同 id 覆盖，最后一次赢）。
    fn record_scene3(&mut self, id: &str, scene: crate::ir::scene3::Scene3);

    fn record_draw_picture(&mut self, target_id: &str, owner_id: &str, x: f32, y: f32);

    fn record_canvas_runtime_effect(
        &mut self,
        id: &str,
        sksl: String,
        uniforms_bytes: Vec<u8>,
        children: Vec<crate::ir::draw_types::ScriptRuntimeEffectChild>,
        dst: crate::ir::draw_op::Rect4,
    );

    fn reset_for_frame(&mut self, current_frame: u32, fps: u32);
    fn snapshot_mutations(&self) -> StyleMutations;

    fn read_style_value(&self, id: &str, property: &str) -> Option<serde_json::Value>;
    fn write_style_value(&mut self, id: &str, property: &str, value: serde_json::Value);

    fn register_text_source(&mut self, id: &str, source: ScriptTextSource);
    fn clear_text_sources(&mut self);
    fn get_text_source(&self, id: &str) -> Option<&ScriptTextSource>;

    /// Script-side DOM overlay (dynamic nodes + per-frame child views).
    /// Recorders that do not model a tree return `None` and the tree bindings
    /// report that they are unavailable.
    fn tree_overlay(&self) -> Option<&crate::script::tree::TreeOverlay> { None }
    fn tree_overlay_mut(&mut self) -> Option<&mut crate::script::tree::TreeOverlay> { None }

    /// Resolved text style per declared node, from the declarative tree.
    /// `getBoundingClientRect` shapes text to answer a box, and shaping needs
    /// the style; recorders that do not shape keep the default no-op.
    fn set_text_styles(&mut self, _styles: &std::collections::HashMap<String, crate::style::ComputedTextStyle>) {}
    fn text_style(&self, _id: &str) -> Option<&crate::style::ComputedTextStyle> { None }

    /// 声明文本节点下所有动态叶子的字节区间（含祖先链）。渲染桥据此把
    /// wrapper 上的样式变更换算成该元素的 `TextUnitOverrideBatch`。
    fn text_leaf_ranges(&mut self, _id: &str) -> Vec<(Vec<String>, usize, usize)> {
        Vec::new()
    }
    fn current_frame(&self) -> u32;
    /// Frame used to evaluate tween progress: the scene-local override when set
    /// (JSX `<tl>` scenes), otherwise the composition frame.
    fn script_frame(&self) -> u32;
    fn set_frame_override(&mut self, frame: Option<u32>);
    /// Time in seconds used to evaluate tween progress. Kept separate from the
    /// frame because tween progress advances by elapsed time, and a `<tl>` scene
    /// window can start between frames.
    fn script_time_secs(&self) -> f64;
    fn set_time_override_secs(&mut self, secs: Option<f64>);
}
