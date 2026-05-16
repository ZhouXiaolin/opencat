use std::collections::HashMap;

use anyhow::anyhow;

use crate::frame_ctx::ScriptFrameCtx;
use crate::scene::script::{driver_id_from_source, ScriptDriverId, ScriptHost, ScriptTextSource};
use crate::script::recorder::MutationRecorder;

pub trait Runner: Sized {
    fn from_source(source: &str) -> anyhow::Result<Self>;
    fn set_text_sources(&mut self, sources: &HashMap<String, ScriptTextSource>);
    fn run_into(
        &mut self,
        frame_ctx: &ScriptFrameCtx,
        current_node_id: Option<&str>,
        recorder: &mut dyn MutationRecorder,
    ) -> anyhow::Result<()>;
}

pub struct ScriptRuntimeCache<R: Runner> {
    runners: HashMap<u64, R>,
    text_sources: HashMap<String, ScriptTextSource>,
}

impl<R: Runner> Default for ScriptRuntimeCache<R> {
    fn default() -> Self {
        Self {
            runners: HashMap::new(),
            text_sources: HashMap::new(),
        }
    }
}

impl<R: Runner> ScriptRuntimeCache<R> {
    pub fn clear_text_sources(&mut self) {
        self.text_sources.clear();
    }

    pub fn register_text_source(&mut self, id: &str, source: ScriptTextSource) {
        self.text_sources.insert(id.to_string(), source);
    }
}

impl<R: Runner> ScriptHost for ScriptRuntimeCache<R> {
    fn install(&mut self, source: &str) -> anyhow::Result<ScriptDriverId> {
        let key = driver_id_from_source(source).0;
        if let std::collections::hash_map::Entry::Vacant(e) = self.runners.entry(key) {
            e.insert(R::from_source(source)?);
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
        runner.run_into(frame_ctx, current_node_id, recorder)
    }
}
