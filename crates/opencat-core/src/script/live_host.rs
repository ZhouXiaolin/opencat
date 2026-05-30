use anyhow::Result;
use serde_json::json;

use crate::frame_ctx::ScriptFrameCtx;
use crate::script::dispatch::{binding_shim_js, dispatch_binding};
use crate::script::js_context::JsContext;
use crate::script::recorder::MutationRecorder;
use crate::script::runtime::{ANIMATION_RUNTIME, CANVAS_API_RUNTIME, NODE_STYLE_RUNTIME};
use crate::script::{
    ScriptDriverId, ScriptHost, ScriptTargetRegistry, ScriptTextSource, driver_id_from_source,
};

pub struct LiveScriptHost<C: JsContext> {
    ctx: C,
    runtime_installed: bool,
    target_registry: Option<ScriptTargetRegistry>,
}

impl<C: JsContext> LiveScriptHost<C> {
    pub fn new(ctx: C) -> Result<Self> {
        Ok(Self {
            ctx,
            runtime_installed: false,
            target_registry: None,
        })
    }

    fn ensure_runtime(&mut self) -> Result<()> {
        if self.runtime_installed {
            return Ok(());
        }
        self.ctx.eval(
            "globalThis.ctx = globalThis.ctx || {\
             frame:0, fps:0, totalFrames:0, currentFrame:0, sceneFrames:0, \
             __currentCanvasTarget:'',\
             __targetRegistry:{visual:Object.create(null),canvas:Object.create(null),nonVisual:Object.create(null)}\
         };",
        )?;
        self.ctx.install_dispatcher(dispatch_binding)?;
        self.ctx.eval(&binding_shim_js())?;
        self.ctx.eval(NODE_STYLE_RUNTIME)?;
        self.ctx.eval(CANVAS_API_RUNTIME)?;
        self.ctx.eval(ANIMATION_RUNTIME)?;
        let flush_fn = String::from(
            "globalThis.__opencatFlushTimelines = function() {\
             if (globalThis.ctx && globalThis.ctx.__flushTimelines) \
             globalThis.ctx.__flushTimelines();\
         };",
        );
        self.ctx.eval(&flush_fn)?;
        if let Some(registry) = &self.target_registry {
            crate::script::script_runner::apply_target_registry(&self.ctx, registry)?;
        }
        self.runtime_installed = true;
        Ok(())
    }
}

impl<C: JsContext> ScriptHost for LiveScriptHost<C> {
    fn install(&mut self, source: &str) -> Result<ScriptDriverId> {
        self.ensure_runtime()?;
        let run_fn = format!("globalThis.__opencatRunFrame = function() {{\n{source}\n}};");
        self.ctx.eval(&run_fn)?;
        Ok(driver_id_from_source(source))
    }

    fn register_text_source(&mut self, node_id: &str, source: ScriptTextSource) {
        self.ctx.with_store_mut(|s| {
            s.register_text_source(node_id, source);
        });
    }

    fn clear_text_sources(&mut self) {
        self.ctx.with_store_mut(|s| {
            s.clear_text_sources();
        });
    }

    fn run_frame(
        &mut self,
        _driver: ScriptDriverId,
        frame_ctx: &ScriptFrameCtx,
        current_node_id: Option<&str>,
        recorder: &mut dyn MutationRecorder,
    ) -> Result<()> {
        self.ctx
            .with_store_mut(|s| s.reset_for_frame(frame_ctx.current_frame));

        self.ctx
            .set_ctx_field("frame", json!(frame_ctx.frame as i64))?;
        self.ctx.set_ctx_field("fps", json!(frame_ctx.fps as i64))?;
        self.ctx
            .set_ctx_field("totalFrames", json!(frame_ctx.total_frames as i64))?;
        self.ctx
            .set_ctx_field("currentFrame", json!(frame_ctx.current_frame as i64))?;
        self.ctx
            .set_ctx_field("sceneFrames", json!(frame_ctx.scene_frames as i64))?;
        self.ctx.set_ctx_field(
            "__currentCanvasTarget",
            json!(current_node_id.unwrap_or("")),
        )?;

        self.ctx.rebind_dispatcher()?;
        self.ctx.call_global_fn("__opencatRunFrame")?;
        self.ctx.call_global_fn("__opencatFlushTimelines")?;

        let snap = self.ctx.with_store_mut(|s| s.snapshot_mutations());
        snap.apply_to_recorder(recorder);

        Ok(())
    }

    fn set_target_registry(&mut self, registry: ScriptTargetRegistry) {
        self.target_registry = Some(registry);
    }

    fn set_style_defaults(
        &mut self,
        defaults: &std::collections::HashMap<
            String,
            std::collections::HashMap<String, serde_json::Value>,
        >,
    ) {
        self.ctx.with_store_mut(|s| {
            for (id, props) in defaults {
                for (prop, val) in props {
                    s.set_initial_style(id, prop, val.clone());
                }
            }
        });
    }

    fn set_initial_style_from_node(&mut self, id: &str, style: &crate::style::NodeStyle) {
        self.ctx.with_store_mut(|s| {
            s.set_initial_style_from_node(id, style);
        });
    }
}
