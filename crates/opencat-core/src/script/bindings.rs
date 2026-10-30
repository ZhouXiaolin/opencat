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
            $rec . record_draw_op($id, DrawOp::ClipPath { anti_alias, fill_type: Default::default() });
        }}
        // ── Cmd: GPU 命令流录制（webgl_api.js 的 WebGL2 录制面）──────────

        $binding! { cmd $store gl_stream_begin (target_id: String, width: u32, height: u32, backend: u8, attrs_bits: u8) -> () {
            $store . gl_stream_begin(&target_id, width, height, backend, attrs_bits);
            Ok(())
        }}

        /// JS 侧编码好的整条命令（[opcode][payload]）原样追加——core 不解释。
        $binding! { cmd $store gl_stream_emit (target_id: String, cmd: Vec<u8>) -> () {
            $store . gl_stream_emit(&target_id, cmd)
        }}

        /// intern 大块数据（buffer/纹理/shader 源码），返回 range id。
        $binding! { cmd $store gl_stream_intern (target_id: String, data: Vec<u8>) -> u32 {
            $store . gl_stream_intern(&target_id, data)
        }}

        /// 分配 GL 资源句柄（realm 级单调）。
        $binding! { cmd $store gl_alloc_handle () -> u32 {
            Ok($store.gl_alloc_handle())
        }}

        /// 真 GL 反射：link 后 three 立刻同步枚举 active uniform/attribute。
        /// 反射器由宿主注入（engine 拿 GL 上下文向驱动问询），core 只转发。
        $binding! { qry $store glsl_reflect (vs: String, fs: String) -> serde_json::Value {
            $store.glsl_reflect(&vs, &fs)
        }}

        $binding! { node $rec $id canvas_draw_line ($id: &str, x0: f32, y0: f32, x1: f32, y1: f32) {
            $rec . record_draw_op($id, DrawOp::BeginPath);
            $rec . record_draw_op($id, DrawOp::Path(PathOp::MoveTo { x: x0, y: y0 }));
            $rec . record_draw_op($id, DrawOp::Path(PathOp::LineTo { x: x1, y: y1 }));
            $rec . record_draw_op($id, DrawOp::StrokePath);
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
            $rec . record_draw_op($id, DrawOp::ClipPath { anti_alias, fill_type: Default::default() });
        }}
        $binding! { node $rec $id canvas_clip_rrect ($id: &str, x: f32, y: f32, width: f32, height: f32, radius: f32, anti_alias: bool) {
            $rec . record_draw_op($id, DrawOp::BeginPath);
            $rec . record_draw_op($id, DrawOp::Path(PathOp::AddRRect { x, y, width, height, radius }));
            $rec . record_draw_op($id, DrawOp::ClipPath { anti_alias, fill_type: Default::default() });
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
        // ── Node: G2 chart (AntV) — text paragraphs and rendered text images
        //    The G2 runtime shim (`g2/g2_api.js`) routes G2's draw calls through
        //    the regular `__canvas_*` bindings above. Text is the one exception:
        //    g-canvaskit draws through CanvasKit Paragraphs, and OpenCat's real
        //    text rasterizer is the offscreen surface (`ctx.ly.surface`).
        //    So a paragraph is measured + drawn on a surface and composited as
        //    a frame-scoped generated image — same pixel path as every other
        //    text in the composition.
        $binding! { cmd $store canvas_draw_paragraph (node_id: String, text: String, size: f32, family: String, weight: u32, color: String, x: f32, y: f32, width: f32, height: f32) -> bool {
            if text.trim().is_empty() {
                return Ok(false);
            }
            let color = $crate::script::helpers::parse_color(&color, "drawParagraph")?;
            // One canvas node carries many labels (axis ticks, legends, …), and
            // each is its own bitmap at its own position. Give every label a
            // distinct surface: a shared per-node buffer would have the last
            // write clobber the earlier ones, and the content-addressed image
            // key would then collide on differing sizes.
            let surface_id = format!("g2-para:{node_id}:{text}:{size}:{weight}:{x}:{y}");
            $crate::text::surface::surface_create_or_resize(&surface_id, width as u32, height as u32)?;
            $crate::text::surface::surface_clear(&surface_id, 0.0, 0.0, width as f64, height as f64)?;
            $crate::text::surface::surface_fill_text(
                &surface_id,
                &text,
                0.0,
                size as f64,
                &family,
                weight as u32,
                size as f64,
                0.0,
                1.0,
                [color.r as f32 / 255.0, color.g as f32 / 255.0, color.b as f32 / 255.0, color.a as f32 / 255.0],
            )?;
            // The label's own identity is the key: same text+size+position
            // across frames reuses one image, a different label gets another.
            let key = format!("g2-paragraph:{surface_id}");
            let image_id = $crate::ir::GeneratedImageId::from_key(&key);
            let rgba = $crate::text::surface::surface_rgba_arc(&surface_id)?;
            $store.record_frame_generated_image(
                &node_id,
                image_id,
                x,
                y,
                width as u32,
                height as u32,
                rgba,
            );
            Ok(true)
        }}
        $binding! { pure canvas_measure_paragraph (text: String, size: f32, family: String, weight: u32) -> serde_json::Value {
            let metrics = $crate::text::surface::surface_measure_text(&text, &family, weight as u32, size as f64, 0.0)?;
            // g-canvaskit shifts text by `paragraph.getHeight()` to honour
            // textBaseline, so the callers need the real line box, not just the
            // advance. Height = ascent + descent of the font box, which is what
            // CanvasKit's own paragraph layout returns.
            Ok(serde_json::json!({
                "width": metrics.width as f32,
                "height": (metrics.font_ascent + metrics.font_descent) as f32,
                "ascent": metrics.font_ascent as f32,
                "descent": metrics.font_descent as f32,
            }))
        }}

        //    批次 B 文本转路径：文本 shape 成 glyph outline path 载荷，facade
        //    把整段烘成一个 Path 一次 drawPath——DrawOp 流里只剩 path，两端
        //    （engine Skia / web 真 CK）各画同一份 path，web 零文本栈，且度
        //    量与绘制同源（此前度量走 surface 的 kern-free 简排、绘制走
        //    swash 位图，两者互不认识）。positions 的 y 以首行基线为原点；
        //    outline 命令由 swash 按请求字号生成（cache_key 含 font_size），
        //    已是 px 域，只翻 y（字体 y-up → 屏幕 y-down；与 render/text.rs
        //    的 build_glyph_path 同构——那边乘 upem/px 是为按设计单位 intern
        //    共享 path，此处按 size 逐份发放，直接 px 直出）。ColorImage
        //    （emoji）不走 path：置 color 标记，facade 回退既有位图通道
        //    （core 不解释 payload）。
        $binding! { pure canvas_glyph_paths (text: String, size: f32, family: String, weight: u32) -> serde_json::Value {
            // Generic `sans-serif` must resolve through fontdb's registered
            // sans-serif family (engine: Noto Sans SC), not as a literal
            // name — a literal lookup misses and cosmic-text's fallback
            // bottom-feeds into Noto Color Emoji, where even '0' has a
            // color keycap bitmap. `None` → `Family::SansSerif` → the
            // registered mapping.
            let family = match family.trim() {
                "" | "sans-serif" => None,
                _ => Some(family),
            };
            let style = $crate::style::ComputedTextStyle {
                text_px: size,
                font_family: family,
                font_weight: $crate::style::FontWeight(weight as u16),
                ..Default::default()
            };
            let raster = $crate::text::rasterize_glyphs(
                &text,
                &style,
                f32::INFINITY,
                false,
                false,
                &$crate::text::script_font_db(),
            );
            let Some(line) = raster.lines.first() else {
                return Ok(serde_json::json!({ "glyphs": [], "positions": [], "width": 0.0, "ascent": size * 0.8, "descent": size * 0.2, "color": false }));
            };
            let mut has_color = false;
            let glyphs: Vec<serde_json::Value> = raster
                .glyphs
                .iter()
                .filter_map(|(key, data)| {
                    let $crate::text::GlyphData::Outline(commands, _upem) = data else {
                        has_color = true;
                        return None;
                    };
                    let ops: Vec<serde_json::Value> = commands
                        .iter()
                        .map(|cmd| match cmd {
                            cosmic_text::Command::MoveTo(p) => serde_json::json!([0, p.x, -p.y]),
                            cosmic_text::Command::LineTo(p) => serde_json::json!([1, p.x, -p.y]),
                            cosmic_text::Command::QuadTo(c, p) => {
                                serde_json::json!([2, c.x, -c.y, p.x, -p.y])
                            }
                            cosmic_text::Command::CurveTo(c1, c2, p) => serde_json::json!([
                                3, c1.x, -c1.y, c2.x, -c2.y, p.x, -p.y
                            ]),
                            cosmic_text::Command::Close => serde_json::json!([4]),
                        })
                        .collect();
                    Some(serde_json::json!({ "k": key, "ops": ops }))
                })
                .collect();
            let positions: Vec<serde_json::Value> = line
                .positions
                .iter()
                .map(|pos| {
                    serde_json::json!({ "k": pos.cache_key, "x": pos.x, "y": pos.y - line.y })
                })
                .collect();
            let (ascent, descent) = raster
                .metrics
                .map_or((size * 0.8, size * 0.2), |m| (m.ascent, m.descent));
            // Baseline anchors follow the browser's OS/2 typo metrics, not the
            // hhea pair the em box uses — see `FontTextMetrics::typo_ascent`.
            let (baseline_ascent, baseline_descent) = raster
                .metrics
                .map_or((ascent, descent), |m| (m.typo_ascent, m.typo_descent));
            Ok(serde_json::json!({
                "glyphs": glyphs,
                "positions": positions,
                "width": line.width,
                "ascent": ascent,
                "descent": descent,
                "baselineAscent": baseline_ascent,
                "baselineDescent": baseline_descent,
                "color": has_color,
            }))
        }}

        //    CanvasKit.Image.MakeFromEncoded → drawImage。过渡落地：字节在
        //    绑定侧经宿主 decoder 解码为 RGBA，走帧生成图通道贴回（与
        //    `g2-para` 位图同构，z 序经 DrawOp::Image 保持）。批次 B 文本
        //    换血时把 image mark 一并转 `ImageRef::Encoded` 真通道
        //    （BytesRangeId 直通，两端重放零解码分歧）。
        $binding! { cmd $store canvas_draw_image_encoded (node_id: String, data: Vec<u8>, x: f32, y: f32, alpha: f32, anti_alias: bool) -> bool {
            let _ = (alpha, anti_alias);
            if data.is_empty() {
                return Ok(false);
            }
            let (width, height, rgba) = $store.image_decode(&data)?;
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            data.hash(&mut hasher);
            let key = format!("g2-encoded:{}", hasher.finish());
            let image_id = $crate::ir::generated_image::GeneratedImageId::from_key(&key);
            $store.record_frame_generated_image(&node_id, image_id, x, y, width, height, std::sync::Arc::from(rgba));
            Ok(true)
        }}

        //    CanvasKit.makePathFromSVGString：SVG path 片段 → usvg 解析 →
        //    绝对段命令 flat 数组（0=moveTo 1=lineTo 2=quadTo 3=cubicTo
        //    4=close，后随坐标）。复用 svg-align 管线（markup 解析 + 段
        //    提取），取树根首个 path 节点。
        $binding! { pure oc_path_from_svg (svg: String) -> Option<Vec<f64>> {
            use tiny_skia_path::PathSegment;
            let Some(tree) = $crate::render::svg::load_tree_markup(&svg) else {
                return Ok(None);
            };
            let mut flat: Vec<f64> = Vec::new();
            for child in tree.root().children() {
                let usvg::Node::Path(path) = child else { continue };
                for seg in path.data().segments() {
                    match seg {
                        PathSegment::MoveTo(p) => {
                            flat.push(0.0);
                            flat.push(p.x as f64);
                            flat.push(p.y as f64);
                        }
                        PathSegment::LineTo(p) => {
                            flat.push(1.0);
                            flat.push(p.x as f64);
                            flat.push(p.y as f64);
                        }
                        PathSegment::QuadTo(c, p) => {
                            flat.push(2.0);
                            flat.push(c.x as f64);
                            flat.push(c.y as f64);
                            flat.push(p.x as f64);
                            flat.push(p.y as f64);
                        }
                        PathSegment::CubicTo(c1, c2, p) => {
                            flat.push(3.0);
                            flat.push(c1.x as f64);
                            flat.push(c1.y as f64);
                            flat.push(c2.x as f64);
                            flat.push(c2.y as f64);
                            flat.push(p.x as f64);
                            flat.push(p.y as f64);
                        }
                        PathSegment::Close => flat.push(4.0),
                    }
                }
                break; // 官方签名返回单条 path：取根级第一条
            }
            Ok(if flat.is_empty() { None } else { Some(flat) })
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
            // The recorder has no f32_pool access, so the dash intervals go
            // out as the pre-intern `ScriptSetLineDash` form;
            // `execute_draw_op` interns them and pushes the standard
            // `SetLineDash`. An empty list is the browser's "solid line".
            if intervals.is_empty() {
                $rec . record_draw_op($id, DrawOp::ClearLineDash);
            } else {
                $rec . record_draw_op($id, DrawOp::ScriptSetLineDash { intervals, phase });
            }
        }}
        $binding! { node $rec $id canvas_clear_line_dash ($id: &str) {
            $rec . record_draw_op($id, DrawOp::ClearLineDash);
        }}
        $binding! { node $rec $id canvas_set_style_gradient (
            $id: &str, stroke: bool, x0: f32, y0: f32, x1: f32, y1: f32,
            offsets: Vec<f32>, colors: Vec<String>
        ) {
            // Pre-intern form like `canvas_set_line_dash`: stop colors go
            // through `parse_color` here, then `execute_draw_op` interns the
            // flat (offset, r, g, b, a) quintuples into the canonical
            // `SetStyleLinearGradient`.
            if offsets.len() != colors.len() {
                return Err(anyhow::anyhow!(
                    "addColorStop: {} offsets vs {} colors", offsets.len(), colors.len()
                ));
            }
            let mut stops = Vec::with_capacity(offsets.len() * 5);
            for (i, off) in offsets.iter().enumerate() {
                let c = $crate::script::helpers::parse_color(&colors[i], "addColorStop")?;
                stops.push(*off);
                stops.push(c.r as f32);
                stops.push(c.g as f32);
                stops.push(c.b as f32);
                stops.push(c.a as f32);
            }
            $rec . record_draw_op($id, DrawOp::ScriptSetStyleLinearGradient {
                stroke,
                start: [x0, y0],
                end: [x1, y1],
                stops,
            });
        }}
        $binding! { node $rec $id canvas_set_style_radial_gradient (
            $id: &str, stroke: bool, x0: f32, y0: f32, r0: f32, x1: f32, y1: f32, r1: f32,
            offsets: Vec<f32>, colors: Vec<String>
        ) {
            // Radial counterpart of `canvas_set_style_gradient`; same stop
            // encoding, geometry is the two circles (x0,y0,r0)/(x1,y1,r1).
            if offsets.len() != colors.len() {
                return Err(anyhow::anyhow!(
                    "addColorStop: {} offsets vs {} colors", offsets.len(), colors.len()
                ));
            }
            let mut stops = Vec::with_capacity(offsets.len() * 5);
            for (i, off) in offsets.iter().enumerate() {
                let c = $crate::script::helpers::parse_color(&colors[i], "addColorStop")?;
                stops.push(*off);
                stops.push(c.r as f32);
                stops.push(c.g as f32);
                stops.push(c.b as f32);
                stops.push(c.a as f32);
            }
            $rec . record_draw_op($id, DrawOp::ScriptSetStyleRadialGradient {
                stroke,
                start: [x0, y0, r0],
                end: [x1, y1, r1],
                stops,
            });
        }}
        $binding! { node $rec $id canvas_set_global_alpha ($id: &str, alpha: f32) {
            $rec . record_draw_op($id, DrawOp::SetGlobalAlpha { alpha: alpha.clamp(0.0, 1.0) });
        }}
        $binding! { node $rec $id canvas_set_shadow ($id: &str, color: String, blur: f32, dx: f32, dy: f32) {
            let c = $crate::script::helpers::parse_color(&color, "shadowColor")?;
            $rec . record_draw_op($id, DrawOp::ScriptSetShadow { color: c, blur, dx, dy });
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
        // 脚本侧 `ctx.drawImage`（image mark / 内联位图）。字节来自 JS 持有的
        // 编码图像，走 `DrawOp::ScriptImage` 中间态，由 execute_draw_op 内联进
        // 帧字节表（与 `ScriptRuntimeEffect` 同一分层：记录器没有 bytes 表）。
        $binding! { node $rec $id canvas_draw_image_bytes ($id: &str, bytes: Vec<u8>, values: Vec<f32>, alpha: f32, anti_alias: bool) {
            let _ = (alpha, anti_alias);
            if values.len() < 4 {
                return Err($crate::script::helpers::script_error(
                    "drawImage",
                    "expected destination rect as [x, y, width, height]".to_string(),
                ));
            }
            let dst = Rect4 { x: values[0], y: values[1], width: values[2], height: values[3] };
            let src = match values.len() {
                8.. => Some(Rect4 { x: values[4], y: values[5], width: values[6], height: values[7] }),
                _ => None,
            };
            $rec . record_draw_op($id, DrawOp::ScriptImage {
                bytes,
                src,
                dst,
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

        // ── Cmd: script-side DOM tree (overlay) ────────────────────────────
        // 官方插件建节点、搬子级（SplitText 的 char/word/line wrapper）。
        // 组合的树是声明式的，脚本改动记进覆盖层；渲染面按覆盖层读这一帧
        // 的子级视图。节点身份是 realm 级（插件会复用模块级草稿元素），
        // 结构是帧级（脚本每帧重跑）。返回的 id 走常规 style 写通道，所以
        // gsap 对 wrapper 的 tween 与对声明节点的 tween 是同一条路。
        $binding! { cmd $store dom_create_element (tag: String) -> String {
            let overlay = $store.tree_overlay_mut()
                .ok_or_else(|| anyhow::anyhow!("createElement: this recorder has no script tree"))?;
            Ok(overlay.create_element(&tag))
        }}
        $binding! { cmd $store dom_create_text_node (text: String) -> String {
            let overlay = $store.tree_overlay_mut()
                .ok_or_else(|| anyhow::anyhow!("createTextNode: this recorder has no script tree"))?;
            Ok(overlay.create_text_node(&text))
        }}
        $binding! { cmd $store dom_append_child (parent: String, child: String) -> bool {
            let overlay = $store.tree_overlay_mut()
                .ok_or_else(|| anyhow::anyhow!("appendChild: this recorder has no script tree"))?;
            Ok(overlay.append_child(&parent, &child))
        }}
        $binding! { cmd $store dom_insert_before (parent: String, child: String, reference: Option<String>) -> bool {
            let overlay = $store.tree_overlay_mut()
                .ok_or_else(|| anyhow::anyhow!("insertBefore: this recorder has no script tree"))?;
            Ok(overlay.insert_before(&parent, &child, reference.as_deref()))
        }}
        $binding! { cmd $store dom_remove_child (parent: String, child: String) -> bool {
            let overlay = $store.tree_overlay_mut()
                .ok_or_else(|| anyhow::anyhow!("removeChild: this recorder has no script tree"))?;
            Ok(overlay.remove_child(&parent, &child))
        }}
        $binding! { cmd $store dom_remove (id: String) -> bool {
            let overlay = $store.tree_overlay_mut()
                .ok_or_else(|| anyhow::anyhow!("remove: this recorder has no script tree"))?;
            Ok(overlay.remove(&id))
        }}
        $binding! { cmd $store dom_replace_child (parent: String, new_child: String, old_child: String) -> bool {
            let overlay = $store.tree_overlay_mut()
                .ok_or_else(|| anyhow::anyhow!("replaceChild: this recorder has no script tree"))?;
            Ok(overlay.replace_child(&parent, &new_child, &old_child))
        }}
        $binding! { cmd $store dom_replace_with (old_child: String, new_children: Vec<String>) -> bool {
            let overlay = $store.tree_overlay_mut()
                .ok_or_else(|| anyhow::anyhow!("replaceWith: this recorder has no script tree"))?;
            Ok(overlay.replace_with(&old_child, &new_children))
        }}
        $binding! { cmd $store dom_clone_node (id: String, deep: bool) -> Option<String> {
            let overlay = $store.tree_overlay_mut()
                .ok_or_else(|| anyhow::anyhow!("cloneNode: this recorder has no script tree"))?;
            Ok(overlay.clone_node(&id, deep))
        }}
        $binding! { cmd $store dom_set_text (id: String, text: String) -> bool {
            let overlay = $store.tree_overlay_mut()
                .ok_or_else(|| anyhow::anyhow!("textContent: this recorder has no script tree"))?;
            Ok(overlay.set_text(&id, &text))
        }}
        $binding! { cmd $store dom_normalize (id: String) -> bool {
            // 相邻文本节点合并。渲染面按 byte range 分组，不读节点切分，
            // 所以合并只影响 JS 侧的 childNodes 视图。
            let overlay = $store.tree_overlay_mut()
                .ok_or_else(|| anyhow::anyhow!("normalize: this recorder has no script tree"))?;
            Ok(overlay.normalize(&id))
        }}

        // ── Cmd: script-side DOM tree reads ────────────────────────────────
        // 读视图要惰性物化文本节点（`<p>` 的文本是合成子节点），所以
        // 读也走可变路径 —— cmd 而不是 qry。
        $binding! { cmd $store dom_children (id: String) -> Vec<(String, u8)> {
            let overlay = $store.tree_overlay_mut()
                .ok_or_else(|| anyhow::anyhow!("childNodes: this recorder has no script tree"))?;
            Ok(overlay.children(&id))
        }}
        $binding! { cmd $store dom_text (id: String) -> String {
            let overlay = $store.tree_overlay_mut()
                .ok_or_else(|| anyhow::anyhow!("textContent: this recorder has no script tree"))?;
            Ok(overlay.text_of(&id))
        }}
        $binding! { cmd $store dom_siblings (id: String) -> Vec<Option<String>> {
            let overlay = $store.tree_overlay_mut()
                .ok_or_else(|| anyhow::anyhow!("siblings: this recorder has no script tree"))?;
            let (prev, next) = overlay.siblings(&id);
            Ok(vec![prev, next])
        }}
        $binding! { qry $store dom_parent (id: String) -> Option<String> {
            Ok($store.tree_overlay().and_then(|overlay| overlay.parent_of(&id)))
        }}
        $binding! { qry $store dom_node_type (id: String) -> u8 {
            Ok($store.tree_overlay()
                .map(|overlay| overlay.node_type_of(&id).dom_node_type())
                .unwrap_or(1))
        }}
        $binding! { qry $store dom_is_dynamic (id: String) -> bool {
            Ok($store.tree_overlay().is_some_and(|overlay| overlay.is_dynamic(&id)))
        }}
        $binding! { qry $store dom_tag (id: String) -> Option<String> {
            Ok($store.tree_overlay()
                .and_then(|overlay| overlay.tag_of(&id))
                .map(str::to_string))
        }}
        $binding! { qry $store dom_class_name (id: String) -> String {
            Ok($store.tree_overlay()
                .map(|overlay| overlay.class_name_of(&id))
                .unwrap_or_default())
        }}
        $binding! { cmd $store dom_set_class_name (id: String, class_name: String) -> bool {
            // `className` 是节点事实而不是可动画样式：SplitText 给 mask 元素
            // 派生 `-mask` 类名并读回，写进样式表通道会被静默丢弃。
            let overlay = $store.tree_overlay_mut()
                .ok_or_else(|| anyhow::anyhow!("className: this recorder has no script tree"))?;
            Ok(overlay.set_class_name(&id, &class_name))
        }}

        $binding! { qry $store dom_get_attribute (id: String, name: String) -> Option<String> {
            // 脚本写入的节点属性。MorphSVG 用 `data-original` 记住原始 `d`
            // （变形后要能 revert），SplitText/DrawSVG 也读写 `class`/`d`。
            // 未写过的属性返回 None，调用方退回声明值。
            Ok($store.tree_overlay().and_then(|overlay| overlay.attribute_of(&id, &name)))
        }}
        $binding! { cmd $store dom_set_attribute (id: String, name: String, value: String) -> bool {
            let overlay = $store.tree_overlay_mut()
                .ok_or_else(|| anyhow::anyhow!("setAttribute: this recorder has no script tree"))?;
            Ok(overlay.set_attribute(&id, &name, &value))
        }}

        $binding! { cmd $store dom_text_range (id: String) -> Option<(u32, u32, String)> {
            // SplitText 的 wrapper 是空壳：它的盒子来自它承载的那段原文。
            // 沿父链上溯到声明文本节点，累加前置兄弟的文本长度得到区间。
            let overlay = $store.tree_overlay_mut()
                .ok_or_else(|| anyhow::anyhow!("text range: this recorder has no script tree"))?;
            Ok(overlay
                .text_range_of(&id)
                .map(|(start, end, owner)| (start as u32, end as u32, owner)))
        }}

        // ── Qry: element boxes (SplitText line detection / transform-origin) ──
        // 一个元素的盒子 = 该元素文本按字节 range 聚合出的字形盒。ranges 为
        // 空时按字素切（调用方要整元素盒）。盒子是墨迹边界，与浏览器对
        // 文本元素报的紧致 inline 盒同义。
        $binding! { qry $store text_element_boxes (id: String, ranges: Vec<(u32, u32)>) -> Vec<serde_json::Value> {
            let text = $store.get_text_source(&id).map(|s| s.text.clone()).unwrap_or_default();
            let style = $store.text_style(&id).cloned().unwrap_or_default();
            let read_f32 = |key: &str| -> f32 {
                $store.read_style_value(&id, key).and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(0.0)
            };
            let origin = (read_f32("left"), read_f32("top"));
            let width = read_f32("width");
            let ranges: Vec<std::ops::Range<usize>> = if ranges.is_empty() {
                vec![0..text.len()]
            } else {
                ranges.into_iter()
                    .map(|(start, end)| (start as usize).min(text.len())..(end as usize).min(text.len()))
                    .collect()
            };
            let db = crate::text::script_font_db();
            let boxes = crate::text::script_text_boxes(
                &text, &style, origin, width, width > 0.0, &db, &ranges,
            );
            Ok(boxes.into_iter()
                .map(|b| serde_json::json!({
                    "left": b.left,
                    "top": b.top,
                    "width": b.width,
                    "height": b.height,
                }))
                .collect())
        }}

        $binding! { qry $store read_style_value (id: String, property: String) -> Option<serde_json::Value> {
            Ok($store.read_style_value(&id, &property))
        }}

        // DOM geometry read: explicit box values for `getBoundingClientRect` /
        // `offsetWidth`. Composed from the same read path as `read_style_value`
        // (frame mutations first, parse-time initial styles as the base), so a
        // script that wrote width/left this frame sees its own write.
        $binding! { qry $store read_box (id: String) -> serde_json::Value {
            fn read_f32(store: &MutationStore, id: &str, key: &str) -> f32 {
                store.read_style_value(id, key).and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(0.0)
            }
            Ok(serde_json::json!({
                "left": read_f32($store, &id, "left"),
                "top": read_f32($store, &id, "top"),
                "width": read_f32($store, &id, "width"),
                "height": read_f32($store, &id, "height"),
            }))
        }}

        // Viewport-relative rect from this frame's geometry pass. Only populated
        // for elements with no declared box (inline text), where `read_box`
        // cannot answer; `None` everywhere else.
        $binding! { qry $store read_layout_box (id: String) -> serde_json::Value {
            Ok(match $crate::script::layout_probe::box_for(&id) {
                Some(b) => serde_json::json!({
                    "left": b.x, "top": b.y, "width": b.width, "height": b.height,
                }),
                None => serde_json::Value::Null,
            })
        }}

        $binding! { node $rec $id write_style_value ($id: &str, property: String, value: serde_json::Value) {
            $rec . write_style_value($id, &property, value);
        }}

        // ── Pure: no store (4 entries: text measure, random, graphemes, easing) ─
        // Local data mirror for offline `fetch` shims: chart libraries fetch
        // remote JSON/CSV; the harness mirrors those files under
        // `<base-dir>/assets/` and the JS shim maps URL → filename first.
        // Goes through the same asset reader as three's FileLoader
        // (base_dir join), so the path stays relative to --base-dir.
        $binding! { cmd $store asset_fetch (name: String) -> String {
            let bytes = $store.asset_read(&format!("assets/{}", name))?;
            let text = String::from_utf8(bytes)
                .map_err(|e| anyhow::anyhow!("asset_fetch: assets/{name} is not utf-8: {e}"))?;
            Ok(text)
        }}
        // 编码图像的自然尺寸（`Image` 元素的 naturalWidth/naturalHeight）。
        // 解码器由宿主注入；失败返回空表（JS 侧当作加载失败）。
        $binding! { cmd $store image_probe (data: Vec<u8>) -> Vec<f32> {
            match $store.image_decode(&data) {
                Ok((width, height, _)) => Ok(vec![width as f32, height as f32]),
                Err(_) => Ok(Vec::new()),
            }
        }}

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
        // 圆角矩形 / 线性渐变填充：`stops` 是 [offset, r, g, b, a] 扁平数组
        // （offset 0..1，分量 0..1），空 = 纯色 `color`。轴 = (ax,ay)->(bx,by)。
        $binding! { pure surface_fill_shape (id: String, x: f64, y: f64, w: f64, h: f64, radius: f64, ax: f64, ay: f64, bx: f64, by: f64, stops: Vec<f64>, color: String) -> bool {
            let c = $crate::script::helpers::parse_color(&color, "surface.fillShape")?;
            let flat = [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0, c.a as f32 / 255.0];
            let mut parsed: Vec<(f32, [f32; 4])> = Vec::with_capacity(stops.len() / 5);
            for chunk in stops.chunks_exact(5) {
                parsed.push((chunk[0] as f32, [chunk[1] as f32, chunk[2] as f32, chunk[3] as f32, chunk[4] as f32]));
            }
            if parsed.is_empty() {
                parsed.push((0.0, flat));
            }
            Ok($crate::text::surface::surface_fill_shape(
                &id, x, y, w, h, radius, ([ax, ay], [bx, by]), &parsed,
            ).is_ok())
        }}
        $binding! { pure surface_measure_text (text: String, family: String, weight: u32, size_px: f64, letter_spacing_px: f64) -> $crate::text::surface::InkMetrics {
            let metrics = $crate::text::surface::surface_measure_text(&text, &family, weight, size_px, letter_spacing_px)?;
            Ok(metrics)
        }}
        $binding! { pure surface_fill_text (id: String, text: String, x: f64, y: f64, family: String, weight: u32, size_px: f64, letter_spacing_px: f64, scale_x: f64, color: String) -> bool {
            let c = $crate::script::helpers::parse_color(&color, "surface.fillText")?;
            Ok($crate::text::surface::surface_fill_text(
                &id, &text, x, y, &family, weight, size_px, letter_spacing_px, scale_x,
                [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0, c.a as f32 / 255.0],
            ).is_ok())
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
        // SVG geometry for the DOM host (`getBBox` / `getTotalLength`). Both
        // measure the path data the composition declared; the same kurbo
        // parsers the render path uses, so a plugin's numbers and the drawn
        // shape can never disagree.
        $binding! { pure path_bbox (data: String) -> Vec<f32> {
            let [x, y, w, h] = $crate::resolve::path_bounds::compute_view_box(&[data])?;
            Ok(vec![x, y, w, h])
        }}
        $binding! { pure path_total_length (data: String) -> f32 {
            Ok($crate::script::animate::path_measure::PathMeasureEntry::from_svg(&data)
                .map(|entry| entry.total_length)
                .unwrap_or(0.0))
        }}
    };
}
