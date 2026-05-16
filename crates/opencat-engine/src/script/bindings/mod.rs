pub mod animate_api;

use std::sync::{Arc, Mutex};

use rquickjs::Function;

use opencat_core::for_each_binding;
use opencat_core::script::recorder::{MutationRecorder, MutationStore, TextUnitValues};
use opencat_core::script::text_units::describe_text_units;
use opencat_core::style::color_token_from_script_string;

use opencat_core::scene::script::mutations::{
    CanvasCommand, ScriptColor, ScriptFontEdging, TextUnitGranularity,
};
use opencat_core::scene::script::object_fit_from_name;
use opencat_core::scene::script::{
    align_items_from_name, box_shadow_from_name, drop_shadow_from_name, flex_direction_from_name,
    font_edging_from_name, inset_shadow_from_name, justify_content_from_name, line_cap_from_name,
    line_join_from_name, parse_drrect_coords, parse_image_rect_coords, point_mode_from_name,
    position_from_name, script_color_from_value, text_align_from_name,
};

use opencat_core::style::{BorderStyle, FontWeight};

pub(crate) const NODE_STYLE_RUNTIME: &str = opencat_core::script::runtime::NODE_STYLE_RUNTIME;
pub(crate) const CANVASKIT_RUNTIME: &str = opencat_core::script::runtime::CANVAS_API_RUNTIME;

pub(crate) fn install_node_style_bindings<'js>(
    ctx: &rquickjs::Ctx<'js>,
    store: &Arc<Mutex<MutationStore>>,
) -> anyhow::Result<()> {
    let globals = ctx.globals();

    macro_rules! install_to_rquickjs {
        ($rec:ident $id:ident $name:ident ($id_owned:ident : &str $(, $param:ident : $param_ty:ty)*) $($body:tt)*) => {{
            let s = store.clone();
            globals.set(
                concat!("__", stringify!($name)),
                Function::new(ctx.clone(), move |$id_owned: String $(, $param: $param_ty)*| {
                    let $id: &str = &$id_owned;
                    let mut guard = s.lock().unwrap();
                    let $rec = &mut *guard as &mut dyn MutationRecorder;
                    $($body)*
                })?,
            )?;
        }};
    }

    for_each_binding!(rec id install_to_rquickjs);

    // ── Excluded bindings (kept hand-written) ──────────────────────────────

    // record_text_unit_override: complex object destructuring
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
                            color: color.and_then(|value| color_token_from_script_string(&value)),
                        },
                    );
                    Ok(())
                },
            )?,
        )?;
    }

    // text_units_describe: returns an array
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

    Ok(())
}

/// Install value-returning canvas bindings that don't fit the for_each_binding! pattern.
pub(crate) fn install_canvaskit_bindings<'js>(
    ctx: &rquickjs::Ctx<'js>,
    _store: &Arc<Mutex<MutationStore>>,
) -> anyhow::Result<()> {
    let globals = ctx.globals();

    // canvas_measure_text: returns f32 (text width), not a mutation
    globals.set(
        "__canvas_measure_text",
        Function::new(
            ctx.clone(),
            move |text: String,
                  font_size: f32,
                  font_scale_x: f32,
                  font_skew_x: f32,
                  font_subpixel: bool,
                  font_edging: String| {
                let font_edging = font_edging_from_name(&font_edging).ok_or_else(|| {
                    js_error(
                        "measureText",
                        format!("unsupported font edging `{font_edging}`"),
                    )
                })?;
                let font = make_script_font(
                    font_size,
                    font_scale_x,
                    font_skew_x,
                    font_subpixel,
                    font_edging,
                );
                let (width, _) = font.measure_str(text, None);
                Ok::<_, rquickjs::Error>(width)
            },
        )?,
    )?;

    Ok(())
}

// ── Helper functions used by the macro-generated bindings ──────────────────

fn js_error(op: &'static str, message: String) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("canvas", op, message)
}

fn parse_color(color: &str, op: &'static str) -> Result<ScriptColor, rquickjs::Error> {
    script_color_from_value(color)
        .ok_or_else(|| js_error(op, format!("unsupported color `{color}`")))
}

fn parse_image_rect_error(op: &'static str, coords: &[f32]) -> Result<[f32; 4], rquickjs::Error> {
    parse_image_rect_coords(coords)
        .ok_or_else(|| js_error(op, "expected source rect as [x, y, width, height]".to_string()))
}

fn parse_drrect_error(
    op: &'static str,
    coords: &[f32],
) -> Result<
    (
        f32, f32, f32, f32, f32,
        f32, f32, f32, f32, f32,
    ),
    rquickjs::Error,
> {
    parse_drrect_coords(coords)
        .ok_or_else(|| js_error(op, "expected 10 coordinate values".to_string()))
}

fn make_script_font(
    font_size: f32,
    font_scale_x: f32,
    font_skew_x: f32,
    font_subpixel: bool,
    font_edging: ScriptFontEdging,
) -> skia_safe::Font {
    let mut font = skia_safe::Font::default();
    if let Some(typeface) =
        skia_safe::FontMgr::new().legacy_make_typeface(None, skia_safe::FontStyle::normal())
    {
        font.set_typeface(typeface);
    }
    font.set_size(font_size.max(1.0));
    font.set_scale_x(font_scale_x);
    font.set_skew_x(font_skew_x);
    font.set_subpixel(font_subpixel);
    font.set_edging(match font_edging {
        ScriptFontEdging::Alias => skia_safe::font::Edging::Alias,
        ScriptFontEdging::AntiAlias => skia_safe::font::Edging::AntiAlias,
        ScriptFontEdging::SubpixelAntiAlias => skia_safe::font::Edging::SubpixelAntiAlias,
    });
    font
}
