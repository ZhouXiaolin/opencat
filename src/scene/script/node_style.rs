use rquickjs::Function;

use crate::style::{ColorToken, LengthPercentageAuto, Transform, color_token_from_script_name};

use super::{
    MutationStore, align_items_from_name, box_shadow_from_name, drop_shadow_from_name,
    flex_direction_from_name, font_weight_from_name, inset_shadow_from_name,
    justify_content_from_name, object_fit_from_name, position_from_name, text_align_from_name,
};

#[derive(Debug, Clone, Default)]
pub struct NodeStyleMutations {
    pub position: Option<crate::style::Position>,
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
    pub flex_direction: Option<crate::style::FlexDirection>,
    pub justify_content: Option<crate::style::JustifyContent>,
    pub align_items: Option<crate::style::AlignItems>,
    pub gap: Option<f32>,
    pub flex_grow: Option<f32>,
    pub opacity: Option<f32>,
    pub bg_color: Option<ColorToken>,
    pub border_radius: Option<f32>,
    pub border_width: Option<f32>,
    pub border_top_width: Option<f32>,
    pub border_right_width: Option<f32>,
    pub border_bottom_width: Option<f32>,
    pub border_left_width: Option<f32>,
    pub border_color: Option<ColorToken>,
    pub border_style: Option<crate::style::BorderStyle>,
    pub object_fit: Option<crate::style::ObjectFit>,
    pub transforms: Vec<Transform>,
    pub text_color: Option<ColorToken>,
    pub text_px: Option<f32>,
    pub font_weight: Option<crate::style::FontWeight>,
    pub letter_spacing: Option<f32>,
    pub text_align: Option<crate::style::TextAlign>,
    pub line_height: Option<f32>,
    pub box_shadow: Option<crate::style::BoxShadow>,
    pub box_shadow_color: Option<ColorToken>,
    pub inset_shadow: Option<crate::style::InsetShadow>,
    pub inset_shadow_color: Option<ColorToken>,
    pub drop_shadow: Option<crate::style::DropShadow>,
    pub drop_shadow_color: Option<ColorToken>,
    pub text_content: Option<String>,
}

