pub use kurbo::{Rect, RoundedRect as RRect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipOp {
    Intersect,
    Difference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillType {
    Winding,
    EvenOdd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointMode {
    Points,
    Lines,
    Polygon,
}

/// 平台无关的路径构建器 trait。
///
/// 各后端（Skia / CanvasKit）将各自的 PathBuilder 适配到此接口，
/// 使 core 层能直接拼装路径，省去 `commands → verbs+points → PathBuilder` 的序列化中转。
///
/// 基本操作（move_to / line_to / quad_to / cubic_to / close）由各后端各自实现；
/// 高级操作（add_rect / add_rrect / add_oval / add_arc）有基于基本操作的默认实现，
/// 后端可覆盖以使用原生 API（如 skia-safe PathBuilder::add_rect）。
pub trait PathBuilder {
    type Path: Clone;

    // -- 基本操作 --
    fn move_to(&mut self, x: f32, y: f32);
    fn line_to(&mut self, x: f32, y: f32);
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32);
    fn cubic_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32);
    fn close(&mut self);

    /// 消费构建器，返回最终的 `Path`。
    fn finish(self) -> Self::Path;

    // -- 高级操作（默认用基本操作模拟，后端可覆盖） --

    fn add_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.move_to(x, y);
        self.line_to(x + w, y);
        self.line_to(x + w, y + h);
        self.line_to(x, y + h);
        self.close();
    }

    fn add_rrect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        let r = r.min(w / 2.0).min(h / 2.0);
        self.move_to(x + r, y);
        self.line_to(x + w - r, y);
        self.quad_to(x + w, y, x + w, y + r);
        self.line_to(x + w, y + h - r);
        self.quad_to(x + w, y + h, x + w - r, y + h);
        self.line_to(x + r, y + h);
        self.quad_to(x, y + h, x, y + h - r);
        self.line_to(x, y + r);
        self.quad_to(x, y, x + r, y);
        self.close();
    }

    fn add_oval(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let cx = x + w / 2.0;
        let cy = y + h / 2.0;
        let rx = w / 2.0;
        let ry = h / 2.0;
        let k = 0.5522847498;
        self.move_to(cx + rx, cy);
        self.cubic_to(cx + rx, cy + ry * k, cx + rx * k, cy + ry, cx, cy + ry);
        self.cubic_to(cx - rx * k, cy + ry, cx - rx, cy + ry * k, cx - rx, cy);
        self.cubic_to(cx - rx, cy - ry * k, cx - rx * k, cy - ry, cx, cy - ry);
        self.cubic_to(cx + rx * k, cy - ry, cx + rx, cy - ry * k, cx + rx, cy);
        self.close();
    }

    fn add_arc(&mut self, x: f32, y: f32, w: f32, h: f32, start_angle: f32, sweep_angle: f32) {
        let cx = x + w / 2.0;
        let cy = y + h / 2.0;
        let rx = w / 2.0;
        let ry = h / 2.0;

        let mut angle = start_angle;
        let end_angle = start_angle + sweep_angle;
        let step = 90.0_f32.to_radians();
        let n = ((sweep_angle.abs() / step).ceil() as usize).max(1);
        let seg_sweep = sweep_angle / n as f32;

        let start_rad = angle.to_radians();
        self.move_to(
            cx + rx * start_rad.cos(),
            cy + ry * start_rad.sin(),
        );

        for _ in 0..n {
            let a1 = angle;
            let a2 = angle + seg_sweep;
            let da = seg_sweep / 2.0;
            let alpha = (4.0 / 3.0) * da.tan();
            let (cos1, sin1) = (a1.to_radians().cos(), a1.to_radians().sin());
            let (cos2, sin2) = (a2.to_radians().cos(), a2.to_radians().sin());

            self.cubic_to(
                cx + rx * (cos1 - alpha * sin1),
                cy + ry * (sin1 + alpha * cos1),
                cx + rx * (cos2 + alpha * sin2),
                cy + ry * (sin2 - alpha * cos2),
                cx + rx * cos2,
                cy + ry * sin2,
            );
            angle = a2;
        }

        if (end_angle - start_angle).abs() >= 360.0 - f32::EPSILON {
            self.close();
        }
    }
}

pub trait Canvas2D {
    type Path: Clone;
    type Image: Clone;
    type Picture: Clone;
    type RuntimeEffect: Clone;
    type PathBuilder: PathBuilder<Path = Self::Path>;

