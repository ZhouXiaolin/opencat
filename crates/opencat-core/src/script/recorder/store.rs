use std::collections::HashMap;

use super::{MutationRecorder, TextUnitValues};
use crate::scene::script::mutations::{
    CanvasCommand, CanvasMutations, NodeStyleMutations, StyleMutations, TextUnitGranularity,
    TextUnitOverride, TextUnitOverrideBatch,
};
use crate::scene::script::ScriptTextSource;
use crate::style::{
    AlignItems, BorderStyle, BoxShadow, ColorToken, DropShadow, FlexDirection, FontWeight,
    InsetShadow, JustifyContent, ObjectFit, Position, TextAlign, Transform,
};

#[derive(Default)]
pub struct MutationStore {
    styles: HashMap<String, NodeStyleMutations>,
    canvases: HashMap<String, CanvasMutations>,
    text_sources: HashMap<String, ScriptTextSource>,
    current_frame: u32,
}

impl MutationStore {
    fn entry(&mut self, id: &str) -> &mut NodeStyleMutations {
        self.styles.entry(id.to_string()).or_default()
    }

    fn canvas_entry(&mut self, id: &str) -> &mut CanvasMutations {
        self.canvases.entry(id.to_string()).or_default()
    }
}

impl MutationRecorder for MutationStore {
    fn record_opacity(&mut self, id: &str, v: f32) {
        self.entry(id).opacity = Some(v);
    }
    fn record_translate(&mut self, id: &str, x: f32, y: f32) {
        self.entry(id).transforms.push(Transform::Translate { x, y });
    }
    fn record_translate_x(&mut self, id: &str, v: f32) {
        self.entry(id).transforms.push(Transform::TranslateX { value: v });
    }
    fn record_translate_y(&mut self, id: &str, v: f32) {
        self.entry(id).transforms.push(Transform::TranslateY { value: v });
    }
    fn record_scale(&mut self, id: &str, v: f32) {
        self.entry(id).transforms.push(Transform::Scale { value: v });
    }
    fn record_scale_x(&mut self, id: &str, v: f32) {
        self.entry(id).transforms.push(Transform::ScaleX { value: v });
    }
    fn record_scale_y(&mut self, id: &str, v: f32) {
        self.entry(id).transforms.push(Transform::ScaleY { value: v });
    }
    fn record_rotate(&mut self, id: &str, deg: f32) {
        self.entry(id).transforms.push(Transform::RotateDeg { value: deg });
    }
    fn record_skew_x(&mut self, id: &str, deg: f32) {
        self.entry(id).transforms.push(Transform::SkewXDeg { value: deg });
    }
    fn record_skew_y(&mut self, id: &str, deg: f32) {
        self.entry(id).transforms.push(Transform::SkewYDeg { value: deg });
    }
    fn record_skew(&mut self, id: &str, x_deg: f32, y_deg: f32) {
        self.entry(id).transforms.push(Transform::SkewDeg { x: x_deg, y: y_deg });
    }
    fn record_left(&mut self, id: &str, v: f32) { self.entry(id).inset_left = Some(v); }
    fn record_top(&mut self, id: &str, v: f32) { self.entry(id).inset_top = Some(v); }
    fn record_right(&mut self, id: &str, v: f32) { self.entry(id).inset_right = Some(v); }
    fn record_bottom(&mut self, id: &str, v: f32) { self.entry(id).inset_bottom = Some(v); }
    fn record_width(&mut self, id: &str, v: f32) { self.entry(id).width = Some(v); }
    fn record_height(&mut self, id: &str, v: f32) { self.entry(id).height = Some(v); }
    fn record_padding(&mut self, id: &str, v: f32) { self.entry(id).padding = Some(v); }
    fn record_padding_x(&mut self, id: &str, v: f32) { self.entry(id).padding_x = Some(v); }
    fn record_padding_y(&mut self, id: &str, v: f32) { self.entry(id).padding_y = Some(v); }
    fn record_margin(&mut self, id: &str, v: f32) { self.entry(id).margin = Some(v); }
    fn record_margin_x(&mut self, id: &str, v: f32) { self.entry(id).margin_x = Some(v); }
    fn record_margin_y(&mut self, id: &str, v: f32) { self.entry(id).margin_y = Some(v); }
    fn record_gap(&mut self, id: &str, v: f32) { self.entry(id).gap = Some(v); }
    fn record_flex_grow(&mut self, id: &str, v: f32) { self.entry(id).flex_grow = Some(v); }
    fn record_border_radius(&mut self, id: &str, v: f32) { self.entry(id).border_radius = Some(v); }
    fn record_border_width(&mut self, id: &str, v: f32) { self.entry(id).border_width = Some(v); }
    fn record_border_top_width(&mut self, id: &str, v: f32) {
        self.entry(id).border_top_width = Some(v);
    }
    fn record_border_right_width(&mut self, id: &str, v: f32) {
        self.entry(id).border_right_width = Some(v);
    }
    fn record_border_bottom_width(&mut self, id: &str, v: f32) {
        self.entry(id).border_bottom_width = Some(v);
    }
    fn record_border_left_width(&mut self, id: &str, v: f32) {
        self.entry(id).border_left_width = Some(v);
    }
    fn record_stroke_width(&mut self, id: &str, v: f32) {
        self.entry(id).stroke_width = Some(v.max(0.0));
    }
    fn record_stroke_dasharray(&mut self, id: &str, v: f32) {
        self.entry(id).stroke_dasharray = Some(v.max(0.0));
    }
    fn record_stroke_dashoffset(&mut self, id: &str, v: f32) {
        self.entry(id).stroke_dashoffset = Some(v);
    }
    fn record_text_size(&mut self, id: &str, v: f32) { self.entry(id).text_px = Some(v); }
    fn record_letter_spacing(&mut self, id: &str, v: f32) {
        self.entry(id).letter_spacing = Some(v);
    }
    fn record_line_height(&mut self, id: &str, v: f32) { self.entry(id).line_height = Some(v); }

