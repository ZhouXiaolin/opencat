use crate::frame_ctx::ScriptFrameCtx;
use crate::scene::script::{
    ScriptDriverId, ScriptHost, ScriptTextSource, StyleMutations,
};
use crate::script::recorder::MutationRecorder;
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
        Ok(crate::scene::script::driver_id_from_source(source))
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::script::{NodeStyleMutations, ScriptHost, StyleMutations};
    use crate::script::recorder::MutationStore;
    use std::collections::HashMap;

    #[test]
    fn from_single_and_returns_mutations() {
        let mut node_mutations = HashMap::new();
        let node_muts = NodeStyleMutations {
            opacity: Some(0.5),
            ..Default::default()
        };
        node_mutations.insert("node1".to_string(), node_muts);

        let mutations = StyleMutations {
            mutations: node_mutations,
            canvas_mutations: HashMap::new(),
        };

        let mut host = PrecomputedScriptHost::from_single(mutations);
        let id = host.install("test script").unwrap();
        let mut store = MutationStore::default();
        host.run_frame(id, &Default::default(), None, &mut store)
            .unwrap();
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
    fn run_frame_with_no_mutations_succeeds_silently() {
        let mut host = PrecomputedScriptHost::from_single(StyleMutations::default());
        let id = host.install("script").unwrap();
        let mut store = MutationStore::default();
        // First call applies empty mutations
        host.run_frame(id, &Default::default(), None, &mut store)
            .unwrap();
        // Second call also succeeds (no drain, so mutations are still available)
        host.run_frame(id, &Default::default(), None, &mut store)
            .unwrap();
    }

    #[test]
    fn from_json_parses_and_returns_mutations() {
        let json =
            r#"{"mutations":{"node1":{"opacity":0.5,"transforms":[]}},"canvasMutations":{}}"#;
        let mut host = PrecomputedScriptHost::from_json(json).unwrap();
        let id = host.install("test script").unwrap();
        let mut store = MutationStore::default();
        host.run_frame(id, &Default::default(), None, &mut store)
            .unwrap();
        let snapshot = store.snapshot_mutations();
        let node_muts = snapshot.mutations.get("node1").unwrap();
        assert_eq!(node_muts.opacity, Some(0.5));
    }
}
