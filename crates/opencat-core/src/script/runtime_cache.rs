//! Back-compat: [`ScriptRuntimeCache`] is a single [`super::ScriptRealm`].
//!
//! Historically this type cached one `ScriptRunner` (one JS context) per script
//! source hash, which broke same-composition shared realm state and forced web
//! to rebind a shared `globalThis`. Isolation is now one realm per pipeline.

use anyhow::Result;

use crate::frame_ctx::ScriptFrameCtx;
use crate::script::js_context::JsContext;
use crate::script::recorder::MutationRecorder;
use crate::script::realm::ScriptRealm;
use crate::script::{
    ScriptDriverId, ScriptHost, ScriptTargetRegistry, ScriptTextSource,
};

/// Pipeline script host that installs many drivers into one realm.
pub struct ScriptRuntimeCache<C: JsContext> {
    realm: Option<ScriptRealm<C>>,
}

impl<C: JsContext> Default for ScriptRuntimeCache<C> {
    fn default() -> Self {
        Self { realm: None }
    }
}

impl<C: JsContext> ScriptRuntimeCache<C> {
    fn realm_mut(&mut self) -> Result<&mut ScriptRealm<C>> {
        if self.realm.is_none() {
            self.realm = Some(ScriptRealm::open()?);
        }
        Ok(self.realm.as_mut().expect("just inserted"))
    }

    pub fn clear_text_sources(&mut self) {
        if let Some(realm) = self.realm.as_mut() {
            realm.clear_text_sources();
        }
    }

    pub fn register_text_source(&mut self, id: &str, source: ScriptTextSource) {
        if let Ok(realm) = self.realm_mut() {
            realm.register_text_source(id, source);
        }
    }
}

impl<C: JsContext> ScriptHost for ScriptRuntimeCache<C> {
    fn install(&mut self, source: &str) -> Result<ScriptDriverId> {
        self.realm_mut()?.install(source)
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
    ) -> Result<()> {
        self.realm_mut()?
            .run_frame(driver, frame_ctx, current_node_id, recorder)
    }

    fn set_target_registry(&mut self, registry: ScriptTargetRegistry) {
        if let Ok(realm) = self.realm_mut() {
            realm.set_target_registry(registry);
        }
    }

    fn set_style_defaults(
        &mut self,
        defaults: &std::collections::HashMap<
            String,
            std::collections::HashMap<String, serde_json::Value>,
        >,
    ) {
        if let Ok(realm) = self.realm_mut() {
            realm.set_style_defaults(defaults);
        }
    }

    fn set_initial_style_from_node(&mut self, id: &str, style: &crate::style::NodeStyle) {
        if let Ok(realm) = self.realm_mut() {
            realm.set_initial_style_from_node(id, style);
        }
    }
}
