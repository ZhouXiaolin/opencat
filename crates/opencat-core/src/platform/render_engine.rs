//! Core-level RenderEngine trait + per-frame borrow contexts.

use std::any::Any;

use anyhow::Result;

use crate::display::list::{DisplayItem, DisplayRect};
use crate::frame_ctx::FrameCtx;
use crate::platform::backend::BackendTypes;
use crate::resource::hash_map_catalog::HashMapResourceCatalog;
use crate::runtime::annotation::{AnnotatedDisplayTree, AnnotatedNodeHandle};
use crate::runtime::compositor::OrderedSceneProgram;
use crate::text::GlyphData;

/// Backend-agnostic frame view.
pub struct FrameView<'a> {
    pub width: u32,
    pub height: u32,
    pub kind: FrameViewKind<'a>,
}

pub enum FrameViewKind<'a> {
    /// Platform native drawing handle.
    Opaque(&'a mut dyn Any),
}

/// Borrow-bundle for backend.record_* operations.
pub struct RecordCtx<'a, B: BackendTypes> {
    pub catalog: &'a HashMapResourceCatalog,
    pub frame_ctx: &'a FrameCtx,
    pub cache: &'a mut crate::runtime::cache::CacheRegistry<B>,
    pub video: &'a mut dyn crate::platform::video::VideoFrameProvider,
    /// Platform-specific userdata (e.g. engine's AssetCatalog + MediaContext bundle).
    /// Allows backends to access concrete types without core knowing about them.
    pub platform_data: &'a mut dyn Any,
}

/// Borrow-bundle for backend.draw_ordered_scene.
pub struct RenderCtx<'a, B: BackendTypes> {
    pub catalog: &'a HashMapResourceCatalog,
    pub frame_ctx: &'a FrameCtx,
    pub display_tree: &'a AnnotatedDisplayTree,
    pub ordered_scene: &'a OrderedSceneProgram,
    pub cache: &'a mut crate::runtime::cache::CacheRegistry<B>,
    pub video: &'a mut dyn crate::platform::video::VideoFrameProvider,
    /// Platform-specific userdata (e.g. engine's AssetCatalog + MediaContext bundle).
    pub platform_data: &'a mut dyn Any,
}

/// Glyph painting parameters for render backends.
#[derive(Clone, Debug)]
pub struct GlyphPaint {
    pub color: (f32, f32, f32, f32), // RGBA
    pub font_size: f32,
}

/// Backend-only render engine surface.
pub trait RenderEngine: BackendTypes + Send + Sync {
    fn target_frame_view_kind(&self) -> &'static str;

    fn draw_scene_snapshot(
        &self,
        snapshot: &Self::Picture,
        frame_view: FrameView<'_>,
    ) -> Result<()>;

    fn record_display_tree_snapshot(
        &self,
        ctx: &mut RecordCtx<'_, Self>,
        display_tree: &AnnotatedDisplayTree,
    ) -> Result<Self::Picture>
    where
        Self: Sized;

    fn draw_ordered_scene(
        &self,
        ctx: &mut RenderCtx<'_, Self>,
        frame_view: FrameView<'_>,
    ) -> Result<()>
    where
        Self: Sized;

    // ── Subtree granularity ──

    fn record_subtree_snapshot(
        &self,
        ctx: &mut RecordCtx<'_, Self>,
        display_tree: &AnnotatedDisplayTree,
        handle: AnnotatedNodeHandle,
    ) -> Result<Self::Picture>
    where
        Self: Sized;

    fn record_subtree_image(
        &self,
        snapshot: &Self::Picture,
        bounds: DisplayRect,
    ) -> Result<Self::Image>;

    fn draw_subtree_snapshot(
        &self,
        snapshot: &Self::Picture,
        opacity: f32,
        backdrop_blur: Option<f32>,
        bounds: DisplayRect,
        frame_view: FrameView<'_>,
    ) -> Result<()>;

    fn draw_subtree_image(
        &self,
        image: &Self::Image,
        opacity: f32,
        backdrop_blur: Option<f32>,
        bounds: DisplayRect,
        frame_view: FrameView<'_>,
    ) -> Result<()>;

    // ── Item granularity ──

    fn record_item_picture(
        &self,
        ctx: &mut RecordCtx<'_, Self>,
        item: &DisplayItem,
    ) -> Result<Self::Picture>
    where
        Self: Sized;

    fn draw_item_picture(
        &self,
        picture: &Self::Picture,
        translation: (f32, f32),
        frame_view: FrameView<'_>,
    ) -> Result<()>;

    // ── Glyph granularity ──

