//! `CanvasKitCanvas2D`：核心 `Canvas2D` trait 的 CanvasKit 后端。
//!
//! Plan B 已填实状态栈/变换/裁剪/基础几何/paint converter（28 个方法）。
//! Image/Picture/Text/RuntimeEffect 将在 Plan C/D 逐一填实（10 个方法仍为 `todo!()`）。

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
        _image: &Self::Image,
        _x: f32,
        _y: f32,
        _paint: Option<&PaintSpec>,
    ) {
        todo!("M2: CKCanvas::drawImage")
    }
    fn draw_image_rect(
        &mut self,
        _image: &Self::Image,
        _src: Option<&Rect>,
        _dst: &Rect,
        _paint: Option<&PaintSpec>,
    ) {
        todo!("M2: CKCanvas::drawImageRect")
    }

    // ── Text ─────────────────────────────────────────────────────

    fn draw_simple_text(
        &mut self,
        _text: &str,
        _x: f32,
        _y: f32,
        _font_size: f32,
        _paint: &PaintSpec,
    ) {
        todo!("M2: CKCanvas::drawSimpleText (needs Typeface bridge)")
    }
    fn draw_glyph_run(&mut self, _run: &GlyphRunSpec, _paint: &PaintSpec) {
        todo!("M2: CKCanvas::drawGlyphs (needs Typeface bridge)")
    }

    // ── Picture ──────────────────────────────────────────────────

    fn make_picture<R>(&mut self, _bounds: &Rect, _record: R) -> Self::Picture
    where
        R: FnOnce(&mut Self),
        Self: Sized,
    {
        todo!("M2: PictureRecorder + finishRecordingAsPicture")
    }
    fn draw_picture(
        &mut self,
        _picture: &Self::Picture,
        _matrix: Option<&[f32; 9]>,
        _paint: Option<&PaintSpec>,
    ) {
        todo!("M2: CKCanvas::drawPicture")
    }

    // ── Runtime Effect ───────────────────────────────────────────

    fn draw_runtime_effect(
        &mut self,
        _effect: &Self::RuntimeEffect,
        _uniforms: &[u8],
        _children: &[RuntimeEffectChild<'_, Self>],
        _dst: &Rect,
    ) {
        todo!("M2: RuntimeEffect::makeShader → setShader → drawRect")
    }
    fn make_runtime_effect(&self, _sksl: &str) -> Result<Self::RuntimeEffect, String> {
        todo!("M2: CanvasKit.RuntimeEffect.Make(sksl)")
    }

    // ── Factory ──────────────────────────────────────────────────

    fn make_path_from_verbs(
        &self,
        verbs: &[u8],
        points: &[f32],
        fill_type: FillType,
    ) -> Self::Path {
        let path_handle = crate::canvaskit::bindings::ck_new_path()
            .expect("CanvasKit.Path() ctor failed; ensure init_canvaskit() was called");
        let path: &crate::canvaskit::bindings::CKPath =
            path_handle.as_js().unchecked_ref();

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
                    path.move_to(points[pi], points[pi + 1]);
                    pi += 2;
                }
                1 => {
                    path.line_to(points[pi], points[pi + 1]);
                    pi += 2;
                }
                2 => {
                    path.quad_to(points[pi], points[pi + 1], points[pi + 2], points[pi + 3]);
                    pi += 4;
                }
                3 => {
                    // Conic verb: points[pi..pi+5] = (x0,y0,x1,y1,w).
                    // Weight w is dropped; quad_to uses only 4 points, pi skips all 5.
                    path.quad_to(points[pi], points[pi + 1], points[pi + 2], points[pi + 3]);
                    pi += 5;
                }
                4 => {
                    path.cubic_to(
                        points[pi], points[pi + 1], points[pi + 2],
                        points[pi + 3], points[pi + 4], points[pi + 5],
                    );
                    pi += 6;
                }
                5 => {
                    path.close_path();
                }
                _ => {
                    break;
                }
            }
        }

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
    fn render_to_image<R>(&mut self, _width: u32, _height: u32, _draw: R) -> Self::Image
    where
        R: FnOnce(&mut Self),
        Self: Sized,
    {
        todo!("M2: MakeSurface → getCanvas → draw → makeImageSnapshot")
    }
}
