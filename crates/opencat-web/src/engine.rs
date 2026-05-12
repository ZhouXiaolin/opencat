//! WebRenderEngine — skeleton implementing core RenderEngine trait.
//!
//! All trait methods are stubs returning `Ok(default)`. Real WebGL/CanvasKit
//! implementations will be wired in Phase D5.

use std::sync::Arc;

use anyhow::Result;
use opencat_core::display::list::{DisplayItem, DisplayRect};
use opencat_core::platform::backend::BackendTypes;
use opencat_core::platform::render_engine::{
    FrameView, GlyphPaint, RecordCtx, RenderCtx, RenderEngine,
};
use opencat_core::runtime::annotation::{AnnotatedDisplayTree, AnnotatedNodeHandle};
use opencat_core::scene::path_bounds::{DefaultPathBounds, PathBoundsComputer};
use opencat_core::text::GlyphData;

use crate::backend::{GlyphPathData, WebPicture};

pub struct WebRenderEngine {
    path_bounds: Box<dyn PathBoundsComputer>,
}

impl Default for WebRenderEngine {
    fn default() -> Self {
        Self {
            path_bounds: Box::new(DefaultPathBounds),
        }
    }
}

impl WebRenderEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn path_bounds(&self) -> &dyn PathBoundsComputer {
        self.path_bounds.as_ref()
    }
}

impl BackendTypes for WebRenderEngine {
    type Picture = WebPicture;
    type Image = Arc<Vec<u8>>;
    type GlyphPath = GlyphPathData;
    type GlyphImage = Arc<Vec<u8>>;
}

impl RenderEngine for WebRenderEngine {
    fn target_frame_view_kind(&self) -> &'static str {
        "webgl"
    }

    fn draw_scene_snapshot(&self, _: &Self::Picture, _: FrameView<'_>) -> Result<()> {
        Ok(())
    }

    fn record_display_tree_snapshot(
        &self,
        _: &mut RecordCtx<'_, Self>,
        _: &AnnotatedDisplayTree,
    ) -> Result<Self::Picture>
    where
        Self: Sized,
    {
        Ok(WebPicture { fingerprint: 0 })
    }

    fn draw_ordered_scene(
        &self,
        _: &mut RenderCtx<'_, Self>,
        _: FrameView<'_>,
    ) -> Result<()>
    where
        Self: Sized,
    {
        Ok(())
    }

    // ── Subtree granularity ──

    fn record_subtree_snapshot(
        &self,
        _: &mut RecordCtx<'_, Self>,
        _: &AnnotatedDisplayTree,
        _: AnnotatedNodeHandle,
    ) -> Result<Self::Picture> {
        Ok(WebPicture { fingerprint: 0 })
    }

    fn record_subtree_image(
        &self,
        _: &Self::Picture,
        _: DisplayRect,
    ) -> Result<Self::Image> {
        Ok(Arc::new(vec![]))
    }

    fn draw_subtree_snapshot(
        &self,
        _: &Self::Picture,
        _: f32,
        _: Option<f32>,
        _: DisplayRect,
        _: FrameView<'_>,
    ) -> Result<()> {
        Ok(())
    }

    fn draw_subtree_image(
        &self,
        _: &Self::Image,
        _: f32,
        _: Option<f32>,
        _: DisplayRect,
        _: FrameView<'_>,
    ) -> Result<()> {
        Ok(())
    }

    // ── Item granularity ──

    fn record_item_picture(
        &self,
        _: &mut RecordCtx<'_, Self>,
        _: &DisplayItem,
    ) -> Result<Self::Picture> {
        Ok(WebPicture { fingerprint: 0 })
    }

    fn draw_item_picture(
        &self,
        _: &Self::Picture,
        _: (f32, f32),
        _: FrameView<'_>,
    ) -> Result<()> {
        Ok(())
    }

    // ── Glyph granularity ──

    fn rasterize_glyph_path(&self, _: &GlyphData) -> Result<Self::GlyphPath> {
        Ok(GlyphPathData {
            commands: vec![],
            bounds_x: 0.0,
            bounds_y: 0.0,
            bounds_w: 0.0,
            bounds_h: 0.0,
        })
    }

    fn rasterize_glyph_image(&self, _: &GlyphData) -> Result<Self::GlyphImage> {
        Ok(Arc::new(vec![]))
    }

    fn draw_glyph_path(
        &self,
        _: &Self::GlyphPath,
        _: &GlyphPaint,
        _: FrameView<'_>,
    ) -> Result<()> {
        Ok(())
    }

    fn draw_glyph_image(
        &self,
        _: &Self::GlyphImage,
        _: DisplayRect,
        _: FrameView<'_>,
    ) -> Result<()> {
        Ok(())
    }
}
