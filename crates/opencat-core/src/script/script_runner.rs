//! 泛型 ScriptRunner —— 把脚本调度流程集中在 core。
//!
//! 端侧只需实现 JsContext trait；reset / set frame ctx / call run /
//! call flush / snapshot / apply 的顺序与字段名均由本模块决定。

use std::collections::HashMap;

use serde_json::json;

use crate::frame_ctx::ScriptFrameCtx;
use crate::script::ScriptTargetRegistry;
use crate::script::ScriptTextSource;
use crate::script::dispatch::{binding_shim_js, dispatch_binding};
use crate::script::js_context::JsContext;
use crate::script::recorder::MutationRecorder;
use crate::script::runtime::{ANIMATION_RUNTIME, CANVAS_API_RUNTIME, NODE_STYLE_RUNTIME};

pub struct ScriptRunner<C: JsContext> {
    ctx: C,
    #[allow(dead_code)]
    first_frame: bool,
    run_fn_source: String,
    flush_fn_source: String,
    target_registry: Option<ScriptTargetRegistry>,
}

impl<C: JsContext> ScriptRunner<C> {
    pub fn new(source: &str) -> anyhow::Result<Self> {
        let ctx = C::new()?;
        let (run_fn_source, flush_fn_source) = install_runtime(&ctx, source)?;
        Ok(Self {
            ctx,
            first_frame: true,
            run_fn_source,
            flush_fn_source,
            target_registry: None,
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

    pub fn set_target_registry(&mut self, registry: ScriptTargetRegistry) {
        self.target_registry = Some(registry);
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

        if let Some(registry) = &self.target_registry {
            apply_target_registry(&self.ctx, registry)?;
        }

        // Web 端共享全局作用域，多个 runner 的 __opencatCallNative 会互相覆盖。
        // 必须在执行脚本前重新绑定，确保 native 调用路由到本 runner 的 store。
        self.ctx.rebind_dispatcher()?;

        // 重新设置全局函数，确保 web 端共享作用域下调用正确的函数。
        self.ctx.eval(&self.run_fn_source)?;
        self.ctx.eval(&self.flush_fn_source)?;

        self.ctx.call_global_fn("__opencatRunFrame")?;
        self.ctx.call_global_fn("__opencatFlushTimelines")?;

        let snap = self.ctx.with_store_mut(|s| s.snapshot_mutations());
        snap.apply_to_recorder(recorder);

        self.first_frame = false;
        Ok(())
    }

    pub fn set_style_defaults(
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

    pub fn set_initial_style_from_node(&mut self, id: &str, style: &crate::style::NodeStyle) {
        self.ctx.with_store_mut(|s| {
            s.set_initial_style_from_node(id, style);
        });
    }
}

pub fn apply_target_registry<C: JsContext>(
    ctx: &C,
    registry: &ScriptTargetRegistry,
) -> anyhow::Result<()> {
    let visual: serde_json::Map<String, serde_json::Value> = registry
        .visual_ids
        .iter()
        .map(|k| (k.clone(), serde_json::Value::Bool(true)))
        .collect();
    let canvas: serde_json::Map<String, serde_json::Value> = registry
        .canvas_ids
        .iter()
        .map(|k| (k.clone(), serde_json::Value::Bool(true)))
        .collect();
    let non_visual: serde_json::Map<String, serde_json::Value> = registry
        .non_visual_ids
        .iter()
        .map(|k| (k.clone(), serde_json::Value::Bool(true)))
        .collect();
    ctx.set_ctx_field(
        "__targetRegistry",
        json!({
            "visual": visual,
            "canvas": canvas,
            "nonVisual": non_visual,
        }),
    )
}

fn install_runtime<C: JsContext>(ctx: &C, user_source: &str) -> anyhow::Result<(String, String)> {
    // 1. 兜底 globalThis.ctx（端侧 new() 已建过，这里只在尚未存在时初始化）。
    ctx.eval(
        "globalThis.ctx = globalThis.ctx || {\
            frame:0, fps:0, totalFrames:0, currentFrame:0, sceneFrames:0, \
            __currentCanvasTarget:''\
         };",
    )?;

    // 1b. Initialize empty target registry (will be populated by apply_target_registry).
    ctx.eval(
        "ctx.__targetRegistry = ctx.__targetRegistry || {\
            visual: Object.create(null),\
            canvas: Object.create(null),\
            nonVisual: Object.create(null)\
         };",
    )?;

    // 2. 注册唯一的 native 入口 __opencatCallNative。
    ctx.install_dispatcher(dispatch_binding)?;

    // 3. 装载 shim：把每个 binding 名包成 wrapper 调用 __opencatCallNative。
    //    必须在 runtime JS 之前——runtime JS 在 eval 时即可能定义引用 __record_* 等的函数。
    ctx.eval(&binding_shim_js())?;

    // 4. 共享 runtime JS。
    ctx.eval(NODE_STYLE_RUNTIME)?;
    ctx.eval(CANVAS_API_RUNTIME)?;
    ctx.eval(ANIMATION_RUNTIME)?;

    // 5. 把用户脚本包装为全局函数。
    let run_fn_source = format!("globalThis.__opencatRunFrame = function() {{\n{user_source}\n}};");
    ctx.eval(&run_fn_source)?;

    // 6. 把 ctx.__flushTimelines 别名为全局函数，与 call_global_fn 单一动词配套。
    //    依赖 ANIMATION_RUNTIME (facade.js) 已经把 ctx.__flushTimelines 装上去。
    let flush_fn_source = String::from(
        "globalThis.__opencatFlushTimelines = function() {\
            if (globalThis.ctx && globalThis.ctx.__flushTimelines) \
                globalThis.ctx.__flushTimelines();\
         };",
    );
    ctx.eval(&flush_fn_source)?;
    Ok((run_fn_source, flush_fn_source))
}
