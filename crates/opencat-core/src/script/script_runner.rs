//! 泛型 ScriptRunner —— 把脚本调度流程集中在 core。
//!
//! 端侧只需实现 JsContext trait；reset / set frame ctx / call run /
//! call flush / snapshot / apply 的顺序与字段名均由本模块决定。

use std::collections::HashMap;

use serde_json::json;

use crate::frame_ctx::ScriptFrameCtx;
use crate::scene::script::ScriptTextSource;
use crate::script::dispatch::{binding_shim_js, dispatch_binding};
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

        self.ctx
            .set_ctx_field("frame", json!(frame_ctx.frame as i64))?;
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

        self.ctx.call_global_fn("__opencatRunFrame")?;
        self.ctx.call_global_fn("__opencatFlushTimelines")?;

        let snap = self.ctx.with_store_mut(|s| s.snapshot_mutations());
        snap.apply_to_recorder(recorder);

        self.first_frame = false;
        Ok(())
    }
}

fn install_runtime<C: JsContext>(ctx: &C, user_source: &str) -> anyhow::Result<()> {
    // 1. 兜底 globalThis.ctx（端侧 new() 已建过，这里只在尚未存在时初始化）。
    ctx.eval(
        "globalThis.ctx = globalThis.ctx || {\
            frame:0, totalFrames:0, currentFrame:0, sceneFrames:0, \
            __currentCanvasTarget:''\
         };",
    )?;

    // 2. 注册唯一的 native 入口 __opencatCallNative。
    ctx.install_dispatcher(|store, name, args| dispatch_binding(store, name, args))?;

    // 3. 装载 shim：把每个 binding 名包成 wrapper 调用 __opencatCallNative。
    //    必须在 runtime JS 之前——runtime JS 在 eval 时即可能定义引用 __record_* 等的函数。
    ctx.eval(&binding_shim_js())?;

    // 4. 共享 runtime JS。
    ctx.eval(NODE_STYLE_RUNTIME)?;
    ctx.eval(CANVAS_API_RUNTIME)?;
    ctx.eval(ANIMATION_RUNTIME)?;

    // 5. 把用户脚本包装为全局函数。
    ctx.eval(&format!(
        "globalThis.__opencatRunFrame = function() {{\n{user_source}\n}};"
    ))?;

    // 6. 把 ctx.__flushTimelines 别名为全局函数，与 call_global_fn 单一动词配套。
    //    依赖 ANIMATION_RUNTIME (facade.js) 已经把 ctx.__flushTimelines 装上去。
    ctx.eval(
        "globalThis.__opencatFlushTimelines = function() {\
            if (globalThis.ctx && globalThis.ctx.__flushTimelines) \
                globalThis.ctx.__flushTimelines();\
         };",
    )?;
    Ok(())
}
