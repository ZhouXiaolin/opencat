use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rquickjs::{Context, Function, Runtime};

use crate::style::{
    AlignItems, ColorToken, FlexDirection, FontWeight, JustifyContent, ObjectFit, Position,
    ShadowStyle, Transform,
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
        }
        if let Some(v) = self.height {
            style.height = Some(v);
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

    pub fn apply_to_node(
        &self,
        node_style: &mut crate::style::NodeStyle,
        data_id: &Option<String>,
    ) {
        if let Some(id) = data_id {
            if let Some(mutation) = self.mutations.get(id) {
                mutation.apply_to(node_style);
            }
        }
    }
}

type MutationStore = Arc<Mutex<HashMap<String, NodeStyleMutations>>>;

fn color_from_name(name: &str) -> Option<ColorToken> {
    match name {
        "white" => Some(ColorToken::White),
        "black" => Some(ColorToken::Black),
        "red" => Some(ColorToken::Red),
        "green" => Some(ColorToken::Green),
        "blue" => Some(ColorToken::Blue),
        "yellow" => Some(ColorToken::Yellow),
        "orange" => Some(ColorToken::Orange),
        "purple" => Some(ColorToken::Purple),
        "pink" => Some(ColorToken::Pink),
        "gray" => Some(ColorToken::Gray),
        "slate50" => Some(ColorToken::Slate50),
        "slate200" => Some(ColorToken::Slate200),
        "slate300" => Some(ColorToken::Slate300),
        "slate400" => Some(ColorToken::Slate400),
        "slate500" => Some(ColorToken::Slate500),
        "slate600" => Some(ColorToken::Slate600),
        "slate700" => Some(ColorToken::Slate700),
        "slate900" => Some(ColorToken::Slate900),
        "primary" => Some(ColorToken::Primary),
        _ => None,
    }
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

pub struct ScriptDriver {
    source: String,
}

const PROXY_RUNTIME: &str = r#"
(function() {
    function applyMutation(id, prop, value) {
        switch (prop) {
            case 'opacity': __record_opacity(id, value); break;
            case 'translateX': __record_translate_x(id, value); break;
            case 'translateY': __record_translate_y(id, value); break;
            case 'scale': __record_scale(id, value); break;
            case 'scaleX': __record_scale_x(id, value); break;
            case 'scaleY': __record_scale_y(id, value); break;
            case 'rotate': __record_rotate(id, value); break;
            case 'skewX': __record_skew_x(id, value); break;
            case 'skewY': __record_skew_y(id, value); break;
            case 'position': __record_position(id, String(value)); break;
            case 'left': __record_left(id, value); break;
            case 'top': __record_top(id, value); break;
            case 'right': __record_right(id, value); break;
            case 'bottom': __record_bottom(id, value); break;
            case 'width': __record_width(id, value); break;
            case 'height': __record_height(id, value); break;
            case 'padding': __record_padding(id, value); break;
            case 'paddingX': __record_padding_x(id, value); break;
            case 'paddingY': __record_padding_y(id, value); break;
            case 'margin': __record_margin(id, value); break;
            case 'marginX': __record_margin_x(id, value); break;
            case 'marginY': __record_margin_y(id, value); break;
            case 'flexDirection': __record_flex_direction(id, String(value)); break;
            case 'justifyContent': __record_justify_content(id, String(value)); break;
            case 'alignItems': __record_align_items(id, String(value)); break;
            case 'gap': __record_gap(id, value); break;
            case 'flexGrow': __record_flex_grow(id, value); break;
            case 'bg': __record_bg(id, String(value)); break;
            case 'borderRadius': __record_border_radius(id, value); break;
            case 'borderWidth': __record_border_width(id, value); break;
            case 'borderColor': __record_border_color(id, String(value)); break;
            case 'objectFit': __record_object_fit(id, String(value)); break;
            case 'textColor': __record_text_color(id, String(value)); break;
            case 'textSize': __record_text_size(id, value); break;
            case 'fontWeight': __record_font_weight(id, String(value)); break;
            case 'letterSpacing': __record_letter_spacing(id, value); break;
            case 'shadow': __record_shadow(id, String(value)); break;
        }
    }

    const nodeCache = {};

    ctx.getNode = function(id) {
        if (!nodeCache[id]) {
            nodeCache[id] = new Proxy({}, {
                set(target, prop, value) {
                    applyMutation(id, prop, value);
                    return true;
                }
            });
        }
        return nodeCache[id];
    };
})();
"#;

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

    pub fn run(&self, frame: u32, total_frames: u32) -> anyhow::Result<StyleMutations> {
        let runtime = Runtime::new()?;
        let context = Context::full(&runtime)?;
        let store: MutationStore = Arc::new(Mutex::new(HashMap::new()));

        context.with(|ctx| {
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
                "__record_shadow",
                Function::new(ctx.clone(), move |id: String, v: String| {
                    if let Some(sh) = shadow_from_name(&v) {
                        let mut map = s.lock().unwrap();
                        map.entry(id).or_default().shadow = Some(sh);
                    }
                })?,
            )?;

            let ctx_obj = rquickjs::Object::new(ctx.clone())?;
            ctx_obj.set("frame", frame)?;
            ctx_obj.set("totalFrames", total_frames)?;
            globals.set("ctx", ctx_obj)?;

            ctx.eval::<(), _>(PROXY_RUNTIME)?;
            ctx.eval::<(), _>(self.source.as_str())?;

            Ok::<_, anyhow::Error>(())
        })?;

        let mutations = store.lock().unwrap();
        Ok(StyleMutations {
            mutations: mutations.clone(),
        })
    }
}
