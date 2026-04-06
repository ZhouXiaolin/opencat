use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rquickjs::{Context, Function, Object, Persistent, Runtime};

use crate::style::{
    AlignItems, ColorToken, FlexDirection, FontWeight, JustifyContent, ObjectFit, Position,
    ShadowStyle, TextAlign, Transform, color_token_from_script_name,
};

#[derive(Debug, Clone, Default)]
pub struct NodeStyleMutations {
    pub position: Option<Position>,
    pub inset_left: Option<f32>,
    pub inset_top: Option<f32>,
    pub inset_right: Option<f32>,
    pub inset_bottom: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub padding: Option<f32>,
    pub padding_x: Option<f32>,
    pub padding_y: Option<f32>,
    pub margin: Option<f32>,
    pub margin_x: Option<f32>,
    pub margin_y: Option<f32>,
    pub flex_direction: Option<FlexDirection>,
    pub justify_content: Option<JustifyContent>,
    pub align_items: Option<AlignItems>,
    pub gap: Option<f32>,
    pub flex_grow: Option<f32>,
    pub opacity: Option<f32>,
    pub bg_color: Option<ColorToken>,
    pub border_radius: Option<f32>,
    pub border_width: Option<f32>,
    pub border_color: Option<ColorToken>,
    pub object_fit: Option<ObjectFit>,
    pub transforms: Vec<Transform>,
    pub text_color: Option<ColorToken>,
    pub text_px: Option<f32>,
    pub font_weight: Option<FontWeight>,
    pub letter_spacing: Option<f32>,
    pub text_align: Option<TextAlign>,
    pub line_height: Option<f32>,
    pub shadow: Option<ShadowStyle>,
}

