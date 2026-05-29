use std::collections::HashSet;

use anyhow::Result;

use crate::frame_ctx::ScriptFrameCtx;
use crate::script::ScriptTextSource;
use crate::script::recorder::MutationRecorder;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScriptDriverId(pub u64);

pub fn driver_id_from_source(source: &str) -> ScriptDriverId {
    use ahash::AHasher;
    use std::hash::{Hash, Hasher};
    let mut h = AHasher::default();
    source.hash(&mut h);
    ScriptDriverId(h.finish())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScriptTargetRegistry {
    pub visual_ids: HashSet<String>,
    pub canvas_ids: HashSet<String>,
    pub non_visual_ids: HashSet<String>,
}

impl ScriptTargetRegistry {
    pub fn contains_visual(&self, id: &str) -> bool {
        self.visual_ids.contains(id)
    }

    pub fn contains_canvas(&self, id: &str) -> bool {
        self.canvas_ids.contains(id)
    }

    pub fn contains_non_visual(&self, id: &str) -> bool {
        self.non_visual_ids.contains(id)
    }
}

pub trait ScriptHost {
    fn install(&mut self, source: &str) -> Result<ScriptDriverId>;
    fn register_text_source(&mut self, node_id: &str, source: ScriptTextSource);
    fn clear_text_sources(&mut self);
    fn run_frame(
        &mut self,
        driver: ScriptDriverId,
        frame_ctx: &ScriptFrameCtx,
        current_node_id: Option<&str>,
        recorder: &mut dyn MutationRecorder,
    ) -> Result<()>;
    fn set_target_registry(&mut self, registry: ScriptTargetRegistry);
    /// Set base style values (from Tailwind/className) for all visible nodes.
    fn set_style_defaults(&mut self, _defaults: &std::collections::HashMap<String, std::collections::HashMap<String, serde_json::Value>>) {}
    /// Set base style for a single node from its resolved NodeStyle.
    fn set_initial_style_from_node(&mut self, _id: &str, _style: &crate::style::NodeStyle) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_returns_stable_id() {
        use crate::test_support::MockScriptHost;
        let mut host: Box<dyn ScriptHost> = Box::new(MockScriptHost::default());
        let id1 = host.install("ctx => {}").unwrap();
        let id2 = host.install("ctx => {}").unwrap();
        assert_eq!(id1, id2);
    }
}
