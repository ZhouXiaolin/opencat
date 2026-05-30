use crate::frame_ctx::ScriptFrameCtx;
use crate::script::recorder::MutationRecorder;
use crate::script::{
    ScriptDriverId, ScriptHost, ScriptTargetRegistry, ScriptTextSource, StyleMutations,
};
use anyhow::Result;
use std::collections::HashMap;

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
        recorder: &mut dyn MutationRecorder,
    ) -> Result<()> {
        let mutations_to_apply: Vec<StyleMutations> = self
            .mutations
            .get(&driver)
            .cloned()
            .map(|m| vec![m])
            .unwrap_or_else(|| {
                if self.mutations.is_empty() {
                    vec![]
                } else {
                    self.mutations.values().cloned().collect()
                }
            });

        for mutations in &mutations_to_apply {
            mutations.apply_to_recorder(recorder);
        }
        Ok(())
    }

    fn set_target_registry(&mut self, _registry: ScriptTargetRegistry) {}

    fn set_style_defaults(
        &mut self,
        _defaults: &std::collections::HashMap<
            String,
            std::collections::HashMap<String, serde_json::Value>,
        >,
    ) {
    }

    fn set_initial_style_from_node(&mut self, _id: &str, _style: &crate::style::NodeStyle) {}
}