impl NodeStyleMutations {
    pub fn apply_to(&self, style: &mut crate::style::NodeStyle) {
        if let Some(v) = self.position {
            style.position = Some(v);
        }
        if let Some(v) = self.inset_left {
            style.inset_left = Some(v);
        }
        if let Some(v) = self.inset_top {
            style.inset_top = Some(v);
        }
        if let Some(v) = self.inset_right {
            style.inset_right = Some(v);
        }
        if let Some(v) = self.inset_bottom {
            style.inset_bottom = Some(v);
        }
        if let Some(v) = self.width {
            style.width = Some(v);
            style.width_full = false;
        }
        if let Some(v) = self.height {
            style.height = Some(v);
            style.height_full = false;
        }
        if let Some(v) = self.padding {
            style.padding = Some(v);
        }
        if let Some(v) = self.padding_x {
            style.padding_x = Some(v);
        }
        if let Some(v) = self.padding_y {
            style.padding_y = Some(v);
        }
        if let Some(v) = self.margin {
            style.margin = Some(v);
        }
        if let Some(v) = self.margin_x {
            style.margin_x = Some(v);
        }
        if let Some(v) = self.margin_y {
            style.margin_y = Some(v);
        }
        if let Some(v) = self.flex_direction {
            style.flex_direction = Some(v);
        }
        if let Some(v) = self.justify_content {
            style.justify_content = Some(v);
        }
        if let Some(v) = self.align_items {
            style.align_items = Some(v);
        }
        if let Some(v) = self.gap {
            style.gap = Some(v);
        }
        if let Some(v) = self.flex_grow {
            style.flex_grow = Some(v);
        }
        if let Some(v) = self.opacity {
            style.opacity = Some(v.clamp(0.0, 1.0));
        }
        if let Some(v) = self.bg_color {
            style.bg_color = Some(v);
        }
        if let Some(v) = self.border_radius {
            style.border_radius = Some(v);
        }
        if let Some(v) = self.border_width {
            style.border_width = Some(v);
        }
        if let Some(v) = self.border_color {
            style.border_color = Some(v);
        }
        if let Some(v) = self.object_fit {
            style.object_fit = Some(v);
        }
        if !self.transforms.is_empty() {
            style.transforms.extend(self.transforms.iter().cloned());
        }
        if let Some(v) = self.text_color {
            style.text_color = Some(v);
        }
        if let Some(v) = self.text_px {
            style.text_px = Some(v);
        }
        if let Some(v) = self.font_weight {
            style.font_weight = Some(v);
        }
        if let Some(v) = self.letter_spacing {
            style.letter_spacing = Some(v);
        }
        if let Some(v) = self.text_align {
            style.text_align = Some(v);
        }
        if let Some(v) = self.line_height {
            style.line_height = Some(v);
        }
        if let Some(v) = self.shadow {
            style.shadow = Some(v);
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct StyleMutations {
    pub mutations: HashMap<String, NodeStyleMutations>,
}

impl StyleMutations {
    pub fn get(&self, id: &str) -> Option<&NodeStyleMutations> {
        self.mutations.get(id)
    }

    pub fn is_empty(&self) -> bool {
        self.mutations.is_empty()
    }

    pub fn apply_to_node(&self, node_style: &mut crate::style::NodeStyle, id: &str) {
        if let Some(mutation) = self.mutations.get(id) {
            mutation.apply_to(node_style);
        }
    }
}

type MutationStore = Arc<Mutex<HashMap<String, NodeStyleMutations>>>;

#[derive(Default)]
pub(crate) struct ScriptRuntimeCache {
    runners: HashMap<u64, ScriptRunner>,
}

fn color_from_name(name: &str) -> Option<ColorToken> {
    color_token_from_script_name(name)
}

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
        "normal" => Some(FontWeight::Normal),
        "medium" => Some(FontWeight::Medium),
        "semibold" => Some(FontWeight::SemiBold),
        "bold" => Some(FontWeight::Bold),
        _ => None,
    }
}

fn shadow_from_name(name: &str) -> Option<ShadowStyle> {
    match name {
        "sm" => Some(ShadowStyle::SM),
        "md" => Some(ShadowStyle::MD),
        "lg" => Some(ShadowStyle::LG),
        "xl" => Some(ShadowStyle::XL),
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

const PROXY_RUNTIME: &str = r#"
(function() {
    function applyMutation(id, prop, ...args) {
        switch (prop) {
            case 'opacity': __record_opacity(id, args[0]); break;
            case 'translateX': __record_translate_x(id, args[0]); break;
            case 'translateY': __record_translate_y(id, args[0]); break;
            case 'translate': __record_translate(id, args[0], args[1]); break;
            case 'scale': __record_scale(id, args[0]); break;
            case 'scaleX': __record_scale_x(id, args[0]); break;
            case 'scaleY': __record_scale_y(id, args[0]); break;
            case 'rotate': __record_rotate(id, args[0]); break;
            case 'skewX': __record_skew_x(id, args[0]); break;
            case 'skewY': __record_skew_y(id, args[0]); break;
            case 'skew': __record_skew(id, args[0], args[1]); break;
            case 'position': __record_position(id, String(args[0])); break;
            case 'left': __record_left(id, args[0]); break;
            case 'top': __record_top(id, args[0]); break;
            case 'right': __record_right(id, args[0]); break;
            case 'bottom': __record_bottom(id, args[0]); break;
            case 'width': __record_width(id, args[0]); break;
            case 'height': __record_height(id, args[0]); break;
            case 'padding': __record_padding(id, args[0]); break;
            case 'paddingX': __record_padding_x(id, args[0]); break;
            case 'paddingY': __record_padding_y(id, args[0]); break;
            case 'margin': __record_margin(id, args[0]); break;
            case 'marginX': __record_margin_x(id, args[0]); break;
            case 'marginY': __record_margin_y(id, args[0]); break;
            case 'flexDirection': __record_flex_direction(id, String(args[0])); break;
            case 'justifyContent': __record_justify_content(id, String(args[0])); break;
            case 'alignItems': __record_align_items(id, String(args[0])); break;
            case 'gap': __record_gap(id, args[0]); break;
            case 'flexGrow': __record_flex_grow(id, args[0]); break;
            case 'bg': __record_bg(id, String(args[0])); break;
            case 'borderRadius': __record_border_radius(id, args[0]); break;
            case 'borderWidth': __record_border_width(id, args[0]); break;
            case 'borderColor': __record_border_color(id, String(args[0])); break;
            case 'strokeWidth': __record_stroke_width(id, args[0]); break;
            case 'strokeColor': __record_stroke_color(id, String(args[0])); break;
            case 'fillColor': __record_fill_color(id, String(args[0])); break;
            case 'objectFit': __record_object_fit(id, String(args[0])); break;
            case 'textColor': __record_text_color(id, String(args[0])); break;
            case 'textSize': __record_text_size(id, args[0]); break;
            case 'fontWeight': __record_font_weight(id, String(args[0])); break;
            case 'letterSpacing': __record_letter_spacing(id, args[0]); break;
            case 'textAlign': __record_text_align(id, String(args[0])); break;
            case 'lineHeight': __record_line_height(id, args[0]); break;
            case 'shadow': __record_shadow(id, String(args[0])); break;
        }
    }

    const nodeCache = {};

    ctx.getNode = function(id) {
        if (!nodeCache[id]) {
            let api = null;
            api = new Proxy({}, {
                get(target, prop) {
                    if (typeof prop !== 'string' || prop === 'then') {
                        return undefined;
                    }

                    return (...args) => {
                        applyMutation(id, prop, ...args);
                        return api;
                    };
                }
            });
            nodeCache[id] = api;
        }
        return nodeCache[id];
    };
})();
"#;

const RUN_FRAME_FN: &str = "__opencatRunFrame";

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

    pub fn run(&self, frame: u32, total_frames: u32) -> anyhow::Result<StyleMutations> {
        let mut runner = self.create_runner()?;
        runner.run(frame, total_frames)
    }
}

impl ScriptRuntimeCache {
    pub(crate) fn run(
        &mut self,
        driver: &ScriptDriver,
        frame: u32,
        total_frames: u32,
    ) -> anyhow::Result<StyleMutations> {
        let key = driver.cache_key();
        let runner = match self.runners.entry(key) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(driver.create_runner()?)
            }
        };
        runner.run(frame, total_frames)
    }
}