    fn record_position(&mut self, id: &str, pos: Position) {
        self.entry(id).position = Some(pos);
    }
    fn record_flex_direction(&mut self, id: &str, fd: FlexDirection) {
        self.entry(id).flex_direction = Some(fd);
    }
    fn record_justify_content(&mut self, id: &str, jc: JustifyContent) {
        self.entry(id).justify_content = Some(jc);
    }
    fn record_align_items(&mut self, id: &str, ai: AlignItems) {
        self.entry(id).align_items = Some(ai);
    }
    fn record_object_fit(&mut self, id: &str, of: ObjectFit) {
        self.entry(id).object_fit = Some(of);
    }
    fn record_text_align(&mut self, id: &str, ta: TextAlign) {
        self.entry(id).text_align = Some(ta);
    }
    fn record_border_style(&mut self, id: &str, bs: BorderStyle) {
        self.entry(id).border_style = Some(bs);
    }
    fn record_font_weight(&mut self, id: &str, w: FontWeight) {
        self.entry(id).font_weight = Some(w);
    }
    fn record_box_shadow(&mut self, id: &str, sh: BoxShadow) {
        self.entry(id).box_shadow = Some(sh);
    }
    fn record_inset_shadow(&mut self, id: &str, sh: InsetShadow) {
        self.entry(id).inset_shadow = Some(sh);
    }
    fn record_drop_shadow(&mut self, id: &str, sh: DropShadow) {
        self.entry(id).drop_shadow = Some(sh);
    }

