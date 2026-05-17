//! ── All JS bindings in one place ─────────────────────────────────────
//!
//! Every JS → Rust binding used by the script engine is defined here.
//! To add a new binding, write ONE entry below in the right section.
//! The core dispatcher (`script::dispatch::dispatch_binding`) handles the
//! rest automatically; engine / web only register a single native entry
//! point and route by name through this table.
//!
//! ── Four categories ──────────────────────────────────────────────────
//!
//! | category | body injection         | typical use                          |
//! |----------|------------------------|--------------------------------------|
//! | `node`   | `$rec`, `$id`          | set a style property on a node       |
//! | `cmd`    | `$store: &mut ...`     | mutate store state (animate/morph)   |
//! | `qry`    | `$store: &...`         | read store state, return a value     |
//! | `pure`   | —                      | no store, pure computation           |
//!
//! ── Body rules ───────────────────────────────────────────────────────
//!
//! **node**  — body evaluates to `()` (use `.into_anyhow()` internally).
//!             Use `return Err(anyhow::anyhow!(...))` for early errors.
//!
//! **cmd/qry/pure** — body must evaluate to `anyhow::Result<T>`.
//!             Infallible: `Ok(value)`
//!             Fallible:  `expr.ok_or_else(|| anyhow::anyhow!(...))`
//!
//! **All** — `$crate::` resolves to `opencat_core`.
//!           Inside the body, `?` works for `anyhow::Error` conversions.
//!
//! ── How to add a new binding ─────────────────────────────────────────
//!
//! 1. Find the right section below (node / cmd / qry / pure)
//! 2. Copy an existing line and fill in the name, params, and body
//! 3. Make sure the engine's `bindings/mod.rs` imports any types/fns used
//!
//! Examples:
//! ```ignore
//! // node — simplest form (no braces, single expression)
//! $binding! { node $rec $id record_foo ($id: &str, v: f32) $rec . record_foo($id, v) }
//!
//! // node — with logic (braces, multi-statement)
//! $binding! { node $rec $id record_foo ($id: &str, v: String) {
//!     let parsed = parse_foo(&v);
//!     $rec . record_foo($id, parsed);
//! }}
//!
//! // cmd — returns a value
//! $binding! { cmd $store do_something (x: f32) -> i32 {
//!     Ok($store.do_something(x))
//! }}
//!
//! // qry — read-only, returns a value
//! $binding! { qry $store get_something (handle: i32) -> f32 {
//!     Ok($store.get_something(handle))
//! }}
//!
//! // pure — no store at all
//! $binding! { pure compute_value (input: f32) -> f32 {
//!     Ok(input * 2.0)
//! }}
//! ```
//!
//! Usage in engine:
//! ```ignore
//! for_each_binding!($rec $id $store $binding);
//! ```

