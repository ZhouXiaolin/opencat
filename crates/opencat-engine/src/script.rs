use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use rquickjs::{
    Context, Error as JsError, Exception, FromJs, Function, Object, Persistent, Runtime,
};

use opencat_core::for_each_binding;
use opencat_core::frame_ctx::ScriptFrameCtx;
use opencat_core::scene::script::mutations::{
    CanvasCommand, TextUnitGranularity,
};
use opencat_core::scene::script::object_fit_from_name;
use opencat_core::scene::script::{
    align_items_from_name, box_shadow_from_name, drop_shadow_from_name, flex_direction_from_name,
    font_edging_from_name, inset_shadow_from_name, justify_content_from_name, line_cap_from_name,
    line_join_from_name, point_mode_from_name,
    position_from_name, text_align_from_name,
};
use opencat_core::scene::script::ScriptTextSource;
use opencat_core::scene::script::StyleMutations;
use opencat_core::scene::script::Runner as CoreRunner;
use opencat_core::script::animate::state::{parse_easing_from_tag, random_from_seed};
use opencat_core::script::recorder::MutationRecorder;
use opencat_core::script::recorder::MutationStore as CoreMutationStore;
use opencat_core::script::recorder::{MutationStore, TextUnitValues};
use opencat_core::script::text_units::describe_text_units;
use opencat_core::script::text_units::grapheme_strings;
use opencat_core::style::color_token_from_script_string;
use opencat_core::text::measure_script_text_width;
use opencat_core::style::{BorderStyle, FontWeight};

const NODE_STYLE_RUNTIME: &str = opencat_core::script::runtime::NODE_STYLE_RUNTIME;
const CANVASKIT_RUNTIME: &str = opencat_core::script::runtime::CANVAS_API_RUNTIME;
const ANIMATE_RUNTIME: &str = opencat_core::script::runtime::ANIMATION_RUNTIME;

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

// ── anyhow → rquickjs conversion ─────────────────────────────────────

trait IntoAnyhow {
    fn into_anyhow(self) -> anyhow::Result<()>;
}

impl IntoAnyhow for () {
    fn into_anyhow(self) -> anyhow::Result<()> {
        Ok(())
    }
}

impl IntoAnyhow for anyhow::Result<()> {
    fn into_anyhow(self) -> anyhow::Result<()> {
        self
    }
}

// ── rquickjs binding installation ────────────────────────────────────

fn install_node_style_bindings<'js>(
    ctx: &rquickjs::Ctx<'js>,
    store: &Arc<Mutex<MutationStore>>,
) -> anyhow::Result<()> {
    let globals = ctx.globals();

    macro_rules! install_to_rquickjs {
        // ── Node commands ($rec: &mut dyn MutationRecorder, $id: &str) ──
        (node $rec:ident $id:ident $name:ident ($first_param:ident : &str $(, $param:ident : $param_ty:ty)*) $($body:tt)*) => {{
            let s = store.clone();
            globals.set(
                concat!("__", stringify!($name)),
                Function::new(ctx.clone(), move |$first_param: String $(, $param: $param_ty)*| -> Result<(), rquickjs::Error> {
                    let mut guard = s.lock().map_err(|e| rquickjs::Error::new_from_js_message("script", "lock", &e.to_string()))?;
                    let $rec = &mut *guard as &mut dyn MutationRecorder;
                    let $id: &str = &$first_param;
                    (|| -> anyhow::Result<()> {
                        { $($body)* }.into_anyhow()
                    })().map_err(|e| rquickjs::Error::new_from_js_message("script", "script", &e.to_string()))
                })?,
            )?;
        }};

        // ── Store commands ($store: &mut MutationStore) ──
        (cmd $store:ident $name:ident ($($param:ident : $param_ty:ty),*) -> $ret:ty $body:block) => {{
            let s = store.clone();
            globals.set(
                concat!("__", stringify!($name)),
                Function::new(ctx.clone(), move |$($param: $param_ty),*| -> Result<$ret, rquickjs::Error> {
                    let mut guard = s.lock().map_err(|e| rquickjs::Error::new_from_js_message("script", "lock", &e.to_string()))?;
                    let $store = &mut *guard;
                    (|| -> anyhow::Result<$ret> { $body })()
                        .map_err(|e| rquickjs::Error::new_from_js_message("script", "script", &e.to_string()))
                })?,
            )?;
        }};

        // ── Store queries ($store: &MutationStore) ──
        (qry $store:ident $name:ident ($($param:ident : $param_ty:ty),*) -> $ret:ty $body:block) => {{
            let s = store.clone();
            globals.set(
                concat!("__", stringify!($name)),
                Function::new(ctx.clone(), move |$($param: $param_ty),*| -> Result<$ret, rquickjs::Error> {
                    let guard = s.lock().map_err(|e| rquickjs::Error::new_from_js_message("script", "lock", &e.to_string()))?;
                    let $store = &*guard;
                    (|| -> anyhow::Result<$ret> { $body })()
                        .map_err(|e| rquickjs::Error::new_from_js_message("script", "script", &e.to_string()))
                })?,
            )?;
        }};

        // ── Pure functions (no store) ──
        (pure $name:ident ($($param:ident : $param_ty:ty),*) -> $ret:ty $body:block) => {{
            globals.set(
                concat!("__", stringify!($name)),
                Function::new(ctx.clone(), move |$($param: $param_ty),*| -> Result<$ret, rquickjs::Error> {
                    (|| -> anyhow::Result<$ret> { $body })()
                        .map_err(|e| rquickjs::Error::new_from_js_message("script", "script", &e.to_string()))
                })?,
            )?;
        }};
    }

    for_each_binding!(rec id store install_to_rquickjs);

    Ok(())
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
