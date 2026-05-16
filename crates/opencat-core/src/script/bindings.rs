//! Unified JS binding definitions shared between engine and future consumers.
//!
//! The `for_each_binding!` macro enumerates every script binding (node-style
//! mutations + canvas commands) so that consumers can generate their own
//! installation code without duplicating the binding list.
//!
//! Usage:
//! ```ignore
//! for_each_binding!($rec $id $apply);
//! ```
//! Where `$rec` and `$id` are identifiers for the recorder and node-id variables
//! that will be in scope in the body expressions, and `$apply` is the consumer
//! macro name.  Each invocation has the form:
//! ```ignore
//! $apply! { $rec $id name ($id: &str, param: Type, ...) body }
//! ```

#[macro_export]
macro_rules! for_each_binding {
    ($rec:ident $id:ident $apply:ident) => {
        // ── 58 node-style entries ──────────────────────────────────────────

        $apply! { $rec $id record_opacity ($id: &str, v: f32) $rec . record_opacity($id, v) }
        $apply! { $rec $id record_translate_x ($id: &str, v: f32) $rec . record_translate_x($id, v) }
        $apply! { $rec $id record_translate_y ($id: &str, v: f32) $rec . record_translate_y($id, v) }
        $apply! { $rec $id record_translate ($id: &str, x: f32, y: f32) $rec . record_translate($id, x, y) }
        $apply! { $rec $id record_scale ($id: &str, v: f32) $rec . record_scale($id, v) }
        $apply! { $rec $id record_scale_x ($id: &str, v: f32) $rec . record_scale_x($id, v) }
        $apply! { $rec $id record_scale_y ($id: &str, v: f32) $rec . record_scale_y($id, v) }
        $apply! { $rec $id record_rotate ($id: &str, v: f32) $rec . record_rotate($id, v) }
        $apply! { $rec $id record_skew_x ($id: &str, v: f32) $rec . record_skew_x($id, v) }
        $apply! { $rec $id record_skew_y ($id: &str, v: f32) $rec . record_skew_y($id, v) }
        $apply! { $rec $id record_skew ($id: &str, x_deg: f32, y_deg: f32) $rec . record_skew($id, x_deg, y_deg) }
        $apply! { $rec $id record_position ($id: &str, v: String) {
            if let Some(pos) = position_from_name(&v) {
                $rec . record_position($id, pos);
            }
        }}
        $apply! { $rec $id record_left ($id: &str, v: f32) $rec . record_left($id, v) }
        $apply! { $rec $id record_top ($id: &str, v: f32) $rec . record_top($id, v) }
        $apply! { $rec $id record_right ($id: &str, v: f32) $rec . record_right($id, v) }
        $apply! { $rec $id record_bottom ($id: &str, v: f32) $rec . record_bottom($id, v) }
        $apply! { $rec $id record_width ($id: &str, v: f32) $rec . record_width($id, v) }
        $apply! { $rec $id record_height ($id: &str, v: f32) $rec . record_height($id, v) }
        $apply! { $rec $id record_padding ($id: &str, v: f32) $rec . record_padding($id, v) }
        $apply! { $rec $id record_padding_x ($id: &str, v: f32) $rec . record_padding_x($id, v) }
        $apply! { $rec $id record_padding_y ($id: &str, v: f32) $rec . record_padding_y($id, v) }
        $apply! { $rec $id record_margin ($id: &str, v: f32) $rec . record_margin($id, v) }
        $apply! { $rec $id record_margin_x ($id: &str, v: f32) $rec . record_margin_x($id, v) }
        $apply! { $rec $id record_margin_y ($id: &str, v: f32) $rec . record_margin_y($id, v) }
        $apply! { $rec $id record_flex_direction ($id: &str, v: String) {
            if let Some(fd) = flex_direction_from_name(&v) {
                $rec . record_flex_direction($id, fd);
            }
        }}
        $apply! { $rec $id record_justify_content ($id: &str, v: String) {
            if let Some(jc) = justify_content_from_name(&v) {
                $rec . record_justify_content($id, jc);
            }
        }}
        $apply! { $rec $id record_align_items ($id: &str, v: String) {
            if let Some(ai) = align_items_from_name(&v) {
                $rec . record_align_items($id, ai);
            }
        }}
        $apply! { $rec $id record_gap ($id: &str, v: f32) $rec . record_gap($id, v) }
        $apply! { $rec $id record_flex_grow ($id: &str, v: f32) $rec . record_flex_grow($id, v) }
        $apply! { $rec $id record_bg ($id: &str, v: String) {
            if let Some(c) = color_token_from_script_string(&v) {
                $rec . record_bg_color($id, c);
            }
        }}
        $apply! { $rec $id record_border_radius ($id: &str, v: f32) $rec . record_border_radius($id, v) }
        $apply! { $rec $id record_border_width ($id: &str, v: f32) $rec . record_border_width($id, v) }
        $apply! { $rec $id record_border_top_width ($id: &str, v: f32) $rec . record_border_top_width($id, v) }
        $apply! { $rec $id record_border_right_width ($id: &str, v: f32) $rec . record_border_right_width($id, v) }
        $apply! { $rec $id record_border_bottom_width ($id: &str, v: f32) $rec . record_border_bottom_width($id, v) }
        $apply! { $rec $id record_border_left_width ($id: &str, v: f32) $rec . record_border_left_width($id, v) }
        $apply! { $rec $id record_border_style ($id: &str, v: String) {
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
        $apply! { $rec $id record_border_color ($id: &str, v: String) {
            if let Some(c) = color_token_from_script_string(&v) {
                $rec . record_border_color($id, c);
            }
        }}
        $apply! { $rec $id record_stroke_width ($id: &str, v: f32) $rec . record_stroke_width($id, v) }
        $apply! { $rec $id record_stroke_dasharray ($id: &str, v: f32) $rec . record_stroke_dasharray($id, v) }
        $apply! { $rec $id record_stroke_dashoffset ($id: &str, v: f32) $rec . record_stroke_dashoffset($id, v) }
        $apply! { $rec $id record_stroke_color ($id: &str, v: String) {
            if let Some(c) = color_token_from_script_string(&v) {
                $rec . record_stroke_color($id, c);
            }
        }}
        $apply! { $rec $id record_fill_color ($id: &str, v: String) {
            if let Some(c) = color_token_from_script_string(&v) {
                $rec . record_fill_color($id, c);
            }
        }}
        $apply! { $rec $id record_object_fit ($id: &str, v: String) {
            if let Some(of) = object_fit_from_name(&v) {
                $rec . record_object_fit($id, of);
            }
        }}
        $apply! { $rec $id record_text_color ($id: &str, v: String) {
            if let Some(c) = color_token_from_script_string(&v) {
                $rec . record_text_color($id, c);
            }
        }}
        $apply! { $rec $id record_text_size ($id: &str, v: f32) $rec . record_text_size($id, v) }
        $apply! { $rec $id record_font_weight ($id: &str, v: f64) {
            $rec . record_font_weight($id, FontWeight(v as u16));
        }}
        $apply! { $rec $id record_letter_spacing ($id: &str, v: f32) $rec . record_letter_spacing($id, v) }
        $apply! { $rec $id record_text_align ($id: &str, v: String) {
            if let Some(align) = text_align_from_name(&v) {
                $rec . record_text_align($id, align);
            }
        }}
        $apply! { $rec $id record_line_height ($id: &str, v: f32) $rec . record_line_height($id, v) }
        $apply! { $rec $id record_shadow ($id: &str, v: String) {
            if let Some(sh) = box_shadow_from_name(&v) {
                $rec . record_box_shadow($id, sh);
            }
        }}
        $apply! { $rec $id record_shadow_color ($id: &str, v: String) {
            if let Some(color) = color_token_from_script_string(&v) {
                $rec . record_box_shadow_color($id, color);
            }
        }}
        $apply! { $rec $id record_inset_shadow ($id: &str, v: String) {
            if let Some(sh) = inset_shadow_from_name(&v) {
                $rec . record_inset_shadow($id, sh);
            }
        }}
        $apply! { $rec $id record_inset_shadow_color ($id: &str, v: String) {
            if let Some(color) = color_token_from_script_string(&v) {
                $rec . record_inset_shadow_color($id, color);
            }
        }}
        $apply! { $rec $id record_drop_shadow ($id: &str, v: String) {
            if let Some(sh) = drop_shadow_from_name(&v) {
                $rec . record_drop_shadow($id, sh);
            }
        }}
        $apply! { $rec $id record_drop_shadow_color ($id: &str, v: String) {
            if let Some(color) = color_token_from_script_string(&v) {
                $rec . record_drop_shadow_color($id, color);
            }
        }}
        $apply! { $rec $id record_text_content ($id: &str, v: String) $rec . record_text_content($id, v) }
        $apply! { $rec $id record_svg_path ($id: &str, v: String) $rec . record_svg_path($id, v) }

        // ── 53 canvas entries ──────────────────────────────────────────────

        $apply! { $rec $id canvas_save ($id: &str) {
            $rec . record_canvas_command($id, CanvasCommand::Save);
        }}
        $apply! { $rec $id canvas_restore ($id: &str) {
            $rec . record_canvas_command($id, CanvasCommand::Restore);
        }}
        $apply! { $rec $id canvas_restore_to_count ($id: &str, count: i32) {
            $rec . record_canvas_command($id, CanvasCommand::RestoreToCount { count: count.max(1) });
        }}
        $apply! { $rec $id canvas_translate ($id: &str, x: f32, y: f32) {
            $rec . record_canvas_command($id, CanvasCommand::Translate { x, y });
        }}
        $apply! { $rec $id canvas_scale ($id: &str, x: f32, y: f32) {
            $rec . record_canvas_command($id, CanvasCommand::Scale { x, y });
        }}
        $apply! { $rec $id canvas_rotate ($id: &str, degrees: f32) {
            $rec . record_canvas_command($id, CanvasCommand::Rotate { degrees });
        }}
        $apply! { $rec $id canvas_clip_rect ($id: &str, x: f32, y: f32, width: f32, height: f32, anti_alias: bool) {
            $rec . record_canvas_command($id, CanvasCommand::ClipRect { x, y, width, height, anti_alias });
        }}
        $apply! { $rec $id canvas_draw_line ($id: &str, x0: f32, y0: f32, x1: f32, y1: f32) {
            $rec . record_canvas_command($id, CanvasCommand::DrawLine { x0, y0, x1, y1 });
        }}
        $apply! { $rec $id canvas_fill_circle ($id: &str, cx: f32, cy: f32, radius: f32) {
            $rec . record_canvas_command($id, CanvasCommand::FillCircle { cx, cy, radius });
        }}
        $apply! { $rec $id canvas_stroke_circle ($id: &str, cx: f32, cy: f32, radius: f32) {
            $rec . record_canvas_command($id, CanvasCommand::StrokeCircle { cx, cy, radius });
        }}
        $apply! { $rec $id canvas_fill_rrect ($id: &str, x: f32, y: f32, width: f32, height: f32, radius: f32) {
            $rec . record_canvas_command($id, CanvasCommand::FillRRect { x, y, width, height, radius });
        }}
        $apply! { $rec $id canvas_stroke_rrect ($id: &str, x: f32, y: f32, width: f32, height: f32, radius: f32) {
            $rec . record_canvas_command($id, CanvasCommand::StrokeRRect { x, y, width, height, radius });
        }}
        $apply! { $rec $id canvas_begin_path ($id: &str) {
            $rec . record_canvas_command($id, CanvasCommand::BeginPath);
        }}
        $apply! { $rec $id canvas_move_to ($id: &str, x: f32, y: f32) {
            $rec . record_canvas_command($id, CanvasCommand::MoveTo { x, y });
        }}
        $apply! { $rec $id canvas_line_to ($id: &str, x: f32, y: f32) {
            $rec . record_canvas_command($id, CanvasCommand::LineTo { x, y });
        }}
        $apply! { $rec $id canvas_quad_to ($id: &str, cx: f32, cy: f32, x: f32, y: f32) {
            $rec . record_canvas_command($id, CanvasCommand::QuadTo { cx, cy, x, y });
        }}
        $apply! { $rec $id canvas_cubic_to ($id: &str, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
            $rec . record_canvas_command($id, CanvasCommand::CubicTo { c1x, c1y, c2x, c2y, x, y });
        }}
        $apply! { $rec $id canvas_close_path ($id: &str) {
            $rec . record_canvas_command($id, CanvasCommand::ClosePath);
        }}
        $apply! { $rec $id canvas_path_add_rect ($id: &str, x: f32, y: f32, width: f32, height: f32) {
            $rec . record_canvas_command($id, CanvasCommand::AddRectPath { x, y, width, height });
        }}
        $apply! { $rec $id canvas_path_add_rrect ($id: &str, x: f32, y: f32, width: f32, height: f32, radius: f32) {
            $rec . record_canvas_command($id, CanvasCommand::AddRRectPath { x, y, width, height, radius });
        }}
        $apply! { $rec $id canvas_path_add_oval ($id: &str, x: f32, y: f32, width: f32, height: f32) {
            $rec . record_canvas_command($id, CanvasCommand::AddOvalPath { x, y, width, height });
        }}
        $apply! { $rec $id canvas_path_add_arc ($id: &str, x: f32, y: f32, width: f32, height: f32, start_angle: f32, sweep_angle: f32) {
            $rec . record_canvas_command($id, CanvasCommand::AddArcPath { x, y, width, height, start_angle, sweep_angle });
        }}
        $apply! { $rec $id canvas_fill_path ($id: &str) {
            $rec . record_canvas_command($id, CanvasCommand::FillPath);
        }}
        $apply! { $rec $id canvas_stroke_path ($id: &str) {
            $rec . record_canvas_command($id, CanvasCommand::StrokePath);
        }}
        $apply! { $rec $id canvas_stroke_arc ($id: &str, cx: f32, cy: f32, rx: f32, ry: f32, start_angle: f32, sweep_angle: f32) {
            $rec . record_canvas_command($id, CanvasCommand::StrokeArc { cx, cy, rx, ry, start_angle, sweep_angle });
        }}
        $apply! { $rec $id canvas_fill_oval ($id: &str, cx: f32, cy: f32, rx: f32, ry: f32) {
            $rec . record_canvas_command($id, CanvasCommand::FillOval { cx, cy, rx, ry });
        }}
        $apply! { $rec $id canvas_stroke_oval ($id: &str, cx: f32, cy: f32, rx: f32, ry: f32) {
            $rec . record_canvas_command($id, CanvasCommand::StrokeOval { cx, cy, rx, ry });
        }}
        $apply! { $rec $id canvas_clip_path ($id: &str, anti_alias: bool) {
            $rec . record_canvas_command($id, CanvasCommand::ClipPath { anti_alias });
        }}
        $apply! { $rec $id canvas_clip_rrect ($id: &str, x: f32, y: f32, width: f32, height: f32, radius: f32, anti_alias: bool) {
            $rec . record_canvas_command($id, CanvasCommand::ClipRRect { x, y, width, height, radius, anti_alias });
        }}
        $apply! { $rec $id canvas_skew ($id: &str, sx: f32, sy: f32) {
            $rec . record_canvas_command($id, CanvasCommand::Skew { sx, sy });
        }}
        $apply! { $rec $id canvas_draw_image_simple ($id: &str, asset_id: String, x: f32, y: f32, alpha: f32, anti_alias: bool) {
            $rec . record_canvas_command($id, CanvasCommand::DrawImageSimple {
                asset_id,
                x,
                y,
                alpha: alpha.clamp(0.0, 1.0),
                anti_alias,
            });
        }}
        $apply! { $rec $id canvas_save_layer ($id: &str, alpha: f32, bounds: Option<Vec<f32>>) {
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
        $apply! { $rec $id canvas_set_fill_style ($id: &str, color: String) {
            let color = $crate::script::helpers::parse_color(&color, "setFillStyle")?;
            $rec . record_canvas_command($id, CanvasCommand::SetFillStyle { color });
            Ok::<_, anyhow::Error>(())
        }}
        $apply! { $rec $id canvas_set_stroke_style ($id: &str, color: String) {
            let color = $crate::script::helpers::parse_color(&color, "setStrokeStyle")?;
            $rec . record_canvas_command($id, CanvasCommand::SetStrokeStyle { color });
            Ok::<_, anyhow::Error>(())
        }}
        $apply! { $rec $id canvas_set_line_width ($id: &str, width: f32) {
            $rec . record_canvas_command($id, CanvasCommand::SetLineWidth { width: width.max(0.0) });
        }}
        $apply! { $rec $id canvas_set_line_cap ($id: &str, cap: String) {
            let cap = line_cap_from_name(&cap)
                .ok_or_else(|| $crate::script::helpers::script_error("setLineCap", format!("unsupported line cap `{cap}`")))?;
            $rec . record_canvas_command($id, CanvasCommand::SetLineCap { cap });
            Ok::<_, anyhow::Error>(())
        }}
        $apply! { $rec $id canvas_set_line_join ($id: &str, join: String) {
            let join = line_join_from_name(&join)
                .ok_or_else(|| $crate::script::helpers::script_error("setLineJoin", format!("unsupported line join `{join}`")))?;
            $rec . record_canvas_command($id, CanvasCommand::SetLineJoin { join });
            Ok::<_, anyhow::Error>(())
        }}
        $apply! { $rec $id canvas_set_line_dash ($id: &str, intervals: Vec<f32>, phase: f32) {
            $rec . record_canvas_command($id, CanvasCommand::SetLineDash { intervals, phase });
        }}
        $apply! { $rec $id canvas_clear_line_dash ($id: &str) {
            $rec . record_canvas_command($id, CanvasCommand::ClearLineDash);
        }}
        $apply! { $rec $id canvas_set_global_alpha ($id: &str, alpha: f32) {
            $rec . record_canvas_command($id, CanvasCommand::SetGlobalAlpha { alpha: alpha.clamp(0.0, 1.0) });
        }}
        $apply! { $rec $id canvas_set_anti_alias ($id: &str, enabled: bool) {
            $rec . record_canvas_command($id, CanvasCommand::SetAntiAlias { enabled });
        }}
        $apply! { $rec $id canvas_clear ($id: &str, color: Option<String>) {
            let color = match color {
                Some(c) => Some($crate::script::helpers::parse_color(&c, "clear")?),
                None => None,
            };
            $rec . record_canvas_command($id, CanvasCommand::Clear { color });
            Ok::<_, anyhow::Error>(())
        }}
        $apply! { $rec $id canvas_draw_paint ($id: &str, color: String, anti_alias: bool) {
            let color = $crate::script::helpers::parse_color(&color, "drawPaint")?;
            $rec . record_canvas_command($id, CanvasCommand::DrawPaint { color, anti_alias });
            Ok::<_, anyhow::Error>(())
        }}
        $apply! { $rec $id canvas_draw_text ($id: &str, text: String, values: Vec<f32>, color: String, flags: Vec<bool>, font_edging: String) {
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
        $apply! { $rec $id canvas_fill_rect ($id: &str, x: f32, y: f32, width: f32, height: f32, color: String) {
            let color = $crate::script::helpers::parse_color(&color, "fillRect")?;
            $rec . record_canvas_command($id, CanvasCommand::FillRect { x, y, width, height, color });
            Ok::<_, anyhow::Error>(())
        }}
        $apply! { $rec $id canvas_stroke_rect ($id: &str, x: f32, y: f32, width: f32, height: f32, color: String, stroke_width: f32) {
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
        $apply! { $rec $id canvas_draw_image ($id: &str, asset_id: String, values: Vec<f32>, fit: String, alpha: f32, anti_alias: bool) {
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
        $apply! { $rec $id canvas_draw_arc ($id: &str, cx: f32, cy: f32, rx: f32, ry: f32, start_angle: f32, sweep_angle: f32) {
            $rec . record_canvas_command($id, CanvasCommand::DrawArc {
                cx, cy, rx, ry, start_angle, sweep_angle, use_center: false,
            });
            Ok::<_, anyhow::Error>(())
        }}
        $apply! { $rec $id canvas_draw_arc_to_center ($id: &str, cx: f32, cy: f32, rx: f32, ry: f32, start_angle: f32, sweep_angle: f32) {
            $rec . record_canvas_command($id, CanvasCommand::DrawArc {
                cx, cy, rx, ry, start_angle, sweep_angle, use_center: true,
            });
            Ok::<_, anyhow::Error>(())
        }}
        $apply! { $rec $id canvas_draw_points ($id: &str, mode: String, points: Vec<f32>) {
            let mode = point_mode_from_name(&mode)
                .ok_or_else(|| $crate::script::helpers::script_error("drawPoints", format!("unsupported point mode `{mode}`")))?;
            $rec . record_canvas_command($id, CanvasCommand::DrawPoints { mode, points });
            Ok::<_, anyhow::Error>(())
        }}
        $apply! { $rec $id canvas_fill_drrect ($id: &str, coords: Vec<f32>) {
            let (outer_x, outer_y, outer_width, outer_height, outer_radius,
                 inner_x, inner_y, inner_width, inner_height, inner_radius) =
                $crate::script::helpers::parse_drrect("fillDRRect", &coords)?;
            $rec . record_canvas_command($id, CanvasCommand::FillDRRect {
                outer_x, outer_y, outer_width, outer_height, outer_radius,
                inner_x, inner_y, inner_width, inner_height, inner_radius,
            });
            Ok::<_, anyhow::Error>(())
        }}
        $apply! { $rec $id canvas_stroke_drrect ($id: &str, coords: Vec<f32>) {
            let (outer_x, outer_y, outer_width, outer_height, outer_radius,
                 inner_x, inner_y, inner_width, inner_height, inner_radius) =
                $crate::script::helpers::parse_drrect("strokeDRRect", &coords)?;
            $rec . record_canvas_command($id, CanvasCommand::StrokeDRRect {
                outer_x, outer_y, outer_width, outer_height, outer_radius,
                inner_x, inner_y, inner_width, inner_height, inner_radius,
            });
            Ok::<_, anyhow::Error>(())
        }}
        $apply! { $rec $id canvas_concat ($id: &str, values: Vec<f32>) {
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
    };
}
