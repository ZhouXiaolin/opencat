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
        // ── Node: unified style write (1 entry, replaces 38 individual record_*) ─

        $binding! { node $rec $id write_style_value ($id: &str, property: String, value: serde_json::Value) {
            $rec . write_style_value($id, &property, value);
        }}

        // ── Node: canvas commands (53 entries) ─────────────────────────────

        $binding! { node $rec $id canvas_save ($id: &str) {
            $rec . record_draw_op($id, DrawOp::Save);
        }}
        $binding! { node $rec $id canvas_restore ($id: &str) {
            $rec . record_draw_op($id, DrawOp::Restore);
        }}
        $binding! { node $rec $id canvas_restore_to_count ($id: &str, count: i32) {
            $rec . record_draw_op($id, DrawOp::RestoreToCount { count: count.max(1) });
        }}
        $binding! { node $rec $id canvas_translate ($id: &str, x: f32, y: f32) {
            $rec . record_draw_op($id, DrawOp::Translate { x, y });
        }}
        $binding! { node $rec $id canvas_scale ($id: &str, x: f32, y: f32) {
            $rec . record_draw_op($id, DrawOp::Scale { x, y });
        }}
        $binding! { node $rec $id canvas_rotate ($id: &str, degrees: f32) {
            $rec . record_draw_op($id, DrawOp::Rotate { degrees, cx: 0.0, cy: 0.0 });
        }}
        $binding! { node $rec $id canvas_clip_rect ($id: &str, x: f32, y: f32, width: f32, height: f32, anti_alias: bool) {
            $rec . record_draw_op($id, DrawOp::BeginPath);
            $rec . record_draw_op($id, DrawOp::Path(PathOp::AddRect { x, y, width, height }));
            $rec . record_draw_op($id, DrawOp::ClipPath { anti_alias });
        }}
        $binding! { node $rec $id canvas_draw_line ($id: &str, x0: f32, y0: f32, x1: f32, y1: f32) {
            $rec . record_draw_op($id, DrawOp::BeginPath);
            $rec . record_draw_op($id, DrawOp::Path(PathOp::MoveTo { x: x0, y: y0 }));
            $rec . record_draw_op($id, DrawOp::Path(PathOp::LineTo { x: x1, y: y1 }));
            $rec . record_draw_op($id, DrawOp::StrokePath);
        }}
        // ── Node: 3D 场景快照（three-like shim 每帧全量重提交）──

        $binding! { node $rec $id scene3_commit ($id: &str, scene: crate::ir::scene3::Scene3) {
            $rec . record_scene3($id, scene);
        }}

        // ── Cmd: 3D 几何注册表（GLB 一次性解码 → 重网格不逐帧过 JSON 桥）──

        $binding! { cmd $store scene3_geometry_register (path: String, id: String) -> String {
            let geometry = $crate::ir::scene3_glb::load_glb_file(&path)?;
            $store . record_geometry(&id, std::sync::Arc::new(geometry));
            Ok(id)
        }}

        // 脚本内建几何（SphereGeometry 等）一次性注册：重网格不逐帧过 JSON 桥。
        $binding! { cmd $store scene3_geometry_register_inline (id: String, positions: Vec<[f32; 3]>, normals: Vec<[f32; 3]>, indices: Vec<u32>) -> String {
            let count = positions.len();
            if normals.len() != count {
                anyhow::bail!(
                    "scene3_geometry_register_inline '{id}': normals 数量 {} != positions 数量 {count}",
                    normals.len()
                );
            }
            if let Some((j, idx)) = indices.iter().enumerate().find(|(_, i)| (**i as usize) >= count) {
                anyhow::bail!(
                    "scene3_geometry_register_inline '{id}': indices[{j}] = {idx} 越界（positions = {count}）"
                );
            }
            $store . record_geometry(&id, std::sync::Arc::new(
                $crate::ir::scene3::Geometry3::Inline {
                    positions,
                    normals,
                    indices,
                    uvs: Vec::new(),
                    colors: Vec::new(),
                    colors1: Vec::new(),
                },
            ));
            Ok(id)
        }}

        $binding! { node $rec $id canvas_fill_circle ($id: &str, cx: f32, cy: f32, radius: f32) {
            $rec . record_draw_op($id, DrawOp::BeginPath);
            $rec . record_draw_op($id, DrawOp::Path(PathOp::AddOval {
                x: cx - radius, y: cy - radius,
                width: radius * 2.0, height: radius * 2.0,
            }));
            $rec . record_draw_op($id, DrawOp::FillPath);
        }}
        $binding! { node $rec $id canvas_stroke_circle ($id: &str, cx: f32, cy: f32, radius: f32) {
            $rec . record_draw_op($id, DrawOp::BeginPath);
            $rec . record_draw_op($id, DrawOp::Path(PathOp::AddOval {
                x: cx - radius, y: cy - radius,
                width: radius * 2.0, height: radius * 2.0,
            }));
            $rec . record_draw_op($id, DrawOp::StrokePath);
        }}
        $binding! { node $rec $id canvas_fill_rrect ($id: &str, x: f32, y: f32, width: f32, height: f32, radius: f32) {
            $rec . record_draw_op($id, DrawOp::BeginPath);
            $rec . record_draw_op($id, DrawOp::Path(PathOp::AddRRect { x, y, width, height, radius }));
            $rec . record_draw_op($id, DrawOp::FillPath);
        }}
        $binding! { node $rec $id canvas_stroke_rrect ($id: &str, x: f32, y: f32, width: f32, height: f32, radius: f32) {
            $rec . record_draw_op($id, DrawOp::BeginPath);
            $rec . record_draw_op($id, DrawOp::Path(PathOp::AddRRect { x, y, width, height, radius }));
            $rec . record_draw_op($id, DrawOp::StrokePath);
        }}
        $binding! { node $rec $id canvas_begin_path ($id: &str) {
            $rec . record_draw_op($id, DrawOp::BeginPath);
        }}
        $binding! { node $rec $id canvas_move_to ($id: &str, x: f32, y: f32) {
            $rec . record_draw_op($id, DrawOp::Path(PathOp::MoveTo { x, y }));
        }}
        $binding! { node $rec $id canvas_line_to ($id: &str, x: f32, y: f32) {
            $rec . record_draw_op($id, DrawOp::Path(PathOp::LineTo { x, y }));
        }}
        $binding! { node $rec $id canvas_quad_to ($id: &str, cx: f32, cy: f32, x: f32, y: f32) {
            $rec . record_draw_op($id, DrawOp::Path(PathOp::QuadTo { cx, cy, x, y }));
        }}
        $binding! { node $rec $id canvas_cubic_to ($id: &str, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
            $rec . record_draw_op($id, DrawOp::Path(PathOp::CubicTo { c1x, c1y, c2x, c2y, x, y }));
        }}
        $binding! { node $rec $id canvas_close_path ($id: &str) {
            $rec . record_draw_op($id, DrawOp::Path(PathOp::Close));
        }}
        $binding! { node $rec $id canvas_path_add_rect ($id: &str, x: f32, y: f32, width: f32, height: f32) {
            $rec . record_draw_op($id, DrawOp::Path(PathOp::AddRect { x, y, width, height }));
        }}
        $binding! { node $rec $id canvas_path_add_rrect ($id: &str, x: f32, y: f32, width: f32, height: f32, radius: f32) {
            $rec . record_draw_op($id, DrawOp::Path(PathOp::AddRRect { x, y, width, height, radius }));
        }}
        $binding! { node $rec $id canvas_path_add_oval ($id: &str, x: f32, y: f32, width: f32, height: f32) {
            $rec . record_draw_op($id, DrawOp::Path(PathOp::AddOval { x, y, width, height }));
        }}
        $binding! { node $rec $id canvas_path_add_arc ($id: &str, x: f32, y: f32, width: f32, height: f32, start_angle: f32, sweep_angle: f32) {
            $rec . record_draw_op($id, DrawOp::Path(PathOp::AddArc { x, y, width, height, start_angle, sweep_angle }));
        }}
        $binding! { node $rec $id canvas_fill_path ($id: &str) {
            $rec . record_draw_op($id, DrawOp::FillPath);
        }}
        $binding! { node $rec $id canvas_stroke_path ($id: &str) {
            $rec . record_draw_op($id, DrawOp::StrokePath);
        }}
        $binding! { node $rec $id canvas_stroke_arc ($id: &str, cx: f32, cy: f32, rx: f32, ry: f32, start_angle: f32, sweep_angle: f32) {
            $rec . record_draw_op($id, DrawOp::BeginPath);
            $rec . record_draw_op($id, DrawOp::Path(PathOp::AddArc {
                x: cx - rx, y: cy - ry,
                width: rx * 2.0, height: ry * 2.0,
                start_angle, sweep_angle,
            }));
            $rec . record_draw_op($id, DrawOp::StrokePath);
        }}
        $binding! { node $rec $id canvas_fill_oval ($id: &str, cx: f32, cy: f32, rx: f32, ry: f32) {
            $rec . record_draw_op($id, DrawOp::BeginPath);
            $rec . record_draw_op($id, DrawOp::Path(PathOp::AddOval {
                x: cx - rx, y: cy - ry,
                width: rx * 2.0, height: ry * 2.0,
            }));
            $rec . record_draw_op($id, DrawOp::FillPath);
        }}
        $binding! { node $rec $id canvas_stroke_oval ($id: &str, cx: f32, cy: f32, rx: f32, ry: f32) {
            $rec . record_draw_op($id, DrawOp::BeginPath);
            $rec . record_draw_op($id, DrawOp::Path(PathOp::AddOval {
                x: cx - rx, y: cy - ry,
                width: rx * 2.0, height: ry * 2.0,
            }));
            $rec . record_draw_op($id, DrawOp::StrokePath);
        }}
        $binding! { node $rec $id canvas_clip_path ($id: &str, anti_alias: bool) {
            $rec . record_draw_op($id, DrawOp::ClipPath { anti_alias });
        }}
        $binding! { node $rec $id canvas_clip_rrect ($id: &str, x: f32, y: f32, width: f32, height: f32, radius: f32, anti_alias: bool) {
            $rec . record_draw_op($id, DrawOp::BeginPath);
            $rec . record_draw_op($id, DrawOp::Path(PathOp::AddRRect { x, y, width, height, radius }));
            $rec . record_draw_op($id, DrawOp::ClipPath { anti_alias });
        }}
        $binding! { node $rec $id canvas_skew ($id: &str, sx: f32, sy: f32) {
            $rec . record_draw_op($id, DrawOp::Skew { sx, sy });
        }}
        $binding! { node $rec $id canvas_draw_image_simple ($id: &str, asset_id: String, x: f32, y: f32, alpha: f32, anti_alias: bool) {
            let _ = (alpha, anti_alias);
            $rec . record_draw_op($id, DrawOp::Image {
                image: ImageRef::Static { asset_id },
                x,
                y,
                paint: None,
            });
        }}
        $binding! { node $rec $id canvas_save_layer ($id: &str, alpha: f32, bounds: Option<Vec<f32>>) {
            let bounds = match bounds {
                Some(b) => Some($crate::script::helpers::parse_image_rect("saveLayer", &b)?),
                None => None,
            };
            let bounds_rect = bounds.map(|bds| Rect4 { x: bds[0], y: bds[1], width: bds[2], height: bds[3] });
            $rec . record_draw_op($id, DrawOp::SaveLayer {
                bounds: bounds_rect,
                paint: None,
                alpha: alpha.clamp(0.0, 1.0),
                backdrop: false,
            });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_set_fill_style ($id: &str, color: String) {
            let color = $crate::script::helpers::parse_color(&color, "setFillStyle")?;
            $rec . record_draw_op($id, DrawOp::SetFillStyle { color });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_set_stroke_style ($id: &str, color: String) {
            let color = $crate::script::helpers::parse_color(&color, "setStrokeStyle")?;
            $rec . record_draw_op($id, DrawOp::SetStrokeStyle { color });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_set_line_width ($id: &str, width: f32) {
            $rec . record_draw_op($id, DrawOp::SetLineWidth { width: width.max(0.0) });
        }}
        $binding! { node $rec $id canvas_set_line_cap ($id: &str, cap: String) {
            let cap = line_cap_from_name(&cap)
                .ok_or_else(|| $crate::script::helpers::script_error("setLineCap", format!("unsupported line cap `{cap}`")))?;
            $rec . record_draw_op($id, DrawOp::SetLineCap { cap });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_set_line_join ($id: &str, join: String) {
            let join = line_join_from_name(&join)
                .ok_or_else(|| $crate::script::helpers::script_error("setLineJoin", format!("unsupported line join `{join}`")))?;
            $rec . record_draw_op($id, DrawOp::SetLineJoin { join });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_set_line_dash ($id: &str, intervals: Vec<f32>, phase: f32) {
            // Phase 2 limitation: setLineDash not yet supported with DrawOp pipeline.
            // Dash intervals require f32_pool allocation in DrawOpBuilder which is not
            // accessible from the MutationRecorder trait. Will be fixed in Phase 3.
            let _ = (intervals, phase);
        }}
        $binding! { node $rec $id canvas_clear_line_dash ($id: &str) {
            $rec . record_draw_op($id, DrawOp::ClearLineDash);
        }}
        $binding! { node $rec $id canvas_set_global_alpha ($id: &str, alpha: f32) {
            $rec . record_draw_op($id, DrawOp::SetGlobalAlpha { alpha: alpha.clamp(0.0, 1.0) });
        }}
        $binding! { node $rec $id canvas_set_anti_alias ($id: &str, enabled: bool) {
            $rec . record_draw_op($id, DrawOp::SetAntiAlias { enabled });
        }}
        $binding! { node $rec $id canvas_clear ($id: &str, color: Option<String>) {
            let color = match color {
                Some(c) => {
                    let c = $crate::script::helpers::parse_color(&c, "clear")?;
                    Some(ColorF32 {
                        r: c.r as f32 / 255.0,
                        g: c.g as f32 / 255.0,
                        b: c.b as f32 / 255.0,
                        a: c.a as f32 / 255.0,
                    })
                }
                None => None,
            };
            $rec . record_draw_op($id, DrawOp::Clear {
                color: color.unwrap_or(ColorF32::TRANSPARENT),
            });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_draw_paint ($id: &str, color: String, anti_alias: bool) {
            let _color = $crate::script::helpers::parse_color(&color, "drawPaint")?;
            $rec . record_draw_op($id, DrawOp::Paint {
                paint: PaintId(u32::MAX),
            });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_draw_text ($id: &str, text: String, values: Vec<f32>, color: String, flags: Vec<bool>, font_edging: String) {
            if values.len() < 6 {
                Err($crate::script::helpers::script_error("drawText", "expected text values [x, y, fontSize, scaleX, skewX, strokeWidth]".to_string()))
            } else if flags.len() < 3 {
                Err($crate::script::helpers::script_error("drawText", "expected text flags [antiAlias, stroke, fontSubpixel]".to_string()))
            } else {
                let _color = $crate::script::helpers::parse_color(&color, "drawText")?;
                let _font_edging = font_edging_from_name(&font_edging)
                    .ok_or_else(|| $crate::script::helpers::script_error("drawText", format!("unsupported font edging `{font_edging}`")))?;
                let _ = (text, values, flags);
                // TODO: Phase 2 — draw text via glyph ops
                Ok::<_, anyhow::Error>(())
            }
        }}
        $binding! { node $rec $id canvas_fill_rect ($id: &str, x: f32, y: f32, width: f32, height: f32, color: String) {
            let color = $crate::script::helpers::parse_color(&color, "fillRect")?;
            $rec . record_draw_op($id, DrawOp::SetFillStyle { color });
            $rec . record_draw_op($id, DrawOp::BeginPath);
            $rec . record_draw_op($id, DrawOp::Path(PathOp::AddRect { x, y, width, height }));
            $rec . record_draw_op($id, DrawOp::FillPath);
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_stroke_rect ($id: &str, x: f32, y: f32, width: f32, height: f32, color: String, stroke_width: f32) {
            let color = $crate::script::helpers::parse_color(&color, "strokeRect")?;
            $rec . record_draw_op($id, DrawOp::SetStrokeStyle { color });
            $rec . record_draw_op($id, DrawOp::SetLineWidth { width: stroke_width.max(0.0) });
            $rec . record_draw_op($id, DrawOp::BeginPath);
            $rec . record_draw_op($id, DrawOp::Path(PathOp::AddRect { x, y, width, height }));
            $rec . record_draw_op($id, DrawOp::StrokePath);
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_draw_image ($id: &str, asset_id: String, values: Vec<f32>, fit: String, alpha: f32, anti_alias: bool) {
            let _object_fit = object_fit_from_name(&fit)
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
            let img_ref = ImageRef::Static { asset_id };
            let dst = Rect4 { x: values[0], y: values[1], width: values[2], height: values[3] };
            let src = src_rect.map(|s| Rect4 { x: s[0], y: s[1], width: s[2], height: s[3] });
            $rec . record_draw_op($id, DrawOp::ImageRect {
                image: img_ref,
                src,
                dst,
                paint: None,
            });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_draw_arc ($id: &str, cx: f32, cy: f32, rx: f32, ry: f32, start_angle: f32, sweep_angle: f32) {
            $rec . record_draw_op($id, DrawOp::Arc {
                rect: Rect4 { x: cx - rx, y: cy - ry, width: rx * 2.0, height: ry * 2.0 },
                start: start_angle,
                sweep: sweep_angle,
                use_center: false,
                paint: PaintId(u32::MAX),
            });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_draw_arc_to_center ($id: &str, cx: f32, cy: f32, rx: f32, ry: f32, start_angle: f32, sweep_angle: f32) {
            $rec . record_draw_op($id, DrawOp::Arc {
                rect: Rect4 { x: cx - rx, y: cy - ry, width: rx * 2.0, height: ry * 2.0 },
                start: start_angle,
                sweep: sweep_angle,
                use_center: true,
                paint: PaintId(u32::MAX),
            });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_draw_points ($id: &str, mode: String, points: Vec<f32>) {
            // Phase 2 limitation: drawPoints not yet supported with DrawOp pipeline.
            // Points require f32_pool allocation in DrawOpBuilder which is not accessible
            // from the MutationRecorder trait. Will be fixed in Phase 3.
            let _ = (points, mode);
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_fill_drrect ($id: &str, coords: Vec<f32>) {
            let (outer_x, outer_y, outer_width, outer_height, outer_radius,
                 inner_x, inner_y, inner_width, inner_height, inner_radius) =
                $crate::script::helpers::parse_drrect("fillDRRect", &coords)?;
            $rec . record_draw_op($id, DrawOp::DRRect {
                outer: DRRectSpec {
                    rect: Rect4 { x: outer_x, y: outer_y, width: outer_width, height: outer_height },
                    radii: Radii4 { top_left: outer_radius, top_right: outer_radius, bottom_right: outer_radius, bottom_left: outer_radius },
                },
                inner: DRRectSpec {
                    rect: Rect4 { x: inner_x, y: inner_y, width: inner_width, height: inner_height },
                    radii: Radii4 { top_left: inner_radius, top_right: inner_radius, bottom_right: inner_radius, bottom_left: inner_radius },
                },
                paint: PaintId(u32::MAX),
            });
            Ok::<_, anyhow::Error>(())
        }}
        $binding! { node $rec $id canvas_stroke_drrect ($id: &str, coords: Vec<f32>) {
            let (outer_x, outer_y, outer_width, outer_height, outer_radius,
                 inner_x, inner_y, inner_width, inner_height, inner_radius) =
                $crate::script::helpers::parse_drrect("strokeDRRect", &coords)?;
            $rec . record_draw_op($id, DrawOp::DRRect {
                outer: DRRectSpec {
                    rect: Rect4 { x: outer_x, y: outer_y, width: outer_width, height: outer_height },
                    radii: Radii4 { top_left: outer_radius, top_right: outer_radius, bottom_right: outer_radius, bottom_left: outer_radius },
                },
                inner: DRRectSpec {
                    rect: Rect4 { x: inner_x, y: inner_y, width: inner_width, height: inner_height },
                    radii: Radii4 { top_left: inner_radius, top_right: inner_radius, bottom_right: inner_radius, bottom_left: inner_radius },
                },
                paint: PaintId(u32::MAX - 1),
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
                $rec . record_draw_op($id, DrawOp::Concat { matrix });
                Ok::<_, anyhow::Error>(())
            }
        }}

        $binding! { node $rec $id canvas_draw_picture ($id: &str, owner_id: String, x: f32, y: f32) {
            $rec . record_draw_picture($id, &owner_id, x, y);
        }}

        $binding! { node $rec $id canvas_runtime_effect_draw ($id: &str, sksl: String, uniforms: Vec<f32>, children_json: String, dst_x: f32, dst_y: f32, dst_w: f32, dst_h: f32) {
            let specs = $crate::script::helpers::parse_script_children(&children_json)?;
            let child_refs: Vec<$crate::ir::draw_types::ScriptRuntimeEffectChild> =
                specs.iter().map(|c| c.to_script_child()).collect::<Result<_, _>>()?;
            let uniforms_bytes: Vec<u8> = uniforms
                .iter()
                .flat_map(|v| v.to_ne_bytes())
                .collect();
            $rec . record_canvas_runtime_effect(
                $id,
                sksl,
                uniforms_bytes,
                child_refs,
                $crate::ir::draw_op::Rect4 {
                    x: dst_x,
                    y: dst_y,
                    width: dst_w,
                    height: dst_h,
                },
            );
            Ok::<_, anyhow::Error>(())
        }}

        // ── Node: effect lambda draw（SKSL 与逐像素效果的统一入口）──────────
        // Rust 编译 lambda 源码并自主派发后端：
        //   Sksl → 生成 SKSL 走 record_canvas_runtime_effect（手写 SKSL 同路，
        //          web CanvasKit 解码零改动；u_oc_rect 追加在 uniforms 末尾）；
        //   Cpu  → 解释器 rayon 逐像素 → record_frame_generated_image（或
        //          直接采样核心内 render target surface）。
        $binding! { cmd $store canvas_lambda_effect_draw (node_id: String, lambda: String, spec_json: String, uniforms: Vec<f32>, children_json: String, dst_x: f32, dst_y: f32, dst_w: f32, dst_h: f32) -> bool {
            let spec_value: serde_json::Value = serde_json::from_str(&spec_json)
                .map_err(|e| anyhow::anyhow!("lambda spec decode: {e}"))?;
            let spec = $crate::opencat_liangyi::EffectSpec::from_json(&spec_value)?;
            let compiled = $crate::opencat_liangyi::compile_effect(&lambda, &spec)?;
            let children = $crate::script::helpers::parse_script_children(&children_json)?;
            match compiled.backend {
                $crate::opencat_liangyi::Backend::Sksl => {
                    // SKSL/wire 侧只认跨线 child（image / picture / generated）。
                    // surface source 在此自动烘成帧级生成图像 —— 内核作者写
                    // `sources: [field]`，不必知道 SKSL 要的是 generated、更
                    // 不必手写 bake(key) 与帧号。CPU 后端直接采样核心内像素。
                    let baked = $crate::script::helpers::auto_bake_surfaces($store, &children)?;
                    let child_refs: Vec<$crate::ir::draw_types::ScriptRuntimeEffectChild> =
                        baked
                            .iter()
                            .map(|c| c.to_script_child())
                            .collect::<Result<_, _>>()?;
                    let sksl = compiled.sksl.clone().ok_or_else(|| {
                        anyhow::anyhow!("lambda compiled to sksl backend but sksl is missing")
                    })?;
                    // u_oc_rect 由 codegen 追加在所有 uniform 之后
                    let mut packed = uniforms.clone();
                    packed.extend_from_slice(&[dst_x, dst_y, dst_w, dst_h]);
                    let uniforms_bytes: Vec<u8> = packed
                        .iter()
                        .flat_map(|v| v.to_ne_bytes())
                        .collect();
                    $store.record_canvas_runtime_effect(
                        &node_id,
                        sksl,
                        uniforms_bytes,
                        child_refs,
                        $crate::ir::draw_op::Rect4 {
                            x: dst_x,
                            y: dst_y,
                            width: dst_w,
                            height: dst_h,
                        },
                    );
                    Ok(true)
                }
                $crate::opencat_liangyi::Backend::Cpu => {
                    use $crate::opencat_liangyi::interp::{ChildImage, InterpCtx};
                    // uniform f32 平铺 → Val（按 spec 声明序消费）
                    let program = &compiled.program;
                    let uniform_vals = $crate::script::helpers::uniform_f32s_to_vals(
                        &program.uniforms,
                        &uniforms,
                    )?;
                    // generated → 本帧已注册像素；surface → 核心内 session 级
                    // render target（溶解 field 等构建一次的数据）
                    let child_images: Vec<ChildImage> =
                        $crate::script::helpers::cpu_child_images($store, &children)?;
                    let w = dst_w.max(1.0).round() as u32;
                    let h = dst_h.max(1.0).round() as u32;
                    let ctx = InterpCtx {
                        uniforms: &uniform_vals,
                        rect: [dst_x as f64, dst_y as f64, dst_w as f64, dst_h as f64],
                        children: &child_images,
                        scan: None,
                    };
                    let rgba: std::sync::Arc<[u8]> = std::sync::Arc::from(
                        $crate::opencat_liangyi::interp::render(program, w, h, &ctx),
                    );
                    // 确定性 key：lambda hash + uniforms 指纹 + dst（同帧同参
                    // 幂等，异参碰撞是 hard error；逐帧 uniform 变化自然产生
                    // 逐帧不同的 key）
                    let mut hf = std::collections::hash_map::DefaultHasher::new();
                    use std::hash::{Hash, Hasher as _};
                    compiled.hash.hash(&mut hf);
                    for v in &uniforms {
                        v.to_bits().hash(&mut hf);
                    }
                    dst_x.to_bits().hash(&mut hf);
                    dst_y.to_bits().hash(&mut hf);
                    w.hash(&mut hf);
                    h.hash(&mut hf);
                    let key = format!(
                        "lambda_{}_{:016x}",
                        node_id,
                        hf.finish()
                    );
                    $store.record_frame_generated_image(
                        &node_id,
                        $crate::ir::GeneratedImageId::from_key(&key),
                        dst_x,
                        dst_y,
                        w,
                        h,
                        rgba,
                    );
                    Ok(true)
                }
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

        // ── Cmd: store mutations (4 entries: animate, morph, along_path) ──
        $binding! { cmd $store animate_create (duration: f32, delay: f32, clamp_flag: i32, easing_tag: String, repeat: i32, yoyo_flag: i32, repeat_delay: f32) -> i32 {
            let clamp = clamp_flag != 0;
            let yoyo = yoyo_flag != 0;
            let ct = $store.script_time_secs() as f32;
            Ok($store.animate_create(ct, duration, delay, clamp, &easing_tag, repeat, yoyo, repeat_delay))
        }}
        $binding! { cmd $store animate_set_progress (handle: i32, progress: f32) -> bool {
            $store.animate_set_progress(handle, progress);
            Ok(true)
        }}
        $binding! { cmd $store morph_svg_create (from_svg: String, to_svg: String, grid_size: f32) -> i32 {
            Ok($store.morph_svg_create(&from_svg, &to_svg, grid_size as u32).unwrap_or(-1))
        }}
        $binding! { cmd $store along_path_create (svg: String) -> i32 {
            $store.along_path_create(&svg).ok_or_else(|| anyhow::anyhow!("invalid SVG path"))
        }}

        // ── Cmd: scene clock override ───────────────────────────────────────
        // The JSX frontend emits ONE root driver whose store frame is the
        // composition frame. Inside a `<tl>` each scene has its own clock, so
        // the JSX runtime sets the scene-local frame around every hook and every
        // timeline flush. `-1` clears it (fall back to the composition frame).
        // Tween progress (`animate_create` / `animate_value`) reads this frame.
        $binding! { cmd $store set_script_frame (frame: i32) -> bool {
            $store.set_frame_override(if frame < 0 { None } else { Some(frame as u32) });
            Ok(true)
        }}

        // Scene-local time in SECONDS for the tween evaluator. A scene window can
        // start between frames, and tween progress advances by elapsed time, so
        // the fractional part must survive. Negative clears it.
        $binding! { cmd $store set_script_time_secs (secs: f64) -> bool {
            $store.set_time_override_secs(if secs < 0.0 { None } else { Some(secs) });
            Ok(true)
        }}

        // ── Qry: store reads (10 entries: animate, morph, text, along_path) ─
        $binding! { qry $store animate_value (handle: i32, _key: String, from: f32, to: f32) -> f32 {
            let ct = $store.script_time_secs() as f32;
            Ok($store.animate_value(ct, handle, from, to))
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

        $binding! { qry $store read_style_value (id: String, property: String) -> Option<serde_json::Value> {
            Ok($store.read_style_value(&id, &property))
        }}

        $binding! { node $rec $id write_style_value ($id: &str, property: String, value: serde_json::Value) {
            $rec . write_style_value($id, &property, value);
        }}

        // ── Pure: no store (4 entries: text measure, random, graphemes, easing) ─
        $binding! { pure canvas_measure_text (text: String, font_size: f32, font_scale_x: f32, _font_skew_x: f32, _font_subpixel: bool, _font_edging: String) -> f32 {
            Ok(measure_script_text_width(&text, font_size, font_scale_x))
        }}
        // ── Pure: offscreen pixel surface — session 级 render target
        //    （canvas 2D 子集 + run/sweep/bake，见 text::surface）
        $binding! { pure surface_create (id: String, width: f64, height: f64) -> bool {
            Ok($crate::text::surface::surface_create(&id, width as u32, height as u32).is_ok())
        }}
        $binding! { pure surface_clear (id: String, x: f64, y: f64, w: f64, h: f64) -> bool {
            Ok($crate::text::surface::surface_clear(&id, x, y, w, h).is_ok())
        }}
        $binding! { pure surface_measure_text (text: String, family: String, weight: u32, size_px: f64, letter_spacing_px: f64) -> $crate::text::surface::InkMetrics {
            $crate::text::surface::surface_measure_text(&text, &family, weight, size_px, letter_spacing_px)
        }}
        $binding! { pure surface_fill_text (id: String, text: String, x: f64, y: f64, family: String, weight: u32, size_px: f64, letter_spacing_px: f64, scale_x: f64, color: String) -> bool {
            let c = $crate::script::helpers::parse_color(&color, "surface.fillText")?;
            Ok($crate::text::surface::surface_fill_text(
                &id, &text, x, y, &family, weight, size_px, letter_spacing_px, scale_x,
                [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0, c.a as f32 / 255.0],
            ).is_ok())
        }}
        // ── Render target ops（通用机制：JS 拥有算法，Rust 只有执行器）────
        // surface_apply_effect：pixel 类 lambda 逐像素渲染进目标 surface（f64 解释器，
        // rayon 按行并行）。children 可采样 generated（本帧烘焙）或其它
        // surface（session 级 render target）。像素缓冲永不跨进 JS。
        $binding! { cmd $store surface_apply_effect (id: String, lambda: String, spec_json: String, uniforms: Vec<f32>, children_json: String) -> bool {
            let spec_value: serde_json::Value = serde_json::from_str(&spec_json)
                .map_err(|e| anyhow::anyhow!("lambda spec decode: {e}"))?;
            let mut spec = $crate::opencat_liangyi::EffectSpec::from_json(&spec_value)?;
            // 本 binding 只做逐像素渲染：未声明 kind 时按 pixel 处理
            if spec_value.get("kind").is_none() {
                spec.kind = $crate::opencat_liangyi::program::ScanKind::Pixel;
            }
            if spec.kind == $crate::opencat_liangyi::program::ScanKind::Scan {
                return Err(anyhow::anyhow!(
                    "surface_apply_effect 需要 pixel 类 lambda（顺序扫描请用 surface_scan_pass）"
                ));
            }
            // 目标运算恒走 f64 解释器（参考语义；SKSL 是绘制期后端，不落目标）
            spec.backend_override = Some($crate::opencat_liangyi::Backend::Cpu);
            let compiled = $crate::opencat_liangyi::compile_effect(&lambda, &spec)?;
            let children = $crate::script::helpers::parse_script_children(&children_json)?;
            let child_images = $crate::script::helpers::cpu_child_images($store, &children)?;
            let (w, h) = $crate::text::surface::surface_dimensions(&id)?;
            let uniform_vals = $crate::script::helpers::uniform_f32s_to_vals(
                &compiled.program.uniforms,
                &uniforms,
            )?;
            let ctx = $crate::opencat_liangyi::interp::InterpCtx {
                uniforms: &uniform_vals,
                rect: [0.0, 0.0, w as f64, h as f64],
                children: &child_images,
                scan: None,
            };
            // 就地执行：Skia surface 的可写像素缓冲经 BufferViewMut 实例化
            // 为 liangyi::pixel::RenderTarget（CPU 后端实例）。
            $crate::text::surface::surface_with_rgba_mut(&id, |w, h, buf| {
                let mut target = $crate::opencat_liangyi::pixel::BufferViewMut::new(w, h, buf)
                    .map_err(|e| anyhow::anyhow!("surface_apply_effect: {e}"))?;
                $crate::opencat_liangyi::exec::run_pixel_into(
                    &compiled.program,
                    &mut target,
                    &ctx,
                );
                Ok(())
            })?;
            Ok(true)
        }}
        // surface_scan_pass：scan 类 lambda（首参 get）对本 surface **就地**一遍顺序
        // 扫描。遍历顺序由 executor 拥有（forward/backward），JS 源码无循环；
        // get(dx, dy) 读 in-progress 缓冲（clamp-to-edge）。单线程。
        $binding! { cmd $store surface_scan_pass (id: String, lambda: String, spec_json: String, uniforms: Vec<f32>, direction: String) -> bool {
            let spec_value: serde_json::Value = serde_json::from_str(&spec_json)
                .map_err(|e| anyhow::anyhow!("lambda spec decode: {e}"))?;
            let mut spec = $crate::opencat_liangyi::EffectSpec::from_json(&spec_value)?;
            // 本 binding 只做顺序扫描：未声明 kind 时按 scan 处理
            if spec_value.get("kind").is_none() {
                spec.kind = $crate::opencat_liangyi::program::ScanKind::Scan;
            }
            if spec.kind != $crate::opencat_liangyi::program::ScanKind::Scan {
                return Err(anyhow::anyhow!(
                    "surface_scan_pass 需要 scan 类 lambda（spec.kind: 'scan'；逐像素请用 surface_apply_effect）"
                ));
            }
            spec.backend_override = Some($crate::opencat_liangyi::Backend::Cpu);
            let compiled = $crate::opencat_liangyi::compile_effect(&lambda, &spec)?;
            let Some(direction) = $crate::opencat_liangyi::interp::ScanDirection::from_name(&direction)
            else {
                return Err(anyhow::anyhow!(
                    "surface_scan_pass: direction 仅支持 'forward' / 'backward'"
                ));
            };
            let uniform_vals = $crate::script::helpers::uniform_f32s_to_vals(
                &compiled.program.uniforms,
                &uniforms,
            )?;
            $crate::text::surface::surface_with_rgba_mut(&id, |w, h, buf| {
                let mut target = $crate::opencat_liangyi::pixel::BufferViewMut::new(w, h, buf)
                    .map_err(|e| anyhow::anyhow!("surface_scan_pass: {e}"))?;
                $crate::opencat_liangyi::exec::run_scan_into(
                    &compiled.program,
                    &mut target,
                    &uniform_vals,
                    direction,
                );
                Ok(())
            })?;
            Ok(true)
        }}
        $binding! { pure util_random_seeded (seed: f32) -> f32 {
            Ok(random_from_seed(seed))
        }}
        // Pure geometry: minimum-area enclosing rectangle of a 2-D point set.
        // `points` is a flat [x0,y0, x1,y1, ...]; returns [angle_rad, w, h].
        // Used to recover a 3-D mesh's projected in-plane orientation from a
        // handful of projected hull points (see examples/liquid-brand-refraction/liquid-brand-refraction.jsx).
        $binding! { pure util_min_area_rect (points: Vec<f64>) -> Vec<f64> {
            if points.len() < 2 {
                return Err(anyhow::anyhow!("util_min_area_rect: need >=1 point"));
            }
            let pts: Vec<[f64; 2]> = points
                .chunks_exact(2)
                .map(|c| [c[0], c[1]])
                .collect();
            let (angle, w, h) = $crate::geom::min_area_rect(&pts)
                .ok_or_else(|| anyhow::anyhow!("util_min_area_rect: empty"))?;
            Ok(vec![angle, w, h])
        }}
        // Pure geometry: pinhole camera view basis (forward, right, up) for
        // `eye` looking at `target`, matching Three.js
        // `PerspectiveCamera.lookAt` semantics. Returns a flat
        // [fx,fy,fz, rx,ry,rz, ux,uy,uz]. Used so the chromatic-plane effect
        // derives its ray basis from the source camera constants rather than
        // picking a cross-product sign by measurement.
        $binding! { pure util_camera_basis (eye: Vec<f64>, target: Vec<f64>) -> Vec<f64> {
            if eye.len() != 3 || target.len() != 3 {
                return Err(anyhow::anyhow!("util_camera_basis: eye/target must be 3"));
            }
            let (f, r, u) = $crate::geom::camera_basis(
                [eye[0], eye[1], eye[2]], [target[0], target[1], target[2]]);
            Ok(vec![f[0], f[1], f[2], r[0], r[1], r[2], u[0], u[1], u[2]])
        }}
        $binding! { pure text_graphemes (text: String) -> Vec<String> {
            Ok(grapheme_strings(&text).into_iter().collect())
        }}
        $binding! { pure easing_apply (tag: String, t: f32) -> f32 {
            let easing = parse_easing_from_tag(&tag);
            Ok(easing.apply(t))
        }}
        // Register a GSAP `CustomEase`-style sampled ease table (the JS side
        // computes the samples, e.g. a physics-spring table) and return the
        // table index; the tag `points:<name>` references it in tweens.
        $binding! { pure ease_register_points (name: String, points: Vec<f32>) -> f64 {
            Ok($crate::parse::easing::register_ease_table(&name, &points) as f64)
        }}
    };
}
