use std::collections::HashMap;

use anyhow::anyhow;

use crate::frame_ctx::ScriptFrameCtx;
use crate::script::js_context::JsContext;
use crate::script::recorder::MutationRecorder;
use crate::script::script_runner::ScriptRunner;
use crate::script::{ScriptDriverId, ScriptHost, ScriptTargetRegistry, ScriptTextSource, driver_id_from_source};

pub struct ScriptRuntimeCache<C: JsContext> {
    runners: HashMap<u64, ScriptRunner<C>>,
    text_sources: HashMap<String, ScriptTextSource>,
    target_registry: Option<ScriptTargetRegistry>,
}

impl<C: JsContext> Default for ScriptRuntimeCache<C> {
    fn default() -> Self {
        Self {
            runners: HashMap::new(),
            text_sources: HashMap::new(),
            target_registry: None,
        }
    }
}

impl<C: JsContext> ScriptRuntimeCache<C> {
    pub fn clear_text_sources(&mut self) {
        self.text_sources.clear();
    }

    pub fn register_text_source(&mut self, id: &str, source: ScriptTextSource) {
        self.text_sources.insert(id.to_string(), source);
    }
}

impl<C: JsContext> ScriptHost for ScriptRuntimeCache<C> {
    fn install(&mut self, source: &str) -> anyhow::Result<ScriptDriverId> {
        let key = driver_id_from_source(source).0;
        if let std::collections::hash_map::Entry::Vacant(e) = self.runners.entry(key) {
            e.insert(ScriptRunner::<C>::new(source)?);
        }
        Ok(ScriptDriverId(key))
    }

    fn register_text_source(&mut self, node_id: &str, source: ScriptTextSource) {
        ScriptRuntimeCache::register_text_source(self, node_id, source);
    }

    fn clear_text_sources(&mut self) {
        ScriptRuntimeCache::clear_text_sources(self);
    }

    fn run_frame(
        &mut self,
        driver: ScriptDriverId,
        frame_ctx: &ScriptFrameCtx,
        current_node_id: Option<&str>,
        recorder: &mut dyn MutationRecorder,
    ) -> anyhow::Result<()> {
        let runner = self
            .runners
            .get_mut(&driver.0)
            .ok_or_else(|| anyhow!("script driver {} not installed", driver.0))?;
        runner.set_text_sources(&self.text_sources);
        if let Some(reg) = &self.target_registry {
            runner.set_target_registry(reg.clone());
        }
        runner.run_into(frame_ctx, current_node_id, recorder)
    }

    fn set_target_registry(&mut self, registry: ScriptTargetRegistry) {
        self.target_registry = Some(registry);
    }
}
