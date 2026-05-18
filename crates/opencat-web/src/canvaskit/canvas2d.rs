//! `CanvasKitCanvas2D`：核心 `Canvas2D` trait 的 CanvasKit 后端。
//!
//! Plan B 填实状态栈/变换/裁剪/基础几何/paint converter；Plan C 填实 Image/Picture。
//! 剩余 Text/RuntimeEffect（4 个 `todo!()`）将在 Plan D 填实。

#![cfg(target_arch = "wasm32")]

use opencat_core::canvas::{
    Canvas2D, ClipOp, FillType, GlyphRunSpec, PaintSpec, PointMode, RRect, Rect,
    RuntimeEffectChild,
};

use wasm_bindgen::{JsCast, JsValue};

use crate::canvaskit::bindings::{CKCanvas, CKPaint};
use crate::canvaskit::handle::{CKImage, CKPath, CKPicture, CKRuntimeEffect};

pub struct CanvasKitCanvas2D {
    canvas: CKCanvas,
    fill_paint: CKPaint,
    stroke_paint: CKPaint,
}

impl CanvasKitCanvas2D {
    pub fn new(canvas: CKCanvas) -> Self {
        Self {
            canvas,
            fill_paint: CKPaint::new(),
            stroke_paint: CKPaint::new(),
        }
    }

    pub fn canvas(&self) -> &CKCanvas {
        &self.canvas
    }
}

impl Canvas2D for CanvasKitCanvas2D {
    type Path = CKPath;
    type Image = CKImage;
    type Picture = CKPicture;
    type RuntimeEffect = CKRuntimeEffect;

    // ── State stack ──────────────────────────────────────────────

    fn save(&mut self) -> i32 {
        CKCanvas::save(&self.canvas)
    }
    fn save_layer(&mut self, bounds: Option<Rect>, alpha: f32) {
        let tmp = CKPaint::new();
        tmp.set_alpha(alpha);
        let bounds_js = match bounds {
            Some(r) => crate::canvaskit::bindings::ck_ltrb_rect(
                r.x0 as f32, r.y0 as f32, r.x1 as f32, r.y1 as f32,
            ),
            None => JsValue::NULL,
        };
        CKCanvas::save_layer(
            &self.canvas,
            tmp.unchecked_ref(),
            &bounds_js,
        );
    }
    fn save_layer_with(&mut self, bounds: Option<Rect>, paint: &PaintSpec) {
        let target = crate::canvaskit::paint::apply_to(&self.fill_paint, &self.stroke_paint, paint);
        let bounds_js = match bounds {
            Some(r) => crate::canvaskit::bindings::ck_ltrb_rect(
                r.x0 as f32, r.y0 as f32, r.x1 as f32, r.y1 as f32,
            ),
            None => JsValue::NULL,
        };
        CKCanvas::save_layer(
            &self.canvas,
            target.unchecked_ref(),
            &bounds_js,
        );
    }
    fn restore(&mut self) {
        CKCanvas::restore(&self.canvas);
    }
    fn restore_to_count(&mut self, count: i32) {
        CKCanvas::restore_to_count(&self.canvas, count);
    }
    fn save_count(&self) -> i32 {
        CKCanvas::save_count(&self.canvas)
    }

    // ── Transforms ───────────────────────────────────────────────

    fn translate(&mut self, dx: f32, dy: f32) {
        CKCanvas::translate(&self.canvas, dx, dy);
    }
    fn scale(&mut self, sx: f32, sy: f32) {
        CKCanvas::scale(&self.canvas, sx, sy);
    }
    fn rotate(&mut self, degrees: f32, cx: f32, cy: f32) {
        CKCanvas::rotate(&self.canvas, degrees, cx, cy);
    }
    fn skew(&mut self, sx: f32, sy: f32) {
        CKCanvas::skew(&self.canvas, sx, sy);
    }
    fn concat(&mut self, matrix: &[f32; 9]) {
        let arr = js_sys::Float32Array::new_with_length(9);
        for (i, v) in matrix.iter().enumerate() {
            arr.set_index(i as u32, *v);
        }
        CKCanvas::concat(&self.canvas, &arr.into());
    }

