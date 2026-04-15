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
mod node_style;

pub use canvas_api::{CanvasCommand, CanvasMutations, ScriptColor, ScriptLineCap, ScriptLineJoin};
pub use node_style::NodeStyleMutations;

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
}

#[derive(Default)]
struct RuntimeMutationStore {
    styles: HashMap<String, NodeStyleMutations>,
    canvases: HashMap<String, CanvasMutations>,
    current_frame: u32,
    animate_state: std::sync::Mutex<animate_api::AnimateState>,
}

type MutationStore = Arc<Mutex<RuntimeMutationStore>>;

#[derive(Default)]
pub(crate) struct ScriptRuntimeCache {
    runners: HashMap<u64, ScriptRunner>,
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

fn font_weight_from_name(name: &str) -> Option<FontWeight> {
    match name {
        "light" => Some(FontWeight::Light),
        "normal" => Some(FontWeight::Normal),
        "medium" => Some(FontWeight::Medium),
        "semibold" => Some(FontWeight::SemiBold),
        "bold" => Some(FontWeight::Bold),
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
            *store = RuntimeMutationStore {
                current_frame: frame_ctx.current_frame,
                ..Default::default()
            };
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
    ctx.eval::<(), _>(node_style::NODE_STYLE_RUNTIME)?;
    ctx.eval::<(), _>(canvas_api::CANVASKIT_RUNTIME)?;
    ctx.eval::<(), _>(animate_api::ANIMATE_RUNTIME)?;

    Ok(ctx_obj)
}

#[cfg(test)]
mod tests {
    use super::{CanvasCommand, ScriptColor, ScriptDriver, ScriptLineCap, ScriptLineJoin};
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

        assert_eq!(icon.border_color, Some(ColorToken::Blue));
        assert_eq!(icon.border_width, Some(3.0));
        assert_eq!(icon.bg_color, Some(ColorToken::Sky200));
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
            canvas.commands[1],
            CanvasCommand::DrawImage {
                asset_id: "hero".to_string(),
                x: 10.0,
                y: 10.0,
                width: 80.0,
                height: 60.0,
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
            canvas.commands[1],
            CanvasCommand::SetLineWidth { width: 3.0 }
        );
        assert_eq!(
            canvas.commands[2],
            CanvasCommand::SetLineCap {
                cap: ScriptLineCap::Round,
            }
        );
        assert_eq!(
            canvas.commands[3],
            CanvasCommand::SetLineJoin {
                join: ScriptLineJoin::Bevel,
            }
        );
        assert!(matches!(canvas.commands[4], CanvasCommand::ClearLineDash));
        assert!(matches!(canvas.commands[5], CanvasCommand::BeginPath));
        assert!(matches!(canvas.commands[10], CanvasCommand::ClosePath));
        assert!(matches!(canvas.commands[11], CanvasCommand::StrokePath));
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
            canvas.commands[5],
            CanvasCommand::FillRRect { .. }
        ));
        assert_eq!(
            canvas.commands[6],
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
            canvas.commands[7],
            CanvasCommand::SetLineWidth { width: 3.0 }
        );
        assert!(matches!(canvas.commands[10], CanvasCommand::ClearLineDash));
        assert!(matches!(
            canvas.commands[11],
            CanvasCommand::StrokeCircle { .. }
        ));
        assert_eq!(canvas.commands[12], CanvasCommand::Restore);
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
            canvas.commands[1],
            CanvasCommand::SetLineWidth { width: 3.0 }
        );
        assert!(matches!(
            canvas.commands[4],
            CanvasCommand::SetLineDash { .. }
        ));
        assert!(matches!(canvas.commands[5], CanvasCommand::DrawLine { .. }));
    }

    #[test]
    fn script_driver_animate_linear_opacity() {
        let driver = ScriptDriver::from_source(
            r#"
            const s = ctx.animate({
                from: { opacity: 0 },
                to: { opacity: 1 },
                duration: 20,
                easing: 'linear',
            });
            ctx.getNode("box").opacity(s.opacity);
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(10, 20, 10, 20, None).expect("script should run");
        let node = mutations.get("box").expect("box mutation should exist");

        assert!((node.opacity.unwrap() - 0.5).abs() < 0.01);
    }

    #[test]
    fn script_driver_animate_ease_out_translate() {
        let driver = ScriptDriver::from_source(
            r#"
            const s = ctx.animate({
                from: { translateX: 0 },
                to: { translateX: 100 },
                duration: 20,
                easing: 'ease-out',
            });
            ctx.getNode("box").translateX(s.translateX);
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
            const s = ctx.animate({
                from: { opacity: 0 },
                to: { opacity: 1 },
                easing: 'spring-stiff',
            });
            ctx.getNode("box").opacity(s.opacity);
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
            const s = ctx.animate({
                from: { opacity: 0 },
                to: { opacity: 1 },
                duration: 20,
                delay: 5,
                easing: 'linear',
            });
            ctx.getNode("box").opacity(s.opacity);
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
            const anims = ctx.stagger(3, {
                from: { opacity: 0 },
                to: { opacity: 1 },
                duration: 10,
                gap: 5,
                easing: 'linear',
            });
            ctx.getNode("a").opacity(anims[0].opacity);
            ctx.getNode("b").opacity(anims[1].opacity);
            ctx.getNode("c").opacity(anims[2].opacity);
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
    fn script_driver_animate_custom_bezier() {
        let driver = ScriptDriver::from_source(
            r#"
            const s = ctx.animate({
                from: { scale: 0.5 },
                to: { scale: 1.0 },
                duration: 20,
                easing: [0.68, -0.6, 0.32, 1.6],
            });
            ctx.getNode("box").scale(s.scale);
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
}
