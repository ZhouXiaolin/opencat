pub mod bindings;

use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use rquickjs::{
    Context, Error as JsError, Exception, FromJs, Function, Object, Persistent, Runtime,
};

use bindings as script_bindings;
use opencat_core::frame_ctx::ScriptFrameCtx;
use opencat_core::scene::script::ScriptTextSource;
use opencat_core::scene::script::StyleMutations;
use opencat_core::scene::script::Runner as CoreRunner;
use opencat_core::script::animate::{AnimateState, MorphSvgState, PathMeasureState};
use opencat_core::script::recorder::MutationRecorder;
use opencat_core::script::recorder::MutationStore as CoreMutationStore;

pub type ScriptRuntimeCache = opencat_core::scene::script::ScriptRuntimeCache<ScriptRunner>;

pub struct ScriptRunner {
    run_fn: Persistent<Function<'static>>,
    ctx_obj: Persistent<Object<'static>>,
    context: Context,
    store: Arc<Mutex<CoreMutationStore>>,
    animate_state: Arc<Mutex<AnimateState>>,
    #[allow(dead_code)]
    morph_svg_state: Arc<Mutex<MorphSvgState>>,
    #[allow(dead_code)]
    path_measure_state: Arc<Mutex<PathMeasureState>>,
    _runtime: Runtime,
}

pub(crate) fn create_runner(driver: &opencat_core::ScriptDriver) -> anyhow::Result<ScriptRunner> {
    ScriptRunner::new(&driver.source)
}

pub fn run_driver(
    driver: &opencat_core::ScriptDriver,
    frame: u32,
    total_frames: u32,
    current_frame: u32,
    scene_frames: u32,
    current_node_id: Option<&str>,
) -> anyhow::Result<StyleMutations> {
    let mut runner = create_runner(driver)?;
    runner.run(
        ScriptFrameCtx {
            frame,
            total_frames,
            current_frame,
            scene_frames,
        },
        current_node_id,
    )
}

const RUN_FRAME_FN: &str = "__opencatRunFrame";

impl ScriptRunner {
    fn new(source: &str) -> anyhow::Result<Self> {
        let runtime = Runtime::new()?;
        let context = Context::full(&runtime)?;
        let store = Arc::new(Mutex::new(CoreMutationStore::default()));
        let animate_state = Arc::new(Mutex::new(AnimateState::default()));
        let morph_svg_state = Arc::new(Mutex::new(MorphSvgState::default()));
        let path_measure_state = Arc::new(Mutex::new(PathMeasureState::default()));

        let (ctx_obj, run_fn) = context.with(|ctx| {
            let globals = ctx.globals();
            let ctx_obj = install_runtime_bindings(
                &ctx,
                &store,
                &animate_state,
                &morph_svg_state,
                &path_measure_state,
            )?;
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
            animate_state,
            morph_svg_state,
            path_measure_state,
            _runtime: runtime,
        })
    }

    pub(crate) fn run(
        &mut self,
        frame_ctx: ScriptFrameCtx,
        current_node_id: Option<&str>,
    ) -> anyhow::Result<StyleMutations> {
        {
            let mut store = self
                .store
                .lock()
                .map_err(|_| anyhow!("script mutation store lock poisoned before execution"))?;
            store.reset_for_frame(frame_ctx.current_frame);
        }
        {
            let mut animate = self
                .animate_state
                .lock()
                .map_err(|_| anyhow!("animate state lock poisoned"))?;
            *animate = AnimateState::default();
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
        Ok(store.snapshot_mutations())
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
        {
            let mut animate = self
                .animate_state
                .lock()
                .map_err(|_| anyhow!("animate state lock poisoned"))?;
            *animate = AnimateState::default();
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

fn map_js_result<T>(
    result: Result<T, JsError>,
    ctx: &rquickjs::Ctx<'_>,
    error_context: &str,
) -> anyhow::Result<T> {
    match result {
        Ok(value) => Ok(value),
        Err(JsError::Exception) => {
            let caught = ctx.catch();
            if let Ok(exception) = Exception::from_js(ctx, caught.clone()) {
                let message = exception
                    .message()
                    .unwrap_or_else(|| "uncaught JavaScript exception".to_string());
                let stack = exception.stack().unwrap_or_default();
                if stack.is_empty() {
                    anyhow::bail!("{error_context}: {message}");
                }
                anyhow::bail!("{error_context}: {message}\n{stack}");
            }
            anyhow::bail!("{error_context}: uncaught JavaScript exception");
        }
        Err(err) => Err(err.into()),
    }
}

fn install_runtime_bindings<'js>(
    ctx: &rquickjs::Ctx<'js>,
    store: &Arc<Mutex<CoreMutationStore>>,
    animate_state: &Arc<Mutex<AnimateState>>,
    morph_svg_state: &Arc<Mutex<MorphSvgState>>,
    path_measure_state: &Arc<Mutex<PathMeasureState>>,
) -> anyhow::Result<Object<'js>> {
    let globals = ctx.globals();
    let ctx_obj = Object::new(ctx.clone())?;
    ctx_obj.set("frame", 0)?;
    ctx_obj.set("totalFrames", 0)?;
    ctx_obj.set("currentFrame", 0)?;
    ctx_obj.set("sceneFrames", 0)?;
    ctx_obj.set("__currentCanvasTarget", "")?;
    globals.set("ctx", ctx_obj.clone())?;

    script_bindings::install_node_style_bindings(ctx, store)?;
    script_bindings::install_canvaskit_bindings(ctx, store)?;
    bindings::animate_api::install_animate_bindings(
        ctx,
        store,
        animate_state,
        morph_svg_state,
        path_measure_state,
    )?;

    // Read-only text source query
    {
        let s = store.clone();
        globals.set(
            "__text_source_get",
            Function::new(ctx.clone(), move |id: String| {
                let guard = s.lock().map_err(|_| {
                    rquickjs::Error::new_from_js_message(
                        "mutex",
                        "mutex",
                        "text source lock poisoned",
                    )
                })?;
                Ok::<_, rquickjs::Error>(guard.get_text_source(&id).map(|s| s.text.clone()))
            })?,
        )?;
    }

    ctx.eval::<(), _>(script_bindings::NODE_STYLE_RUNTIME)?;
    ctx.eval::<(), _>(script_bindings::CANVASKIT_RUNTIME)?;
    ctx.eval::<(), _>(bindings::animate_api::ANIMATE_RUNTIME)?;

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
