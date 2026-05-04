#![cfg(feature = "host-default")]
pub mod bindings;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use rquickjs::{
    Context, Error as JsError, Exception, FromJs, Function, Object, Persistent, Runtime,
};

use crate::{
    core::frame_ctx::ScriptFrameCtx,
    core::scene::script::{
        CanvasMutations, ScriptTextSource,
    },
    core::style::{
        AlignItems, BoxShadow, BoxShadowStyle, DropShadow, DropShadowStyle, FlexDirection,
        InsetShadow, InsetShadowStyle, JustifyContent, ObjectFit, Position, TextAlign,
    },
};

use bindings::animate_api::{
    AnimateState, MorphSvgState, PathMeasureState,
};
use bindings::canvas_api;
use bindings::node_style;
use crate::core::scene::script::{
    NodeStyleMutations, ScriptDriver, StyleMutations, ScriptDriverId,
    ScriptHost,
};

#[derive(Default)]
struct RuntimeMutationStore {
    styles: HashMap<String, NodeStyleMutations>,
    canvases: HashMap<String, CanvasMutations>,
    current_frame: u32,
    animate_state: std::sync::Mutex<AnimateState>,
    path_measure_state: std::sync::Mutex<PathMeasureState>,
    morph_svg_state: std::sync::Mutex<MorphSvgState>,
    text_sources: HashMap<String, ScriptTextSource>,
}

impl RuntimeMutationStore {
    fn reset_for_frame(&mut self, current_frame: u32) {
        self.styles.clear();
        self.canvases.clear();
        self.current_frame = current_frame;
        if let Ok(mut animate_state) = self.animate_state.lock() {
            *animate_state = AnimateState::default();
        }
    }
}

pub(crate) type MutationStore = Arc<Mutex<RuntimeMutationStore>>;

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
            store.text_sources = self.text_sources.clone();
        }
        runner.run(frame_ctx, current_node_id)
    }
}

pub(crate) struct ScriptRunner {
    run_fn: Persistent<Function<'static>>,
    ctx_obj: Persistent<Object<'static>>,
    context: Context,
    store: MutationStore,
    _runtime: Runtime,
}

impl ScriptDriver {
    pub(crate) fn create_runner(&self) -> anyhow::Result<ScriptRunner> {
        ScriptRunner::new(&self.source)
    }

