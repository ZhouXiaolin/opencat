use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use rquickjs::{Context, Function, Object, Persistent, Runtime};

use opencat_core::frame_ctx::ScriptFrameCtx;
use opencat_core::scene::script::Runner as CoreRunner;
use opencat_core::scene::script::ScriptTextSource;
use opencat_core::script::recorder::MutationRecorder;
use opencat_core::script::recorder::MutationStore as CoreMutationStore;

use crate::js_context::{
    install_node_style_bindings, map_js_result, ANIMATE_RUNTIME, CANVASKIT_RUNTIME,
    NODE_STYLE_RUNTIME,
};

pub type ScriptRuntimeCache = opencat_core::scene::script::ScriptRuntimeCache<ScriptRunner>;

pub struct ScriptRunner {
    run_fn: Persistent<Function<'static>>,
    ctx_obj: Persistent<Object<'static>>,
    context: Context,
    store: Arc<Mutex<CoreMutationStore>>,
}


const RUN_FRAME_FN: &str = "__opencatRunFrame";

impl ScriptRunner {
    fn new(source: &str) -> anyhow::Result<Self> {
        let runtime = Runtime::new()?;
        let context = Context::full(&runtime)?;
        let store = Arc::new(Mutex::new(CoreMutationStore::default()));

        let (ctx_obj, run_fn) = context.with(|ctx| {
            let globals = ctx.globals();
            let ctx_obj = install_runtime_bindings(&ctx, &store)?;
            let wrapped = format!("globalThis.{RUN_FRAME_FN} = function() {{\n{source}\n}};");
            map_js_result(
                ctx.eval::<(), _>(wrapped.as_str()),
                &ctx,
                "failed to initialize script runtime",
            )?;
            let run_fn: Function<'_> = globals.get(RUN_FRAME_FN)?;
            Ok::<_, anyhow::Error>((
                Persistent::save(&ctx, ctx_obj),
                Persistent::save(&ctx, run_fn),
            ))
        })?;

        Ok(Self {
            run_fn,
            ctx_obj,
            context,
            store,
        })
    }

    pub(crate) fn run_into(
        &mut self,
        frame_ctx: ScriptFrameCtx,
        current_node_id: Option<&str>,
        recorder: &mut dyn MutationRecorder,
    ) -> anyhow::Result<()> {
        {
            let mut store = self
                .store
                .lock()
                .map_err(|_| anyhow!("script mutation store lock poisoned before execution"))?;
            store.reset_for_frame(frame_ctx.current_frame);
        }

        self.context.with(|ctx| {
            let ctx_obj = self.ctx_obj.clone().restore(&ctx)?;
            ctx_obj.set("frame", frame_ctx.frame)?;
            ctx_obj.set("totalFrames", frame_ctx.total_frames)?;
            ctx_obj.set("currentFrame", frame_ctx.current_frame)?;
            ctx_obj.set("sceneFrames", frame_ctx.scene_frames)?;
            ctx_obj.set("__currentCanvasTarget", current_node_id.unwrap_or(""))?;

            let run_fn = self.run_fn.clone().restore(&ctx)?;
            let node_label = current_node_id.unwrap_or("<global>");
            let error_context = format!(
                "script execution failed for node `{node_label}` at frame {}/{} (scene {}/{})",
                frame_ctx.frame,
                frame_ctx.total_frames,
                frame_ctx.current_frame,
                frame_ctx.scene_frames
            );
            map_js_result(run_fn.call::<(), ()>(()), &ctx, &error_context)?;
            let flush_fn: Function<'_> = ctx_obj.get("__flushTimelines")?;
            map_js_result(flush_fn.call::<(), ()>(()), &ctx, &error_context)?;
            Ok::<_, anyhow::Error>(())
        })?;

        let store = self
            .store
            .lock()
            .map_err(|_| anyhow!("script mutation store lock poisoned after execution"))?;
        let snap = store.snapshot_mutations();
        snap.apply_to_recorder(recorder);
        Ok(())
    }
}

// ── Runtime binding installation ─────────────────────────────────────

fn install_runtime_bindings<'js>(
    ctx: &rquickjs::Ctx<'js>,
    store: &Arc<Mutex<CoreMutationStore>>,
) -> anyhow::Result<Object<'js>> {
    let globals = ctx.globals();
    let ctx_obj = Object::new(ctx.clone())?;
    ctx_obj.set("frame", 0)?;
    ctx_obj.set("totalFrames", 0)?;
    ctx_obj.set("currentFrame", 0)?;
    ctx_obj.set("sceneFrames", 0)?;
    ctx_obj.set("__currentCanvasTarget", "")?;
    globals.set("ctx", ctx_obj.clone())?;

    install_node_style_bindings(ctx, store)?;

    ctx.eval::<(), _>(NODE_STYLE_RUNTIME)?;
    ctx.eval::<(), _>(CANVASKIT_RUNTIME)?;
    ctx.eval::<(), _>(ANIMATE_RUNTIME)?;

    Ok(ctx_obj)
}

impl CoreRunner for ScriptRunner {
    fn from_source(source: &str) -> anyhow::Result<Self> {
        ScriptRunner::new(source)
    }

    fn set_text_sources(&mut self, sources: &std::collections::HashMap<String, ScriptTextSource>) {
        if let Ok(mut store) = self.store.lock() {
            store.clear_text_sources();
            for (id, source) in sources {
                store.register_text_source(id, source.clone());
            }
        }
    }

    fn run_into(
        &mut self,
        frame_ctx: &ScriptFrameCtx,
        current_node_id: Option<&str>,
        recorder: &mut dyn MutationRecorder,
    ) -> anyhow::Result<()> {
        ScriptRunner::run_into(self, *frame_ctx, current_node_id, recorder)
    }
}
