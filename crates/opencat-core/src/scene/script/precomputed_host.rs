use std::collections::HashMap;
use anyhow::{Result, anyhow};
use crate::frame_ctx::ScriptFrameCtx;
use crate::scene::script::{ScriptDriverId, ScriptHost, ScriptTextSource, StyleMutations};

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
    ) -> Result<StyleMutations> {
        self.mutations
            .drain()
            .next()
            .map(|(_, m)| m)
            .ok_or_else(|| anyhow!("no precomputed mutations available"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::script::{NodeStyleMutations, StyleMutations, ScriptHost};
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
        let result = host.run_frame(id, &Default::default(), None).unwrap();
        let node_muts = result.mutations.get("node1").unwrap();
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
        host.run_frame(id, &Default::default(), None).unwrap();
        assert!(host.run_frame(id, &Default::default(), None).is_err());
    }

    #[test]
    fn from_json_parses_and_returns_mutations() {
        let json = r#"{"mutations":{"node1":{"opacity":0.5,"transforms":[]}},"canvasMutations":{}}"#;
        let mut host = PrecomputedScriptHost::from_json(json).unwrap();
        let id = host.install("test script").unwrap();
        let result = host.run_frame(id, &Default::default(), None).unwrap();
        let node_muts = result.mutations.get("node1").unwrap();
        assert_eq!(node_muts.opacity, Some(0.5));
    }
}
