use std::sync::OnceLock;
use tracing::{Level, span};

use anyhow::{Result, anyhow};
use skia_safe::Canvas;
use std::ffi::c_void;

use opencat_core::runtime::annotation::AnnotatedDisplayTree;

use super::canvas as skia;

pub struct SkiaRenderEngine;

/// Returns a shared `Arc<SkiaRenderEngine>` for use with `EnginePlatform`.
pub fn shared_raster_engine_typed() -> std::sync::Arc<SkiaRenderEngine> {
    static ENGINE: OnceLock<std::sync::Arc<SkiaRenderEngine>> = OnceLock::new();
    ENGINE
        .get_or_init(|| std::sync::Arc::new(SkiaRenderEngine))
        .clone()
}

// ---------------------------------------------------------------------------
// Core trait implementations — bridge to existing Skia canvas functions.
// ---------------------------------------------------------------------------

use crate::resource::media::MediaContext;
use crate::resource::path_store::AssetPathStore;
use opencat_core::platform::backend::BackendTypes;
use opencat_core::platform::render_engine::{
    FrameView, RecordCtx, RenderCtx, RenderEngine as CoreRenderEngine,
};

/// Bundle passed through `platform_data` from engine driver to SkiaRenderEngine core trait impls.
/// Allows the core pipeline to render via existing canvas functions without canvas knowing about core types.
///
/// `media_ctx` is a raw pointer to the engine's `MediaContext` stored on `EnginePlatform`.
/// `asset_paths` holds physical file paths (catalog metadata is in ctx.catalog).
/// SAFETY: The pointer is valid for the lifetime `'a` because `EnginePlatform` outlives the
/// render call, and the core pipeline doesn't move or drop the platform during rendering.
pub struct SkiaRenderData<'a> {
    pub asset_paths: &'a AssetPathStore,
    pub media_ctx: *mut MediaContext,
}

// SAFETY: SkiaRenderData is only used on a single thread during rendering.
unsafe impl Send for SkiaRenderData<'_> {}
unsafe impl Sync for SkiaRenderData<'_> {}

impl BackendTypes for SkiaRenderEngine {
    type Picture = skia_safe::Picture;
    type Image = skia_safe::Image;
    type GlyphPath = skia_safe::Path;
    type GlyphImage = skia_safe::Image;
}

impl CoreRenderEngine for SkiaRenderEngine {
    fn target_frame_view_kind(&self) -> &'static str {
        "DrawContext2D"
    }

    fn draw_scene_snapshot(
        &self,
        snapshot: &Self::Picture,
        frame_view: FrameView<'_>,
    ) -> Result<()> {
        let canvas = skia_canvas_from_core(frame_view)?;
        if snapshot.cull_rect().is_empty() {
            return Err(anyhow!("scene snapshot has empty bounds"));
        }
        canvas.draw_picture(snapshot, None, None);
        Ok(())
    }

    fn record_display_tree_snapshot(
        &self,
        ctx: &mut RecordCtx<'_, Self>,
        display_tree: &AnnotatedDisplayTree,
    ) -> Result<Self::Picture>
    where
        Self: Sized,
    {
        let data = ctx
            .platform_data
            .downcast_mut::<SkiaRenderData<'_>>()
            .ok_or_else(|| anyhow!("platform_data must be SkiaRenderData"))?;
        // SAFETY: media_ctx pointer was created from EnginePlatform::with_render_context
        // and is valid for the duration of this call. The core pipeline doesn't move
        // or drop the platform during rendering.
        let media_ctx = unsafe { &mut *data.media_ctx };
        let snapshot = skia::record_display_tree_snapshot(
            display_tree,
            ctx.frame_ctx.width,
            ctx.frame_ctx.height,
            ctx.catalog,
            data.asset_paths,
            ctx.cache.image_cache(),
            ctx.cache.glyph_path_cache(),
            ctx.cache.glyph_image_cache(),
            ctx.cache.item_picture_cache(),
            ctx.cache.subtree_snapshot_cache(),
            ctx.cache.subtree_image_cache(),
            Some(media_ctx),
            ctx.frame_ctx,
        )?;
        Ok(snapshot)
    }

    fn draw_ordered_scene(
        &self,
        ctx: &mut RenderCtx<'_, Self>,
        frame_view: FrameView<'_>,
    ) -> Result<()>
    where
        Self: Sized,
    {
        let direct_draw_span =
            span!(target: "render.backend", Level::TRACE, "display_tree_direct_draw");
        let _profile_span = direct_draw_span.enter();
        let canvas = skia_canvas_from_core(frame_view)?;
        let data = ctx
            .platform_data
            .downcast_mut::<SkiaRenderData<'_>>()
            .ok_or_else(|| anyhow!("platform_data must be SkiaRenderData"))?;
        // SAFETY: media_ctx pointer was created from EnginePlatform::with_render_context
        // and is valid for the duration of this call.
        let media_ctx = unsafe { &mut *data.media_ctx };
        skia::draw_ordered_scene_cached(
            ctx.display_tree,
            ctx.ordered_scene,
            canvas,
            ctx.catalog,
            data.asset_paths,
            ctx.cache.image_cache(),
            ctx.cache.glyph_path_cache(),
            ctx.cache.glyph_image_cache(),
            ctx.cache.item_picture_cache(),
            ctx.cache.subtree_snapshot_cache(),
            ctx.cache.subtree_image_cache(),
            Some(media_ctx),
            ctx.frame_ctx,
        )?;
        Ok(())
    }
}

fn skia_canvas_from_core(frame_view: FrameView<'_>) -> Result<&'static Canvas> {
    use opencat_core::platform::render_engine::FrameViewKind;
    let FrameViewKind::Opaque(kind) = &frame_view.kind;
    // Try to downcast to the engine's raw pointer convention.
    // The engine passes raw *mut c_void through FrameView.
    let raw: Option<*mut c_void> = kind
        .downcast_ref()
        .copied()
        .or_else(|| kind.downcast_ref::<*mut c_void>().copied());
    let raw = raw.ok_or_else(|| anyhow!("frame view does not contain a raw pointer"))?;
    if raw.is_null() {
        return Err(anyhow!("frame view raw pointer is null"));
    }
    // SAFETY: Skia backend only accepts Canvas surface views and the raw pointer is owned by the
    // active target or raster surface for the duration of the call chain.
    Ok(unsafe { &*(raw as *const Canvas) })
}
