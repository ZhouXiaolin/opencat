use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use rquickjs::{
    Context, Error as JsError, Exception, FromJs, Function, Object, Persistent, Runtime,
};

use crate::{
    frame_ctx::ScriptFrameCtx,
    style::{
        AlignItems, BoxShadow, BoxShadowStyle, DropShadow, DropShadowStyle, FlexDirection,
        FontWeight, InsetShadow, InsetShadowStyle, JustifyContent, ObjectFit, Position, TextAlign,
    },
};

mod animate_api;
mod canvas_api;
pub mod host;
mod morph_svg;
mod node_style;

pub use host::{ScriptDriverId, ScriptHost};

pub use canvas_api::{
    CanvasCommand, CanvasMutations, ScriptColor, ScriptFontEdging, ScriptLineCap, ScriptLineJoin,
    ScriptPointMode,
};
pub use node_style::{
    NodeStyleMutations, TextUnitGranularity, TextUnitOverride, TextUnitOverrideBatch,
};

#[derive(Debug, Clone, Default)]
pub struct StyleMutations {
    pub mutations: HashMap<String, NodeStyleMutations>,
    pub canvas_mutations: HashMap<String, CanvasMutations>,
}

impl StyleMutations {
    pub fn get(&self, id: &str) -> Option<&NodeStyleMutations> {
        self.mutations.get(id)
    }

    pub fn is_empty(&self) -> bool {
        self.mutations.is_empty() && self.canvas_mutations.is_empty()
    }

    pub fn apply_to_node(&self, node_style: &mut crate::style::NodeStyle, id: &str) {
        if let Some(mutation) = self.mutations.get(id) {
            mutation.apply_to(node_style);
        }
    }

    pub fn get_canvas(&self, id: &str) -> Option<&CanvasMutations> {
        self.canvas_mutations.get(id)
    }

    pub fn apply_to_canvas(&self, commands: &mut Vec<CanvasCommand>, id: &str) {
        if let Some(mutation) = self.canvas_mutations.get(id) {
            commands.extend(mutation.commands.iter().cloned());
        }
    }

