use rquickjs::Function;

use super::{MutationStore, object_fit_from_name};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScriptColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl ScriptColor {
    pub fn to_skia(self) -> skia_safe::Color {
        skia_safe::Color::from_argb(self.a, self.r, self.g, self.b)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScriptLineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScriptLineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CanvasCommand {
    Save,
    Restore,
    SetFillStyle {
        color: ScriptColor,
    },
    SetStrokeStyle {
        color: ScriptColor,
    },
    SetLineWidth {
        width: f32,
    },
    SetLineCap {
        cap: ScriptLineCap,
    },
    SetLineJoin {
        join: ScriptLineJoin,
    },
    SetGlobalAlpha {
        alpha: f32,
    },
    Translate {
        x: f32,
        y: f32,
    },
    Scale {
        x: f32,
        y: f32,
    },
    Rotate {
        degrees: f32,
    },
    ClipRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    Clear {
        color: Option<ScriptColor>,
    },
    FillRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: ScriptColor,
    },
    FillRRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
    },
    StrokeRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: ScriptColor,
        stroke_width: f32,
    },
    StrokeRRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
    },
    DrawLine {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    },
    FillCircle {
        cx: f32,
        cy: f32,
        radius: f32,
    },
    StrokeCircle {
        cx: f32,
        cy: f32,
        radius: f32,
    },
    BeginPath,
    MoveTo {
        x: f32,
        y: f32,
    },
    LineTo {
        x: f32,
        y: f32,
    },
    QuadTo {
        cx: f32,
        cy: f32,
        x: f32,
        y: f32,
    },
    CubicTo {
        c1x: f32,
        c1y: f32,
        c2x: f32,
        c2y: f32,
        x: f32,
        y: f32,
    },
    ClosePath,
    FillPath,
    StrokePath,
    DrawImage {
        asset_id: String,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        object_fit: crate::style::ObjectFit,
    },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CanvasMutations {
    pub commands: Vec<CanvasCommand>,
}

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

