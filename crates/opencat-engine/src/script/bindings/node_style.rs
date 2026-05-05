use rquickjs::Function;

use opencat_core::style::{ColorToken, FontWeight, Transform, color_token_from_script_name};
use opencat_core::script::text_units::describe_text_units;

use crate::script::{
    MutationStore, align_items_from_name, box_shadow_from_name, drop_shadow_from_name,
    flex_direction_from_name, inset_shadow_from_name,
    justify_content_from_name, object_fit_from_name, position_from_name, text_align_from_name,
};
use opencat_core::scene::script::mutations::{
    TextUnitGranularity, TextUnitOverride, TextUnitOverrideBatch,
};

// color_from_name is used by runtime bindings and references animate_api

fn color_from_name(name: &str) -> Option<ColorToken> {
    if let Some(c) = color_token_from_script_name(name) {
        return Some(c);
    }
    let hsla = super::animate_api::parse_color(name)?;
    let (r, g, b) = super::animate_api::hsl_to_rgb(hsla.h, hsla.s, hsla.l);
    let a = (hsla.a.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some(ColorToken::Custom(r, g, b, a))
}

pub(crate) const NODE_STYLE_RUNTIME: &str = opencat_core::script::runtime::NODE_STYLE_RUNTIME;

pub(crate) fn install_node_style_bindings<'js>(
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
            .push(Transform::TranslateX { value: v });
    });
    set_style_binding!("__record_translate_y", map, |id, v: f32| {
        map.entry(id)
            .or_default()
            .transforms
            .push(Transform::TranslateY { value: v });
    });
    set_style_binding!("__record_translate", map, |id, x: f32, y: f32| {
        map.entry(id)
            .or_default()
            .transforms
            .push(Transform::Translate { x, y });
    });
    set_style_binding!("__record_scale", map, |id, v: f32| {
        map.entry(id)
            .or_default()
            .transforms
            .push(Transform::Scale { value: v });
    });
    set_style_binding!("__record_scale_x", map, |id, v: f32| {
        map.entry(id)
            .or_default()
            .transforms
            .push(Transform::ScaleX { value: v });
    });
    set_style_binding!("__record_scale_y", map, |id, v: f32| {
        map.entry(id)
            .or_default()
            .transforms
            .push(Transform::ScaleY { value: v });
    });
    set_style_binding!("__record_rotate", map, |id, v: f32| {
        map.entry(id)
            .or_default()
            .transforms
            .push(Transform::RotateDeg { value: v });
    });
    set_style_binding!("__record_skew_x", map, |id, v: f32| {
        map.entry(id)
            .or_default()
            .transforms
            .push(Transform::SkewXDeg { value: v });
    });
    set_style_binding!("__record_skew_y", map, |id, v: f32| {
        map.entry(id)
            .or_default()
            .transforms
            .push(Transform::SkewYDeg { value: v });
    });
    set_style_binding!("__record_skew", map, |id, x_deg: f32, y_deg: f32| {
        map.entry(id)
            .or_default()
            .transforms
            .push(Transform::SkewDeg { x: x_deg, y: y_deg });
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
            "solid" => Some(opencat_core::style::BorderStyle::Solid),
            "dashed" => Some(opencat_core::style::BorderStyle::Dashed),
            "dotted" => Some(opencat_core::style::BorderStyle::Dotted),
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
        map.entry(id).or_default().stroke_width = Some(v.max(0.0));
    });
    set_style_binding!("__record_stroke_dasharray", map, |id, v: f32| {
        map.entry(id).or_default().stroke_dasharray = Some(v.max(0.0));
    });
    set_style_binding!("__record_stroke_dashoffset", map, |id, v: f32| {
        map.entry(id).or_default().stroke_dashoffset = Some(v);
    });
    set_style_binding!("__record_stroke_color", map, |id, v: String| {
        if let Some(c) = color_from_name(&v) {
            map.entry(id).or_default().stroke_color = Some(c);
        }
    });
    set_style_binding!("__record_fill_color", map, |id, v: String| {
        if let Some(c) = color_from_name(&v) {
            map.entry(id).or_default().fill_color = Some(c);
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
    set_style_binding!("__record_font_weight", map, |id, v: f64| {
        map.entry(id).or_default().font_weight = Some(FontWeight(v as u16));
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
                    opencat_core::scene::script::ScriptTextSource {
                        text: v,
                        kind: opencat_core::scene::script::ScriptTextSourceKind::TextNode,
                    },
                );
            })?,
        )?;
    }

    // Record a per-unit (grapheme/word) visual override for a text node.
    // Arguments: id, granularity ("graphemes"|"words"), index, values_object
    // values_object keys: opacity, translateX, translateY, scale, rotation, color/textColor
    {
        let s = store.clone();
        globals.set(
            "__record_text_unit_override",
            Function::new(
                ctx.clone(),
                move |id: String,
                      granularity: String,
                      index: u32,
                      values: rquickjs::Object<'js>|
                      -> Result<(), rquickjs::Error> {
                    let index = index as usize;
                    let gran = match granularity.as_str() {
                        "graphemes" => TextUnitGranularity::Grapheme,
                        "words" => TextUnitGranularity::Word,
                        _ => {
                            return Err(rquickjs::Error::new_from_js_message(
                                "text",
                                "granularity",
                                "unsupported granularity",
                            ));
                        }
                    };

                    let opacity: Option<f64> = values.get("opacity").ok().flatten();
                    let translate_x: Option<f64> = values.get("translateX").ok().flatten();
                    let translate_y: Option<f64> = values.get("translateY").ok().flatten();
                    let scale: Option<f64> = values.get("scale").ok().flatten();
                    let rotation_deg: Option<f64> = values.get("rotation").ok().flatten();
                    let color: Option<String> = values
                        .get("textColor")
                        .ok()
                        .flatten()
                        .or_else(|| values.get("color").ok().flatten());

                    let mut guard = s.lock().unwrap();
                    let mutations = guard.styles.entry(id).or_default();
                    match &mut mutations.text_unit_overrides {
                        Some(batch) => {
                            if batch.granularity != gran {
                                return Err(rquickjs::Error::new_from_js_message(
                                    "text",
                                    "granularity",
                                    "mixed text unit granularities are not allowed in one node",
                                ));
                            }
                            if index >= batch.overrides.len() {
                                batch
                                    .overrides
                                    .resize_with(index + 1, TextUnitOverride::default);
                            }
                        }
                        None => {
                            let mut batch = TextUnitOverrideBatch {
                                granularity: gran,
                                overrides: Vec::new(),
                            };
                            batch
                                .overrides
                                .resize_with(index + 1, TextUnitOverride::default);
                            mutations.text_unit_overrides = Some(batch);
                        }
                    }
                    let entry =
                        &mut mutations.text_unit_overrides.as_mut().unwrap().overrides[index];
                    if let Some(v) = opacity {
                        entry.opacity = Some(v as f32);
                    }
                    if let Some(v) = translate_x {
                        entry.translate_x = Some(v as f32);
                    }
                    if let Some(v) = translate_y {
                        entry.translate_y = Some(v as f32);
                    }
                    if let Some(v) = scale {
                        entry.scale = Some(v as f32);
                    }
                    if let Some(v) = rotation_deg {
                        entry.rotation_deg = Some(v as f32);
                    }
                    if let Some(v) = color.and_then(|value| color_from_name(&value)) {
                        entry.color = Some(v);
                    }
                    Ok(())
                },
            )?,
        )?;
    }

    // Describes text units using proper grapheme-cluster or word-boundary segmentation.
    {
        let s = store.clone();
        globals.set(
            "__text_units_describe",
            Function::new(
                ctx.clone(),
                move |ctx_inner: rquickjs::Ctx<'js>,
                      id: String,
                      granularity_str: String|
                      -> Result<rquickjs::Array<'js>, rquickjs::Error> {
                    let text = {
                        let guard = s.lock().unwrap();
                        guard
                            .text_sources
                            .get(&id)
                            .map(|src| src.text.clone())
                            .ok_or_else(|| {
                                rquickjs::Error::new_from_js_message(
                                    "text",
                                    "id",
                                    "no text source found for node",
                                )
                            })?
                    };
                    // guard dropped here
                    let granularity = match granularity_str.as_str() {
                        "graphemes" => TextUnitGranularity::Grapheme,
                        "words" => TextUnitGranularity::Word,
                        _ => {
                            return Err(rquickjs::Error::new_from_js_message(
                                "granularity",
                                "invalid value",
                                "unknown granularity; expected 'graphemes' or 'words'",
                            ));
                        }
                    };
                    let units = describe_text_units(&text, granularity);
                    let result = rquickjs::Array::new(ctx_inner.clone())?;
                    for (i, unit) in units.iter().enumerate() {
                        let entry = rquickjs::Array::new(ctx_inner.clone())?;
                        entry.set(0, unit.index as f64)?;
                        entry.set(1, unit.text.clone())?;
                        entry.set(2, unit.start as f64)?;
                        entry.set(3, unit.end as f64)?;
                        result.set(i, entry)?;
                    }
                    Ok(result)
                },
            )?,
        )?;
    }

    // SVG path data override (used by morph-svg animation)
    {
        let s = store.clone();
        globals.set(
            "__record_svg_path",
            Function::new(ctx.clone(), move |id: String, v: String| {
                let mut guard = s.lock().unwrap();
                guard.styles.entry(id).or_default().svg_path = Some(v);
            })?,
        )?;
    }

    Ok(())
}