    pub fn text_content_for(&self, id: &str) -> Option<&str> {
        self.mutations
            .get(id)
            .and_then(|m| m.text_content.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScriptTextSource {
    pub text: String,
    pub kind: ScriptTextSourceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScriptTextSourceKind {
    TextNode,
    Caption,
}

#[derive(Default)]
struct RuntimeMutationStore {
    styles: HashMap<String, NodeStyleMutations>,
    canvases: HashMap<String, CanvasMutations>,
    current_frame: u32,
    animate_state: std::sync::Mutex<animate_api::AnimateState>,
    path_measure_state: std::sync::Mutex<animate_api::PathMeasureState>,
    morph_svg_state: std::sync::Mutex<animate_api::MorphSvgState>,
    text_sources: HashMap<String, ScriptTextSource>,
}

impl RuntimeMutationStore {
    fn reset_for_frame(&mut self, current_frame: u32) {
        self.styles.clear();
        self.canvases.clear();
        self.current_frame = current_frame;
        if let Ok(mut animate_state) = self.animate_state.lock() {
            *animate_state = animate_api::AnimateState::default();
        }
        // Intentionally do NOT reset path_measure_state — SVG path handles are
        // cached on `ctx.__foo` across frames and must remain valid.
    }
}

type MutationStore = Arc<Mutex<RuntimeMutationStore>>;

#[derive(Default)]
pub(crate) struct ScriptRuntimeCache {
    runners: HashMap<u64, ScriptRunner>,
    text_sources: HashMap<String, ScriptTextSource>,
}

impl ScriptRuntimeCache {
    pub(crate) fn clear_text_sources(&mut self) {
        self.text_sources.clear();
    }

    /// Register a resolved text source for the given node id.
    /// This will be visible to scripts via `__text_source_get()` on the next frame.
    pub(crate) fn register_text_source(&mut self, id: &str, source: ScriptTextSource) {
        self.text_sources.insert(id.to_string(), source);
    }
}

#[derive(Debug, Clone)]
pub struct ScriptDriver {
    source: String,
}

pub(crate) struct ScriptRunner {
    run_fn: Persistent<Function<'static>>,
    ctx_obj: Persistent<Object<'static>>,
    context: Context,
    store: MutationStore,
    _runtime: Runtime,
}

const RUN_FRAME_FN: &str = "__opencatRunFrame";

fn position_from_name(name: &str) -> Option<Position> {
    match name {
        "relative" => Some(Position::Relative),
        "absolute" => Some(Position::Absolute),
        _ => None,
    }
}

fn flex_direction_from_name(name: &str) -> Option<FlexDirection> {
    match name {
        "row" => Some(FlexDirection::Row),
        "col" | "column" => Some(FlexDirection::Col),
        _ => None,
    }
}

fn justify_content_from_name(name: &str) -> Option<JustifyContent> {
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

fn align_items_from_name(name: &str) -> Option<AlignItems> {
    match name {
        "start" => Some(AlignItems::Start),
        "center" => Some(AlignItems::Center),
        "end" => Some(AlignItems::End),
        "stretch" => Some(AlignItems::Stretch),
        _ => None,
    }
}

fn object_fit_from_name(name: &str) -> Option<ObjectFit> {
    match name {
        "contain" => Some(ObjectFit::Contain),
        "cover" => Some(ObjectFit::Cover),
        "fill" => Some(ObjectFit::Fill),
        _ => None,
    }
}

fn box_shadow_from_name(name: &str) -> Option<BoxShadow> {
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

fn inset_shadow_from_name(name: &str) -> Option<InsetShadow> {
    match name {
        "2xs" => Some(InsetShadow::from_style(InsetShadowStyle::TwoXs)),
        "xs" => Some(InsetShadow::from_style(InsetShadowStyle::Xs)),
        "base" | "default" => Some(InsetShadow::from_style(InsetShadowStyle::Base)),
        "sm" => Some(InsetShadow::from_style(InsetShadowStyle::Sm)),
        "md" => Some(InsetShadow::from_style(InsetShadowStyle::Md)),
        _ => None,
    }
}

fn drop_shadow_from_name(name: &str) -> Option<DropShadow> {
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

fn text_align_from_name(name: &str) -> Option<TextAlign> {
    match name {
        "left" => Some(TextAlign::Left),
        "center" => Some(TextAlign::Center),
        "right" => Some(TextAlign::Right),
        _ => None,
    }
}

impl ScriptDriver {
    pub fn from_source(source: &str) -> anyhow::Result<Self> {
        Ok(Self {
            source: source.to_string(),
        })
    }

    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let source = std::fs::read_to_string(path)?;
        Self::from_source(&source)
    }

    pub(crate) fn create_runner(&self) -> anyhow::Result<ScriptRunner> {
        ScriptRunner::new(&self.source)
    }

    pub(crate) fn cache_key(&self) -> u64 {
        use std::hash::{DefaultHasher, Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.source.hash(&mut hasher);
        hasher.finish()
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

    pub fn source(&self) -> &str {
        &self.source
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

        // Sync text sources from the resolve-phase registry into the runner's store
        // so that __text_source_get() can query them during script execution.
        // Always sync unconditionally: an empty set clears stale entries from
        // previous frames where nodes may have been removed.
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
    animate_api::install_animate_bindings(ctx, store)?;

    // Read-only text source query: returns resolved text for a node id.
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
    ctx.eval::<(), _>(animate_api::ANIMATE_RUNTIME)?;

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

#[cfg(test)]
mod tests {
    use super::{
        CanvasCommand, ScriptColor, ScriptDriver, ScriptFontEdging, ScriptLineCap, ScriptLineJoin,
    };
    use crate::style::{ColorToken, ObjectFit, TextAlign, Transform};

    #[test]
    fn script_driver_records_text_alignment_and_line_height() {
        let driver = ScriptDriver::from_source(
            r#"
            const title = ctx.getNode("title");
            title.textAlign("center").lineHeight(1.8);
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(0, 1, 0, 1, None).expect("script should run");
        let title = mutations.get("title").expect("title mutation should exist");

        assert_eq!(title.text_align, Some(TextAlign::Center));
        assert_eq!(title.line_height, Some(1.8));
    }

    #[test]
    fn script_driver_exposes_global_and_scene_frame_fields() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.getNode("box")
                .translateX(ctx.frame + ctx.totalFrames)
                .translateY(ctx.currentFrame + ctx.sceneFrames);
        "#,
        )
        .expect("script should compile");

        let mutations = driver
            .run(12, 240, 3, 30, Some("box"))
            .expect("script should run");
        let node = mutations.get("box").expect("box mutation should exist");

        assert_eq!(
            node.transforms,
            vec![Transform::TranslateX(252.0), Transform::TranslateY(33.0)]
        );
    }

    #[test]
    fn script_driver_preserves_transform_call_order() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.getNode("box")
                .translateX(40)
                .rotate(15)
                .scale(1.2);
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(0, 1, 0, 1, None).expect("script should run");
        let node = mutations.get("box").expect("box mutation should exist");

        assert_eq!(
            node.transforms,
            vec![
                Transform::TranslateX(40.0),
                Transform::RotateDeg(15.0),
                Transform::Scale(1.2),
            ]
        );
    }

    #[test]
    fn script_driver_records_lucide_fill_and_stroke() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.getNode("icon")
                .strokeColor("blue")
                .strokeWidth(3)
                .fillColor("sky200");
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(0, 1, 0, 1, None).expect("script should run");
        let icon = mutations.get("icon").expect("icon mutation should exist");

        assert_eq!(icon.stroke_color, Some(ColorToken::Blue));
        assert_eq!(icon.stroke_width, Some(3.0));
        assert_eq!(icon.fill_color, Some(ColorToken::Sky200));
        assert_eq!(icon.border_color, None);
        assert_eq!(icon.border_width, None);
        assert_eq!(icon.bg_color, None);
    }

    #[test]
    fn script_driver_records_standard_canvaskit_rect_and_image_commands() {
        let driver = ScriptDriver::from_source(
            r##"
            const CK = ctx.CanvasKit;
            const canvas = ctx.getCanvas();
            const fill = new CK.Paint();
            fill.setStyle(CK.PaintStyle.Fill);
            fill.setColor(CK.Color(255, 0, 0, 1));

            const image = ctx.getImage("hero");
            canvas
                .drawRect(CK.XYWHRect(0, 0, 40, 20), fill)
                .drawImageRect(
                    image,
                    CK.XYWHRect(0, 0, 1, 1),
                    CK.XYWHRect(10, 10, 80, 60),
                );
        "##,
        )
        .expect("script should compile");

        let mutations = driver
            .run(0, 1, 0, 1, Some("card"))
            .expect("script should run");
        let canvas = mutations
            .get_canvas("card")
            .expect("canvas mutation should exist");

        assert_eq!(
            canvas.commands[0],
            CanvasCommand::SetAntiAlias { enabled: true }
        );
        assert_eq!(
            canvas.commands[1],
            CanvasCommand::FillRect {
                x: 0.0,
                y: 0.0,
                width: 40.0,
                height: 20.0,
                color: ScriptColor {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 255,
                },
            }
        );
        assert_eq!(
            canvas.commands[2],
            CanvasCommand::DrawImage {
                asset_id: "hero".to_string(),
                x: 10.0,
                y: 10.0,
                width: 80.0,
                height: 60.0,
                src_rect: Some([0.0, 0.0, 1.0, 1.0]),
                alpha: 1.0,
                anti_alias: true,
                object_fit: ObjectFit::Fill,
            }
        );
    }

    #[test]
    fn script_driver_applies_standard_stroke_paint_to_path_commands() {
        let driver = ScriptDriver::from_source(
            r##"
            const CK = ctx.CanvasKit;
            const canvas = ctx.getCanvas();
            const stroke = new CK.Paint();
            stroke.setStyle(CK.PaintStyle.Stroke);
            stroke.setColor(CK.parseColorString("rgba(251,191,36,0.2)"));
            stroke.setStrokeWidth(3);
            stroke.setStrokeCap(CK.StrokeCap.Round);
            stroke.setStrokeJoin(CK.StrokeJoin.Bevel);

            const path = new CK.Path();
            path
                .moveTo(0, 0)
                .lineTo(10, 0)
                .quadTo(15, 5, 10, 10)
                .cubicTo(8, 14, 2, 14, 0, 10)
                .close();

            canvas.drawPath(path, stroke);
        "##,
        )
        .expect("script should compile");

        let mutations = driver
            .run(0, 1, 0, 1, Some("card"))
            .expect("script should run");
        let canvas = mutations
            .get_canvas("card")
            .expect("canvas mutation should exist");

        assert_eq!(
            canvas.commands[0],
            CanvasCommand::SetAntiAlias { enabled: true }
        );
        assert_eq!(
            canvas.commands[1],
            CanvasCommand::SetStrokeStyle {
                color: ScriptColor {
                    r: 251,
                    g: 191,
                    b: 36,
                    a: 51,
                },
            }
        );
        assert_eq!(
            canvas.commands[2],
            CanvasCommand::SetLineWidth { width: 3.0 }
        );
        assert_eq!(
            canvas.commands[3],
            CanvasCommand::SetLineCap {
                cap: ScriptLineCap::Round,
            }
        );
        assert_eq!(
            canvas.commands[4],
            CanvasCommand::SetLineJoin {
                join: ScriptLineJoin::Bevel,
            }
        );
        assert!(matches!(canvas.commands[5], CanvasCommand::ClearLineDash));
        assert!(matches!(canvas.commands[6], CanvasCommand::BeginPath));
        assert!(matches!(canvas.commands[11], CanvasCommand::ClosePath));
        assert!(matches!(canvas.commands[12], CanvasCommand::StrokePath));
    }

    #[test]
    fn script_driver_supports_standard_rrect_circle_and_rotate_pivot() {
        let driver = ScriptDriver::from_source(
            r##"
            const CK = ctx.CanvasKit;
            const canvas = ctx.getCanvas();
            const fill = new CK.Paint();
            fill.setStyle(CK.PaintStyle.Fill);
            fill.setColor(CK.parseColorString("#112233"));

            const stroke = new CK.Paint();
            stroke.setStyle(CK.PaintStyle.Stroke);
            stroke.setColor(CK.parseColorString("#445566"));
            stroke.setStrokeWidth(3);

            canvas.save();
            canvas.rotate(15, 20, 30);
            canvas.drawRRect(CK.RRectXY(CK.XYWHRect(1, 2, 30, 40), 6, 6), fill);
            canvas.drawCircle(12, 14, 8, stroke);
            canvas.restore();
        "##,
        )
        .expect("script should compile");

        let mutations = driver
            .run(0, 1, 0, 1, Some("card"))
            .expect("script should run");
        let canvas = mutations
            .get_canvas("card")
            .expect("canvas mutation should exist");

        assert_eq!(canvas.commands[0], CanvasCommand::Save);
        assert_eq!(
            canvas.commands[1],
            CanvasCommand::Translate { x: 20.0, y: 30.0 }
        );
        assert_eq!(canvas.commands[2], CanvasCommand::Rotate { degrees: 15.0 });
        assert_eq!(
            canvas.commands[3],
            CanvasCommand::Translate { x: -20.0, y: -30.0 }
        );
        assert_eq!(
            canvas.commands[4],
            CanvasCommand::SetAntiAlias { enabled: true }
        );
        assert_eq!(
            canvas.commands[5],
            CanvasCommand::SetFillStyle {
                color: ScriptColor {
                    r: 17,
                    g: 34,
                    b: 51,
                    a: 255,
                },
            }
        );
        assert!(matches!(
            canvas.commands[6],
            CanvasCommand::FillRRect { .. }
        ));
        assert_eq!(
            canvas.commands[7],
            CanvasCommand::SetAntiAlias { enabled: true }
        );
        assert_eq!(
            canvas.commands[8],
            CanvasCommand::SetStrokeStyle {
                color: ScriptColor {
                    r: 68,
                    g: 85,
                    b: 102,
                    a: 255,
                },
            }
        );
        assert_eq!(
            canvas.commands[9],
            CanvasCommand::SetLineWidth { width: 3.0 }
        );
        assert!(matches!(canvas.commands[12], CanvasCommand::ClearLineDash));
        assert!(matches!(
            canvas.commands[13],
            CanvasCommand::StrokeCircle { .. }
        ));
        assert_eq!(canvas.commands[14], CanvasCommand::Restore);
    }

    #[test]
    fn script_driver_supports_canvas_global_alpha() {
        let driver = ScriptDriver::from_source(
            r##"
            const canvas = ctx.getCanvas();
            canvas.setAlphaf(0.25);
        "##,
        )
        .expect("script should compile");

        let mutations = driver
            .run(0, 1, 0, 1, Some("card"))
            .expect("script should run");
        let canvas = mutations
            .get_canvas("card")
            .expect("canvas mutation should exist");

        assert_eq!(
            canvas.commands[0],
            CanvasCommand::SetGlobalAlpha { alpha: 0.25 }
        );
    }

    #[test]
    fn script_driver_accepts_stroke_dash_paint_api() {
        let driver = ScriptDriver::from_source(
            r##"
            const CK = ctx.CanvasKit;
            const canvas = ctx.getCanvas();
            const stroke = new CK.Paint();
            stroke.setStyle(CK.PaintStyle.Stroke);
            stroke.setColor(CK.parseColorString("#445566"));
            stroke.setStrokeWidth(3);
            stroke.setStrokeDash([6, 4]);

            canvas.drawLine(0, 0, 10, 10, stroke);
        "##,
        )
        .expect("script should compile");

        let mutations = driver
            .run(0, 1, 0, 1, Some("card"))
            .expect("script should run");
        let canvas = mutations
            .get_canvas("card")
            .expect("canvas mutation should exist");

        assert_eq!(
            canvas.commands[0],
            CanvasCommand::SetAntiAlias { enabled: true }
        );
        assert_eq!(
            canvas.commands[1],
            CanvasCommand::SetStrokeStyle {
                color: ScriptColor {
                    r: 68,
                    g: 85,
                    b: 102,
                    a: 255,
                },
            }
        );
        assert_eq!(
            canvas.commands[2],
            CanvasCommand::SetLineWidth { width: 3.0 }
        );
        assert!(matches!(
            canvas.commands[5],
            CanvasCommand::SetLineDash { .. }
        ));
        assert!(matches!(canvas.commands[6], CanvasCommand::DrawLine { .. }));
    }

    #[test]
    fn script_driver_accepts_path_effect_dash_api() {
        let driver = ScriptDriver::from_source(
            r##"
            const CK = ctx.CanvasKit;
            const canvas = ctx.getCanvas();
            const stroke = new CK.Paint();
            stroke.setStyle(CK.PaintStyle.Stroke);
            stroke.setColor(CK.parseColorString("#445566"));
            stroke.setStrokeWidth(2);
            stroke.setPathEffect(CK.PathEffect.MakeDash([3, 2], 1));

            canvas.drawLine(1, 2, 11, 12, stroke);
        "##,
        )
        .expect("script should compile");

        let mutations = driver
            .run(0, 1, 0, 1, Some("card"))
            .expect("script should run");
        let canvas = mutations
            .get_canvas("card")
            .expect("canvas mutation should exist");

        assert_eq!(
            canvas.commands[5],
            CanvasCommand::SetLineDash {
                intervals: vec![3.0, 2.0],
                phase: 1.0,
            }
        );
        assert_eq!(
            canvas.commands[6],
            CanvasCommand::DrawLine {
                x0: 1.0,
                y0: 2.0,
                x1: 11.0,
                y1: 12.0,
            }
        );
    }

    #[test]
    fn script_driver_supports_path_shape_builders() {
        let driver = ScriptDriver::from_source(
            r##"
            const CK = ctx.CanvasKit;
            const canvas = ctx.getCanvas();
            const fill = new CK.Paint();
            fill.setStyle(CK.PaintStyle.Fill);
            fill.setColor(CK.BLACK);

            const path = new CK.Path();
            path
                .addRect(CK.LTRBRect(0, 0, 10, 5))
                .addRRect(CK.RRectXY(CK.LTRBRect(2, 3, 8, 9), 1, 1))
                .addOval(CK.LTRBRect(10, 20, 30, 40))
                .addArc(CK.LTRBRect(5, 6, 15, 26), 10, 90);

            canvas.drawPath(path, fill);
        "##,
        )
        .expect("script should compile");

        let mutations = driver
            .run(0, 1, 0, 1, Some("card"))
            .expect("script should run");
        let canvas = mutations
            .get_canvas("card")
            .expect("canvas mutation should exist");

        assert_eq!(
            canvas.commands[3],
            CanvasCommand::AddRectPath {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 5.0,
            }
        );
        assert_eq!(
            canvas.commands[4],
            CanvasCommand::AddRRectPath {
                x: 2.0,
                y: 3.0,
                width: 6.0,
                height: 6.0,
                radius: 1.0,
            }
        );
        assert_eq!(
            canvas.commands[5],
            CanvasCommand::AddOvalPath {
                x: 10.0,
                y: 20.0,
                width: 20.0,
                height: 20.0,
            }
        );
        assert_eq!(
            canvas.commands[6],
            CanvasCommand::AddArcPath {
                x: 5.0,
                y: 6.0,
                width: 10.0,
                height: 20.0,
                start_angle: 10.0,
                sweep_angle: 90.0,
            }
        );
        assert!(matches!(canvas.commands[7], CanvasCommand::FillPath));
    }

    #[test]
    fn script_driver_reset_and_rewind_clear_path_ops() {
        let driver = ScriptDriver::from_source(
            r##"
            const CK = ctx.CanvasKit;
            const canvas = ctx.getCanvas();
            const fill = new CK.Paint();
            fill.setStyle(CK.PaintStyle.Fill);
            fill.setColor(CK.BLACK);

            const path = new CK.Path();
            path.addRect(CK.LTRBRect(0, 0, 10, 5));
            path.reset();
            path.addOval(CK.LTRBRect(1, 2, 11, 12));
            path.rewind();
            path.addArc(CK.LTRBRect(3, 4, 13, 24), 15, 180);

            canvas.drawPath(path, fill);
        "##,
        )
        .expect("script should compile");

        let mutations = driver
            .run(0, 1, 0, 1, Some("card"))
            .expect("script should run");
        let canvas = mutations
            .get_canvas("card")
            .expect("canvas mutation should exist");

        assert_eq!(canvas.commands.len(), 5);
        assert!(matches!(canvas.commands[2], CanvasCommand::BeginPath));
        assert_eq!(
            canvas.commands[3],
            CanvasCommand::AddArcPath {
                x: 3.0,
                y: 4.0,
                width: 10.0,
                height: 20.0,
                start_angle: 15.0,
                sweep_angle: 180.0,
            }
        );
        assert!(matches!(canvas.commands[4], CanvasCommand::FillPath));
    }

    #[test]
    fn script_driver_uses_ltrb_rects_for_arc_and_oval() {
        let driver = ScriptDriver::from_source(
            r##"
            const CK = ctx.CanvasKit;
            const canvas = ctx.getCanvas();
            const fill = new CK.Paint();
            fill.setStyle(CK.PaintStyle.Fill);
            fill.setColor(CK.BLACK);

            const stroke = new CK.Paint();
            stroke.setStyle(CK.PaintStyle.Stroke);
            stroke.setColor(CK.WHITE);

            canvas.drawArc(CK.LTRBRect(10, 20, 30, 50), 15, 120, true, fill);
            canvas.drawOval(CK.LTRBRect(40, 60, 90, 120), stroke);
        "##,
        )
        .expect("script should compile");

        let mutations = driver
            .run(0, 1, 0, 1, Some("card"))
            .expect("script should run");
        let canvas = mutations
            .get_canvas("card")
            .expect("canvas mutation should exist");

        assert_eq!(
            canvas.commands[2],
            CanvasCommand::DrawArc {
                cx: 20.0,
                cy: 35.0,
                rx: 10.0,
                ry: 15.0,
                start_angle: 15.0,
                sweep_angle: 120.0,
                use_center: true,
            }
        );
        assert_eq!(
            canvas.commands[9],
            CanvasCommand::StrokeOval {
                cx: 65.0,
                cy: 90.0,
                rx: 25.0,
                ry: 30.0,
            }
        );
    }

    #[test]
    fn script_driver_records_restore_to_count_and_antialias_flags() {
        let driver = ScriptDriver::from_source(
            r##"
            const CK = ctx.CanvasKit;
            const canvas = ctx.getCanvas();
            const fill = new CK.Paint();
            fill.setStyle(CK.PaintStyle.Fill);
            fill.setColor(CK.BLACK);
            fill.setAntiAlias(false);

            const saveCount = canvas.save();
            canvas.clipRect(CK.LTRBRect(0, 0, 20, 10), CK.ClipOp.Intersect, false);
            canvas.drawRect(CK.XYWHRect(0, 0, 10, 5), fill);
            canvas.restoreToCount(saveCount);
        "##,
        )
        .expect("script should compile");

        let mutations = driver
            .run(0, 1, 0, 1, Some("card"))
            .expect("script should run");
        let canvas = mutations
            .get_canvas("card")
            .expect("canvas mutation should exist");

        assert_eq!(canvas.commands[0], CanvasCommand::Save);
        assert_eq!(
            canvas.commands[1],
            CanvasCommand::ClipRect {
                x: 0.0,
                y: 0.0,
                width: 20.0,
                height: 10.0,
                anti_alias: false,
            }
        );
        assert_eq!(
            canvas.commands[2],
            CanvasCommand::SetAntiAlias { enabled: false }
        );
        assert_eq!(
            canvas.commands[3],
            CanvasCommand::FillRect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 5.0,
                color: ScriptColor {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                },
            }
        );
        assert_eq!(
            canvas.commands[4],
            CanvasCommand::RestoreToCount { count: 2 }
        );
    }

    #[test]
    fn script_driver_records_image_source_rect_and_paint_alpha() {
        let driver = ScriptDriver::from_source(
            r##"
            const CK = ctx.CanvasKit;
            const canvas = ctx.getCanvas();
            const image = ctx.getImage("hero");
            const paint = new CK.Paint();
            paint.setAlphaf(0.5);
            paint.setAntiAlias(false);

            canvas.drawImageRect(
                image,
                CK.LTRBRect(2, 4, 12, 14),
                CK.LTRBRect(10, 20, 30, 50),
                paint,
            );
            canvas.drawImage(image, 8, 9, paint);
        "##,
        )
        .expect("script should compile");

        let mutations = driver
            .run(0, 1, 0, 1, Some("card"))
            .expect("script should run");
        let canvas = mutations
            .get_canvas("card")
            .expect("canvas mutation should exist");

        assert_eq!(
            canvas.commands[0],
            CanvasCommand::DrawImage {
                asset_id: "hero".to_string(),
                x: 10.0,
                y: 20.0,
                width: 20.0,
                height: 30.0,
                src_rect: Some([2.0, 4.0, 10.0, 10.0]),
                alpha: 0.5,
                anti_alias: false,
                object_fit: ObjectFit::Fill,
            }
        );
        assert_eq!(
            canvas.commands[1],
            CanvasCommand::DrawImageSimple {
                asset_id: "hero".to_string(),
                x: 8.0,
                y: 9.0,
                alpha: 0.5,
                anti_alias: false,
            }
        );
    }

    #[test]
    fn script_driver_records_draw_paint_and_draw_color_variants() {
        let driver = ScriptDriver::from_source(
            r##"
            const CK = ctx.CanvasKit;
            const canvas = ctx.getCanvas();
            const paint = new CK.Paint();
            paint.setColor(CK.Color4f(0.25, 0.5, 0.75, 0.5));
            paint.setAntiAlias(false);

            canvas.drawPaint(paint);
            canvas.drawColor(CK.parseColorString("#112233"));
            canvas.drawColorInt(CK.ColorAsInt(68, 85, 102, 0.5));
            canvas.drawColorComponents(0.2, 0.4, 0.6, 0.8);
        "##,
        )
        .expect("script should compile");

        let mutations = driver
            .run(0, 1, 0, 1, Some("card"))
            .expect("script should run");
        let canvas = mutations
            .get_canvas("card")
            .expect("canvas mutation should exist");

        assert_eq!(
            canvas.commands[0],
            CanvasCommand::DrawPaint {
                color: ScriptColor {
                    r: 64,
                    g: 128,
                    b: 191,
                    a: 128,
                },
                anti_alias: false,
            }
        );
        assert_eq!(
            canvas.commands[1],
            CanvasCommand::DrawPaint {
                color: ScriptColor {
                    r: 17,
                    g: 34,
                    b: 51,
                    a: 255,
                },
                anti_alias: true,
            }
        );
        assert_eq!(
            canvas.commands[2],
            CanvasCommand::DrawPaint {
                color: ScriptColor {
                    r: 68,
                    g: 85,
                    b: 102,
                    a: 128,
                },
                anti_alias: true,
            }
        );
        assert_eq!(
            canvas.commands[3],
            CanvasCommand::DrawPaint {
                color: ScriptColor {
                    r: 51,
                    g: 102,
                    b: 153,
                    a: 204,
                },
                anti_alias: true,
            }
        );
    }

    #[test]
    fn script_driver_records_save_layer_bounds_and_alpha() {
        let driver = ScriptDriver::from_source(
            r##"
            const CK = ctx.CanvasKit;
            const canvas = ctx.getCanvas();
            const paint = new CK.Paint();
            paint.setAlphaf(0.25);

            const count = canvas.saveLayer(paint, CK.LTRBRect(1, 2, 11, 12));
            canvas.drawColor(CK.WHITE);
            canvas.restoreToCount(count - 1);
        "##,
        )
        .expect("script should compile");

        let mutations = driver
            .run(0, 1, 0, 1, Some("card"))
            .expect("script should run");
        let canvas = mutations
            .get_canvas("card")
            .expect("canvas mutation should exist");

        assert_eq!(
            canvas.commands[0],
            CanvasCommand::SaveLayer {
                alpha: 0.25,
                bounds: Some([1.0, 2.0, 10.0, 10.0]),
            }
        );
        assert_eq!(
            canvas.commands[1],
            CanvasCommand::DrawPaint {
                color: ScriptColor {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 255,
                },
                anti_alias: true,
            }
        );
        assert_eq!(
            canvas.commands[2],
            CanvasCommand::RestoreToCount { count: 1 }
        );
    }

    #[test]
    fn script_driver_records_canvas_text_commands() {
        let driver = ScriptDriver::from_source(
            r##"
            const CK = ctx.CanvasKit;
            const canvas = ctx.getCanvas();
            const paint = new CK.Paint();
            paint.setStyle(CK.PaintStyle.Stroke);
            paint.setColor(CK.parseColorString("#112233"));
            paint.setStrokeWidth(2);
            paint.setAntiAlias(false);

            const font = new CK.Font(null, 28, 1.2, 0.1);
            font.setSubpixel(false);
            font.setEdging(CK.FontEdging.Alias);

            canvas.drawText("Type", 12, 34, paint, font);
        "##,
        )
        .expect("script should compile");

        let mutations = driver
            .run(0, 1, 0, 1, Some("card"))
            .expect("script should run");
        let canvas = mutations
            .get_canvas("card")
            .expect("canvas mutation should exist");

        assert_eq!(
            canvas.commands[0],
            CanvasCommand::DrawText {
                text: "Type".to_string(),
                x: 12.0,
                y: 34.0,
                color: ScriptColor {
                    r: 17,
                    g: 34,
                    b: 51,
                    a: 255,
                },
                anti_alias: false,
                stroke: true,
                stroke_width: 2.0,
                font_size: 28.0,
                font_scale_x: 1.2,
                font_skew_x: 0.1,
                font_subpixel: false,
                font_edging: ScriptFontEdging::Alias,
            }
        );
    }

    #[test]
    fn script_driver_exposes_font_measure_text() {
        let driver = ScriptDriver::from_source(
            r##"
            const CK = ctx.CanvasKit;
            const font = new CK.Font(null, 24);
            const width = font.measureText("Hello");
            ctx.getNode("box").translateX(width);
        "##,
        )
        .expect("script should compile");

        let mutations = driver.run(0, 1, 0, 1, None).expect("script should run");
        let node = mutations.get("box").expect("box mutation should exist");

        let tx = match &node.transforms[0] {
            Transform::TranslateX(v) => *v,
            _ => panic!("expected TranslateX"),
        };
        assert!(
            tx > 20.0,
            "measureText should return usable width, got {}",
            tx
        );
    }

    #[test]
    fn script_driver_animate_linear_opacity() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.fromTo("box", { opacity: 0 }, {
                opacity: 1,
                duration: 20,
                ease: 'linear',
            });
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(10, 20, 10, 20, None).expect("script should run");
        let node = mutations.get("box").expect("box mutation should exist");

        assert!((node.opacity.unwrap() - 0.5).abs() < 0.01);
    }

    #[test]
    fn script_driver_animation_plugin_registers_custom_property() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.registerPlugin({
                name: 'pulse-plugin',
                properties: {
                    pulse: {
                        defaultValue: 0,
                        interpolate: 'number',
                        apply: function(target, value) {
                            target.node.scale(1 + value);
                        },
                    },
                },
            });

            ctx.fromTo("box", { pulse: 0 }, {
                pulse: 0.5,
                duration: 10,
                ease: 'linear',
            });
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(5, 20, 5, 20, None).expect("script should run");
        let node = mutations.get("box").expect("box mutation should exist");

        let scale = match &node.transforms[0] {
            Transform::Scale(v) => *v,
            _ => panic!("expected Scale from custom animation plugin"),
        };
        assert!(
            (scale - 1.25).abs() < 0.01,
            "custom plugin should sample pulse=0.25 and apply scale=1.25, got {}",
            scale
        );
    }

    #[test]
    fn script_driver_animation_plugins_report_installed_modules() {
        let driver = ScriptDriver::from_source(
            r#"
            var names = ctx.animation.plugins();
            [
                'style-props',
                'color',
                'text',
                'split-text',
                'motion-path',
                'utils',
            ].forEach(function(name) {
                if (names.indexOf(name) === -1) {
                    throw new Error('missing animation plugin: ' + name + ' in ' + names.join(','));
                }
            });

            if (typeof ctx.splitText !== 'function') {
                throw new Error('split-text plugin did not install ctx.splitText');
            }
            if (typeof ctx.utils.random !== 'function') {
                throw new Error('utils plugin did not install ctx.utils');
            }
        "#,
        )
        .expect("script should compile");

        driver
            .run(0, 1, 0, 1, None)
            .expect("plugin module introspection should run");
    }

    #[test]
    fn script_driver_animate_ease_out_translate() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.fromTo("box", { x: 0 }, {
                x: 100,
                duration: 20,
                ease: 'ease-out',
            });
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(10, 20, 10, 20, None).expect("script should run");
        let node = mutations.get("box").expect("box mutation should exist");

        let tx = match &node.transforms[0] {
            Transform::TranslateX(v) => *v,
            _ => panic!("expected TranslateX"),
        };
        assert!(tx > 50.0, "ease-out should be > 50% at halfway, got {}", tx);
    }

    #[test]
    fn script_driver_animate_spring_auto_duration() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.fromTo("box", { opacity: 0 }, {
                opacity: 1,
                ease: 'spring.stiff',
            });
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(0, 60, 0, 60, None).expect("script should run");
        let node = mutations.get("box").expect("box mutation should exist");

        assert!((node.opacity.unwrap() - 0.0).abs() < 0.01);
    }

    #[test]
    fn script_driver_animate_settle_frame() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.fromTo("box", { opacity: 0 }, {
                opacity: 1,
                duration: 20,
                delay: 5,
                ease: 'linear',
            });
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(30, 60, 30, 60, None).expect("script should run");
        let node = mutations.get("box").expect("box mutation should exist");

        assert!((node.opacity.unwrap() - 1.0).abs() < 0.01);
    }

    #[test]
    fn script_driver_stagger_animations() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.fromTo(["a", "b", "c"], { opacity: 0 }, {
                opacity: 1,
                duration: 10,
                stagger: 5,
                ease: 'linear',
            });
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(5, 30, 5, 30, None).expect("script should run");

        let a = mutations.get("a").expect("a mutation should exist");
        let b = mutations.get("b").expect("b mutation should exist");

        let a_opacity = a.opacity.unwrap();
        let b_opacity = b.opacity.unwrap();

        assert!(
            a_opacity > b_opacity,
            "a should be more animated than b: a={} b={}",
            a_opacity,
            b_opacity
        );
    }