impl ScriptRunner {
    fn new(source: &str) -> anyhow::Result<Self> {
        let runtime = Runtime::new()?;
        let context = Context::full(&runtime)?;
        let store: MutationStore = Arc::new(Mutex::new(HashMap::new()));

        let (ctx_obj, run_fn) = context.with(|ctx| {
            let globals = ctx.globals();
            let ctx_obj = install_runtime_bindings(&ctx, &store)?;
            let wrapped = format!("globalThis.{RUN_FRAME_FN} = function() {{\n{source}\n}};");
            ctx.eval::<(), _>(wrapped.as_str())?;
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

    pub(crate) fn run(&mut self, frame: u32, total_frames: u32) -> anyhow::Result<StyleMutations> {
        self.store.lock().unwrap().clear();

        self.context.with(|ctx| {
            let ctx_obj = self.ctx_obj.clone().restore(&ctx)?;
            ctx_obj.set("frame", frame)?;
            ctx_obj.set("totalFrames", total_frames)?;

            let run_fn = self.run_fn.clone().restore(&ctx)?;
            run_fn.call::<(), ()>(())?;
            Ok::<_, anyhow::Error>(())
        })?;

        let mutations = self.store.lock().unwrap();
        Ok(StyleMutations {
            mutations: mutations.clone(),
        })
    }
}

fn install_runtime_bindings<'js>(
    ctx: &rquickjs::Ctx<'js>,
    store: &MutationStore,
) -> anyhow::Result<Object<'js>> {
    let globals = ctx.globals();

    let s = store.clone();
    globals.set(
        "__record_opacity",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().opacity = Some(v);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_translate_x",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id)
                .or_default()
                .transforms
                .push(Transform::TranslateX(v));
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_translate_y",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id)
                .or_default()
                .transforms
                .push(Transform::TranslateY(v));
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_translate",
        Function::new(ctx.clone(), move |id: String, x: f32, y: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id)
                .or_default()
                .transforms
                .push(Transform::Translate(x, y));
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_scale",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id)
                .or_default()
                .transforms
                .push(Transform::Scale(v));
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_scale_x",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id)
                .or_default()
                .transforms
                .push(Transform::ScaleX(v));
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_scale_y",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id)
                .or_default()
                .transforms
                .push(Transform::ScaleY(v));
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_rotate",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id)
                .or_default()
                .transforms
                .push(Transform::RotateDeg(v));
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_skew_x",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id)
                .or_default()
                .transforms
                .push(Transform::SkewXDeg(v));
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_skew_y",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id)
                .or_default()
                .transforms
                .push(Transform::SkewYDeg(v));
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_skew",
        Function::new(ctx.clone(), move |id: String, x_deg: f32, y_deg: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id)
                .or_default()
                .transforms
                .push(Transform::SkewDeg(x_deg, y_deg));
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_position",
        Function::new(ctx.clone(), move |id: String, v: String| {
            if let Some(pos) = position_from_name(&v) {
                let mut map = s.lock().unwrap();
                map.entry(id).or_default().position = Some(pos);
            }
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_left",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().inset_left = Some(v);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_top",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().inset_top = Some(v);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_right",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().inset_right = Some(v);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_bottom",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().inset_bottom = Some(v);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_width",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().width = Some(v);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_height",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().height = Some(v);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_padding",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().padding = Some(v);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_padding_x",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().padding_x = Some(v);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_padding_y",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().padding_y = Some(v);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_margin",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().margin = Some(v);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_margin_x",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().margin_x = Some(v);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_margin_y",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().margin_y = Some(v);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_flex_direction",
        Function::new(ctx.clone(), move |id: String, v: String| {
            if let Some(fd) = flex_direction_from_name(&v) {
                let mut map = s.lock().unwrap();
                map.entry(id).or_default().flex_direction = Some(fd);
            }
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_justify_content",
        Function::new(ctx.clone(), move |id: String, v: String| {
            if let Some(jc) = justify_content_from_name(&v) {
                let mut map = s.lock().unwrap();
                map.entry(id).or_default().justify_content = Some(jc);
            }
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_align_items",
        Function::new(ctx.clone(), move |id: String, v: String| {
            if let Some(ai) = align_items_from_name(&v) {
                let mut map = s.lock().unwrap();
                map.entry(id).or_default().align_items = Some(ai);
            }
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_gap",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().gap = Some(v);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_flex_grow",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().flex_grow = Some(v);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_bg",
        Function::new(ctx.clone(), move |id: String, v: String| {
            if let Some(c) = color_from_name(&v) {
                let mut map = s.lock().unwrap();
                map.entry(id).or_default().bg_color = Some(c);
            }
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_border_radius",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().border_radius = Some(v);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_border_width",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().border_width = Some(v);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_border_color",
        Function::new(ctx.clone(), move |id: String, v: String| {
            if let Some(c) = color_from_name(&v) {
                let mut map = s.lock().unwrap();
                map.entry(id).or_default().border_color = Some(c);
            }
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_stroke_width",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().border_width = Some(v.max(0.0));
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_stroke_color",
        Function::new(ctx.clone(), move |id: String, v: String| {
            if let Some(c) = color_from_name(&v) {
                let mut map = s.lock().unwrap();
                map.entry(id).or_default().border_color = Some(c);
            }
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_fill_color",
        Function::new(ctx.clone(), move |id: String, v: String| {
            if let Some(c) = color_from_name(&v) {
                let mut map = s.lock().unwrap();
                map.entry(id).or_default().bg_color = Some(c);
            }
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_object_fit",
        Function::new(ctx.clone(), move |id: String, v: String| {
            if let Some(of) = object_fit_from_name(&v) {
                let mut map = s.lock().unwrap();
                map.entry(id).or_default().object_fit = Some(of);
            }
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_text_color",
        Function::new(ctx.clone(), move |id: String, v: String| {
            if let Some(c) = color_from_name(&v) {
                let mut map = s.lock().unwrap();
                map.entry(id).or_default().text_color = Some(c);
            }
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_text_size",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().text_px = Some(v);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_font_weight",
        Function::new(ctx.clone(), move |id: String, v: String| {
            if let Some(fw) = font_weight_from_name(&v) {
                let mut map = s.lock().unwrap();
                map.entry(id).or_default().font_weight = Some(fw);
            }
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_letter_spacing",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().letter_spacing = Some(v);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_text_align",
        Function::new(ctx.clone(), move |id: String, v: String| {
            if let Some(align) = text_align_from_name(&v) {
                let mut map = s.lock().unwrap();
                map.entry(id).or_default().text_align = Some(align);
            }
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_line_height",
        Function::new(ctx.clone(), move |id: String, v: f32| {
            let mut map = s.lock().unwrap();
            map.entry(id).or_default().line_height = Some(v);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__record_shadow",
        Function::new(ctx.clone(), move |id: String, v: String| {
            if let Some(sh) = shadow_from_name(&v) {
                let mut map = s.lock().unwrap();
                map.entry(id).or_default().shadow = Some(sh);
            }
        })?,
    )?;

    let ctx_obj = Object::new(ctx.clone())?;
    ctx_obj.set("frame", 0)?;
    ctx_obj.set("totalFrames", 0)?;
    globals.set("ctx", ctx_obj.clone())?;

    ctx.eval::<(), _>(PROXY_RUNTIME)?;
    Ok(ctx_obj)
}

#[cfg(test)]
mod tests {
    use super::ScriptDriver;
    use crate::style::{ColorToken, TextAlign, Transform};

    #[test]
    fn script_driver_records_text_alignment_and_line_height() {
        let driver = ScriptDriver::from_source(
            r#"
            const title = ctx.getNode("title");
            title.textAlign("center").lineHeight(1.8);
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(0, 1).expect("script should run");
        let title = mutations.get("title").expect("title mutation should exist");

        assert_eq!(title.text_align, Some(TextAlign::Center));
        assert_eq!(title.line_height, Some(1.8));
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

        let mutations = driver.run(0, 1).expect("script should run");
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

        let mutations = driver.run(0, 1).expect("script should run");
        let icon = mutations.get("icon").expect("icon mutation should exist");

        assert_eq!(icon.border_color, Some(ColorToken::Blue));
        assert_eq!(icon.border_width, Some(3.0));
        assert_eq!(icon.bg_color, Some(ColorToken::Sky200));
    }
}