    // -- State stack --
    fn save(&mut self) -> i32;
    fn save_layer(&mut self, bounds: Option<Rect>, alpha: f32);
    fn save_layer_with(&mut self, bounds: Option<Rect>, paint: &PaintSpec);
    fn restore(&mut self);
    fn restore_to_count(&mut self, count: i32);
    fn save_count(&self) -> i32;

    // -- Transforms --
    fn translate(&mut self, dx: f32, dy: f32);
    fn scale(&mut self, sx: f32, sy: f32);
    fn rotate(&mut self, degrees: f32, cx: f32, cy: f32);
    fn skew(&mut self, sx: f32, sy: f32);
    fn concat(&mut self, matrix: &[f32; 9]);

    // -- Clipping --
    fn clip_rect(&mut self, rect: &Rect, op: ClipOp, anti_alias: bool);
    fn clip_rrect(&mut self, rrect: &RRect, op: ClipOp, anti_alias: bool);
    fn clip_path(&mut self, path: &Self::Path, op: ClipOp, anti_alias: bool);

    // -- Basic geometry --
    fn clear(&mut self, color: [f32; 4]);
    fn draw_paint(&mut self, paint: &PaintSpec);
    fn draw_rect(&mut self, rect: &Rect, paint: &PaintSpec);
    fn draw_rrect(&mut self, rrect: &RRect, paint: &PaintSpec);
    fn draw_drrect(&mut self, outer: &RRect, inner: &RRect, paint: &PaintSpec);
    fn draw_oval(&mut self, oval: &Rect, paint: &PaintSpec);
    fn draw_circle(&mut self, cx: f32, cy: f32, radius: f32, paint: &PaintSpec);
    fn draw_arc(
        &mut self,
        oval: &Rect,
        start: f32,
        sweep: f32,
        use_center: bool,
        paint: &PaintSpec,
    );
    fn draw_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, paint: &PaintSpec);
    fn draw_points(&mut self, mode: PointMode, points: &[f32], paint: &PaintSpec);
    fn draw_path(&mut self, path: &Self::Path, paint: &PaintSpec);

    // -- Image --
    fn draw_image(
        &mut self,
        image: &Self::Image,
        x: f32,
        y: f32,
        paint: Option<&PaintSpec>,
    );
    fn draw_image_rect(
        &mut self,
        image: &Self::Image,
        src: Option<&Rect>,
        dst: &Rect,
        paint: Option<&PaintSpec>,
    );

    // -- Text --
    fn draw_simple_text(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        paint: &PaintSpec,
    );
    fn draw_glyph_run(&mut self, run: &GlyphRunSpec, paint: &PaintSpec);

    // -- Picture --
    fn make_picture<R>(&mut self, bounds: &Rect, record: R) -> Self::Picture
    where
        R: FnOnce(&mut Self),
        Self: Sized;
    fn draw_picture(
        &mut self,
        picture: &Self::Picture,
        matrix: Option<&[f32; 9]>,
        paint: Option<&PaintSpec>,
    );

    // -- Runtime Effect --
    fn draw_runtime_effect(
        &mut self,
        effect: &Self::RuntimeEffect,
        uniforms: &[u8],
        children: &[RuntimeEffectChild<'_, Self>],
        dst: &Rect,
    );
    fn make_runtime_effect(&self, sksl: &str) -> Result<Self::RuntimeEffect, String>;

    // -- Factory --
    fn create_path_builder(&self, fill_type: FillType) -> Self::PathBuilder;
    fn make_path_from_svg(&self, svg_path_data: &str) -> Option<Self::Path>;
    fn make_image_from_rgba(&self, bytes: &[u8], width: u32, height: u32) -> Self::Image;
    fn make_image_from_encoded(&self, bytes: &[u8]) -> Option<Self::Image>;

    /// Try to obtain a video frame as a GPU-backed image (zero-copy).
    /// Returns `(image, width, height)` when the backend supports external
    /// texture sources (e.g. CanvasKit `MakeLazyImageFromTextureSource`).
    /// Falls back to `frame_rgba` + `make_image_from_rgba` when `None`.
    fn video_frame_as_image(
        &mut self,
        _provider: &mut dyn crate::platform::video::VideoFrameProvider,
        _asset_id: &crate::resource::asset_id::AssetId,
        _frame: u32,
    ) -> Option<(Self::Image, u32, u32)> {
        None
    }

    /// Render into an offscreen surface and return the result as an image.
    fn render_to_image<R>(&mut self, width: u32, height: u32, draw: R) -> Self::Image
    where
        R: FnOnce(&mut Self),
        Self: Sized;
}

pub mod paint;
pub mod glyph;
pub mod runtime_effect;

pub use paint::*;
pub use glyph::*;
pub use runtime_effect::*;