    // ── Clipping ─────────────────────────────────────────────────

    fn clip_rect(&mut self, rect: &Rect, op: ClipOp, anti_alias: bool) {
        let js_rect = crate::canvaskit::bindings::ck_ltrb_rect(
            rect.x0 as f32, rect.y0 as f32, rect.x1 as f32, rect.y1 as f32,
        );
        let js_op = crate::canvaskit::convert::ck_clip_op(op);
        CKCanvas::clip_rect(&self.canvas, &js_rect, &js_op, anti_alias);
    }
    fn clip_rrect(&mut self, rrect: &RRect, op: ClipOp, anti_alias: bool) {
        let js_rrect = crate::canvaskit::convert::ck_rrect_from_kurbo(rrect);
        let js_op = crate::canvaskit::convert::ck_clip_op(op);
        CKCanvas::clip_rrect(&self.canvas, &js_rrect, &js_op, anti_alias);
    }
    fn clip_path(&mut self, path: &Self::Path, op: ClipOp, anti_alias: bool) {
        let js_op = crate::canvaskit::convert::ck_clip_op(op);
        CKCanvas::clip_path(&self.canvas, path.as_js(), &js_op, anti_alias);
    }

    // ── Basic geometry ───────────────────────────────────────────

    fn clear(&mut self, color: [f32; 4]) {
        let js_color = crate::canvaskit::bindings::ck_color4f(
            color[0], color[1], color[2], color[3],
        );
        CKCanvas::clear(&self.canvas, &js_color);
    }
    fn draw_paint(&mut self, paint: &PaintSpec) {
        let target = crate::canvaskit::paint::apply_to(&self.fill_paint, &self.stroke_paint, paint);
        CKCanvas::draw_paint(&self.canvas, target.unchecked_ref());
    }
    fn draw_rect(&mut self, rect: &Rect, paint: &PaintSpec) {
        let js_rect = crate::canvaskit::bindings::ck_ltrb_rect(
            rect.x0 as f32, rect.y0 as f32, rect.x1 as f32, rect.y1 as f32,
        );
        let target = crate::canvaskit::paint::apply_to(&self.fill_paint, &self.stroke_paint, paint);
        CKCanvas::draw_rect(&self.canvas, &js_rect, target.unchecked_ref());
    }
    fn draw_rrect(&mut self, rrect: &RRect, paint: &PaintSpec) {
        let js_rrect = crate::canvaskit::convert::ck_rrect_from_kurbo(rrect);
        let target = crate::canvaskit::paint::apply_to(&self.fill_paint, &self.stroke_paint, paint);
        CKCanvas::draw_rrect(&self.canvas, &js_rrect, target.unchecked_ref());
    }
    fn draw_drrect(&mut self, outer: &RRect, inner: &RRect, paint: &PaintSpec) {
        let js_outer = crate::canvaskit::convert::ck_rrect_from_kurbo(outer);
        let js_inner = crate::canvaskit::convert::ck_rrect_from_kurbo(inner);
        let target = crate::canvaskit::paint::apply_to(&self.fill_paint, &self.stroke_paint, paint);
        CKCanvas::draw_drrect(&self.canvas, &js_outer, &js_inner, target.unchecked_ref());
    }
    fn draw_oval(&mut self, oval: &Rect, paint: &PaintSpec) {
        let js_oval = crate::canvaskit::bindings::ck_ltrb_rect(
            oval.x0 as f32, oval.y0 as f32, oval.x1 as f32, oval.y1 as f32,
        );
        let target = crate::canvaskit::paint::apply_to(&self.fill_paint, &self.stroke_paint, paint);
        CKCanvas::draw_oval(&self.canvas, &js_oval, target.unchecked_ref());
    }
    fn draw_circle(&mut self, cx: f32, cy: f32, radius: f32, paint: &PaintSpec) {
        let target = crate::canvaskit::paint::apply_to(&self.fill_paint, &self.stroke_paint, paint);
        CKCanvas::draw_circle(&self.canvas, cx, cy, radius, target.unchecked_ref());
    }
    fn draw_arc(
        &mut self,
        oval: &Rect,
        start: f32,
        sweep: f32,
        use_center: bool,
        paint: &PaintSpec,
    ) {
        let js_oval = crate::canvaskit::bindings::ck_ltrb_rect(
            oval.x0 as f32, oval.y0 as f32, oval.x1 as f32, oval.y1 as f32,
        );
        let target = crate::canvaskit::paint::apply_to(&self.fill_paint, &self.stroke_paint, paint);
        CKCanvas::draw_arc(
            &self.canvas, &js_oval, start, sweep, use_center, target.unchecked_ref(),
        );
    }
    fn draw_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, paint: &PaintSpec) {
        let target = crate::canvaskit::paint::apply_to(&self.fill_paint, &self.stroke_paint, paint);
        CKCanvas::draw_line(&self.canvas, x0, y0, x1, y1, target.unchecked_ref());
    }
    fn draw_points(&mut self, mode: PointMode, points: &[f32], paint: &PaintSpec) {
        let js_mode = crate::canvaskit::convert::ck_point_mode(mode);
        let arr = js_sys::Float32Array::new_with_length(points.len() as u32);
        for (i, v) in points.iter().enumerate() {
            arr.set_index(i as u32, *v);
        }
        let target = crate::canvaskit::paint::apply_to(&self.fill_paint, &self.stroke_paint, paint);
        CKCanvas::draw_points(
            &self.canvas, &js_mode, &arr.into(), target.unchecked_ref(),
        );
    }
    fn draw_path(&mut self, path: &Self::Path, paint: &PaintSpec) {
        let target = crate::canvaskit::paint::apply_to(&self.fill_paint, &self.stroke_paint, paint);
        CKCanvas::draw_path(&self.canvas, path.as_js(), target.unchecked_ref());
    }

    // ── Image ────────────────────────────────────────────────────

    fn draw_image(
        &mut self,
        image: &Self::Image,
        x: f32,
        y: f32,
        paint: Option<&PaintSpec>,
    ) {
        let target_opt = paint.map(|spec| {
            crate::canvaskit::paint::apply_to(&self.fill_paint, &self.stroke_paint, spec)
        });
        let null_js = wasm_bindgen::JsValue::NULL;
        let paint_ref: &wasm_bindgen::JsValue = match target_opt {
            Some(t) => t.unchecked_ref(),
            None => &null_js,
        };
        crate::canvaskit::bindings::CKCanvas::draw_image(&self.canvas, image.as_js(), x, y, paint_ref);
    }
    fn draw_image_rect(
        &mut self,
        image: &Self::Image,
        src: Option<&Rect>,
        dst: &Rect,
        paint: Option<&PaintSpec>,
    ) {
        let src_js = match src {
            Some(r) => crate::canvaskit::bindings::ck_ltrb_rect(
                r.x0 as f32, r.y0 as f32, r.x1 as f32, r.y1 as f32,
            ),
            None => {
                let img_js = image.as_js();
                let img_inst: &crate::canvaskit::bindings::CKImageJs = img_js.unchecked_ref();
                let w = img_inst.image_width() as f32;
                let h = img_inst.image_height() as f32;
                crate::canvaskit::bindings::ck_ltrb_rect(0.0, 0.0, w, h)
            }
        };
        let dst_js = crate::canvaskit::bindings::ck_ltrb_rect(
            dst.x0 as f32, dst.y0 as f32, dst.x1 as f32, dst.y1 as f32,
        );
        let target_opt = paint.map(|spec| {
            crate::canvaskit::paint::apply_to(&self.fill_paint, &self.stroke_paint, spec)
        });
        let null_js = wasm_bindgen::JsValue::NULL;
        let paint_ref: &wasm_bindgen::JsValue = match target_opt {
            Some(t) => t.unchecked_ref(),
            None => &null_js,
        };
        crate::canvaskit::bindings::CKCanvas::draw_image_rect(
            &self.canvas, image.as_js(), &src_js, &dst_js, paint_ref,
        );
    }

    // ── Text ─────────────────────────────────────────────────────

    fn draw_simple_text(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        paint: &PaintSpec,
    ) {
        use opencat_core::text::{GlyphData, commands_to_verbs_points, rasterize_glyphs};
        use opencat_core::style::ComputedTextStyle;

        let mut style = ComputedTextStyle::default();
        style.text_px = font_size;
        let raster = rasterize_glyphs(text, &style, f32::INFINITY, false, false);

        for line in &raster.lines {
            for pos in &line.positions {
                let glyph_data = match raster.glyphs.get(&pos.cache_key) {
                    Some(d) => d,
                    None => continue,
                };
                let abs_x = x + pos.x;
                let abs_y = y + pos.y;

                match glyph_data {
                    GlyphData::Outline(commands, upem) => {
                        let scale = font_size / *upem;
                        let (verbs, pts) = commands_to_verbs_points(commands, 1.0);
                        let path = self.make_path_from_verbs(&verbs, &pts, FillType::Winding);
                        self.save();
                        self.translate(abs_x, abs_y);
                        if (scale - 1.0).abs() > f32::EPSILON {
                            self.scale(scale, scale);
                        }
                        self.draw_path(&path, paint);
                        self.restore();
                    }
                    GlyphData::ColorImage {
                        rgba,
                        width,
                        height,
                        placement_left,
                        placement_top,
                    } => {
                        let img = self.make_image_from_rgba(rgba, *width, *height);
                        self.draw_image(
                            &img,
                            abs_x + *placement_left as f32,
                            abs_y - *placement_top as f32,
                            Some(paint),
                        );
                    }
                }
            }
        }
    }
    fn draw_glyph_run(&mut self, _run: &GlyphRunSpec, _paint: &PaintSpec) {
        // GlyphRunSpec-based rendering deferred — core pipeline uses cosmic-text path/image.
    }

    // ── Picture ──────────────────────────────────────────────────

    fn make_picture<R>(&mut self, bounds: &Rect, record: R) -> Self::Picture
    where
        R: FnOnce(&mut Self),
        Self: Sized,
    {
        let bounds_js = crate::canvaskit::bindings::ck_ltrb_rect(
            bounds.x0 as f32, bounds.y0 as f32, bounds.x1 as f32, bounds.y1 as f32,
        );
        let recorder = crate::canvaskit::bindings::ck_new_picture_recorder()
            .expect("PictureRecorder() ctor failed; ensure init_canvaskit() was called");
        let recording_canvas = crate::canvaskit::bindings::CKPictureRecorder::begin_recording(
            &recorder, &bounds_js,
        );

        let mut temp = CanvasKitCanvas2D::new(recording_canvas);
        record(&mut temp);
        drop(temp);

        let picture_js = crate::canvaskit::bindings::CKPictureRecorder::finish_recording_as_picture(&recorder);
        crate::canvaskit::bindings::CKPictureRecorder::delete_recorder(&recorder);
        crate::canvaskit::handle::CKHandle::wrap(picture_js)
    }
    fn draw_picture(
        &mut self,
        picture: &Self::Picture,
        matrix: Option<&[f32; 9]>,
        paint: Option<&PaintSpec>,
    ) {
        let need_save = matrix.is_some() || paint.is_some();
        if need_save {
            crate::canvaskit::bindings::CKCanvas::save(&self.canvas);
        }

        if let Some(spec) = paint {
            let target = crate::canvaskit::paint::apply_to(&self.fill_paint, &self.stroke_paint, spec);
            crate::canvaskit::bindings::CKCanvas::save_layer(
                &self.canvas,
                target.unchecked_ref(),
                &wasm_bindgen::JsValue::NULL,
            );
        }

        if let Some(m) = matrix {
            let arr = js_sys::Float32Array::new_with_length(9);
            for (i, v) in m.iter().enumerate() {
                arr.set_index(i as u32, *v);
            }
            crate::canvaskit::bindings::CKCanvas::concat(&self.canvas, &arr.into());
        }

        crate::canvaskit::bindings::CKCanvas::draw_picture(&self.canvas, picture.as_js());

        if paint.is_some() {
            crate::canvaskit::bindings::CKCanvas::restore(&self.canvas);
        }
        if need_save {
            crate::canvaskit::bindings::CKCanvas::restore(&self.canvas);
        }
    }

    // ── Runtime Effect ───────────────────────────────────────────

    fn draw_runtime_effect(
        &mut self,
        effect: &Self::RuntimeEffect,
        uniforms: &[u8],
        children: &[RuntimeEffectChild<'_, Self>],
        dst: &Rect,
    ) {
        // 1. uniforms bytes → Float32Array (interpreting bytes as f32 LE pairs)
        let uniforms_arr = if uniforms.len() % 4 == 0 {
            let n = uniforms.len() / 4;
            let arr = js_sys::Float32Array::new_with_length(n as u32);
            for i in 0..n {
                let bytes = [
                    uniforms[i * 4],
                    uniforms[i * 4 + 1],
                    uniforms[i * 4 + 2],
                    uniforms[i * 4 + 3],
                ];
                arr.set_index(i as u32, f32::from_le_bytes(bytes));
            }
            arr
        } else {
            js_sys::Float32Array::new_with_length(0)
        };

        // 2. children → JS Array of shader-like values
        let children_arr = js_sys::Array::new();
        for child in children {
            let shader_js: wasm_bindgen::JsValue = match child {
                RuntimeEffectChild::Texture(img) => img.as_js().clone(),
                RuntimeEffectChild::Picture(picture) => {
                    let pic_js = picture.as_js();
                    let make_shader_fn = js_sys::Reflect::get(
                        pic_js,
                        &wasm_bindgen::JsValue::from_str("makeShader"),
                    )
                    .ok();
                    match make_shader_fn {
                        Some(f) if f.is_function() => {
                            let func = f.unchecked_ref::<js_sys::Function>();
                            let m = crate::canvaskit::module::ck();
                            let tile_clamp = js_sys::Reflect::get(m, &wasm_bindgen::JsValue::from_str("TileMode"))
                                .ok()
                                .and_then(|g| {
                                    js_sys::Reflect::get(&g, &wasm_bindgen::JsValue::from_str("Decal")).ok()
                                })
                                .unwrap_or(wasm_bindgen::JsValue::UNDEFINED);
                            let filter = wasm_bindgen::JsValue::NULL;
                            match func.call3(pic_js, &tile_clamp, &tile_clamp, &filter).ok() {
                                Some(s) if !s.is_null() && !s.is_undefined() => s,
                                _ => wasm_bindgen::JsValue::NULL,
                            }
                        }
                        _ => wasm_bindgen::JsValue::NULL,
                    }
                }
                RuntimeEffectChild::Shader(shader_spec) => {
                    crate::canvaskit::bindings::build_ck_shader(shader_spec)
                        .map(|h| h.as_js().clone())
                        .unwrap_or(wasm_bindgen::JsValue::NULL)
                }
            };
            children_arr.push(&shader_js);
        }

        // 3. effect.makeShader(uniforms, children) → shader
        let effect_js: &crate::canvaskit::bindings::CKRuntimeEffectJs =
            effect.as_js().unchecked_ref();
        let shader_js = crate::canvaskit::bindings::CKRuntimeEffectJs::make_shader(
            effect_js,
            &uniforms_arr.into(),
            &children_arr.into(),
        );
        if shader_js.is_null() || shader_js.is_undefined() {
            return;
        }

        // 4. set shader on fill_paint, drawRect(dst, paint)
        self.fill_paint.set_shader(&shader_js);
        let dst_js = crate::canvaskit::bindings::ck_ltrb_rect(
            dst.x0 as f32,
            dst.y0 as f32,
            dst.x1 as f32,
            dst.y1 as f32,
        );
        crate::canvaskit::bindings::CKCanvas::draw_rect(
            &self.canvas,
            &dst_js,
            self.fill_paint.unchecked_ref(),
        );

        // 5. Reset shader to avoid polluting subsequent draws
        self.fill_paint.set_shader(&wasm_bindgen::JsValue::NULL);
    }
    fn make_runtime_effect(&self, sksl: &str) -> Result<Self::RuntimeEffect, String> {
        crate::canvaskit::bindings::ck_make_runtime_effect(sksl).ok_or_else(|| {
            format!(
                "CanvasKit.RuntimeEffect.Make failed for SkSL: {}",
                sksl.chars().take(80).collect::<String>()
            )
        })
    }

    // ── Factory ──────────────────────────────────────────────────

    fn make_path_from_verbs(
        &self,
        verbs: &[u8],
        points: &[f32],
        fill_type: FillType,
    ) -> Self::Path {
        let builder = crate::canvaskit::bindings::ck_new_path_builder()
            .expect("CanvasKit.PathBuilder() ctor failed; ensure init_canvaskit() was called");

        let mut pi = 0usize;
        let n = points.len();
        for v in verbs {
            let needed = match *v {
                0 | 1 => 2,
                2 => 4,
                3 => 5,
                4 => 6,
                5 => 0,
                _ => 0,
            };
            if pi + needed > n {
                break;
            }
            match *v {
                0 => {
                    crate::canvaskit::bindings::CKPathBuilder::pb_move_to(&builder, points[pi], points[pi + 1]);
                    pi += 2;
                }
                1 => {
                    crate::canvaskit::bindings::CKPathBuilder::pb_line_to(&builder, points[pi], points[pi + 1]);
                    pi += 2;
                }
                2 => {
                    crate::canvaskit::bindings::CKPathBuilder::pb_quad_to(&builder, points[pi], points[pi + 1], points[pi + 2], points[pi + 3]);
                    pi += 4;
                }
                3 => {
                    // Conic verb: points[pi..pi+5] = (x0,y0,x1,y1,w).
                    // Weight w is dropped; quad_to uses only 4 points, pi skips all 5.
                    crate::canvaskit::bindings::CKPathBuilder::pb_quad_to(&builder, points[pi], points[pi + 1], points[pi + 2], points[pi + 3]);
                    pi += 5;
                }
                4 => {
                    crate::canvaskit::bindings::CKPathBuilder::pb_cubic_to(
                        &builder,
                        points[pi], points[pi + 1], points[pi + 2],
                        points[pi + 3], points[pi + 4], points[pi + 5],
                    );
                    pi += 6;
                }
                5 => {
                    crate::canvaskit::bindings::CKPathBuilder::pb_close(&builder);
                }
                _ => {
                    break;
                }
            }
        }

        let path_js = crate::canvaskit::bindings::CKPathBuilder::snapshot(&builder);
        crate::canvaskit::bindings::CKPathBuilder::delete_builder(&builder);

        let path_handle: crate::canvaskit::handle::CKPath = crate::canvaskit::handle::CKHandle::wrap(path_js);
        let path: &crate::canvaskit::bindings::CKPath = path_handle.as_js().unchecked_ref();
        path.set_fill_type(&crate::canvaskit::convert::ck_fill_type(fill_type));
        path_handle
    }
    fn make_path_from_svg(&self, svg_path_data: &str) -> Option<Self::Path> {
        crate::canvaskit::bindings::ck_path_from_svg(svg_path_data)
    }
    fn make_image_from_rgba(&self, bytes: &[u8], width: u32, height: u32) -> Self::Image {
        crate::canvaskit::bindings::ck_make_image_from_rgba(bytes, width, height)
            .expect("CanvasKit.MakeImage failed; check info/colorType/bytes length")
    }
    fn make_image_from_encoded(&self, bytes: &[u8]) -> Option<Self::Image> {
        crate::canvaskit::bindings::ck_make_image_from_encoded(bytes)
    }
    fn render_to_image<R>(&mut self, width: u32, height: u32, draw: R) -> Self::Image
    where
        R: FnOnce(&mut Self),
        Self: Sized,
    {
        let surface = crate::canvaskit::bindings::ck_make_surface(width, height)
            .expect("CanvasKit.MakeSurface failed; check width/height");
        let offscreen_canvas = crate::canvaskit::bindings::CKSurfaceJs::surface_get_canvas(&surface);

        let mut temp = CanvasKitCanvas2D::new(offscreen_canvas);
        draw(&mut temp);
        drop(temp);

        crate::canvaskit::bindings::CKSurfaceJs::surface_flush(&surface);
        let img_js = crate::canvaskit::bindings::CKSurfaceJs::make_image_snapshot(&surface);
        crate::canvaskit::bindings::CKSurfaceJs::delete_surface(&surface);

        crate::canvaskit::handle::CKHandle::wrap(img_js)
    }
}
