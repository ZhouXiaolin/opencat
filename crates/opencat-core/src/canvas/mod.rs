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

pub trait Canvas2D {
    type Path: Clone;
    type Image: Clone;
    type Picture: Clone;
    type RuntimeEffect: Clone;

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
    fn make_path_from_verbs(
        &self,
        verbs: &[u8],
        points: &[f32],
        fill_type: FillType,
    ) -> Self::Path;
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
