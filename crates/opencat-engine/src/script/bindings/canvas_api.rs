use std::sync::{Arc, Mutex};

use rquickjs::Function;

use opencat_core::script::recorder::{MutationRecorder, MutationStore};
use opencat_core::scene::script::mutations::{
    CanvasCommand, ScriptColor, ScriptFontEdging, ScriptLineCap, ScriptLineJoin, ScriptPointMode,
};
use opencat_core::scene::script::object_fit_from_name;

fn line_cap_from_name(name: &str) -> Option<ScriptLineCap> {
    match name {
        "butt" => Some(ScriptLineCap::Butt),
        "round" => Some(ScriptLineCap::Round),
        "square" => Some(ScriptLineCap::Square),
        _ => None,
    }
}

fn line_join_from_name(name: &str) -> Option<ScriptLineJoin> {
    match name {
        "miter" => Some(ScriptLineJoin::Miter),
        "round" => Some(ScriptLineJoin::Round),
        "bevel" => Some(ScriptLineJoin::Bevel),
        _ => None,
    }
}

fn point_mode_from_name(name: &str) -> Option<ScriptPointMode> {
    match name {
        "points" => Some(ScriptPointMode::Points),
        "lines" => Some(ScriptPointMode::Lines),
        "polygon" => Some(ScriptPointMode::Polygon),
        _ => None,
    }
}

fn font_edging_from_name(name: &str) -> Option<ScriptFontEdging> {
    match name {
        "alias" => Some(ScriptFontEdging::Alias),
        "antiAlias" => Some(ScriptFontEdging::AntiAlias),
        "subpixelAntiAlias" => Some(ScriptFontEdging::SubpixelAntiAlias),
        _ => None,
    }
}

fn parse_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_rgb_channel(value: &str) -> Option<u8> {
    let channel = value.trim().parse::<f32>().ok()?;
    if !(0.0..=255.0).contains(&channel) {
        return None;
    }
    Some(channel.round() as u8)
}

fn parse_alpha_channel(value: &str) -> Option<u8> {
    let alpha = value.trim().parse::<f32>().ok()?;
    if !(0.0..=1.0).contains(&alpha) {
        return None;
    }
    Some((alpha * 255.0).round() as u8)
}

fn parse_rgb_function(value: &str) -> Option<ScriptColor> {
    let (is_rgba, body) = if let Some(body) = value
        .strip_prefix("rgba(")
        .and_then(|body| body.strip_suffix(')'))
    {
        (true, body)
    } else if let Some(body) = value
        .strip_prefix("rgb(")
        .and_then(|body| body.strip_suffix(')'))
    {
        (false, body)
    } else {
        return None;
    };

    let parts: Vec<_> = body.split(',').map(str::trim).collect();
    if (!is_rgba && parts.len() != 3) || (is_rgba && parts.len() != 4) {
        return None;
    }

    let r = parse_rgb_channel(parts[0])?;
    let g = parse_rgb_channel(parts[1])?;
    let b = parse_rgb_channel(parts[2])?;
    let a = if is_rgba {
        parse_alpha_channel(parts[3])?
    } else {
        255
    };

    Some(ScriptColor { r, g, b, a })
}

fn script_color_from_value(value: &str) -> Option<ScriptColor> {
    let color = opencat_core::style::color_token_from_script_name(value).map(|color| color.rgba());
    if let Some((r, g, b, a)) = color {
        return Some(ScriptColor { r, g, b, a });
    }

    if let Some(color) = parse_rgb_function(value) {
        return Some(color);
    }

    let hex = value.strip_prefix('#')?;
    let (r, g, b, a) = match hex.len() {
        3 => {
            let r = parse_hex_nibble(hex.as_bytes()[0])?;
            let g = parse_hex_nibble(hex.as_bytes()[1])?;
            let b = parse_hex_nibble(hex.as_bytes()[2])?;
            (r * 17, g * 17, b * 17, 255)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b, 255)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            (r, g, b, a)
        }
        _ => return None,
    };

    Some(ScriptColor { r, g, b, a })
}

pub(crate) const CANVASKIT_RUNTIME: &str = opencat_core::script::runtime::CANVAS_API_RUNTIME;

