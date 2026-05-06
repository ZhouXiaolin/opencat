use std::collections::HashMap;
use anyhow::{Result, anyhow};
use crate::frame_ctx::ScriptFrameCtx;
use crate::scene::script::{NodeStyleMutations, ScriptDriverId, ScriptHost, ScriptTextSource, StyleMutations};
use crate::script::recorder::{MutationRecorder, TextUnitValues};
use crate::style::Transform;

/// ScriptHost that reads from precomputed mutations.
/// Web side runs scripts natively in JS and passes mutations via insert().
pub struct PrecomputedScriptHost {
    mutations: HashMap<ScriptDriverId, StyleMutations>,
}

impl PrecomputedScriptHost {
    /// Build an empty host.
    pub fn new() -> Self {
        Self {
            mutations: HashMap::new(),
        }
    }

    /// Build with pre-constructed StyleMutations.
    pub fn from_single(mutations: StyleMutations) -> Self {
        let mut map = HashMap::new();
        map.insert(ScriptDriverId(0), mutations);
        Self { mutations: map }
    }

    /// Insert mutations for a specific script driver.
    pub fn insert(&mut self, id: ScriptDriverId, mutations: StyleMutations) {
        self.mutations.insert(id, mutations);
    }

    /// Build host from JSON string. Format matches StyleMutations serialization.
    /// `{ "mutations": { "node-id": { "opacity": 0.5, ... } }, "canvasMutations": {} }`
    pub fn from_json(json: &str) -> Result<Self> {
        let mutations: StyleMutations = serde_json::from_str(json)?;
        Ok(Self::from_single(mutations))
    }
}

impl Default for PrecomputedScriptHost {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptHost for PrecomputedScriptHost {
    fn install(&mut self, source: &str) -> Result<ScriptDriverId> {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut h = DefaultHasher::new();
        source.hash(&mut h);
        Ok(ScriptDriverId(h.finish()))
    }

    fn register_text_source(&mut self, _node_id: &str, _source: ScriptTextSource) {
        // no-op
    }

    fn clear_text_sources(&mut self) {}