    fn rasterize_glyph_path(&self, glyph: &GlyphData) -> Result<Self::GlyphPath>;
    fn rasterize_glyph_image(&self, glyph: &GlyphData) -> Result<Self::GlyphImage>;
    fn draw_glyph_path(
        &self,
        path: &Self::GlyphPath,
        paint: &GlyphPaint,
        frame_view: FrameView<'_>,
    ) -> Result<()>;
    fn draw_glyph_image(
        &self,
        image: &Self::GlyphImage,
        bounds: DisplayRect,
        frame_view: FrameView<'_>,
    ) -> Result<()>;
}

#[cfg(test)]
mod ctx_tests {
    use super::*;
    use crate::frame_ctx::FrameCtx;
    use crate::platform::backend::BackendTypes;
    use crate::platform::video::{FrameBitmap, VideoFrameProvider};
    use crate::resource::asset_id::AssetId;
    use crate::resource::hash_map_catalog::HashMapResourceCatalog;
    use crate::runtime::cache::CacheRegistry;
    use anyhow::Result;

    struct MockBackend;
    impl BackendTypes for MockBackend {
        type Picture = String;
        type Image = String;
        type GlyphPath = String;
        type GlyphImage = String;
    }

    struct MockVideo;
    impl VideoFrameProvider for MockVideo {
        fn frame_rgba(&mut self, _id: &AssetId, _frame: u32) -> Result<FrameBitmap> {
            Ok(FrameBitmap {
                data: std::sync::Arc::new(vec![0; 4]),
                width: 1,
                height: 1,
            })
        }
    }

    #[test]
    fn record_ctx_carries_cache_and_video() {
        let catalog = HashMapResourceCatalog::from_json("{}").unwrap();
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 100,
            height: 100,
            frames: 60,
        };
        let mut cache: CacheRegistry<MockBackend> = CacheRegistry::default();
        let mut video = MockVideo;
        let mut platform_data: Box<dyn Any> = Box::new(());
        let ctx: RecordCtx<'_, MockBackend> = RecordCtx {
            catalog: &catalog,
            frame_ctx: &frame_ctx,
            cache: &mut cache,
            video: &mut video,
            platform_data: &mut *platform_data,
        };
        assert_eq!(ctx.frame_ctx.width, 100);
    }

    #[test]
    fn render_ctx_carries_display_tree_and_cache() {
        use crate::display::list::{
            DisplayItem, DisplayRect, DisplayTransform, RectDisplayItem, RectPaintStyle,
        };
        use crate::display::tree::{DisplayNode, DisplayTree};
        use crate::element::tree::ElementId;
        use crate::runtime::annotation::annotate_display_tree;
        use crate::runtime::compositor::OrderedSceneProgram;
        use crate::style::BorderRadius;

        let catalog = HashMapResourceCatalog::from_json("{}").unwrap();
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 16,
            height: 16,
            frames: 1,
        };
        let bounds = DisplayRect {
            x: 0.0,
            y: 0.0,
            width: 16.0,
            height: 16.0,
        };
        let display_tree = DisplayTree {
            root: DisplayNode {
                element_id: ElementId(0),
                transform: DisplayTransform {
                    translation_x: 0.0,
                    translation_y: 0.0,
                    bounds,
                    transforms: vec![],
                },
                opacity: 1.0,
                backdrop_blur_sigma: None,
                clip: None,
                item: DisplayItem::Rect(RectDisplayItem {
                    bounds,
                    paint: RectPaintStyle {
                        background: None,
                        border_radius: BorderRadius::uniform(0.0),
                        border_width: None,
                        border_top_width: None,
                        border_right_width: None,
                        border_bottom_width: None,
                        border_left_width: None,
                        border_color: None,
                        border_style: None,
                        blur_sigma: None,
                        box_shadow: None,
                        inset_shadow: None,
                        drop_shadow: None,
                        backdrop_blur_sigma: None,
                    },
                }),
                children: vec![],
            },
        };
        let annotated = annotate_display_tree(&display_tree);
        let ordered = OrderedSceneProgram::build(&annotated);
        let mut cache: CacheRegistry<MockBackend> = CacheRegistry::default();
        let mut video = MockVideo;
        let mut platform_data: Box<dyn Any> = Box::new(());
        let ctx: RenderCtx<'_, MockBackend> = RenderCtx {
            catalog: &catalog,
            frame_ctx: &frame_ctx,
            display_tree: &annotated,
            ordered_scene: &ordered,
            cache: &mut cache,
            video: &mut video,
            platform_data: &mut *platform_data,
        };
        assert_eq!(ctx.frame_ctx.width, 16);
    }
}
