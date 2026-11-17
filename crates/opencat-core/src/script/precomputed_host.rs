use crate::frame_ctx::ScriptFrameCtx;
use crate::script::{
    ScriptDriverId, ScriptHost, ScriptTargetRegistry, ScriptTextSource, StyleMutations,
};
use anyhow::Result;
use std::collections::HashMap;

/// ScriptHost that reads from precomputed mutations.
/// Web side runs scripts natively in JS and passes mutations via insert().
pub struct PrecomputedScriptHost {
    mutations: HashMap<ScriptDriverId, StyleMutations>,
    text_styles: std::collections::HashMap<String, crate::style::ComputedTextStyle>,
}

impl PrecomputedScriptHost {
    /// Build an empty host.
    pub fn new() -> Self {
        Self {
            mutations: HashMap::new(),
            text_styles: Default::default(),
        }
    }

    /// Build with pre-constructed StyleMutations.
    pub fn from_single(mutations: StyleMutations) -> Self {
        let mut map = HashMap::new();
        map.insert(ScriptDriverId(0), mutations);
        Self {
            mutations: map,
            text_styles: Default::default(),
        }
    }

    pub fn set_text_styles(
        &mut self,
        styles: std::collections::HashMap<String, crate::style::ComputedTextStyle>,
    ) {
        self.text_styles = styles;
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
    fn text_styles(&self) -> &HashMap<String, crate::style::ComputedTextStyle> {
        &self.text_styles
    }

    fn install(&mut self, source: &str) -> Result<ScriptDriverId> {
        Ok(crate::script::driver_id_from_source(source))
    }

    fn register_text_source(&mut self, _node_id: &str, _source: ScriptTextSource) {
        // no-op
    }

    fn clear_text_sources(&mut self) {}

    fn run_frame(
        &mut self,
        driver: ScriptDriverId,
        _frame_ctx: &ScriptFrameCtx,
        _current_node_id: Option<&str>,
    ) -> Result<Vec<StyleMutations>> {
        if let Some(mutations) = self.mutations.get(&driver) {
            return Ok(vec![mutations.clone()]);
        }
        if !self.mutations.is_empty() {
            // Fallback（transport 时代残留语义）：全部层按 driver id 定序
            // 给出——HashMap 迭代序不定会让 last-wins 字段在层间漂移。
            // 合并统一由 resolve 侧 mutation_stack 消费方处理。
            let mut keys: Vec<_> = self.mutations.keys().map(|k| k.0).collect();
            keys.sort();
            return Ok(keys
                .into_iter()
                .filter_map(|k| self.mutations.get(&ScriptDriverId(k)).cloned())
                .collect());
        }
        Ok(Vec::new())
    }

    fn set_target_registry(&mut self, registry: ScriptTargetRegistry) {
        self.text_styles = registry.text_styles;
    }

    fn set_initial_style_from_node(&mut self, _id: &str, _style: &crate::style::NodeStyle) {}
}
