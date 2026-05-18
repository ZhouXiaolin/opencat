//! `CanvasKitCanvas2D`：核心 `Canvas2D` trait 的 CanvasKit 后端。
//!
//! M1 阶段为骨架：实现 trait 的所有方法以满足编译，方法体 `todo!()`。
//! M2（后续计划）会按 `Canvas2D` trait 顺序逐一填实。

#![cfg(target_arch = "wasm32")]

use opencat_core::canvas::{
    Canvas2D, ClipOp, FillType, GlyphRunSpec, PaintSpec, PointMode, RRect, Rect,
    RuntimeEffectChild,
};

use crate::canvaskit::bindings::{CKCanvas, CKPaint};
use crate::canvaskit::handle::{CKImage, CKPath, CKPicture, CKRuntimeEffect};

pub struct CanvasKitCanvas2D {
    canvas: CKCanvas,
    #[allow(dead_code)]
    fill_paint: CKPaint,
    #[allow(dead_code)]
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
        todo!("M2: CKCanvas::save")
    }
    fn save_layer(&mut self, _bounds: Option<Rect>, _alpha: f32) {
        todo!("M2: CKCanvas::saveLayer with alpha paint")
    }
    fn save_layer_with(&mut self, _bounds: Option<Rect>, _paint: &PaintSpec) {
        todo!("M2: CKCanvas::saveLayer with PaintSpec")
    }
    fn restore(&mut self) {
        todo!("M2: CKCanvas::restore")
    }
    fn restore_to_count(&mut self, _count: i32) {
        todo!("M2: CKCanvas::restoreToCount")
    }
    fn save_count(&self) -> i32 {
        todo!("M2: CKCanvas::getSaveCount")
    }

    // ── Transforms ───────────────────────────────────────────────

    fn translate(&mut self, _dx: f32, _dy: f32) {
        todo!("M2: CKCanvas::translate")
    }
    fn scale(&mut self, _sx: f32, _sy: f32) {
        todo!("M2: CKCanvas::scale")
    }
    fn rotate(&mut self, _degrees: f32, _cx: f32, _cy: f32) {
        todo!("M2: CKCanvas::rotate")
    }
    fn skew(&mut self, _sx: f32, _sy: f32) {
        todo!("M2: CKCanvas::skew")
    }
    fn concat(&mut self, _matrix: &[f32; 9]) {
        todo!("M2: CKCanvas::concat")
    }

    // ── Clipping ─────────────────────────────────────────────────

    fn clip_rect(&mut self, _rect: &Rect, _op: ClipOp, _anti_alias: bool) {
        todo!("M2: CKCanvas::clipRect")
    }
    fn clip_rrect(&mut self, _rrect: &RRect, _op: ClipOp, _anti_alias: bool) {
        todo!("M2: CKCanvas::clipRRect")
    }
    fn clip_path(&mut self, _path: &Self::Path, _op: ClipOp, _anti_alias: bool) {
        todo!("M2: CKCanvas::clipPath")
    }

    // ── Basic geometry ───────────────────────────────────────────

    fn clear(&mut self, _color: [f32; 4]) {
        todo!("M2: CKCanvas::clear with Color4f")
    }
    fn draw_paint(&mut self, _paint: &PaintSpec) {
        todo!("M2: CKCanvas::drawPaint")
    }
    fn draw_rect(&mut self, _rect: &Rect, _paint: &PaintSpec) {
        todo!("M2: CKCanvas::drawRect")
    }
    fn draw_rrect(&mut self, _rrect: &RRect, _paint: &PaintSpec) {
        todo!("M2: CKCanvas::drawRRect")
    }
    fn draw_drrect(&mut self, _outer: &RRect, _inner: &RRect, _paint: &PaintSpec) {
        todo!("M2: CKCanvas::drawDRRect")
    }
    fn draw_oval(&mut self, _oval: &Rect, _paint: &PaintSpec) {
        todo!("M2: CKCanvas::drawOval")
    }
    fn draw_circle(&mut self, _cx: f32, _cy: f32, _radius: f32, _paint: &PaintSpec) {
        todo!("M2: CKCanvas::drawCircle")
    }
    fn draw_arc(
        &mut self,
        _oval: &Rect,
        _start: f32,
        _sweep: f32,
        _use_center: bool,
        _paint: &PaintSpec,
    ) {
        todo!("M2: CKCanvas::drawArc")
    }
    fn draw_line(&mut self, _x0: f32, _y0: f32, _x1: f32, _y1: f32, _paint: &PaintSpec) {
        todo!("M2: CKCanvas::drawLine")
    }
    fn draw_points(&mut self, _mode: PointMode, _points: &[f32], _paint: &PaintSpec) {
        todo!("M2: CKCanvas::drawPoints")
    }
    fn draw_path(&mut self, _path: &Self::Path, _paint: &PaintSpec) {
        todo!("M2: CKCanvas::drawPath")
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
        _verbs: &[u8],
        _points: &[f32],
        _fill_type: FillType,
    ) -> Self::Path {
        todo!("M2: PathBuilder via CK.Path")
    }
    fn make_path_from_svg(&self, _svg_path_data: &str) -> Option<Self::Path> {
        todo!("M2: CK.Path.MakeFromSVGString")
    }
    fn make_image_from_rgba(&self, _bytes: &[u8], _width: u32, _height: u32) -> Self::Image {
        todo!("M2: CK.MakeImage(info, bytes, rowBytes)")
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