#[macro_export]
macro_rules! for_each_binding {
    ($rec:ident $id:ident $store:ident $binding:ident) => {
        // ── Node: style mutations (111 entries) ────────────────────────────

        $binding! { node $rec $id record_opacity ($id: &str, v: f32) $rec . record_opacity($id, v) }
        $binding! { node $rec $id record_translate_x ($id: &str, v: f32) $rec . record_translate_x($id, v) }
        $binding! { node $rec $id record_translate_y ($id: &str, v: f32) $rec . record_translate_y($id, v) }
        $binding! { node $rec $id record_translate ($id: &str, x: f32, y: f32) $rec . record_translate($id, x, y) }
        $binding! { node $rec $id record_scale ($id: &str, v: f32) $rec . record_scale($id, v) }
        $binding! { node $rec $id record_scale_x ($id: &str, v: f32) $rec . record_scale_x($id, v) }
        $binding! { node $rec $id record_scale_y ($id: &str, v: f32) $rec . record_scale_y($id, v) }
        $binding! { node $rec $id record_rotate ($id: &str, v: f32) $rec . record_rotate($id, v) }
        $binding! { node $rec $id record_skew_x ($id: &str, v: f32) $rec . record_skew_x($id, v) }
        $binding! { node $rec $id record_skew_y ($id: &str, v: f32) $rec . record_skew_y($id, v) }
        $binding! { node $rec $id record_skew ($id: &str, x_deg: f32, y_deg: f32) $rec . record_skew($id, x_deg, y_deg) }
        $binding! { node $rec $id record_position ($id: &str, v: String) {
            if let Some(pos) = position_from_name(&v) {
                $rec . record_position($id, pos);
            }
        }}
        $binding! { node $rec $id record_left ($id: &str, v: f32) $rec . record_left($id, v) }
        $binding! { node $rec $id record_top ($id: &str, v: f32) $rec . record_top($id, v) }
        $binding! { node $rec $id record_right ($id: &str, v: f32) $rec . record_right($id, v) }
        $binding! { node $rec $id record_bottom ($id: &str, v: f32) $rec . record_bottom($id, v) }
        $binding! { node $rec $id record_width ($id: &str, v: f32) $rec . record_width($id, v) }
        $binding! { node $rec $id record_height ($id: &str, v: f32) $rec . record_height($id, v) }
        $binding! { node $rec $id record_padding ($id: &str, v: f32) $rec . record_padding($id, v) }
        $binding! { node $rec $id record_padding_x ($id: &str, v: f32) $rec . record_padding_x($id, v) }
        $binding! { node $rec $id record_padding_y ($id: &str, v: f32) $rec . record_padding_y($id, v) }
        $binding! { node $rec $id record_margin ($id: &str, v: f32) $rec . record_margin($id, v) }
        $binding! { node $rec $id record_margin_x ($id: &str, v: f32) $rec . record_margin_x($id, v) }
        $binding! { node $rec $id record_margin_y ($id: &str, v: f32) $rec . record_margin_y($id, v) }
        $binding! { node $rec $id record_flex_direction ($id: &str, v: String) {
            if let Some(fd) = flex_direction_from_name(&v) {
                $rec . record_flex_direction($id, fd);
            }
        }}
        $binding! { node $rec $id record_justify_content ($id: &str, v: String) {
            if let Some(jc) = justify_content_from_name(&v) {
                $rec . record_justify_content($id, jc);
            }
        }}
        $binding! { node $rec $id record_align_items ($id: &str, v: String) {
            if let Some(ai) = align_items_from_name(&v) {
                $rec . record_align_items($id, ai);
            }
        }}
        $binding! { node $rec $id record_gap ($id: &str, v: f32) $rec . record_gap($id, v) }
        $binding! { node $rec $id record_flex_grow ($id: &str, v: f32) $rec . record_flex_grow($id, v) }
        $binding! { node $rec $id record_bg ($id: &str, v: String) {
            if let Some(c) = color_token_from_script_string(&v) {
                $rec . record_bg_color($id, c);
            }
        }}
        $binding! { node $rec $id record_border_radius ($id: &str, v: f32) $rec . record_border_radius($id, v) }
        $binding! { node $rec $id record_border_width ($id: &str, v: f32) $rec . record_border_width($id, v) }
        $binding! { node $rec $id record_border_top_width ($id: &str, v: f32) $rec . record_border_top_width($id, v) }
        $binding! { node $rec $id record_border_right_width ($id: &str, v: f32) $rec . record_border_right_width($id, v) }
        $binding! { node $rec $id record_border_bottom_width ($id: &str, v: f32) $rec . record_border_bottom_width($id, v) }
        $binding! { node $rec $id record_border_left_width ($id: &str, v: f32) $rec . record_border_left_width($id, v) }
        $binding! { node $rec $id record_border_style ($id: &str, v: String) {
            let parsed = match v.as_str() {
                "solid" => Some(BorderStyle::Solid),
                "dashed" => Some(BorderStyle::Dashed),
                "dotted" => Some(BorderStyle::Dotted),
                _ => None,
            };
            if let Some(bs) = parsed {
                $rec . record_border_style($id, bs);
            }
        }}
        $binding! { node $rec $id record_border_color ($id: &str, v: String) {
            if let Some(c) = color_token_from_script_string(&v) {
                $rec . record_border_color($id, c);
            }
        }}
        $binding! { node $rec $id record_stroke_width ($id: &str, v: f32) $rec . record_stroke_width($id, v) }
        $binding! { node $rec $id record_stroke_dasharray ($id: &str, v: f32) $rec . record_stroke_dasharray($id, v) }
        $binding! { node $rec $id record_stroke_dashoffset ($id: &str, v: f32) $rec . record_stroke_dashoffset($id, v) }
        $binding! { node $rec $id record_stroke_color ($id: &str, v: String) {
            if let Some(c) = color_token_from_script_string(&v) {
                $rec . record_stroke_color($id, c);
            }
        }}
        $binding! { node $rec $id record_fill_color ($id: &str, v: String) {
            if let Some(c) = color_token_from_script_string(&v) {
                $rec . record_fill_color($id, c);
            }
        }}
        $binding! { node $rec $id record_object_fit ($id: &str, v: String) {
            if let Some(of) = object_fit_from_name(&v) {
                $rec . record_object_fit($id, of);
            }
        }}
        $binding! { node $rec $id record_text_color ($id: &str, v: String) {
            if let Some(c) = color_token_from_script_string(&v) {
                $rec . record_text_color($id, c);
            }
        }}
        $binding! { node $rec $id record_text_size ($id: &str, v: f32) $rec . record_text_size($id, v) }
        $binding! { node $rec $id record_font_weight ($id: &str, v: f64) {
            $rec . record_font_weight($id, FontWeight(v as u16));
        }}
        $binding! { node $rec $id record_letter_spacing ($id: &str, v: f32) $rec . record_letter_spacing($id, v) }
        $binding! { node $rec $id record_text_align ($id: &str, v: String) {
            if let Some(align) = text_align_from_name(&v) {
                $rec . record_text_align($id, align);
            }
        }}
        $binding! { node $rec $id record_line_height ($id: &str, v: f32) $rec . record_line_height($id, v) }
        $binding! { node $rec $id record_shadow ($id: &str, v: String) {
            if let Some(sh) = box_shadow_from_name(&v) {
                $rec . record_box_shadow($id, sh);
            }
        }}
        $binding! { node $rec $id record_shadow_color ($id: &str, v: String) {
            if let Some(color) = color_token_from_script_string(&v) {
                $rec . record_box_shadow_color($id, color);
            }
        }}
        $binding! { node $rec $id record_inset_shadow ($id: &str, v: String) {
            if let Some(sh) = inset_shadow_from_name(&v) {
                $rec . record_inset_shadow($id, sh);
            }
        }}
        $binding! { node $rec $id record_inset_shadow_color ($id: &str, v: String) {
            if let Some(color) = color_token_from_script_string(&v) {
                $rec . record_inset_shadow_color($id, color);
            }
        }}
        $binding! { node $rec $id record_drop_shadow ($id: &str, v: String) {
            if let Some(sh) = drop_shadow_from_name(&v) {
                $rec . record_drop_shadow($id, sh);
            }
        }}
        $binding! { node $rec $id record_drop_shadow_color ($id: &str, v: String) {
            if let Some(color) = color_token_from_script_string(&v) {
                $rec . record_drop_shadow_color($id, color);
            }
        }}
        $binding! { node $rec $id record_text_content ($id: &str, v: String) $rec . record_text_content($id, v) }
        $binding! { node $rec $id record_svg_path ($id: &str, v: String) $rec . record_svg_path($id, v) }

        // ── Node: canvas commands (53 entries) ─────────────────────────────

        $binding! { node $rec $id canvas_save ($id: &str) {
            $rec . record_canvas_command($id, CanvasCommand::Save);
        }}
        $binding! { node $rec $id canvas_restore ($id: &str) {
            $rec . record_canvas_command($id, CanvasCommand::Restore);
        }}
        $binding! { node $rec $id canvas_restore_to_count ($id: &str, count: i32) {
            $rec . record_canvas_command($id, CanvasCommand::RestoreToCount { count: count.max(1) });
        }}
        $binding! { node $rec $id canvas_translate ($id: &str, x: f32, y: f32) {
            $rec . record_canvas_command($id, CanvasCommand::Translate { x, y });
        }}
        $binding! { node $rec $id canvas_scale ($id: &str, x: f32, y: f32) {
            $rec . record_canvas_command($id, CanvasCommand::Scale { x, y });
        }}
        $binding! { node $rec $id canvas_rotate ($id: &str, degrees: f32) {
            $rec . record_canvas_command($id, CanvasCommand::Rotate { degrees });
        }}
        $binding! { node $rec $id canvas_clip_rect ($id: &str, x: f32, y: f32, width: f32, height: f32, anti_alias: bool) {
            $rec . record_canvas_command($id, CanvasCommand::ClipRect { x, y, width, height, anti_alias });
        }}
        $binding! { node $rec $id canvas_draw_line ($id: &str, x0: f32, y0: f32, x1: f32, y1: f32) {
            $rec . record_canvas_command($id, CanvasCommand::DrawLine { x0, y0, x1, y1 });
        }}
        $binding! { node $rec $id canvas_fill_circle ($id: &str, cx: f32, cy: f32, radius: f32) {
            $rec . record_canvas_command($id, CanvasCommand::FillCircle { cx, cy, radius });
        }}
        $binding! { node $rec $id canvas_stroke_circle ($id: &str, cx: f32, cy: f32, radius: f32) {
            $rec . record_canvas_command($id, CanvasCommand::StrokeCircle { cx, cy, radius });
        }}
        $binding! { node $rec $id canvas_fill_rrect ($id: &str, x: f32, y: f32, width: f32, height: f32, radius: f32) {
            $rec . record_canvas_command($id, CanvasCommand::FillRRect { x, y, width, height, radius });
        }}
        $binding! { node $rec $id canvas_stroke_rrect ($id: &str, x: f32, y: f32, width: f32, height: f32, radius: f32) {
            $rec . record_canvas_command($id, CanvasCommand::StrokeRRect { x, y, width, height, radius });
        }}
        $binding! { node $rec $id canvas_begin_path ($id: &str) {
            $rec . record_canvas_command($id, CanvasCommand::BeginPath);
        }}
        $binding! { node $rec $id canvas_move_to ($id: &str, x: f32, y: f32) {
            $rec . record_canvas_command($id, CanvasCommand::MoveTo { x, y });
        }}
        $binding! { node $rec $id canvas_line_to ($id: &str, x: f32, y: f32) {
            $rec . record_canvas_command($id, CanvasCommand::LineTo { x, y });
        }}
        $binding! { node $rec $id canvas_quad_to ($id: &str, cx: f32, cy: f32, x: f32, y: f32) {
            $rec . record_canvas_command($id, CanvasCommand::QuadTo { cx, cy, x, y });
        }}
        $binding! { node $rec $id canvas_cubic_to ($id: &str, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
            $rec . record_canvas_command($id, CanvasCommand::CubicTo { c1x, c1y, c2x, c2y, x, y });
        }}
        $binding! { node $rec $id canvas_close_path ($id: &str) {
            $rec . record_canvas_command($id, CanvasCommand::ClosePath);
        }}
        $binding! { node $rec $id canvas_path_add_rect ($id: &str, x: f32, y: f32, width: f32, height: f32) {
            $rec . record_canvas_command($id, CanvasCommand::AddRectPath { x, y, width, height });
        }}
        $binding! { node $rec $id canvas_path_add_rrect ($id: &str, x: f32, y: f32, width: f32, height: f32, radius: f32) {
            $rec . record_canvas_command($id, CanvasCommand::AddRRectPath { x, y, width, height, radius });
        }}
        $binding! { node $rec $id canvas_path_add_oval ($id: &str, x: f32, y: f32, width: f32, height: f32) {
            $rec . record_canvas_command($id, CanvasCommand::AddOvalPath { x, y, width, height });
        }}
        $binding! { node $rec $id canvas_path_add_arc ($id: &str, x: f32, y: f32, width: f32, height: f32, start_angle: f32, sweep_angle: f32) {
            $rec . record_canvas_command($id, CanvasCommand::AddArcPath { x, y, width, height, start_angle, sweep_angle });
        }}
        $binding! { node $rec $id canvas_fill_path ($id: &str) {
            $rec . record_canvas_command($id, CanvasCommand::FillPath);
        }}
        $binding! { node $rec $id canvas_stroke_path ($id: &str) {
            $rec . record_canvas_command($id, CanvasCommand::StrokePath);
        }}
        $binding! { node $rec $id canvas_stroke_arc ($id: &str, cx: f32, cy: f32, rx: f32, ry: f32, start_angle: f32, sweep_angle: f32) {
            $rec . record_canvas_command($id, CanvasCommand::StrokeArc { cx, cy, rx, ry, start_angle, sweep_angle });
        }}
        $binding! { node $rec $id canvas_fill_oval ($id: &str, cx: f32, cy: f32, rx: f32, ry: f32) {
            $rec . record_canvas_command($id, CanvasCommand::FillOval { cx, cy, rx, ry });
        }}
        $binding! { node $rec $id canvas_stroke_oval ($id: &str, cx: f32, cy: f32, rx: f32, ry: f32) {
            $rec . record_canvas_command($id, CanvasCommand::StrokeOval { cx, cy, rx, ry });
        }}
        $binding! { node $rec $id canvas_clip_path ($id: &str, anti_alias: bool) {
            $rec . record_canvas_command($id, CanvasCommand::ClipPath { anti_alias });
        }}
        $binding! { node $rec $id canvas_clip_rrect ($id: &str, x: f32, y: f32, width: f32, height: f32, radius: f32, anti_alias: bool) {
            $rec . record_canvas_command($id, CanvasCommand::ClipRRect { x, y, width, height, radius, anti_alias });
        }}
        $binding! { node $rec $id canvas_skew ($id: &str, sx: f32, sy: f32) {
            $rec . record_canvas_command($id, CanvasCommand::Skew { sx, sy });
        }}
        $binding! { node $rec $id canvas_draw_image_simple ($id: &str, asset_id: String, x: f32, y: f32, alpha: f32, anti_alias: bool) {
            $rec . record_canvas_command($id, CanvasCommand::DrawImageSimple {
                asset_id,
                x,
                y,
                alpha: alpha.clamp(0.0, 1.0),
                anti_alias,
            });
        }}
        $binding! { node $rec $id canvas_save_layer ($id: &str, alpha: f32, bounds: Option<Vec<f32>>) {
            let bounds = match bounds {
                Some(b) => Some($crate::script::helpers::parse_image_rect("saveLayer", &b)?),
                None => None,
            };
            $rec . record_canvas_command($id, CanvasCommand::SaveLayer {
                alpha: alpha.clamp(0.0, 1.0),
                bounds,
            });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_set_fill_style ($id: &str, color: String) {
            let color = $crate::script::helpers::parse_color(&color, "setFillStyle")?;
            $rec . record_canvas_command($id, CanvasCommand::SetFillStyle { color });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_set_stroke_style ($id: &str, color: String) {
            let color = $crate::script::helpers::parse_color(&color, "setStrokeStyle")?;
            $rec . record_canvas_command($id, CanvasCommand::SetStrokeStyle { color });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_set_line_width ($id: &str, width: f32) {
            $rec . record_canvas_command($id, CanvasCommand::SetLineWidth { width: width.max(0.0) });
        }}
        $binding! { node $rec $id canvas_set_line_cap ($id: &str, cap: String) {
            let cap = line_cap_from_name(&cap)
                .ok_or_else(|| $crate::script::helpers::script_error("setLineCap", format!("unsupported line cap `{cap}`")))?;
            $rec . record_canvas_command($id, CanvasCommand::SetLineCap { cap });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_set_line_join ($id: &str, join: String) {
            let join = line_join_from_name(&join)
                .ok_or_else(|| $crate::script::helpers::script_error("setLineJoin", format!("unsupported line join `{join}`")))?;
            $rec . record_canvas_command($id, CanvasCommand::SetLineJoin { join });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_set_line_dash ($id: &str, intervals: Vec<f32>, phase: f32) {
            $rec . record_canvas_command($id, CanvasCommand::SetLineDash { intervals, phase });
        }}
        $binding! { node $rec $id canvas_clear_line_dash ($id: &str) {
            $rec . record_canvas_command($id, CanvasCommand::ClearLineDash);
        }}
        $binding! { node $rec $id canvas_set_global_alpha ($id: &str, alpha: f32) {
            $rec . record_canvas_command($id, CanvasCommand::SetGlobalAlpha { alpha: alpha.clamp(0.0, 1.0) });
        }}
        $binding! { node $rec $id canvas_set_anti_alias ($id: &str, enabled: bool) {
            $rec . record_canvas_command($id, CanvasCommand::SetAntiAlias { enabled });
        }}
        $binding! { node $rec $id canvas_clear ($id: &str, color: Option<String>) {
            let color = match color {
                Some(c) => Some($crate::script::helpers::parse_color(&c, "clear")?),
                None => None,
            };
            $rec . record_canvas_command($id, CanvasCommand::Clear { color });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_draw_paint ($id: &str, color: String, anti_alias: bool) {
            let color = $crate::script::helpers::parse_color(&color, "drawPaint")?;
            $rec . record_canvas_command($id, CanvasCommand::DrawPaint { color, anti_alias });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_draw_text ($id: &str, text: String, values: Vec<f32>, color: String, flags: Vec<bool>, font_edging: String) {
            if values.len() < 6 {
                Err($crate::script::helpers::script_error("drawText", "expected text values [x, y, fontSize, scaleX, skewX, strokeWidth]".to_string()))
            } else if flags.len() < 3 {
                Err($crate::script::helpers::script_error("drawText", "expected text flags [antiAlias, stroke, fontSubpixel]".to_string()))
            } else {
                let color = $crate::script::helpers::parse_color(&color, "drawText")?;
                let font_edging = font_edging_from_name(&font_edging)
                    .ok_or_else(|| $crate::script::helpers::script_error("drawText", format!("unsupported font edging `{font_edging}`")))?;
                $rec . record_canvas_command($id, CanvasCommand::DrawText {
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
                });
                Ok::<_, anyhow::Error>(())
            }
        }}
        $binding! { node $rec $id canvas_fill_rect ($id: &str, x: f32, y: f32, width: f32, height: f32, color: String) {
            let color = $crate::script::helpers::parse_color(&color, "fillRect")?;
            $rec . record_canvas_command($id, CanvasCommand::FillRect { x, y, width, height, color });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_stroke_rect ($id: &str, x: f32, y: f32, width: f32, height: f32, color: String, stroke_width: f32) {
            let color = $crate::script::helpers::parse_color(&color, "strokeRect")?;
            $rec . record_canvas_command($id, CanvasCommand::StrokeRect {
                x,
                y,
                width,
                height,
                color,
                stroke_width: stroke_width.max(0.0),
            });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_draw_image ($id: &str, asset_id: String, values: Vec<f32>, fit: String, alpha: f32, anti_alias: bool) {
            let object_fit = object_fit_from_name(&fit)
                .ok_or_else(|| $crate::script::helpers::script_error("drawImage", format!("unsupported objectFit `{fit}`")))?;
            let src_rect = if values.len() < 4 {
                Err($crate::script::helpers::script_error("drawImageRect", "expected destination rect as [x, y, width, height]".to_string()))
            } else {
                match values.len() {
                    4 => Ok(None),
                    8.. => Ok(Some($crate::script::helpers::parse_image_rect("drawImageRect", &values[4..8])?)),
                    _ => Err($crate::script::helpers::script_error("drawImageRect", "expected either 4 or 8 image rect values".to_string())),
                }
            }?;
            $rec . record_canvas_command($id, CanvasCommand::DrawImage {
                asset_id,
                x: values[0],
                y: values[1],
                width: values[2],
                height: values[3],
                src_rect,
                alpha: alpha.clamp(0.0, 1.0),
                anti_alias,
                object_fit,
            });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_draw_arc ($id: &str, cx: f32, cy: f32, rx: f32, ry: f32, start_angle: f32, sweep_angle: f32) {
            $rec . record_canvas_command($id, CanvasCommand::DrawArc {
                cx, cy, rx, ry, start_angle, sweep_angle, use_center: false,
            });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_draw_arc_to_center ($id: &str, cx: f32, cy: f32, rx: f32, ry: f32, start_angle: f32, sweep_angle: f32) {
            $rec . record_canvas_command($id, CanvasCommand::DrawArc {
                cx, cy, rx, ry, start_angle, sweep_angle, use_center: true,
            });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_draw_points ($id: &str, mode: String, points: Vec<f32>) {
            let mode = point_mode_from_name(&mode)
                .ok_or_else(|| $crate::script::helpers::script_error("drawPoints", format!("unsupported point mode `{mode}`")))?;
            $rec . record_canvas_command($id, CanvasCommand::DrawPoints { mode, points });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_fill_drrect ($id: &str, coords: Vec<f32>) {
            let (outer_x, outer_y, outer_width, outer_height, outer_radius,
                 inner_x, inner_y, inner_width, inner_height, inner_radius) =
                $crate::script::helpers::parse_drrect("fillDRRect", &coords)?;
            $rec . record_canvas_command($id, CanvasCommand::FillDRRect {
                outer_x, outer_y, outer_width, outer_height, outer_radius,
                inner_x, inner_y, inner_width, inner_height, inner_radius,
            });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_stroke_drrect ($id: &str, coords: Vec<f32>) {
            let (outer_x, outer_y, outer_width, outer_height, outer_radius,
                 inner_x, inner_y, inner_width, inner_height, inner_radius) =
                $crate::script::helpers::parse_drrect("strokeDRRect", &coords)?;
            $rec . record_canvas_command($id, CanvasCommand::StrokeDRRect {
                outer_x, outer_y, outer_width, outer_height, outer_radius,
                inner_x, inner_y, inner_width, inner_height, inner_radius,
            });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_concat ($id: &str, values: Vec<f32>) {
            if values.len() < 9 {
                Err($crate::script::helpers::script_error("concat", "expected 9 matrix values".to_string()))
            } else {
                let matrix = [
                    values[0], values[1], values[2],
                    values[3], values[4], values[5],
                    values[6], values[7], values[8],
                ];
                $rec . record_canvas_command($id, CanvasCommand::Concat { matrix });
                Ok::<_, anyhow::Error>(())
            }
        }}

        // ── Node: text unit overrides (complex Object destructuring) ──────
        $binding! { node $rec $id record_text_unit_override ($id: &str, granularity: String, index: u32, values: serde_json::Map<String, serde_json::Value>) {
            let index = index as usize;
            let gran = match granularity.as_str() {
                "graphemes" => TextUnitGranularity::Grapheme,
                "words" => TextUnitGranularity::Word,
                _ => return Err(anyhow::anyhow!("unsupported granularity")),
            };
            let opacity = values.get("opacity").and_then(|v| v.as_f64());
            let translate_x = values.get("translateX").and_then(|v| v.as_f64());
            let translate_y = values.get("translateY").and_then(|v| v.as_f64());
            let scale = values.get("scale").and_then(|v| v.as_f64());
            let rotation_deg = values.get("rotation").and_then(|v| v.as_f64());
            let color = values.get("textColor").and_then(|v| v.as_str()).map(String::from)
                .or_else(|| values.get("color").and_then(|v| v.as_str()).map(String::from));
            $rec.record_text_unit_override(
                $id,
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
        }}

        // ── Cmd: store mutations (5 entries: animate, morph, along_path) ──
        $binding! { cmd $store animate_create (duration: f32, delay: f32, clamp_flag: i32, easing_tag: String, repeat: i32, yoyo_flag: i32, repeat_delay: f32) -> i32 {
            let clamp = clamp_flag != 0;
            let yoyo = yoyo_flag != 0;
            let cf = $store.current_frame();
            Ok($store.animate_create(cf, duration, delay, clamp, &easing_tag, repeat, yoyo, repeat_delay))
        }}
        $binding! { cmd $store morph_svg_create (from_svg: String, to_svg: String, grid_size: f32) -> i32 {
            Ok($store.morph_svg_create(&from_svg, &to_svg, grid_size as u32).unwrap_or(-1))
        }}
        $binding! { cmd $store morph_svg_dispose (handle: i32) -> () {
            $store.morph_svg_dispose(handle);
            Ok(())
        }}
        $binding! { cmd $store along_path_create (svg: String) -> i32 {
            $store.along_path_create(&svg).ok_or_else(|| anyhow::anyhow!("invalid SVG path"))
        }}
        $binding! { cmd $store along_path_dispose (handle: i32) -> () {
            $store.along_path_dispose(handle);
            Ok(())
        }}

        // ── Qry: store reads (10 entries: animate, morph, text, along_path) ─
        $binding! { qry $store animate_value (handle: i32, _key: String, from: f32, to: f32) -> f32 {
            let cf = $store.current_frame();
            Ok($store.animate_value(cf, handle, from, to))
        }}
        $binding! { qry $store animate_color (handle: i32, _key: String, from: String, to: String) -> String {
            Ok($store.animate_color(handle, &from, &to))
        }}
        $binding! { qry $store animate_progress (handle: i32) -> f32 {
            Ok($store.animate_progress(handle))
        }}
        $binding! { qry $store animate_settled (handle: i32) -> bool {
            Ok($store.animate_settled(handle))
        }}
        $binding! { qry $store animate_settle_frame (handle: i32) -> u32 {
            Ok($store.animate_settle_frame(handle))
        }}
        $binding! { qry $store morph_svg_sample (handle: i32, t: f32, tolerance: f32) -> String {
            Ok($store.morph_svg_sample(handle, t, tolerance))
        }}
        $binding! { qry $store along_path_length (handle: i32) -> f32 {
            Ok($store.along_path_length(handle))
        }}
        $binding! { qry $store along_path_at (handle: i32, t: f32) -> Vec<f32> {
            let (x, y, angle) = $store.along_path_at(handle, t);
            Ok(vec![x, y, angle])
        }}
        $binding! { qry $store text_units_describe (id: String, granularity_str: String) -> Vec<(u32, String, u32, u32)> {
            let text = $store.get_text_source(&id).map(|src| src.text.clone())
                .ok_or_else(|| anyhow::anyhow!("no text source found for node"))?;
            let granularity = match granularity_str.as_str() {
                "graphemes" => TextUnitGranularity::Grapheme,
                "words" => TextUnitGranularity::Word,
                _ => return Err(anyhow::anyhow!("unknown granularity; expected 'graphemes' or 'words'")),
            };
            Ok(describe_text_units(&text, granularity)
                .into_iter()
                .map(|u| (u.index as u32, u.text, u.start as u32, u.end as u32))
                .collect())
        }}
        $binding! { qry $store text_source_get (id: String) -> Option<String> {
            Ok($store.get_text_source(&id).map(|s| s.text.clone()))
        }}

        // ── Pure: no store (4 entries: text measure, random, graphemes, easing) ─
        $binding! { pure canvas_measure_text (text: String, font_size: f32, font_scale_x: f32, _font_skew_x: f32, _font_subpixel: bool, _font_edging: String) -> f32 {
            Ok(measure_script_text_width(&text, font_size, font_scale_x))
        }}
        $binding! { pure util_random_seeded (seed: f32) -> f32 {
            Ok(random_from_seed(seed))
        }}
        $binding! { pure text_graphemes (text: String) -> Vec<String> {
            Ok(grapheme_strings(&text).into_iter().collect())
        }}
        $binding! { pure easing_apply (tag: String, t: f32) -> f32 {
            let easing = parse_easing_from_tag(&tag);
            Ok(easing.apply(t))
        }}
    };
}
