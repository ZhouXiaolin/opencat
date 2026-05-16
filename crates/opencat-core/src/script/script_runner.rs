//! 泛型 ScriptRunner —— 把脚本调度流程集中在 core。
//!
//! 端侧只需实现 JsContext trait；reset / set frame ctx / call run /
//! call flush / snapshot / apply 的顺序与字段名均由本模块决定。

use std::collections::HashMap;

use crate::frame_ctx::ScriptFrameCtx;
use crate::scene::script::ScriptTextSource;
use crate::script::js_context::JsContext;
use crate::script::recorder::MutationRecorder;
use crate::script::runtime::{ANIMATION_RUNTIME, CANVAS_API_RUNTIME, NODE_STYLE_RUNTIME};

pub struct ScriptRunner<C: JsContext> {
    ctx: C,
    #[allow(dead_code)]
    first_frame: bool,
}

impl<C: JsContext> ScriptRunner<C> {
    pub fn new(source: &str) -> anyhow::Result<Self> {
        let ctx = C::new()?;
        install_runtime(&ctx, source)?;
        Ok(Self {
            ctx,
            first_frame: true,
        })
    }

    pub fn set_text_sources(&mut self, sources: &HashMap<String, ScriptTextSource>) {
        self.ctx.with_store_mut(|s| {
            s.clear_text_sources();
            for (id, src) in sources {
                s.register_text_source(id, src.clone());
            }
        });
    }

    pub fn run_into(
        &mut self,
        frame_ctx: &ScriptFrameCtx,
        current_node_id: Option<&str>,
        recorder: &mut dyn MutationRecorder,
    ) -> anyhow::Result<()> {
        self.ctx
            .with_store_mut(|s| s.reset_for_frame(frame_ctx.current_frame));

        self.ctx.set_ctx_field_i64("frame", frame_ctx.frame as i64)?;
        self.ctx
            .set_ctx_field_i64("totalFrames", frame_ctx.total_frames as i64)?;
        self.ctx
            .set_ctx_field_i64("currentFrame", frame_ctx.current_frame as i64)?;
        self.ctx
            .set_ctx_field_i64("sceneFrames", frame_ctx.scene_frames as i64)?;
        self.ctx
            .set_ctx_field_str("__currentCanvasTarget", current_node_id.unwrap_or(""))?;

        self.ctx.call_global_fn("__opencatRunFrame")?;
        self.ctx.call_global_fn("__opencatFlushTimelines")?;

        let snap = self.ctx.with_store_mut(|s| s.snapshot_mutations());
        snap.apply_to_recorder(recorder);

        self.first_frame = false;
        Ok(())
    }
}

fn install_runtime<C: JsContext>(ctx: &C, user_source: &str) -> anyhow::Result<()> {
    ctx.eval(
        "globalThis.ctx = globalThis.ctx || {\
            frame:0, totalFrames:0, currentFrame:0, sceneFrames:0, \
            __currentCanvasTarget:''\
         };",
    )?;
    ctx.install_all_bindings()?;
    ctx.eval(NODE_STYLE_RUNTIME)?;
    ctx.eval(CANVAS_API_RUNTIME)?;
    ctx.eval(ANIMATION_RUNTIME)?;
    ctx.eval(&format!(
        "globalThis.__opencatRunFrame = function() {{\n{user_source}\n}};"
    ))?;
    ctx.eval(
        "globalThis.__opencatFlushTimelines = function() {\
            if (globalThis.ctx && globalThis.ctx.__flushTimelines) \
                globalThis.ctx.__flushTimelines();\
         };",
    )?;
    Ok(())
}