    pub fn run(
        &self,
        frame: u32,
        total_frames: u32,
        current_frame: u32,
        scene_frames: u32,
        current_node_id: Option<&str>,
    ) -> anyhow::Result<StyleMutations> {
        let mut runner = self.create_runner()?;
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
}

const RUN_FRAME_FN: &str = "__opencatRunFrame";

pub(crate) fn position_from_name(name: &str) -> Option<Position> {
    match name {
        "relative" => Some(Position::Relative),
        "absolute" => Some(Position::Absolute),
        _ => None,
    }
}

pub(crate) fn flex_direction_from_name(name: &str) -> Option<FlexDirection> {
    match name {
        "row" => Some(FlexDirection::Row),
        "col" | "column" => Some(FlexDirection::Col),
        _ => None,
    }
}

pub(crate) fn justify_content_from_name(name: &str) -> Option<JustifyContent> {
    match name {
        "start" => Some(JustifyContent::Start),
        "center" => Some(JustifyContent::Center),
        "end" => Some(JustifyContent::End),
        "between" => Some(JustifyContent::Between),
        "around" => Some(JustifyContent::Around),
        "evenly" => Some(JustifyContent::Evenly),
        _ => None,
    }
}

pub(crate) fn align_items_from_name(name: &str) -> Option<AlignItems> {
    match name {
        "start" => Some(AlignItems::Start),
        "center" => Some(AlignItems::Center),
        "end" => Some(AlignItems::End),
        "stretch" => Some(AlignItems::Stretch),
        _ => None,
    }
}

pub(crate) fn object_fit_from_name(name: &str) -> Option<ObjectFit> {
    match name {
        "contain" => Some(ObjectFit::Contain),
        "cover" => Some(ObjectFit::Cover),
        "fill" => Some(ObjectFit::Fill),
        _ => None,
    }
}

pub(crate) fn box_shadow_from_name(name: &str) -> Option<BoxShadow> {
    match name {
        "2xs" => Some(BoxShadow::from_style(BoxShadowStyle::TwoXs)),
        "xs" => Some(BoxShadow::from_style(BoxShadowStyle::Xs)),
        "sm" => Some(BoxShadow::from_style(BoxShadowStyle::Sm)),
        "base" | "default" => Some(BoxShadow::from_style(BoxShadowStyle::Base)),
        "md" => Some(BoxShadow::from_style(BoxShadowStyle::Md)),
        "lg" => Some(BoxShadow::from_style(BoxShadowStyle::Lg)),
        "xl" => Some(BoxShadow::from_style(BoxShadowStyle::Xl)),
        "2xl" => Some(BoxShadow::from_style(BoxShadowStyle::TwoXl)),
        "3xl" => Some(BoxShadow::from_style(BoxShadowStyle::ThreeXl)),
        _ => None,
    }
}

pub(crate) fn inset_shadow_from_name(name: &str) -> Option<InsetShadow> {
    match name {
        "2xs" => Some(InsetShadow::from_style(InsetShadowStyle::TwoXs)),
        "xs" => Some(InsetShadow::from_style(InsetShadowStyle::Xs)),
        "base" | "default" => Some(InsetShadow::from_style(InsetShadowStyle::Base)),
        "sm" => Some(InsetShadow::from_style(InsetShadowStyle::Sm)),
        "md" => Some(InsetShadow::from_style(InsetShadowStyle::Md)),
        _ => None,
    }
}

pub(crate) fn drop_shadow_from_name(name: &str) -> Option<DropShadow> {
    match name {
        "xs" => Some(DropShadow::from_style(DropShadowStyle::Xs)),
        "sm" => Some(DropShadow::from_style(DropShadowStyle::Sm)),
        "base" | "default" => Some(DropShadow::from_style(DropShadowStyle::Base)),
        "md" => Some(DropShadow::from_style(DropShadowStyle::Md)),
        "lg" => Some(DropShadow::from_style(DropShadowStyle::Lg)),
        "xl" => Some(DropShadow::from_style(DropShadowStyle::Xl)),
        "2xl" => Some(DropShadow::from_style(DropShadowStyle::TwoXl)),
        "3xl" => Some(DropShadow::from_style(DropShadowStyle::ThreeXl)),
        _ => None,
    }
}

pub(crate) fn text_align_from_name(name: &str) -> Option<TextAlign> {
    match name {
        "left" => Some(TextAlign::Left),
        "center" => Some(TextAlign::Center),
        "right" => Some(TextAlign::Right),
        _ => None,
    }
}

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
                entry.insert(driver.create_runner()?)
            }
        };

        if let Ok(mut store) = runner.store.lock() {
            store.text_sources = self.text_sources.clone();
        }

        runner.run(frame_ctx, current_node_id)
    }
}

impl ScriptRunner {
    fn new(source: &str) -> anyhow::Result<Self> {
        let runtime = Runtime::new()?;
        let context = Context::full(&runtime)?;
        let store: MutationStore = Arc::new(Mutex::new(RuntimeMutationStore::default()));

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

        let mutations = self
            .store
            .lock()
            .map_err(|_| anyhow!("script mutation store lock poisoned after execution"))?;
        Ok(StyleMutations {
            mutations: mutations.styles.clone(),
            canvas_mutations: mutations.canvases.clone(),
        })
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
    store: &MutationStore,
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
    bindings::animate_api::install_animate_bindings(ctx, store)?;

    // Read-only text source query
    {
        let store = store.clone();
        globals.set(
            "__text_source_get",
            Function::new(ctx.clone(), move |id: String| {
                let store = store.lock().map_err(|_| {
                    rquickjs::Error::new_from_js_message(
                        "mutex",
                        "mutex",
                        "text source lock poisoned",
                    )
                })?;
                Ok::<_, rquickjs::Error>(store.text_sources.get(&id).map(|s| s.text.clone()))
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
    ) -> anyhow::Result<StyleMutations> {
        let runner = self
            .runners
            .get_mut(&driver.0)
            .ok_or_else(|| anyhow::anyhow!("script driver {} not installed", driver.0))?;
        if let Ok(mut store) = runner.store.lock() {
            store.text_sources = self.text_sources.clone();
        }
        runner.run(*frame_ctx, None)
    }
}
