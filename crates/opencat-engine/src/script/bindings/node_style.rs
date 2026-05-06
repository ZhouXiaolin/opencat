use std::sync::{Arc, Mutex};

use rquickjs::Function;

use opencat_core::style::{ColorToken, FontWeight, color_token_from_script_name};
use opencat_core::script::text_units::describe_text_units;
use opencat_core::script::recorder::{MutationRecorder, MutationStore, TextUnitValues};
use opencat_core::script::animate::{hsl_to_rgb, parse_color};

use opencat_core::scene::script::{
    align_items_from_name, box_shadow_from_name, drop_shadow_from_name,
    flex_direction_from_name, inset_shadow_from_name,
    justify_content_from_name, object_fit_from_name, position_from_name, text_align_from_name,
};
use opencat_core::scene::script::mutations::{
    TextUnitGranularity,
};

fn color_from_name(name: &str) -> Option<ColorToken> {
    if let Some(c) = color_token_from_script_name(name) {
        return Some(c);
    }
    let hsla = parse_color(name)?;
    let (r, g, b) = hsl_to_rgb(hsla.h, hsla.s, hsla.l);
    let a = (hsla.a.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some(ColorToken::Custom(r, g, b, a))
}

pub(crate) const NODE_STYLE_RUNTIME: &str = opencat_core::script::runtime::NODE_STYLE_RUNTIME;

pub(crate) fn install_node_style_bindings<'js>(
    ctx: &rquickjs::Ctx<'js>,
    store: &Arc<Mutex<MutationStore>>,
) -> anyhow::Result<()> {
    let globals = ctx.globals();

    macro_rules! bind_recorder {
        ($name:literal, |$rec:ident, $id:ident $(, $arg:ident : $arg_ty:ty)*| $body:block) => {{
            let s = store.clone();
            globals.set(
                $name,
                Function::new(ctx.clone(), move |$id: String $(, $arg: $arg_ty)*| {
                    let mut guard = s.lock().unwrap();
                    let $rec = &mut *guard as &mut dyn MutationRecorder;
                    $body
                })?,
            )?;
        }};
    }

    bind_recorder!("__record_opacity", |rec, id, v: f32| {
        rec.record_opacity(&id, v);
    });
    bind_recorder!("__record_translate_x", |rec, id, v: f32| {
        rec.record_translate_x(&id, v);
    });
    bind_recorder!("__record_translate_y", |rec, id, v: f32| {
        rec.record_translate_y(&id, v);
    });
    bind_recorder!("__record_translate", |rec, id, x: f32, y: f32| {
        rec.record_translate(&id, x, y);
    });
    bind_recorder!("__record_scale", |rec, id, v: f32| {
        rec.record_scale(&id, v);
    });
    bind_recorder!("__record_scale_x", |rec, id, v: f32| {
        rec.record_scale_x(&id, v);
    });
    bind_recorder!("__record_scale_y", |rec, id, v: f32| {
        rec.record_scale_y(&id, v);
    });
    bind_recorder!("__record_rotate", |rec, id, v: f32| {
        rec.record_rotate(&id, v);
    });
    bind_recorder!("__record_skew_x", |rec, id, v: f32| {
        rec.record_skew_x(&id, v);
    });
    bind_recorder!("__record_skew_y", |rec, id, v: f32| {
        rec.record_skew_y(&id, v);
    });
    bind_recorder!("__record_skew", |rec, id, x_deg: f32, y_deg: f32| {
        rec.record_skew(&id, x_deg, y_deg);
    });
    bind_recorder!("__record_position", |rec, id, v: String| {
        if let Some(pos) = position_from_name(&v) {
            rec.record_position(&id, pos);
        }
    });
    bind_recorder!("__record_left", |rec, id, v: f32| {
        rec.record_left(&id, v);
    });
    bind_recorder!("__record_top", |rec, id, v: f32| {
        rec.record_top(&id, v);
    });
    bind_recorder!("__record_right", |rec, id, v: f32| {
        rec.record_right(&id, v);
    });
    bind_recorder!("__record_bottom", |rec, id, v: f32| {
        rec.record_bottom(&id, v);
    });
    bind_recorder!("__record_width", |rec, id, v: f32| {
        rec.record_width(&id, v);
    });
    bind_recorder!("__record_height", |rec, id, v: f32| {
        rec.record_height(&id, v);
    });
    bind_recorder!("__record_padding", |rec, id, v: f32| {
        rec.record_padding(&id, v);
    });
    bind_recorder!("__record_padding_x", |rec, id, v: f32| {
        rec.record_padding_x(&id, v);
    });
    bind_recorder!("__record_padding_y", |rec, id, v: f32| {
        rec.record_padding_y(&id, v);
    });
    bind_recorder!("__record_margin", |rec, id, v: f32| {
        rec.record_margin(&id, v);
    });
    bind_recorder!("__record_margin_x", |rec, id, v: f32| {
        rec.record_margin_x(&id, v);
    });
    bind_recorder!("__record_margin_y", |rec, id, v: f32| {
        rec.record_margin_y(&id, v);
    });
    bind_recorder!("__record_flex_direction", |rec, id, v: String| {
        if let Some(fd) = flex_direction_from_name(&v) {
            rec.record_flex_direction(&id, fd);
        }
    });
    bind_recorder!("__record_justify_content", |rec, id, v: String| {
        if let Some(jc) = justify_content_from_name(&v) {
            rec.record_justify_content(&id, jc);
        }
    });
    bind_recorder!("__record_align_items", |rec, id, v: String| {
        if let Some(ai) = align_items_from_name(&v) {
            rec.record_align_items(&id, ai);
        }
    });
    bind_recorder!("__record_gap", |rec, id, v: f32| {
        rec.record_gap(&id, v);
    });
    bind_recorder!("__record_flex_grow", |rec, id, v: f32| {
        rec.record_flex_grow(&id, v);
    });
    bind_recorder!("__record_bg", |rec, id, v: String| {
        if let Some(c) = color_from_name(&v) {
            rec.record_bg_color(&id, c);
        }
    });
    bind_recorder!("__record_border_radius", |rec, id, v: f32| {
        rec.record_border_radius(&id, v);
    });
    bind_recorder!("__record_border_width", |rec, id, v: f32| {
        rec.record_border_width(&id, v);
    });
    bind_recorder!("__record_border_top_width", |rec, id, v: f32| {
        rec.record_border_top_width(&id, v);
    });
    bind_recorder!("__record_border_right_width", |rec, id, v: f32| {
        rec.record_border_right_width(&id, v);
    });
    bind_recorder!("__record_border_bottom_width", |rec, id, v: f32| {
        rec.record_border_bottom_width(&id, v);
    });
    bind_recorder!("__record_border_left_width", |rec, id, v: f32| {
        rec.record_border_left_width(&id, v);
    });
    bind_recorder!("__record_border_style", |rec, id, v: String| {
        let parsed = match v.as_str() {
            "solid" => Some(opencat_core::style::BorderStyle::Solid),
            "dashed" => Some(opencat_core::style::BorderStyle::Dashed),
            "dotted" => Some(opencat_core::style::BorderStyle::Dotted),
            _ => None,
        };
        if let Some(bs) = parsed {
            rec.record_border_style(&id, bs);
        }
    });
    bind_recorder!("__record_border_color", |rec, id, v: String| {
        if let Some(c) = color_from_name(&v) {
            rec.record_border_color(&id, c);
        }
    });
    bind_recorder!("__record_stroke_width", |rec, id, v: f32| {
        rec.record_stroke_width(&id, v);
    });
    bind_recorder!("__record_stroke_dasharray", |rec, id, v: f32| {
        rec.record_stroke_dasharray(&id, v);
    });
    bind_recorder!("__record_stroke_dashoffset", |rec, id, v: f32| {
        rec.record_stroke_dashoffset(&id, v);
    });
    bind_recorder!("__record_stroke_color", |rec, id, v: String| {
        if let Some(c) = color_from_name(&v) {
            rec.record_stroke_color(&id, c);
        }
    });
    bind_recorder!("__record_fill_color", |rec, id, v: String| {
        if let Some(c) = color_from_name(&v) {
            rec.record_fill_color(&id, c);
        }
    });
    bind_recorder!("__record_object_fit", |rec, id, v: String| {
        if let Some(of) = object_fit_from_name(&v) {
            rec.record_object_fit(&id, of);
        }
    });
    bind_recorder!("__record_text_color", |rec, id, v: String| {
        if let Some(c) = color_from_name(&v) {
            rec.record_text_color(&id, c);
        }
    });
    bind_recorder!("__record_text_size", |rec, id, v: f32| {
        rec.record_text_size(&id, v);
    });
    bind_recorder!("__record_font_weight", |rec, id, v: f64| {
        rec.record_font_weight(&id, FontWeight(v as u16));
    });
    bind_recorder!("__record_letter_spacing", |rec, id, v: f32| {
        rec.record_letter_spacing(&id, v);
    });
    bind_recorder!("__record_text_align", |rec, id, v: String| {
        if let Some(align) = text_align_from_name(&v) {
            rec.record_text_align(&id, align);
        }
    });
    bind_recorder!("__record_line_height", |rec, id, v: f32| {
        rec.record_line_height(&id, v);
    });
    bind_recorder!("__record_shadow", |rec, id, v: String| {
        if let Some(sh) = box_shadow_from_name(&v) {
            rec.record_box_shadow(&id, sh);
        }
    });
    bind_recorder!("__record_shadow_color", |rec, id, v: String| {
        if let Some(color) = color_from_name(&v) {
            rec.record_box_shadow_color(&id, color);
        }
    });
    bind_recorder!("__record_inset_shadow", |rec, id, v: String| {
        if let Some(sh) = inset_shadow_from_name(&v) {
            rec.record_inset_shadow(&id, sh);
        }
    });
    bind_recorder!("__record_inset_shadow_color", |rec, id, v: String| {
        if let Some(color) = color_from_name(&v) {
            rec.record_inset_shadow_color(&id, color);
        }
    });
    bind_recorder!("__record_drop_shadow", |rec, id, v: String| {
        if let Some(sh) = drop_shadow_from_name(&v) {
            rec.record_drop_shadow(&id, sh);
        }
    });
    bind_recorder!("__record_drop_shadow_color", |rec, id, v: String| {
        if let Some(color) = color_from_name(&v) {
            rec.record_drop_shadow_color(&id, color);
        }
    });
    bind_recorder!("__record_text_content", |rec, id, v: String| {
        rec.record_text_content(&id, v);
    });

    // Record a per-unit (grapheme/word) visual override for a text node.
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
                    guard.record_text_unit_override(
                        &id,
                        gran,
                        index,
                        TextUnitValues {
                            opacity: opacity.map(|v| v as f32),
                            translate_x: translate_x.map(|v| v as f32),
                            translate_y: translate_y.map(|v| v as f32),
                            scale: scale.map(|v| v as f32),
                            rotation_deg: rotation_deg.map(|v| v as f32),
                            color: color.and_then(|value| color_from_name(&value)),
                        },
                    );
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
                            .get_text_source(&id)
                            .map(|src| src.text.clone())
                            .ok_or_else(|| {
                                rquickjs::Error::new_from_js_message(
                                    "text",
                                    "id",
                                    "no text source found for node",
                                )
                            })?
                    };
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
    bind_recorder!("__record_svg_path", |rec, id, v: String| {
        rec.record_svg_path(&id, v);
    });

    Ok(())
}