    fn record_bg_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).bg_color = Some(color);
    }
    fn record_fill_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).fill_color = Some(color);
    }
    fn record_stroke_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).stroke_color = Some(color);
    }
    fn record_border_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).border_color = Some(color);
    }
    fn record_text_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).text_color = Some(color);
    }
    fn record_box_shadow_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).box_shadow_color = Some(color);
    }
    fn record_inset_shadow_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).inset_shadow_color = Some(color);
    }
    fn record_drop_shadow_color(&mut self, id: &str, color: ColorToken) {
        self.entry(id).drop_shadow_color = Some(color);
    }

    fn record_transform(&mut self, id: &str, t: Transform) {
        self.entry(id).transforms.push(t);
    }

    fn record_text_content(&mut self, id: &str, text: String) {
        self.entry(id).text_content = Some(text.clone());
        self.text_sources.insert(
            id.to_string(),
            ScriptTextSource {
                text,
                kind: crate::scene::script::ScriptTextSourceKind::TextNode,
            },
        );
    }

    fn record_text_unit_override(
        &mut self,
        id: &str,
        granularity: TextUnitGranularity,
        index: usize,
        values: TextUnitValues,
    ) {
        let mutations = self.entry(id);
        match &mut mutations.text_unit_overrides {
            Some(batch) => {
                if batch.granularity != granularity {
                    return;
                }
                if index >= batch.overrides.len() {
                    batch.overrides.resize_with(index + 1, TextUnitOverride::default);
                }
            }
            None => {
                let mut batch = TextUnitOverrideBatch {
                    granularity,
                    overrides: Vec::new(),
                };
                batch.overrides.resize_with(index + 1, TextUnitOverride::default);
                mutations.text_unit_overrides = Some(batch);
            }
        }
        let entry = &mut mutations.text_unit_overrides.as_mut().unwrap().overrides[index];
        if let Some(v) = values.opacity { entry.opacity = Some(v); }
        if let Some(v) = values.translate_x { entry.translate_x = Some(v); }
        if let Some(v) = values.translate_y { entry.translate_y = Some(v); }
        if let Some(v) = values.scale { entry.scale = Some(v); }
        if let Some(v) = values.rotation_deg { entry.rotation_deg = Some(v); }
        if let Some(c) = values.color { entry.color = Some(c); }
    }

    fn record_svg_path(&mut self, id: &str, data: String) {
        self.entry(id).svg_path = Some(data);
    }

    fn record_canvas_command(&mut self, id: &str, cmd: CanvasCommand) {
        self.canvas_entry(id).commands.push(cmd);
    }

    fn reset_for_frame(&mut self, current_frame: u32) {
        self.styles.clear();
        self.canvases.clear();
        self.current_frame = current_frame;
    }

    fn snapshot_mutations(&self) -> StyleMutations {
        StyleMutations {
            mutations: self.styles.clone(),
            canvas_mutations: self.canvases.clone(),
        }
    }

    fn register_text_source(&mut self, id: &str, source: ScriptTextSource) {
        self.text_sources.insert(id.to_string(), source);
    }
    fn clear_text_sources(&mut self) {
        self.text_sources.clear();
    }
    fn get_text_source(&self, id: &str) -> Option<&ScriptTextSource> {
        self.text_sources.get(id)
    }
    fn current_frame(&self) -> u32 {
        self.current_frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::ColorToken;

    #[test]
    fn record_opacity_writes_into_styles() {
        let mut store = MutationStore::default();
        store.record_opacity("node-a", 0.7);
        let snap = store.snapshot_mutations();
        let entry = snap.mutations.get("node-a").expect("node-a recorded");
        assert_eq!(entry.opacity, Some(0.7));
    }

    #[test]
    fn record_translate_pushes_transform() {
        let mut store = MutationStore::default();
        store.record_translate("node-a", 12.0, -8.0);
        let snap = store.snapshot_mutations();
        let entry = snap.mutations.get("node-a").expect("node-a recorded");
        assert_eq!(entry.transforms.len(), 1);
    }

    #[test]
    fn record_bg_color_uses_color_token() {
        let mut store = MutationStore::default();
        store.record_bg_color("node-a", ColorToken::Custom(255, 0, 0, 255));
        let snap = store.snapshot_mutations();
        assert_eq!(
            snap.mutations.get("node-a").unwrap().bg_color,
            Some(ColorToken::Custom(255, 0, 0, 255))
        );
    }

    #[test]
    fn snapshot_does_not_clear_styles() {
        let mut store = MutationStore::default();
        store.record_opacity("node-a", 0.5);
        let _ = store.snapshot_mutations();
        let snap = store.snapshot_mutations();
        assert!(snap.mutations.contains_key("node-a"));
    }

    #[test]
    fn reset_for_frame_clears_styles_and_canvases() {
        let mut store = MutationStore::default();
        store.record_opacity("node-a", 0.5);
        store.reset_for_frame(7);
        let snap = store.snapshot_mutations();
        assert!(snap.mutations.is_empty());
    }
}