    fn run_frame(
        &mut self,
        _driver: ScriptDriverId,
        _frame_ctx: &ScriptFrameCtx,
        _current_node_id: Option<&str>,
        recorder: &mut dyn MutationRecorder,
    ) -> Result<()> {
        let all_mutations: Vec<StyleMutations> = self.mutations.drain().map(|(_, m)| m).collect();
        if all_mutations.is_empty() {
            return Err(anyhow!("no precomputed mutations available"));
        }
        for mutations in &all_mutations {
            for (node_id, node_mutations) in &mutations.mutations {
                apply_node_to_recorder(recorder, node_id, node_mutations);
            }
            for (canvas_id, canvas_mutations) in &mutations.canvas_mutations {
                for cmd in &canvas_mutations.commands {
                    recorder.record_canvas_command(canvas_id, cmd.clone());
                }
            }
        }
        Ok(())
    }
}

pub fn apply_node_to_recorder(recorder: &mut dyn MutationRecorder, id: &str, m: &NodeStyleMutations) {
    if let Some(v) = m.opacity {
        recorder.record_opacity(id, v);
    }
    if let Some(v) = m.inset_left {
        recorder.record_left(id, v);
    }
    if let Some(v) = m.inset_top {
        recorder.record_top(id, v);
    }
    if let Some(v) = m.inset_right {
        recorder.record_right(id, v);
    }
    if let Some(v) = m.inset_bottom {
        recorder.record_bottom(id, v);
    }
    if let Some(v) = m.width {
        recorder.record_width(id, v);
    }
    if let Some(v) = m.height {
        recorder.record_height(id, v);
    }
    if let Some(v) = m.padding {
        recorder.record_padding(id, v);
    }
    if let Some(v) = m.padding_x {
        recorder.record_padding_x(id, v);
    }
    if let Some(v) = m.padding_y {
        recorder.record_padding_y(id, v);
    }
    if let Some(v) = m.margin {
        recorder.record_margin(id, v);
    }
    if let Some(v) = m.margin_x {
        recorder.record_margin_x(id, v);
    }
    if let Some(v) = m.margin_y {
        recorder.record_margin_y(id, v);
    }
    if let Some(v) = m.gap {
        recorder.record_gap(id, v);
    }
    if let Some(v) = m.flex_grow {
        recorder.record_flex_grow(id, v);
    }
    if let Some(v) = m.border_radius {
        recorder.record_border_radius(id, v);
    }
    if let Some(v) = m.border_width {
        recorder.record_border_width(id, v);
    }
    if let Some(v) = m.border_top_width {
        recorder.record_border_top_width(id, v);
    }
    if let Some(v) = m.border_right_width {
        recorder.record_border_right_width(id, v);
    }
    if let Some(v) = m.border_bottom_width {
        recorder.record_border_bottom_width(id, v);
    }
    if let Some(v) = m.border_left_width {
        recorder.record_border_left_width(id, v);
    }
    if let Some(v) = m.stroke_width {
        recorder.record_stroke_width(id, v);
    }
    if let Some(v) = m.stroke_dasharray {
        recorder.record_stroke_dasharray(id, v);
    }
    if let Some(v) = m.stroke_dashoffset {
        recorder.record_stroke_dashoffset(id, v);
    }
    if let Some(v) = m.text_px {
        recorder.record_text_size(id, v);
    }
    if let Some(v) = m.letter_spacing {
        recorder.record_letter_spacing(id, v);
    }
    if let Some(v) = m.line_height {
        recorder.record_line_height(id, v);
    }
    if let Some(pos) = m.position {
        recorder.record_position(id, pos);
    }
    if let Some(fd) = m.flex_direction {
        recorder.record_flex_direction(id, fd);
    }
    if let Some(jc) = m.justify_content {
        recorder.record_justify_content(id, jc);
    }
    if let Some(ai) = m.align_items {
        recorder.record_align_items(id, ai);
    }
    if let Some(of) = m.object_fit {
        recorder.record_object_fit(id, of);
    }
    if let Some(ta) = m.text_align {
        recorder.record_text_align(id, ta);
    }
    if let Some(bs) = m.border_style {
        recorder.record_border_style(id, bs);
    }
    if let Some(w) = m.font_weight {
        recorder.record_font_weight(id, w);
    }
    if let Some(sh) = m.box_shadow {
        recorder.record_box_shadow(id, sh);
    }
    if let Some(sh) = m.inset_shadow {
        recorder.record_inset_shadow(id, sh);
    }
    if let Some(sh) = m.drop_shadow {
        recorder.record_drop_shadow(id, sh);
    }
    if let Some(color) = m.bg_color {
        recorder.record_bg_color(id, color);
    }
    if let Some(color) = m.fill_color {
        recorder.record_fill_color(id, color);
    }
    if let Some(color) = m.stroke_color {
        recorder.record_stroke_color(id, color);
    }
    if let Some(color) = m.border_color {
        recorder.record_border_color(id, color);
    }
    if let Some(color) = m.text_color {
        recorder.record_text_color(id, color);
    }
    if let Some(color) = m.box_shadow_color {
        recorder.record_box_shadow_color(id, color);
    }
    if let Some(color) = m.inset_shadow_color {
        recorder.record_inset_shadow_color(id, color);
    }
    if let Some(color) = m.drop_shadow_color {
        recorder.record_drop_shadow_color(id, color);
    }
    for t in &m.transforms {
        match *t {
            Transform::Translate { x, y } => recorder.record_translate(id, x, y),
            Transform::TranslateX { value } => recorder.record_translate_x(id, value),
            Transform::TranslateY { value } => recorder.record_translate_y(id, value),
            Transform::Scale { value } => recorder.record_scale(id, value),
            Transform::ScaleX { value } => recorder.record_scale_x(id, value),
            Transform::ScaleY { value } => recorder.record_scale_y(id, value),
            Transform::RotateDeg { value } => recorder.record_rotate(id, value),
            Transform::SkewXDeg { value } => recorder.record_skew_x(id, value),
            Transform::SkewYDeg { value } => recorder.record_skew_y(id, value),
            Transform::SkewDeg { x, y } => recorder.record_skew(id, x, y),
        }
    }
    if let Some(ref text) = m.text_content {
        recorder.record_text_content(id, text.clone());
    }
    if let Some(ref overrides_batch) = m.text_unit_overrides {
        let granularity = overrides_batch.granularity;
        for (index, override_val) in overrides_batch.overrides.iter().enumerate() {
            let values = TextUnitValues {
                opacity: override_val.opacity,
                translate_x: override_val.translate_x,
                translate_y: override_val.translate_y,
                scale: override_val.scale,
                rotation_deg: override_val.rotation_deg,
                color: override_val.color,
            };
            recorder.record_text_unit_override(id, granularity, index, values);
        }
    }
    if let Some(ref data) = m.svg_path {
        recorder.record_svg_path(id, data.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::script::{NodeStyleMutations, StyleMutations, ScriptHost};
    use crate::script::recorder::MutationStore;
    use std::collections::HashMap;

    #[test]
    fn from_single_and_returns_mutations() {
        let mut node_mutations = HashMap::new();
        let mut node_muts = NodeStyleMutations::default();
        node_muts.opacity = Some(0.5);
        node_mutations.insert("node1".to_string(), node_muts);

        let mutations = StyleMutations {
            mutations: node_mutations,
            canvas_mutations: HashMap::new(),
        };

        let mut host = PrecomputedScriptHost::from_single(mutations);
        let id = host.install("test script").unwrap();
        let mut store = MutationStore::default();
        host.run_frame(id, &Default::default(), None, &mut store).unwrap();
        let snapshot = store.snapshot_mutations();
        let node_muts = snapshot.mutations.get("node1").unwrap();
        assert_eq!(node_muts.opacity, Some(0.5));
    }

    #[test]
    fn install_returns_stable_hash() {
        let mut host = PrecomputedScriptHost::from_single(StyleMutations::default());
        let id1 = host.install("var x = 1;").unwrap();
        let id2 = host.install("var x = 1;").unwrap();
        assert_eq!(id1, id2);
        let id3 = host.install("var y = 2;").unwrap();
        assert_ne!(id1, id3);
    }

    #[test]
    fn run_frame_with_no_mutations_returns_error() {
        let mut host = PrecomputedScriptHost::from_single(StyleMutations::default());
        let id = host.install("script").unwrap();
        let mut store = MutationStore::default();
        host.run_frame(id, &Default::default(), None, &mut store).unwrap();
        assert!(host.run_frame(id, &Default::default(), None, &mut store).is_err());
    }

    #[test]
    fn from_json_parses_and_returns_mutations() {
        let json = r#"{"mutations":{"node1":{"opacity":0.5,"transforms":[]}},"canvasMutations":{}}"#;
        let mut host = PrecomputedScriptHost::from_json(json).unwrap();
        let id = host.install("test script").unwrap();
        let mut store = MutationStore::default();
        host.run_frame(id, &Default::default(), None, &mut store).unwrap();
        let snapshot = store.snapshot_mutations();
        let node_muts = snapshot.mutations.get("node1").unwrap();
        assert_eq!(node_muts.opacity, Some(0.5));
    }
}
