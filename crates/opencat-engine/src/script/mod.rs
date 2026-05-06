pub mod bindings;

#[cfg(test)]
mod driver_tests;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use rquickjs::{
    Context, Error as JsError, Exception, FromJs, Function, Object, Persistent, Runtime,
};

use opencat_core::frame_ctx::ScriptFrameCtx;
use opencat_core::scene::script::{
    ScriptTextSource,
};
use opencat_core::script::animate::{
    AnimateState, MorphSvgState, PathMeasureState,
};
use opencat_core::script::recorder::MutationRecorder;
use opencat_core::script::recorder::MutationStore as CoreMutationStore;
use bindings::canvas_api;
use bindings::node_style;
use opencat_core::scene::script::{
    ScriptDriver, StyleMutations, ScriptDriverId,
    ScriptHost,
};

#[derive(Default)]
pub struct ScriptRuntimeCache {
    runners: HashMap<u64, ScriptRunner>,
    text_sources: HashMap<String, ScriptTextSource>,
}

impl ScriptRuntimeCache {
    pub(crate) fn clear_text_sources(&mut self) {
        self.text_sources.clear();
    }

    pub(crate) fn register_text_source(&mut self, id: &str, source: ScriptTextSource) {
        self.text_sources.insert(id.to_string(), source);
    }

    pub(crate) fn run_by_id(
        &mut self,
        id: ScriptDriverId,
        frame_ctx: ScriptFrameCtx,
        current_node_id: Option<&str>,
    ) -> anyhow::Result<StyleMutations> {
        let runner = self
            .runners
            .get_mut(&id.0)
            .ok_or_else(|| anyhow!("script driver {} not installed", id.0))?;
        if let Ok(mut store) = runner.store.lock() {
            store.clear_text_sources();
            for (id, source) in &self.text_sources {
                store.register_text_source(id, source.clone());
            }
        }
        runner.run(frame_ctx, current_node_id)
    }
}

pub(crate) struct ScriptRunner {
    run_fn: Persistent<Function<'static>>,
    ctx_obj: Persistent<Object<'static>>,
    context: Context,
    store: Arc<Mutex<CoreMutationStore>>,
    animate_state: Arc<Mutex<AnimateState>>,
    morph_svg_state: Arc<Mutex<MorphSvgState>>,
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


impl ScriptRuntimeCache {
    pub(crate) fn run(
        &mut self,
        driver: &ScriptDriver,
        frame_ctx: ScriptFrameCtx,
        current_node_id: Option<&str>,
    ) -> anyhow::Result<StyleMutations> {
        let key = driver.cache_key();
        let runner = match self.runners.entry(key) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(create_runner(driver)?)
            }
        };

        if let Ok(mut store) = runner.store.lock() {
            store.clear_text_sources();
            for (id, source) in &self.text_sources {
                store.register_text_source(id, source.clone());
            }
        }

        runner.run(frame_ctx, current_node_id)
    }
}

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
            let mut animate = self.animate_state.lock().map_err(|_| {
                anyhow!("animate state lock poisoned")
            })?;
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
            let mut animate = self.animate_state.lock().map_err(|_| {
                anyhow!("animate state lock poisoned")
            })?;
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
        for (node_id, node_mutations) in &snap.mutations {
            opencat_core::scene::script::precomputed_host::apply_node_to_recorder(
                recorder,
                node_id,
                node_mutations,
            );
        }
        for (canvas_id, canvas_mutations) in &snap.canvas_mutations {
            for cmd in &canvas_mutations.commands {
                recorder.record_canvas_command(canvas_id, cmd.clone());
            }
        }
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

    node_style::install_node_style_bindings(ctx, store)?;
    canvas_api::install_canvaskit_bindings(ctx, store)?;
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
                Ok::<_, rquickjs::Error>(
                    guard.get_text_source(&id).map(|s| s.text.clone()),
                )
            })?,
        )?;
    }

    ctx.eval::<(), _>(node_style::NODE_STYLE_RUNTIME)?;
    ctx.eval::<(), _>(canvas_api::CANVASKIT_RUNTIME)?;
    ctx.eval::<(), _>(bindings::animate_api::ANIMATE_RUNTIME)?;

    Ok(ctx_obj)
}

impl ScriptHost for ScriptRuntimeCache {
    fn install(&mut self, source: &str) -> anyhow::Result<ScriptDriverId> {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut h = DefaultHasher::new();
        source.hash(&mut h);
        let key = h.finish();
        if let std::collections::hash_map::Entry::Vacant(e) = self.runners.entry(key) {
            e.insert(ScriptRunner::new(source)?);
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
        if let Ok(mut store) = runner.store.lock() {
            store.clear_text_sources();
            for (id, source) in &self.text_sources {
                store.register_text_source(id, source.clone());
            }
        }
        runner.run_into(*frame_ctx, current_node_id, recorder)
    }
}
