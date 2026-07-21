//! Back-compat thin wrapper around [`super::ScriptRealm`].
//!
//! Historical `ScriptRunner` owned one context per script source. The isolation
//! unit is now the pipeline realm; a runner is a realm with a single preinstalled
//! driver. Prefer [`super::ScriptRealm`] / [`crate::script::ScriptHost`] directly.

use std::collections::HashMap;

use anyhow::Result;

use crate::frame_ctx::ScriptFrameCtx;
use crate::script::js_context::JsContext;
use crate::script::recorder::MutationRecorder;
use crate::script::realm::ScriptRealm;
use crate::script::{ScriptDriverId, ScriptHost, ScriptTargetRegistry, ScriptTextSource};

pub use super::realm::apply_target_registry;

/// Single-driver convenience over [`ScriptRealm`].
pub struct ScriptRunner<C: JsContext> {
    realm: ScriptRealm<C>,
    driver: ScriptDriverId,
}

impl<C: JsContext> ScriptRunner<C> {
    pub fn new(source: &str) -> Result<Self> {
        let mut realm = ScriptRealm::<C>::open()?;
        let driver = realm.install(source)?;
        Ok(Self { realm, driver })
    }

    pub fn set_text_sources(&mut self, sources: &HashMap<String, ScriptTextSource>) {
        self.realm.clear_text_sources();
        for (id, src) in sources {
            self.realm.register_text_source(id, src.clone());
        }
    }

    pub fn set_target_registry(&mut self, registry: ScriptTargetRegistry) {
        self.realm.set_target_registry(registry);
    }

    pub fn run_into(
        &mut self,
        frame_ctx: &ScriptFrameCtx,
        current_node_id: Option<&str>,
        recorder: &mut dyn MutationRecorder,
    ) -> Result<()> {
        self.realm
            .run_frame(self.driver, frame_ctx, current_node_id, recorder)
    }

    pub fn set_style_defaults(
        &mut self,
        defaults: &HashMap<String, HashMap<String, serde_json::Value>>,
    ) {
        self.realm.set_style_defaults(defaults);
    }

    pub fn set_initial_style_from_node(&mut self, id: &str, style: &crate::style::NodeStyle) {
        self.realm.set_initial_style_from_node(id, style);
    }
}