fn parse_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn script_color_from_value(value: &str) -> Option<ScriptColor> {
    let color = crate::style::color_token_from_script_name(value).map(|color| color.to_skia());
    if let Some(color) = color {
        return Some(ScriptColor {
            r: color.r(),
            g: color.g(),
            b: color.b(),
            a: color.a(),
        });
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

pub(super) const CANVASKIT_RUNTIME: &str = r#"
(function() {
    const canvasCache = {};

    function applyCanvasMutation(id, prop, ...args) {
        switch (prop) {
            case 'save': __canvas_save(id); break;
            case 'restore': __canvas_restore(id); break;
            case 'setFillStyle':
            case 'fillStyle': __canvas_set_fill_style(id, String(args[0])); break;
            case 'setStrokeStyle':
            case 'strokeStyle': __canvas_set_stroke_style(id, String(args[0])); break;
            case 'setLineWidth':
            case 'lineWidth': __canvas_set_line_width(id, args[0]); break;
            case 'setLineCap': __canvas_set_line_cap(id, String(args[0])); break;
            case 'setLineJoin': __canvas_set_line_join(id, String(args[0])); break;
            case 'setGlobalAlpha':
            case 'globalAlpha': __canvas_set_global_alpha(id, args[0]); break;
            case 'translate': __canvas_translate(id, args[0], args[1]); break;
            case 'scale':
                if (args.length < 2) {
                    __canvas_scale(id, args[0], args[0]);
                } else {
                    __canvas_scale(id, args[0], args[1]);
                }
                break;
            case 'rotate': __canvas_rotate(id, args[0]); break;
            case 'clipRect': __canvas_clip_rect(id, args[0], args[1], args[2], args[3]); break;
            case 'drawLine': __canvas_draw_line(id, args[0], args[1], args[2], args[3]); break;
            case 'fillCircle': __canvas_fill_circle(id, args[0], args[1], args[2]); break;
            case 'strokeCircle': __canvas_stroke_circle(id, args[0], args[1], args[2]); break;
            case 'clear':
                if (args.length === 0 || args[0] == null) {
                    __canvas_clear(id, null);
                } else {
                    __canvas_clear(id, String(args[0]));
                }
                break;
            case 'fillRect': __canvas_fill_rect(id, args[0], args[1], args[2], args[3], String(args[4])); break;
            case 'fillRRect': __canvas_fill_rrect(id, args[0], args[1], args[2], args[3], args[4]); break;
            case 'strokeRect': __canvas_stroke_rect(id, args[0], args[1], args[2], args[3], String(args[4]), args[5]); break;
            case 'strokeRRect': __canvas_stroke_rrect(id, args[0], args[1], args[2], args[3], args[4]); break;
            case 'beginPath': __canvas_begin_path(id); break;
            case 'moveTo': __canvas_move_to(id, args[0], args[1]); break;
            case 'lineTo': __canvas_line_to(id, args[0], args[1]); break;
            case 'quadTo':
            case 'quadraticCurveTo': __canvas_quad_to(id, args[0], args[1], args[2], args[3]); break;
            case 'bezierTo':
            case 'bezierCurveTo': __canvas_cubic_to(id, args[0], args[1], args[2], args[3], args[4], args[5]); break;
            case 'closePath': __canvas_close_path(id); break;
            case 'fill': __canvas_fill_path(id); break;
            case 'stroke': __canvas_stroke_path(id); break;
            case 'drawImage': {
                const fit = args.length >= 6 && args[5] != null ? String(args[5]) : 'contain';
                __canvas_draw_image(id, String(args[0]), args[1], args[2], args[3], args[4], fit);
                break;
            }
        }
    }

    ctx.getCanvas = function() {
        const id = ctx.__currentCanvasTarget;
        if (!id) {
            return null;
        }
        if (!canvasCache[id]) {
            let api = null;
            api = new Proxy({}, {
                get(target, prop) {
                    if (typeof prop !== 'string' || prop === 'then') {
                        return undefined;
                    }
                    return (...args) => {
                        applyCanvasMutation(id, prop, ...args);
                        return api;
                    };
                }
            });
            canvasCache[id] = api;
        }
        return canvasCache[id];
    };
})();
"#;

pub(super) fn install_canvaskit_bindings<'js>(
    ctx: &rquickjs::Ctx<'js>,
    store: &MutationStore,
) -> anyhow::Result<()> {
    let globals = ctx.globals();

    macro_rules! push_canvas_command {
        ($name:literal, |$id:ident $(, $arg:ident : $arg_ty:ty)*| $command:expr) => {{
            let s = store.clone();
            globals.set(
                $name,
                Function::new(ctx.clone(), move |$id: String $(, $arg: $arg_ty)*| {
                    let mut map = s.lock().unwrap();
                    map.canvases
                        .entry($id)
                        .or_default()
                        .commands
                        .push($command);
                    Ok::<_, rquickjs::Error>(())
                })?,
            )?;
        }};
    }

    push_canvas_command!("__canvas_save", |id| CanvasCommand::Save);
    push_canvas_command!("__canvas_restore", |id| CanvasCommand::Restore);
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
        |id, x: f32, y: f32, width: f32, height: f32| CanvasCommand::ClipRect {
            x,
            y,
            width,
            height
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
    push_canvas_command!("__canvas_fill_path", |id| CanvasCommand::FillPath);
    push_canvas_command!("__canvas_stroke_path", |id| CanvasCommand::StrokePath);

    let s = store.clone();
    globals.set(
        "__canvas_set_fill_style",
        Function::new(ctx.clone(), move |id: String, color: String| {
            let color = parse_color(&color, "setFillStyle")?;
            let mut map = s.lock().unwrap();
            map.canvases
                .entry(id)
                .or_default()
                .commands
                .push(CanvasCommand::SetFillStyle { color });
            Ok::<_, rquickjs::Error>(())
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_set_stroke_style",
        Function::new(ctx.clone(), move |id: String, color: String| {
            let color = parse_color(&color, "setStrokeStyle")?;
            let mut map = s.lock().unwrap();
            map.canvases
                .entry(id)
                .or_default()
                .commands
                .push(CanvasCommand::SetStrokeStyle { color });
            Ok::<_, rquickjs::Error>(())
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_set_line_width",
        Function::new(ctx.clone(), move |id: String, width: f32| {
            let mut map = s.lock().unwrap();
            map.canvases
                .entry(id)
                .or_default()
                .commands
                .push(CanvasCommand::SetLineWidth {
                    width: width.max(0.0),
                });
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_set_line_cap",
        Function::new(ctx.clone(), move |id: String, cap: String| {
            let cap = line_cap_from_name(&cap)
                .ok_or_else(|| js_error("setLineCap", format!("unsupported line cap `{cap}`")))?;
            let mut map = s.lock().unwrap();
            map.canvases
                .entry(id)
                .or_default()
                .commands
                .push(CanvasCommand::SetLineCap { cap });
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
            let mut map = s.lock().unwrap();
            map.canvases
                .entry(id)
                .or_default()
                .commands
                .push(CanvasCommand::SetLineJoin { join });
            Ok::<_, rquickjs::Error>(())
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_set_global_alpha",
        Function::new(ctx.clone(), move |id: String, alpha: f32| {
            let mut map = s.lock().unwrap();
            map.canvases
                .entry(id)
                .or_default()
                .commands
                .push(CanvasCommand::SetGlobalAlpha {
                    alpha: alpha.clamp(0.0, 1.0),
                });
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
            let mut map = s.lock().unwrap();
            map.canvases
                .entry(id)
                .or_default()
                .commands
                .push(CanvasCommand::Clear { color });
            Ok::<_, rquickjs::Error>(())
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_fill_rect",
        Function::new(
            ctx.clone(),
            move |id: String, x: f32, y: f32, width: f32, height: f32, color: String| {
                let color = parse_color(&color, "fillRect")?;
                let mut map = s.lock().unwrap();
                map.canvases
                    .entry(id)
                    .or_default()
                    .commands
                    .push(CanvasCommand::FillRect {
                        x,
                        y,
                        width,
                        height,
                        color,
                    });
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
                let mut map = s.lock().unwrap();
                map.canvases
                    .entry(id)
                    .or_default()
                    .commands
                    .push(CanvasCommand::StrokeRect {
                        x,
                        y,
                        width,
                        height,
                        color,
                        stroke_width: stroke_width.max(0.0),
                    });
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
                  x: f32,
                  y: f32,
                  width: f32,
                  height: f32,
                  fit: String| {
                let object_fit = object_fit_from_name(&fit).ok_or_else(|| {
                    js_error("drawImage", format!("unsupported objectFit `{fit}`"))
                })?;
                let mut map = s.lock().unwrap();
                map.canvases
                    .entry(id)
                    .or_default()
                    .commands
                    .push(CanvasCommand::DrawImage {
                        asset_id,
                        x,
                        y,
                        width,
                        height,
                        object_fit,
                    });
                Ok::<_, rquickjs::Error>(())
            },
        )?,
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