pub(crate) fn install_canvaskit_bindings<'js>(
    ctx: &rquickjs::Ctx<'js>,
    store: &Arc<Mutex<MutationStore>>,
) -> anyhow::Result<()> {
    let globals = ctx.globals();

    macro_rules! push_canvas_command {
        ($name:literal, |$id:ident $(, $arg:ident : $arg_ty:ty)*| $command:expr) => {{
            let s = store.clone();
            globals.set(
                $name,
                Function::new(ctx.clone(), move |$id: String $(, $arg: $arg_ty)*| {
                    let mut guard = s.lock().unwrap();
                    let rec = &mut *guard as &mut dyn MutationRecorder;
                    rec.record_canvas_command(&$id, $command);
                    Ok::<_, rquickjs::Error>(())
                })?,
            )?;
        }};
    }

    push_canvas_command!("__canvas_save", |id| CanvasCommand::Save);
    let s = store.clone();
    globals.set(
        "__canvas_save_layer",
        Function::new(
            ctx.clone(),
            move |id: String, alpha: f32, bounds: Option<Vec<f32>>| {
                let bounds = match bounds {
                    Some(bounds) => Some(parse_image_rect_coords(&bounds, "saveLayer")?),
                    None => None,
                };
                let mut guard = s.lock().unwrap();
                let rec = &mut *guard as &mut dyn MutationRecorder;
                rec.record_canvas_command(
                    &id,
                    CanvasCommand::SaveLayer {
                        alpha: alpha.clamp(0.0, 1.0),
                        bounds,
                    },
                );
                Ok::<_, rquickjs::Error>(())
            },
        )?,
    )?;
    push_canvas_command!("__canvas_restore", |id| CanvasCommand::Restore);
    push_canvas_command!("__canvas_restore_to_count", |id, count: i32| {
        CanvasCommand::RestoreToCount {
            count: count.max(1),
        }
    });
    push_canvas_command!("__canvas_translate", |id, x: f32, y: f32| {
        CanvasCommand::Translate { x, y }
    });
    push_canvas_command!("__canvas_scale", |id, x: f32, y: f32| {
        CanvasCommand::Scale { x, y }
    });
    push_canvas_command!("__canvas_rotate", |id, degrees: f32| {
        CanvasCommand::Rotate { degrees }
    });
    push_canvas_command!(
        "__canvas_clip_rect",
        |id, x: f32, y: f32, width: f32, height: f32, anti_alias: bool| CanvasCommand::ClipRect {
            x,
            y,
            width,
            height,
            anti_alias,
        }
    );
    push_canvas_command!(
        "__canvas_draw_line",
        |id, x0: f32, y0: f32, x1: f32, y1: f32| CanvasCommand::DrawLine { x0, y0, x1, y1 }
    );
    push_canvas_command!(
        "__canvas_fill_circle",
        |id, cx: f32, cy: f32, radius: f32| CanvasCommand::FillCircle { cx, cy, radius }
    );
    push_canvas_command!(
        "__canvas_stroke_circle",
        |id, cx: f32, cy: f32, radius: f32| CanvasCommand::StrokeCircle { cx, cy, radius }
    );
    push_canvas_command!(
        "__canvas_fill_rrect",
        |id, x: f32, y: f32, width: f32, height: f32, radius: f32| CanvasCommand::FillRRect {
            x,
            y,
            width,
            height,
            radius
        }
    );
    push_canvas_command!(
        "__canvas_stroke_rrect",
        |id, x: f32, y: f32, width: f32, height: f32, radius: f32| CanvasCommand::StrokeRRect {
            x,
            y,
            width,
            height,
            radius
        }
    );
    push_canvas_command!("__canvas_begin_path", |id| CanvasCommand::BeginPath);
    push_canvas_command!("__canvas_move_to", |id, x: f32, y: f32| {
        CanvasCommand::MoveTo { x, y }
    });
    push_canvas_command!("__canvas_line_to", |id, x: f32, y: f32| {
        CanvasCommand::LineTo { x, y }
    });
    push_canvas_command!(
        "__canvas_quad_to",
        |id, cx: f32, cy: f32, x: f32, y: f32| CanvasCommand::QuadTo { cx, cy, x, y }
    );
    push_canvas_command!(
        "__canvas_cubic_to",
        |id, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32| CanvasCommand::CubicTo {
            c1x,
            c1y,
            c2x,
            c2y,
            x,
            y
        }
    );
    push_canvas_command!("__canvas_close_path", |id| CanvasCommand::ClosePath);
    push_canvas_command!(
        "__canvas_path_add_rect",
        |id, x: f32, y: f32, width: f32, height: f32| CanvasCommand::AddRectPath {
            x,
            y,
            width,
            height,
        }
    );
    push_canvas_command!(
        "__canvas_path_add_rrect",
        |id, x: f32, y: f32, width: f32, height: f32, radius: f32| CanvasCommand::AddRRectPath {
            x,
            y,
            width,
            height,
            radius,
        }
    );
    push_canvas_command!(
        "__canvas_path_add_oval",
        |id, x: f32, y: f32, width: f32, height: f32| CanvasCommand::AddOvalPath {
            x,
            y,
            width,
            height,
        }
    );
    push_canvas_command!(
        "__canvas_path_add_arc",
        |id, x: f32, y: f32, width: f32, height: f32, start_angle: f32, sweep_angle: f32| {
            CanvasCommand::AddArcPath {
                x,
                y,
                width,
                height,
                start_angle,
                sweep_angle,
            }
        }
    );
    push_canvas_command!("__canvas_fill_path", |id| CanvasCommand::FillPath);
    push_canvas_command!("__canvas_stroke_path", |id| CanvasCommand::StrokePath);

    let s = store.clone();
    globals.set(
        "__canvas_set_fill_style",
        Function::new(ctx.clone(), move |id: String, color: String| {
            let color = parse_color(&color, "setFillStyle")?;
            let mut guard = s.lock().unwrap();
            let rec = &mut *guard as &mut dyn MutationRecorder;
            rec.record_canvas_command(&id, CanvasCommand::SetFillStyle { color });
            Ok::<_, rquickjs::Error>(())
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_set_stroke_style",
        Function::new(ctx.clone(), move |id: String, color: String| {
            let color = parse_color(&color, "setStrokeStyle")?;
            let mut guard = s.lock().unwrap();
            let rec = &mut *guard as &mut dyn MutationRecorder;
            rec.record_canvas_command(&id, CanvasCommand::SetStrokeStyle { color });
            Ok::<_, rquickjs::Error>(())
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_set_line_width",
        Function::new(ctx.clone(), move |id: String, width: f32| {
            let mut guard = s.lock().unwrap();
            let rec = &mut *guard as &mut dyn MutationRecorder;
            rec.record_canvas_command(
                &id,
                CanvasCommand::SetLineWidth {
                    width: width.max(0.0),
                },
            );
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_set_line_cap",
        Function::new(ctx.clone(), move |id: String, cap: String| {
            let cap = line_cap_from_name(&cap)
                .ok_or_else(|| js_error("setLineCap", format!("unsupported line cap `{cap}`")))?;
            let mut guard = s.lock().unwrap();
            let rec = &mut *guard as &mut dyn MutationRecorder;
            rec.record_canvas_command(&id, CanvasCommand::SetLineCap { cap });
            Ok::<_, rquickjs::Error>(())
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_set_line_join",
        Function::new(ctx.clone(), move |id: String, join: String| {
            let join = line_join_from_name(&join).ok_or_else(|| {
                js_error("setLineJoin", format!("unsupported line join `{join}`"))
            })?;
            let mut guard = s.lock().unwrap();
            let rec = &mut *guard as &mut dyn MutationRecorder;
            rec.record_canvas_command(&id, CanvasCommand::SetLineJoin { join });
            Ok::<_, rquickjs::Error>(())
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_set_line_dash",
        Function::new(
            ctx.clone(),
            move |id: String, intervals: Vec<f32>, phase: f32| {
                let mut guard = s.lock().unwrap();
                let rec = &mut *guard as &mut dyn MutationRecorder;
                rec.record_canvas_command(&id, CanvasCommand::SetLineDash { intervals, phase });
            },
        )?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_clear_line_dash",
        Function::new(ctx.clone(), move |id: String| {
            let mut guard = s.lock().unwrap();
            let rec = &mut *guard as &mut dyn MutationRecorder;
            rec.record_canvas_command(&id, CanvasCommand::ClearLineDash);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_set_global_alpha",
        Function::new(ctx.clone(), move |id: String, alpha: f32| {
            let mut guard = s.lock().unwrap();
            let rec = &mut *guard as &mut dyn MutationRecorder;
            rec.record_canvas_command(
                &id,
                CanvasCommand::SetGlobalAlpha {
                    alpha: alpha.clamp(0.0, 1.0),
                },
            );
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_set_anti_alias",
        Function::new(ctx.clone(), move |id: String, enabled: bool| {
            let mut guard = s.lock().unwrap();
            let rec = &mut *guard as &mut dyn MutationRecorder;
            rec.record_canvas_command(&id, CanvasCommand::SetAntiAlias { enabled });
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_clear",
        Function::new(ctx.clone(), move |id: String, color: Option<String>| {
            let color = match color {
                Some(color) => Some(parse_color(&color, "clear")?),
                None => None,
            };
            let mut guard = s.lock().unwrap();
            let rec = &mut *guard as &mut dyn MutationRecorder;
            rec.record_canvas_command(&id, CanvasCommand::Clear { color });
            Ok::<_, rquickjs::Error>(())
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_draw_paint",
        Function::new(
            ctx.clone(),
            move |id: String, color: String, anti_alias: bool| {
                let color = parse_color(&color, "drawPaint")?;
                let mut guard = s.lock().unwrap();
                let rec = &mut *guard as &mut dyn MutationRecorder;
                rec.record_canvas_command(
                    &id,
                    CanvasCommand::DrawPaint { color, anti_alias },
                );
                Ok::<_, rquickjs::Error>(())
            },
        )?,
    )?;

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

    let s = store.clone();
    globals.set(
        "__canvas_draw_text",
        Function::new(
            ctx.clone(),
            move |id: String,
                  text: String,
                  values: Vec<f32>,
                  color: String,
                  flags: Vec<bool>,
                  font_edging: String| {
                if values.len() < 6 {
                    return Err(js_error(
                        "drawText",
                        "expected text values [x, y, fontSize, scaleX, skewX, strokeWidth]"
                            .to_string(),
                    ));
                }
                if flags.len() < 3 {
                    return Err(js_error(
                        "drawText",
                        "expected text flags [antiAlias, stroke, fontSubpixel]".to_string(),
                    ));
                }
                let color = parse_color(&color, "drawText")?;
                let font_edging = font_edging_from_name(&font_edging).ok_or_else(|| {
                    js_error(
                        "drawText",
                        format!("unsupported font edging `{font_edging}`"),
                    )
                })?;
                let mut guard = s.lock().unwrap();
                let rec = &mut *guard as &mut dyn MutationRecorder;
                rec.record_canvas_command(
                    &id,
                    CanvasCommand::DrawText {
                        text,
                        x: values[0],
                        y: values[1],
                        color,
                        anti_alias: flags[0],
                        stroke: flags[1],
                        stroke_width: values[5].max(0.0),
                        font_size: values[2].max(1.0),
                        font_scale_x: values[3],
                        font_skew_x: values[4],
                        font_subpixel: flags[2],
                        font_edging,
                    },
                );
                Ok::<_, rquickjs::Error>(())
            },
        )?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_fill_rect",
        Function::new(
            ctx.clone(),
            move |id: String, x: f32, y: f32, width: f32, height: f32, color: String| {
                let color = parse_color(&color, "fillRect")?;
                let mut guard = s.lock().unwrap();
                let rec = &mut *guard as &mut dyn MutationRecorder;
                rec.record_canvas_command(
                    &id,
                    CanvasCommand::FillRect {
                        x,
                        y,
                        width,
                        height,
                        color,
                    },
                );
                Ok::<_, rquickjs::Error>(())
            },
        )?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_stroke_rect",
        Function::new(
            ctx.clone(),
            move |id: String,
                  x: f32,
                  y: f32,
                  width: f32,
                  height: f32,
                  color: String,
                  stroke_width: f32| {
                let color = parse_color(&color, "strokeRect")?;
                let mut guard = s.lock().unwrap();
                let rec = &mut *guard as &mut dyn MutationRecorder;
                rec.record_canvas_command(
                    &id,
                    CanvasCommand::StrokeRect {
                        x,
                        y,
                        width,
                        height,
                        color,
                        stroke_width: stroke_width.max(0.0),
                    },
                );
                Ok::<_, rquickjs::Error>(())
            },
        )?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_draw_image",
        Function::new(
            ctx.clone(),
            move |id: String,
                  asset_id: String,
                  values: Vec<f32>,
                  fit: String,
                  alpha: f32,
                  anti_alias: bool| {
                let object_fit = object_fit_from_name(&fit).ok_or_else(|| {
                    js_error("drawImage", format!("unsupported objectFit `{fit}`"))
                })?;
                if values.len() < 4 {
                    return Err(js_error(
                        "drawImageRect",
                        "expected destination rect as [x, y, width, height]".to_string(),
                    ));
                }
                let src_rect = match values.len() {
                    4 => None,
                    8.. => Some(parse_image_rect_coords(&values[4..8], "drawImageRect")?),
                    _ => {
                        return Err(js_error(
                            "drawImageRect",
                            "expected either 4 or 8 image rect values".to_string(),
                        ));
                    }
                };
                let mut guard = s.lock().unwrap();
                let rec = &mut *guard as &mut dyn MutationRecorder;
                rec.record_canvas_command(
                    &id,
                    CanvasCommand::DrawImage {
                        asset_id,
                        x: values[0],
                        y: values[1],
                        width: values[2],
                        height: values[3],
                        src_rect,
                        alpha: alpha.clamp(0.0, 1.0),
                        anti_alias,
                        object_fit,
                    },
                );
                Ok::<_, rquickjs::Error>(())
            },
        )?,
    )?;

    // --- drawArc (fill) ---
    let s = store.clone();
    globals.set(
        "__canvas_draw_arc",
        Function::new(
            ctx.clone(),
            move |id: String,
                  cx: f32,
                  cy: f32,
                  rx: f32,
                  ry: f32,
                  start_angle: f32,
                  sweep_angle: f32| {
                let mut guard = s.lock().unwrap();
                let rec = &mut *guard as &mut dyn MutationRecorder;
                rec.record_canvas_command(
                    &id,
                    CanvasCommand::DrawArc {
                        cx,
                        cy,
                        rx,
                        ry,
                        start_angle,
                        sweep_angle,
                        use_center: false,
                    },
                );
                Ok::<_, rquickjs::Error>(())
            },
        )?,
    )?;

    // --- drawArc (fill, useCenter) ---
    let s = store.clone();
    globals.set(
        "__canvas_draw_arc_to_center",
        Function::new(
            ctx.clone(),
            move |id: String,
                  cx: f32,
                  cy: f32,
                  rx: f32,
                  ry: f32,
                  start_angle: f32,
                  sweep_angle: f32| {
                let mut guard = s.lock().unwrap();
                let rec = &mut *guard as &mut dyn MutationRecorder;
                rec.record_canvas_command(
                    &id,
                    CanvasCommand::DrawArc {
                        cx,
                        cy,
                        rx,
                        ry,
                        start_angle,
                        sweep_angle,
                        use_center: true,
                    },
                );
                Ok::<_, rquickjs::Error>(())
            },
        )?,
    )?;

    // --- drawArc (stroke) ---
    push_canvas_command!(
        "__canvas_stroke_arc",
        |id, cx: f32, cy: f32, rx: f32, ry: f32, start_angle: f32, sweep_angle: f32| {
            CanvasCommand::StrokeArc {
                cx,
                cy,
                rx,
                ry,
                start_angle,
                sweep_angle,
            }
        }
    );

    // --- drawOval (fill) ---
    push_canvas_command!(
        "__canvas_fill_oval",
        |id, cx: f32, cy: f32, rx: f32, ry: f32| CanvasCommand::FillOval { cx, cy, rx, ry }
    );

    // --- drawOval (stroke) ---
    push_canvas_command!(
        "__canvas_stroke_oval",
        |id, cx: f32, cy: f32, rx: f32, ry: f32| CanvasCommand::StrokeOval { cx, cy, rx, ry }
    );

    // --- clipPath ---
    push_canvas_command!("__canvas_clip_path", |id, anti_alias: bool| {
        CanvasCommand::ClipPath { anti_alias }
    });

    // --- clipRRect ---
    push_canvas_command!(
        "__canvas_clip_rrect",
        |id, x: f32, y: f32, width: f32, height: f32, radius: f32, anti_alias: bool| {
            CanvasCommand::ClipRRect {
                x,
                y,
                width,
                height,
                radius,
                anti_alias,
            }
        }
    );

    // --- drawPoints ---
    let s = store.clone();
    globals.set(
        "__canvas_draw_points",
        Function::new(
            ctx.clone(),
            move |id: String, mode: String, points: Vec<f32>| {
                let mode = point_mode_from_name(&mode).ok_or_else(|| {
                    js_error("drawPoints", format!("unsupported point mode `{mode}`"))
                })?;
                let mut guard = s.lock().unwrap();
                let rec = &mut *guard as &mut dyn MutationRecorder;
                rec.record_canvas_command(&id, CanvasCommand::DrawPoints { mode, points });
                Ok::<_, rquickjs::Error>(())
            },
        )?,
    )?;

    // --- drawDRRect (fill) ---
    let s = store.clone();
    globals.set(
        "__canvas_fill_drrect",
        Function::new(ctx.clone(), move |id: String, coords: Vec<f32>| {
            let (
                outer_x,
                outer_y,
                outer_width,
                outer_height,
                outer_radius,
                inner_x,
                inner_y,
                inner_width,
                inner_height,
                inner_radius,
            ) = parse_drrect_coords(&coords, "fillDRRect")?;
            let mut guard = s.lock().unwrap();
            let rec = &mut *guard as &mut dyn MutationRecorder;
            rec.record_canvas_command(
                &id,
                CanvasCommand::FillDRRect {
                    outer_x,
                    outer_y,
                    outer_width,
                    outer_height,
                    outer_radius,
                    inner_x,
                    inner_y,
                    inner_width,
                    inner_height,
                    inner_radius,
                },
            );
            Ok::<_, rquickjs::Error>(())
        })?,
    )?;

    // --- drawDRRect (stroke) ---
    let s = store.clone();
    globals.set(
        "__canvas_stroke_drrect",
        Function::new(ctx.clone(), move |id: String, coords: Vec<f32>| {
            let (
                outer_x,
                outer_y,
                outer_width,
                outer_height,
                outer_radius,
                inner_x,
                inner_y,
                inner_width,
                inner_height,
                inner_radius,
            ) = parse_drrect_coords(&coords, "strokeDRRect")?;
            let mut guard = s.lock().unwrap();
            let rec = &mut *guard as &mut dyn MutationRecorder;
            rec.record_canvas_command(
                &id,
                CanvasCommand::StrokeDRRect {
                    outer_x,
                    outer_y,
                    outer_width,
                    outer_height,
                    outer_radius,
                    inner_x,
                    inner_y,
                    inner_width,
                    inner_height,
                    inner_radius,
                },
            );
            Ok::<_, rquickjs::Error>(())
        })?,
    )?;

    // --- skew ---
    push_canvas_command!("__canvas_skew", |id, sx: f32, sy: f32| {
        CanvasCommand::Skew { sx, sy }
    });

    // --- drawImage (simple, no dest) ---
    push_canvas_command!(
        "__canvas_draw_image_simple",
        |id, asset_id: String, x: f32, y: f32, alpha: f32, anti_alias: bool| {
            CanvasCommand::DrawImageSimple {
                asset_id,
                x,
                y,
                alpha: alpha.clamp(0.0, 1.0),
                anti_alias,
            }
        }
    );

    // --- concat ---
    let s = store.clone();
    globals.set(
        "__canvas_concat",
        Function::new(ctx.clone(), move |id: String, values: Vec<f32>| {
            if values.len() < 9 {
                return Err(js_error("concat", "expected 9 matrix values".to_string()));
            }
            let matrix = [
                values[0], values[1], values[2], values[3], values[4], values[5], values[6],
                values[7], values[8],
            ];
            let mut guard = s.lock().unwrap();
            let rec = &mut *guard as &mut dyn MutationRecorder;
            rec.record_canvas_command(&id, CanvasCommand::Concat { matrix });
            Ok::<_, rquickjs::Error>(())
        })?,
    )?;

    Ok(())
}

fn js_error(op: &'static str, message: String) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("canvas", op, message)
}

fn parse_color(color: &str, op: &'static str) -> Result<ScriptColor, rquickjs::Error> {
    script_color_from_value(color)
        .ok_or_else(|| js_error(op, format!("unsupported color `{color}`")))
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

fn parse_image_rect_coords(coords: &[f32], op: &'static str) -> Result<[f32; 4], rquickjs::Error> {
    if coords.len() < 4 {
        return Err(js_error(
            op,
            "expected source rect as [x, y, width, height]".to_string(),
        ));
    }
    Ok([coords[0], coords[1], coords[2], coords[3]])
}

fn parse_drrect_coords(
    coords: &[f32],
    op: &'static str,
) -> Result<(f32, f32, f32, f32, f32, f32, f32, f32, f32, f32), rquickjs::Error> {
    if coords.len() < 10 {
        return Err(js_error(op, "expected 10 coordinate values".to_string()));
    }
    Ok((
        coords[0], coords[1], coords[2], coords[3], coords[4], coords[5], coords[6], coords[7],
        coords[8], coords[9],
    ))
}