    #[test]
    fn script_driver_stagger_defaults_to_scene_fit() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.fromTo(["a", "b", "c", "d"], { opacity: 0 }, {
                opacity: 1,
                duration: 10,
                delay: 5,
                stagger: 10,
                ease: 'linear',
            });
        "#,
        )
        .expect("script should compile");

        let mutations = driver
            .run(25, 100, 25, 30, None)
            .expect("script should run");

        let d = mutations.get("d").expect("d mutation should exist");
        let d_opacity = d.opacity.unwrap();

        assert!(
            d_opacity > 0.4,
            "stagger should be compressed to fit within the scene; d opacity={}",
            d_opacity
        );
    }

    #[test]
    fn script_driver_stagger_keeps_user_value_when_it_fits_scene() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.fromTo(["a", "b", "c"], { opacity: 0 }, {
                opacity: 1,
                duration: 10,
                delay: 0,
                stagger: 5,
                ease: 'linear',
            });
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(5, 100, 5, 30, None).expect("script should run");

        let a = mutations.get("a").expect("a mutation should exist");
        let b = mutations.get("b").expect("b mutation should exist");
        let c = mutations.get("c").expect("c mutation should exist");

        assert!(
            a.opacity.unwrap() > 0.4,
            "a should have progressed at frame 5"
        );
        assert_eq!(
            b.opacity,
            Some(0.0),
            "b should start exactly at frame 5 if stagger remains 5"
        );
        assert_eq!(
            c.opacity,
            Some(0.0),
            "c should not start before frame 10 if stagger remains 5"
        );
    }

    #[test]
    fn script_driver_animate_custom_bezier() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.fromTo("box", { scale: 0.5 }, {
                scale: 1.0,
                duration: 20,
                ease: [0.68, -0.6, 0.32, 1.6],
            });
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(10, 20, 10, 20, None).expect("script should run");
        let node = mutations.get("box").expect("box mutation should exist");

        let sc = match &node.transforms[0] {
            Transform::Scale(v) => *v,
            _ => panic!("expected Scale"),
        };
        assert!(
            sc >= 0.75,
            "bezier should produce value >= 0.75 at halfway, got {}",
            sc
        );
    }

    #[test]
    fn script_driver_sequence_accumulates_cursor() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.timeline()
                .fromTo("a", { opacity: 0 }, { opacity: 1, duration: 10, ease: 'linear' })
                .fromTo("b", { opacity: 0 }, { opacity: 1, duration: 10, ease: 'linear' });
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(10, 30, 10, 30, None).expect("script should run");
        let a = mutations.get("a").expect("a mutation should exist");
        let b = mutations.get("b").expect("b mutation should exist");

        assert!(
            (a.opacity.unwrap() - 1.0).abs() < 0.01,
            "step 0 should be settled at frame 10, got {}",
            a.opacity.unwrap()
        );
        assert!(
            b.opacity.unwrap() < 0.05,
            "step 1 should barely have started at frame 10, got {}",
            b.opacity.unwrap()
        );
    }

    #[test]
    fn script_driver_sequence_future_step_does_not_override_same_property() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.timeline()
                .fromTo("box", { opacity: 0 }, { opacity: 1, duration: 10, ease: 'linear' })
                .fromTo("box", { opacity: 1 }, { opacity: 0, duration: 10, ease: 'linear' });
        "#,
        )
        .expect("script should compile");

        let first_step = driver
            .run(5, 20, 5, 20, None)
            .expect("first step should run");
        let box_node = first_step.get("box").expect("box mutation should exist");
        assert!(
            (box_node.opacity.unwrap() - 0.5).abs() < 0.01,
            "future step should not force opacity to its from-value before it starts, got {}",
            box_node.opacity.unwrap()
        );

        let second_step = driver
            .run(15, 20, 15, 20, None)
            .expect("second step should run");
        let box_node = second_step.get("box").expect("box mutation should exist");
        assert!(
            (box_node.opacity.unwrap() - 0.5).abs() < 0.01,
            "second step should animate from the first step's end value, got {}",
            box_node.opacity.unwrap()
        );
    }

    #[test]
    fn script_driver_timeline_future_from_step_holds_initial_values() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.timeline({ defaults: { duration: 10, ease: 'linear' } })
                .from("a", { opacity: 0, x: -30 })
                .from("b", { opacity: 0, x: -30 })
                .from("c", { opacity: 0, x: -30 });
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(5, 40, 5, 40, None).expect("script should run");
        let a = mutations.get("a").expect("a mutation should exist");
        let b = mutations.get("b").expect("b mutation should exist");
        let c = mutations.get("c").expect("c mutation should exist");

        assert!(
            (a.opacity.unwrap() - 0.5).abs() < 0.01,
            "active step should interpolate opacity, got {}",
            a.opacity.unwrap()
        );
        assert_eq!(
            b.opacity,
            Some(0.0),
            "future step should hold from opacity before it starts"
        );
        assert_eq!(
            c.opacity,
            Some(0.0),
            "future step should hold from opacity before it starts"
        );

        let bx = match &b.transforms[0] {
            Transform::TranslateX(v) => *v,
            _ => panic!("expected b TranslateX"),
        };
        let cx = match &c.transforms[0] {
            Transform::TranslateX(v) => *v,
            _ => panic!("expected c TranslateX"),
        };
        assert_eq!(bx, -30.0);
        assert_eq!(cx, -30.0);
    }

    #[test]
    fn script_driver_timeline_relative_cursor_positions_advance_sequence() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.timeline({ defaults: { duration: 10, ease: 'linear' } })
                .fromTo("a", { opacity: 0 }, { opacity: 1 })
                .fromTo("b", { opacity: 0 }, { opacity: 1 }, "+=3")
                .fromTo("c", { opacity: 0 }, { opacity: 1 }, "+=3");
        "#,
        )
        .expect("script should compile");

        let before_c = driver.run(25, 50, 25, 50, None).expect("script should run");
        let b = before_c.get("b").expect("b mutation should exist");
        let c = before_c.get("c").expect("c mutation should exist");

        assert!(
            (b.opacity.unwrap() - 1.0).abs() < 0.01,
            "b should be settled before c starts, got {}",
            b.opacity.unwrap()
        );
        assert_eq!(
            c.opacity,
            Some(0.0),
            "c should still hold its from opacity before its delayed start"
        );

        let during_c = driver.run(31, 50, 31, 50, None).expect("script should run");
        let c = during_c.get("c").expect("c mutation should exist");
        assert!(
            c.opacity.unwrap() > 0.0 && c.opacity.unwrap() < 1.0,
            "c should animate after the second relative delay, got {}",
            c.opacity.unwrap()
        );
    }

    #[test]
    fn script_driver_timeline_supports_previous_start_position() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.timeline()
                .fromTo("a", { opacity: 0 }, { opacity: 1, duration: 10, ease: 'linear' })
                .fromTo("b", { opacity: 0 }, { opacity: 1, duration: 20, ease: 'linear' }, "<");
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(5, 30, 5, 30, None).expect("script should run");
        let a = mutations.get("a").expect("a mutation should exist");
        let b = mutations.get("b").expect("b mutation should exist");

        assert!(
            (a.opacity.unwrap() - 0.5).abs() < 0.01,
            "first tween should be halfway at frame 5, got {}",
            a.opacity.unwrap()
        );
        assert!(
            (b.opacity.unwrap() - 0.25).abs() < 0.01,
            "`<` should align second tween with previous start, got {}",
            b.opacity.unwrap()
        );
    }

    #[test]
    fn script_driver_sequence_absolute_position_extends_timeline_end() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.timeline()
                .fromTo("a", { opacity: 0 }, { opacity: 1, duration: 10, ease: 'linear' })
                .fromTo("b", { opacity: 0 }, { opacity: 1, duration: 20, ease: 'linear' }, 5)
                .fromTo("c", { opacity: 0 }, { opacity: 1, duration: 10, ease: 'linear' });
        "#,
        )
        .expect("script should compile");

        let mutations = driver
            .run(20, 100, 20, 100, None)
            .expect("script should run");
        let a = mutations.get("a").expect("a mutation should exist");
        let b = mutations.get("b").expect("b mutation should exist");
        let c = mutations.get("c").expect("c mutation should exist");

        assert!(
            (a.opacity.unwrap() - 1.0).abs() < 0.01,
            "step 0 should be settled at frame 20, got {}",
            a.opacity.unwrap()
        );
        assert!(
            (b.opacity.unwrap() - 0.75).abs() < 0.01,
            "step 1 at=5 d=20 should be 0.75 at frame 20, got {}",
            b.opacity.unwrap()
        );
        assert_eq!(
            c.opacity,
            Some(0.0),
            "step 2 should not start before the timeline end at frame 25"
        );

        let later = driver
            .run(30, 100, 30, 100, None)
            .expect("script should run");
        let c = later.get("c").expect("c mutation should exist");
        assert!(
            (c.opacity.unwrap() - 0.5).abs() < 0.01,
            "step 2 should start from the timeline end at frame 25, got {}",
            c.opacity.unwrap()
        );
    }

    #[test]
    fn script_driver_sequence_negative_gap_overlaps() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.timeline()
                .fromTo("a", { opacity: 0 }, { opacity: 1, duration: 10, ease: 'linear' })
                .fromTo("b", { opacity: 0 }, { opacity: 1, duration: 10, ease: 'linear' }, "-=4");
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(8, 30, 8, 30, None).expect("script should run");
        let a = mutations.get("a").expect("a mutation should exist");
        let b = mutations.get("b").expect("b mutation should exist");

        assert!(
            (a.opacity.unwrap() - 0.8).abs() < 0.01,
            "step 0 should be 0.8 at frame 8, got {}",
            a.opacity.unwrap()
        );
        assert!(
            (b.opacity.unwrap() - 0.2).abs() < 0.01,
            "step 1 should be 0.2 at frame 8 (started at frame 6 due to gap=-4), got {}",
            b.opacity.unwrap()
        );
    }

    #[test]
    fn script_driver_timeline_supports_previous_anchor_shorthand_offsets() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.timeline()
                .fromTo("a", { opacity: 0 }, { opacity: 1, duration: 10, ease: 'linear' })
                .fromTo("b", { opacity: 0 }, { opacity: 1, duration: 10, ease: 'linear' }, "<3")
                .fromTo("c", { opacity: 0 }, { opacity: 1, duration: 10, ease: 'linear' }, ">-2");
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(8, 30, 8, 30, None).expect("script should run");
        let b = mutations.get("b").expect("b mutation should exist");
        let c = mutations.get("c").expect("c mutation should exist");

        assert!(
            (b.opacity.unwrap() - 0.5).abs() < 0.01,
            "`<3` should start 3 frames after previous start, got {}",
            b.opacity.unwrap()
        );
        assert_eq!(
            c.opacity,
            Some(0.0),
            "`>-2` should not start before frame 11 in this sequence"
        );
    }

    #[test]
    fn script_driver_timeline_scales_to_scene_frames_when_oversized() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.timeline()
                .fromTo("a", { opacity: 0 }, { opacity: 1, duration: 20, ease: 'linear' })
                .fromTo("b", { opacity: 0 }, { opacity: 1, duration: 20, ease: 'linear' })
                .fromTo("c", { opacity: 0 }, { opacity: 1, duration: 20, ease: 'linear' });
        "#,
        )
        .expect("script should compile");

        let last_frame = driver
            .run(30, 100, 30, 30, None)
            .expect("script should run");
        let c = last_frame.get("c").expect("c mutation should exist");

        assert!(
            (c.opacity.unwrap() - 1.0).abs() < 0.01,
            "oversized timeline should fit the current scene and finish by its last frame, got {}",
            c.opacity.unwrap()
        );
    }

    #[test]
    fn script_driver_sequence_per_step_duration_and_easing() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.timeline()
                .fromTo("box", { x: 0 }, { x: 100, duration: 20, ease: 'linear' })
                .fromTo("box", { y: 0 }, { y: 50, duration: 10, ease: 'ease-out' });
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(25, 30, 25, 30, None).expect("script should run");
        let node = mutations.get("box").expect("box mutation should exist");

        let tx = match &node.transforms[0] {
            Transform::TranslateX(v) => *v,
            _ => panic!("expected TranslateX"),
        };
        let ty = match &node.transforms[1] {
            Transform::TranslateY(v) => *v,
            _ => panic!("expected TranslateY"),
        };
        assert!(
            (tx - 100.0).abs() < 0.01,
            "step 0 (linear, 0..20) should be settled at frame 25, got {}",
            tx
        );
        assert!(
            ty > 25.0 && ty < 50.0,
            "step 1 (ease-out, 20..30) at halfway should be between 25 and 50, got {}",
            ty
        );
    }

    #[test]
    fn script_driver_records_text_content_override() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.getNode("title").text("Hello");
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(0, 1, 0, 1, None).expect("script should run");
        let title = mutations.get("title").expect("title mutation should exist");
        assert_eq!(title.text_content, Some("Hello".to_string()));
    }

    #[test]
    fn script_driver_typewriter_progresses_through_characters() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.to("title", { text: "Hello", duration: 10, ease: 'linear' });
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(6, 30, 6, 30, None).expect("script should run");
        let title = mutations.get("title").expect("title mutation should exist");
        assert_eq!(title.text_content, Some("Hel".to_string()));
    }

    #[test]
    fn script_driver_typewriter_start_and_end_bounds() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.to("t", { text: "Cat", duration: 6, ease: 'linear' });
        "#,
        )
        .expect("script should compile");

        let start = driver.run(0, 10, 0, 10, None).expect("frame 0 should run");
        assert_eq!(
            start.get("t").unwrap().text_content,
            Some(String::new()),
            "empty before any chars are revealed"
        );

        let end = driver.run(6, 10, 6, 10, None).expect("frame 6 should run");
        assert_eq!(
            end.get("t").unwrap().text_content,
            Some("Cat".to_string()),
            "full string at end of duration"
        );

        let past = driver
            .run(20, 30, 20, 30, None)
            .expect("frame 20 should run (clamped)");
        assert_eq!(
            past.get("t").unwrap().text_content,
            Some("Cat".to_string()),
            "clamped to full string past duration"
        );
    }

    #[test]
    fn script_driver_typewriter_grapheme_safe_for_cjk() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.to("t", { text: "你好世界", duration: 8, ease: 'linear' });
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(4, 30, 4, 30, None).expect("script should run");
        let t = mutations.get("t").expect("t mutation should exist");
        assert_eq!(t.text_content, Some("你好".to_string()));
    }

    #[test]
    fn script_driver_typewriter_does_not_append_caret() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.to("t", { text: "Hi", duration: 4, ease: 'linear' });
        "#,
        )
        .expect("script should compile");

        let typing = driver.run(2, 10, 2, 10, None).expect("frame 2 should run");
        assert_eq!(
            typing.get("t").unwrap().text_content,
            Some("H".to_string()),
            "text tween should expose only text content; caret is a separate visual concern"
        );

        let settled = driver
            .run(10, 20, 10, 20, None)
            .expect("frame 10 should run");
        assert_eq!(
            settled.get("t").unwrap().text_content,
            Some("Hi".to_string()),
            "no caret once settled"
        );
    }

    #[test]
    fn script_driver_typewriter_supports_cursor_option() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.to("t", { text: "Hi", cursor: "|", cursorBlink: false, duration: 4, ease: 'linear' });
        "#,
        )
        .expect("script should compile");

        let typing = driver.run(2, 10, 2, 10, None).expect("frame 2 should run");
        assert_eq!(
            typing.get("t").unwrap().text_content,
            Some("H|".to_string()),
            "cursor should be appended while typewriter tween is active"
        );

        let settled = driver
            .run(10, 20, 10, 20, None)
            .expect("frame 10 should run");
        assert_eq!(
            settled.get("t").unwrap().text_content,
            Some("Hi".to_string()),
            "cursor should disappear after typewriter tween settles"
        );
    }

    #[test]
    fn script_driver_typewriter_uses_grapheme_clusters() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.to("t", { text: "👨‍👩‍👧‍👦", duration: 2, ease: 'linear' });
        "#,
        )
        .expect("script should compile");

        let f1 = driver.run(1, 10, 1, 10, None).expect("frame 1 should run");
        let t = f1.get("t").expect("t mutation should exist");
        assert_eq!(
            t.text_content,
            Some(String::new()),
            "partial progress must not split a grapheme cluster"
        );

        let f2 = driver.run(2, 10, 2, 10, None).expect("frame 2 should run");
        assert_eq!(f2.get("t").unwrap().text_content, Some("👨‍👩‍👧‍👦".to_string()));
    }

    #[test]
    fn script_runtime_exposes_resolved_text_source_to_split_queries() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.getNode("title").text("Hello");
            var text = __text_source_get("title");
            if (text !== "Hello") {
                throw new Error("unexpected text source: " + text);
            }
        "#,
        )
        .expect("script should compile");

        let result = driver.run(0, 1, 0, 1, None);
        assert!(
            result.is_ok(),
            "text source query should succeed once registry is wired: {:?}",
            result.err()
        );
    }

    #[test]
    fn script_driver_records_text_unit_override_via_part_set() {
        let driver = ScriptDriver::from_source(
            r##"
            ctx.getNode("title").text("Hello");
            var parts = ctx.splitText("title", { type: "chars" });
            if (parts.length !== 5) {
                throw new Error("expected 5 parts, got " + parts.length);
            }
            if (parts[0].text !== "H") {
                throw new Error("expected parts[0].text === 'H', got " + parts[0].text);
            }
            parts[0].set({ opacity: 0.5, translateX: 10, translateY: 20 });
            parts[1].set({ color: "#FFFFFF" });
            parts[2].set({ scale: 1.5, rotation: 45 });
        "##,
        )
        .expect("script should compile");

        let result = driver.run(0, 1, 0, 1, None);
        assert!(
            result.is_ok(),
            "splitText + part.set should succeed: {:?}",
            result.err()
        );

        let mutations = &result.unwrap().mutations;
        let node_mut = mutations
            .get("title")
            .expect("should have mutations for 'title'");
        let batch = node_mut
            .text_unit_overrides
            .as_ref()
            .expect("should have text_unit_overrides");

        use super::node_style::TextUnitGranularity;
        assert!(matches!(batch.granularity, TextUnitGranularity::Grapheme));

        // Index 0: opacity=0.5, translateX=10, translateY=20
        let o0 = &batch.overrides[0];
        assert_eq!(o0.opacity, Some(0.5));
        assert_eq!(o0.translate_x, Some(10.0));
        assert_eq!(o0.translate_y, Some(20.0));
        assert_eq!(o0.scale, None);
        assert_eq!(o0.rotation_deg, None);

        // Index 1: default (not set)
        let o1 = &batch.overrides[1];
        assert_eq!(o1.opacity, None);
        assert_eq!(
            o1.color,
            Some(crate::style::ColorToken::Custom(255, 255, 255, 255))
        );

        // Index 2: scale=1.5, rotation=45
        let o2 = &batch.overrides[2];
        assert_eq!(o2.opacity, None);
        assert_eq!(o2.scale, Some(1.5));
        assert_eq!(o2.rotation_deg, Some(45.0));

        // Vec is resized to max(index) + 1 = 3, not to the full text length
        assert_eq!(batch.overrides.len(), 3);
    }

    #[test]
    fn script_driver_split_text_color_animation_targets_parts() {
        let driver = ScriptDriver::from_source(
            r##"
            ctx.getNode("title").text("Hi");
            ctx.fromTo(
                ctx.splitText("title", { type: "chars" }),
                { color: "#D946EF" },
                { color: "#FFFFFF", duration: 10, ease: "linear" }
            );
        "##,
        )
        .expect("script should compile");

        let result = driver.run(0, 1, 0, 1, None);
        assert!(
            result.is_ok(),
            "splitText color animation should target text units: {:?}",
            result.err()
        );

        let mutations = &result.unwrap().mutations;
        let batch = mutations
            .get("title")
            .and_then(|node| node.text_unit_overrides.as_ref())
            .expect("splitText color animation should record text unit overrides");
        assert_eq!(batch.overrides.len(), 2);
        assert_eq!(
            batch.overrides[0].color,
            Some(crate::style::ColorToken::Custom(217, 70, 239, 255))
        );
        assert_eq!(
            batch.overrides[1].color,
            Some(crate::style::ColorToken::Custom(217, 70, 239, 255))
        );
    }

    #[test]
    fn split_text_node_uses_grapheme_clusters_for_zwj_emoji() {
        use super::node_style::{TextUnitGranularity, describe_text_units};
        let units = describe_text_units(
            "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}",
            TextUnitGranularity::Grapheme,
        );
        assert_eq!(units.len(), 1);
        assert_eq!(
            units[0].text,
            "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}"
        );
    }

    #[test]
    fn split_text_node_uses_grapheme_clusters_for_combining_marks() {
        use super::node_style::{TextUnitGranularity, describe_text_units};
        let units = describe_text_units("e\u{0301}", TextUnitGranularity::Grapheme);
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].text, "e\u{0301}");
    }

    #[test]
    fn split_text_node_describes_words_from_resolved_text() {
        use super::node_style::{TextUnitGranularity, describe_text_units};
        let units = describe_text_units("Hello world", TextUnitGranularity::Word);
        assert_eq!(
            units.iter().map(|u| u.text.as_str()).collect::<Vec<_>>(),
            vec!["Hello", " ", "world"]
        );
    }

    #[test]
    fn split_text_node_words_falls_back_to_graphemes_for_cjk() {
        use super::node_style::{TextUnitGranularity, describe_text_units};
        let units = describe_text_units("你好世界", TextUnitGranularity::Word);
        assert_eq!(
            units.iter().map(|u| u.text.as_str()).collect::<Vec<_>>(),
            vec!["你", "好", "世", "界"]
        );
    }

    #[test]
    fn script_driver_split_text_node_stagger_grapheme_opacity() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.getNode("t").text("Cat");
            var parts = ctx.splitText("t", { type: "chars" });
            ctx.fromTo(parts, { opacity: 0, y: 18 }, {
                opacity: 1,
                y: 0,
                duration: 6,
                stagger: 2,
                ease: "linear"
            });
        "#,
        )
        .expect("script should compile");

        let f0 = driver.run(0, 30, 0, 30, None).expect("frame 0");
        let t0 = f0.get("t").expect("t mutation");
        assert!(t0.text_unit_overrides.is_some());

        let f10 = driver.run(10, 30, 10, 30, None).expect("frame 10");
        let batch = f10
            .get("t")
            .expect("t mutation should exist")
            .text_unit_overrides
            .as_ref()
            .expect("text unit overrides should exist");
        assert_eq!(batch.overrides[0].opacity, Some(1.0));
    }

    #[test]
    fn script_driver_split_text_supports_function_based_values() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.getNode("t").text("Cat");
            var parts = ctx.splitText("t", { type: "chars" });
            ctx.fromTo(parts, {
                opacity: 0,
                x: function(i) { return i === 0 ? -30 : 30; },
            }, {
                opacity: 1,
                x: 0,
                duration: 10,
                stagger: 2,
                ease: "linear"
            });
        "#,
        )
        .expect("script should compile");

        let f0 = driver.run(0, 30, 0, 30, None).expect("frame 0");
        let batch = f0
            .get("t")
            .expect("t mutation should exist")
            .text_unit_overrides
            .as_ref()
            .expect("text unit overrides should exist");
        assert_eq!(batch.overrides[0].translate_x, Some(-30.0));
        assert_eq!(batch.overrides[1].translate_x, Some(30.0));
    }

    #[test]
    fn split_text_node_uses_post_text_content_value_as_source() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.getNode("t").text("Hello");
            var parts = ctx.splitText("t", { type: "chars" });
            if (parts.length !== 5) {
                throw new Error("expected 5 graphemes, got " + parts.length);
            }
            parts[0].set({ opacity: 0.2 });
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(0, 1, 0, 1, None).expect("script should run");
        let batch = mutations
            .get("t")
            .expect("t mutation should exist")
            .text_unit_overrides
            .as_ref()
            .expect("text unit overrides should exist");
        assert_eq!(batch.overrides[0].opacity, Some(0.2));
    }
}