impl NodeStyleMutations {
    pub fn apply_to(&self, style: &mut crate::style::NodeStyle) {
        if let Some(v) = self.position {
            style.position = Some(v);
        }
        if let Some(v) = self.inset_left {
            style.inset_left = Some(crate::style::LengthPercentageAuto::length(v));
        }
        if let Some(v) = self.inset_top {
            style.inset_top = Some(crate::style::LengthPercentageAuto::length(v));
        }
        if let Some(v) = self.inset_right {
            style.inset_right = Some(crate::style::LengthPercentageAuto::length(v));
        }
        if let Some(v) = self.inset_bottom {
            style.inset_bottom = Some(crate::style::LengthPercentageAuto::length(v));
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
            style.margin = Some(LengthPercentageAuto::Length(v));
        }
        if let Some(v) = self.margin_x {
            style.margin_x = Some(LengthPercentageAuto::Length(v));
        }
        if let Some(v) = self.margin_y {
            style.margin_y = Some(LengthPercentageAuto::Length(v));
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
            style.border_radius = Some(crate::style::BorderRadius::uniform(v));
        }
        if let Some(v) = self.border_width {
            style.border_width = Some(v);
        }
        if let Some(v) = self.border_top_width {
            style.border_top_width = Some(v);
        }
        if let Some(v) = self.border_right_width {
            style.border_right_width = Some(v);
        }
        if let Some(v) = self.border_bottom_width {
            style.border_bottom_width = Some(v);
        }
        if let Some(v) = self.border_left_width {
            style.border_left_width = Some(v);
        }
        if let Some(v) = self.border_color {
            style.border_color = Some(v);
        }
        if let Some(v) = self.border_style {
            style.border_style = Some(v);
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
        if let Some(v) = self.box_shadow {
            style.box_shadow = Some(v);
        }
        if let Some(v) = self.box_shadow_color {
            style.box_shadow_color = Some(v);
        }
        if let Some(v) = self.inset_shadow {
            style.inset_shadow = Some(v);
        }
        if let Some(v) = self.inset_shadow_color {
            style.inset_shadow_color = Some(v);
        }
        if let Some(v) = self.drop_shadow {
            style.drop_shadow = Some(v);
        }
        if let Some(v) = self.drop_shadow_color {
            style.drop_shadow_color = Some(v);
        }
    }
}

fn color_from_name(name: &str) -> Option<ColorToken> {
    if let Some(c) = color_token_from_script_name(name) {
        return Some(c);
    }
    let hsla = super::animate_api::parse_color(name)?;
    let (r, g, b) = super::animate_api::hsl_to_rgb(hsla.h, hsla.s, hsla.l);
    let a = (hsla.a.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some(ColorToken::Custom(r, g, b, a))
}

pub(super) const NODE_STYLE_RUNTIME: &str = include_str!("runtime/node_style.js");

pub(super) fn install_node_style_bindings<'js>(
    ctx: &rquickjs::Ctx<'js>,
    store: &MutationStore,
) -> anyhow::Result<()> {
    let globals = ctx.globals();

    macro_rules! set_style_binding {
        ($name:literal, $map:ident, |$id:ident $(, $arg:ident : $arg_ty:ty)*| $body:block) => {{
            let s = store.clone();
            globals.set(
                $name,
                Function::new(ctx.clone(), move |$id: String $(, $arg: $arg_ty)*| {
                    let mut guard = s.lock().unwrap();
                    let $map = &mut guard.styles;
                    $body
                })?,
            )?;
        }};
    }

    set_style_binding!("__record_opacity", map, |id, v: f32| {
        map.entry(id).or_default().opacity = Some(v);
    });
    set_style_binding!("__record_translate_x", map, |id, v: f32| {
        map.entry(id)
            .or_default()
            .transforms
            .push(Transform::TranslateX(v));
    });
    set_style_binding!("__record_translate_y", map, |id, v: f32| {
        map.entry(id)
            .or_default()
            .transforms
            .push(Transform::TranslateY(v));
    });
    set_style_binding!("__record_translate", map, |id, x: f32, y: f32| {
        map.entry(id)
            .or_default()
            .transforms
            .push(Transform::Translate(x, y));
    });
    set_style_binding!("__record_scale", map, |id, v: f32| {
        map.entry(id)
            .or_default()
            .transforms
            .push(Transform::Scale(v));
    });
    set_style_binding!("__record_scale_x", map, |id, v: f32| {
        map.entry(id)
            .or_default()
            .transforms
            .push(Transform::ScaleX(v));
    });
    set_style_binding!("__record_scale_y", map, |id, v: f32| {
        map.entry(id)
            .or_default()
            .transforms
            .push(Transform::ScaleY(v));
    });
    set_style_binding!("__record_rotate", map, |id, v: f32| {
        map.entry(id)
            .or_default()
            .transforms
            .push(Transform::RotateDeg(v));
    });
    set_style_binding!("__record_skew_x", map, |id, v: f32| {
        map.entry(id)
            .or_default()
            .transforms
            .push(Transform::SkewXDeg(v));
    });
    set_style_binding!("__record_skew_y", map, |id, v: f32| {
        map.entry(id)
            .or_default()
            .transforms
            .push(Transform::SkewYDeg(v));
    });
    set_style_binding!("__record_skew", map, |id, x_deg: f32, y_deg: f32| {
        map.entry(id)
            .or_default()
            .transforms
            .push(Transform::SkewDeg(x_deg, y_deg));
    });
    set_style_binding!("__record_position", map, |id, v: String| {
        if let Some(pos) = position_from_name(&v) {
            map.entry(id).or_default().position = Some(pos);
        }
    });
    set_style_binding!("__record_left", map, |id, v: f32| {
        map.entry(id).or_default().inset_left = Some(v);
    });
    set_style_binding!("__record_top", map, |id, v: f32| {
        map.entry(id).or_default().inset_top = Some(v);
    });
    set_style_binding!("__record_right", map, |id, v: f32| {
        map.entry(id).or_default().inset_right = Some(v);
    });
    set_style_binding!("__record_bottom", map, |id, v: f32| {
        map.entry(id).or_default().inset_bottom = Some(v);
    });
    set_style_binding!("__record_width", map, |id, v: f32| {
        map.entry(id).or_default().width = Some(v);
    });
    set_style_binding!("__record_height", map, |id, v: f32| {
        map.entry(id).or_default().height = Some(v);
    });
    set_style_binding!("__record_padding", map, |id, v: f32| {
        map.entry(id).or_default().padding = Some(v);
    });
    set_style_binding!("__record_padding_x", map, |id, v: f32| {
        map.entry(id).or_default().padding_x = Some(v);
    });
    set_style_binding!("__record_padding_y", map, |id, v: f32| {
        map.entry(id).or_default().padding_y = Some(v);
    });
    set_style_binding!("__record_margin", map, |id, v: f32| {
        map.entry(id).or_default().margin = Some(v);
    });
    set_style_binding!("__record_margin_x", map, |id, v: f32| {
        map.entry(id).or_default().margin_x = Some(v);
    });
    set_style_binding!("__record_margin_y", map, |id, v: f32| {
        map.entry(id).or_default().margin_y = Some(v);
    });
    set_style_binding!("__record_flex_direction", map, |id, v: String| {
        if let Some(fd) = flex_direction_from_name(&v) {
            map.entry(id).or_default().flex_direction = Some(fd);
        }
    });
    set_style_binding!("__record_justify_content", map, |id, v: String| {
        if let Some(jc) = justify_content_from_name(&v) {
            map.entry(id).or_default().justify_content = Some(jc);
        }
    });
    set_style_binding!("__record_align_items", map, |id, v: String| {
        if let Some(ai) = align_items_from_name(&v) {
            map.entry(id).or_default().align_items = Some(ai);
        }
    });
    set_style_binding!("__record_gap", map, |id, v: f32| {
        map.entry(id).or_default().gap = Some(v);
    });
    set_style_binding!("__record_flex_grow", map, |id, v: f32| {
        map.entry(id).or_default().flex_grow = Some(v);
    });
    set_style_binding!("__record_bg", map, |id, v: String| {
        if let Some(c) = color_from_name(&v) {
            map.entry(id).or_default().bg_color = Some(c);
        }
    });
    set_style_binding!("__record_border_radius", map, |id, v: f32| {
        map.entry(id).or_default().border_radius = Some(v);
    });
    set_style_binding!("__record_border_width", map, |id, v: f32| {
        map.entry(id).or_default().border_width = Some(v);
    });
    set_style_binding!("__record_border_top_width", map, |id, v: f32| {
        map.entry(id).or_default().border_top_width = Some(v);
    });
    set_style_binding!("__record_border_right_width", map, |id, v: f32| {
        map.entry(id).or_default().border_right_width = Some(v);
    });
    set_style_binding!("__record_border_bottom_width", map, |id, v: f32| {
        map.entry(id).or_default().border_bottom_width = Some(v);
    });
    set_style_binding!("__record_border_left_width", map, |id, v: f32| {
        map.entry(id).or_default().border_left_width = Some(v);
    });
    set_style_binding!("__record_border_style", map, |id, v: String| {
        let parsed = match v.as_str() {
            "solid" => Some(crate::style::BorderStyle::Solid),
            "dashed" => Some(crate::style::BorderStyle::Dashed),
            "dotted" => Some(crate::style::BorderStyle::Dotted),
            _ => None,
        };
        if let Some(bs) = parsed {
            map.entry(id).or_default().border_style = Some(bs);
        }
    });
    set_style_binding!("__record_border_color", map, |id, v: String| {
        if let Some(c) = color_from_name(&v) {
            map.entry(id).or_default().border_color = Some(c);
        }
    });
    set_style_binding!("__record_stroke_width", map, |id, v: f32| {
        map.entry(id).or_default().border_width = Some(v.max(0.0));
    });
    set_style_binding!("__record_stroke_color", map, |id, v: String| {
        if let Some(c) = color_from_name(&v) {
            map.entry(id).or_default().border_color = Some(c);
        }
    });
    set_style_binding!("__record_fill_color", map, |id, v: String| {
        if let Some(c) = color_from_name(&v) {
            map.entry(id).or_default().bg_color = Some(c);
        }
    });
    set_style_binding!("__record_object_fit", map, |id, v: String| {
        if let Some(of) = object_fit_from_name(&v) {
            map.entry(id).or_default().object_fit = Some(of);
        }
    });
    set_style_binding!("__record_text_color", map, |id, v: String| {
        if let Some(c) = color_from_name(&v) {
            map.entry(id).or_default().text_color = Some(c);
        }
    });
    set_style_binding!("__record_text_size", map, |id, v: f32| {
        map.entry(id).or_default().text_px = Some(v);
    });
    set_style_binding!("__record_font_weight", map, |id, v: String| {
        if let Some(fw) = font_weight_from_name(&v) {
            map.entry(id).or_default().font_weight = Some(fw);
        }
    });
    set_style_binding!("__record_letter_spacing", map, |id, v: f32| {
        map.entry(id).or_default().letter_spacing = Some(v);
    });
    set_style_binding!("__record_text_align", map, |id, v: String| {
        if let Some(align) = text_align_from_name(&v) {
            map.entry(id).or_default().text_align = Some(align);
        }
    });
    set_style_binding!("__record_line_height", map, |id, v: f32| {
        map.entry(id).or_default().line_height = Some(v);
    });
    set_style_binding!("__record_shadow", map, |id, v: String| {
        if let Some(sh) = box_shadow_from_name(&v) {
            map.entry(id).or_default().box_shadow = Some(sh);
        }
    });
    set_style_binding!("__record_shadow_color", map, |id, v: String| {
        if let Some(color) = color_from_name(&v) {
            map.entry(id).or_default().box_shadow_color = Some(color);
        }
    });
    set_style_binding!("__record_inset_shadow", map, |id, v: String| {
        if let Some(sh) = inset_shadow_from_name(&v) {
            map.entry(id).or_default().inset_shadow = Some(sh);
        }
    });
    set_style_binding!("__record_inset_shadow_color", map, |id, v: String| {
        if let Some(color) = color_from_name(&v) {
            map.entry(id).or_default().inset_shadow_color = Some(color);
        }
    });
    set_style_binding!("__record_drop_shadow", map, |id, v: String| {
        if let Some(sh) = drop_shadow_from_name(&v) {
            map.entry(id).or_default().drop_shadow = Some(sh);
        }
    });
    set_style_binding!("__record_drop_shadow_color", map, |id, v: String| {
        if let Some(color) = color_from_name(&v) {
            map.entry(id).or_default().drop_shadow_color = Some(color);
        }
    });
    // Text content binding also populates the script-visible text source registry
    // so that __text_source_get() can query it within the same frame.
    {
        let s = store.clone();
        globals.set(
            "__record_text_content",
            Function::new(ctx.clone(), move |id: String, v: String| {
                let mut guard = s.lock().unwrap();
                guard.styles.entry(id.clone()).or_default().text_content = Some(v.clone());
                guard.text_sources.insert(
                    id,
                    super::ScriptTextSource {
                        text: v,
                        kind: super::ScriptTextSourceKind::TextNode,
                    },
                );
            })?,
        )?;
    }

    Ok(())
}
