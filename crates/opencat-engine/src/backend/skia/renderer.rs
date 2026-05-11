use std::sync::OnceLock;
use tracing::{Level, span};

use anyhow::{Result, anyhow};
use skia_safe::{AlphaType, Canvas, ColorType, ImageInfo, image::CachingHint, surfaces};
use std::ffi::c_void;

#[cfg(target_os = "macos")]
use crate::runtime::surface::MetalEncodeBridge;
use crate::runtime::{
    compositor::OrderedSceneProgram,
    frame_view::RenderFrameView,
    render_engine::{RenderEngine, SceneRenderContext, SceneSnapshot, SharedRenderEngine},
    session::RenderSession,
    target::{RenderFrameViewKind, RenderTargetHandle},
};
use opencat_core::runtime::annotation::AnnotatedDisplayTree;
use opencat_core::scene::composition::Composition;

use super::canvas as skia;

enum SkiaFrameSurface {
    Raster,
    MetalOffscreen,
}

pub struct SkiaRenderEngine {
    frame_surface: SkiaFrameSurface,
}

pub fn shared_raster_engine() -> SharedRenderEngine {
    static ENGINE: OnceLock<SharedRenderEngine> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            std::sync::Arc::new(SkiaRenderEngine {
                frame_surface: SkiaFrameSurface::Raster,
            }) as SharedRenderEngine
        })
        .clone()
}

pub fn shared_metal_engine() -> SharedRenderEngine {
    static ENGINE: OnceLock<SharedRenderEngine> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            std::sync::Arc::new(SkiaRenderEngine {
                frame_surface: SkiaFrameSurface::MetalOffscreen,
            }) as SharedRenderEngine
        })
        .clone()
}

/// Typed version: returns `Arc<SkiaRenderEngine>` instead of the trait object.
pub fn shared_raster_engine_typed() -> std::sync::Arc<SkiaRenderEngine> {
    static ENGINE: OnceLock<std::sync::Arc<SkiaRenderEngine>> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            std::sync::Arc::new(SkiaRenderEngine {
                frame_surface: SkiaFrameSurface::Raster,
            })
        })
        .clone()
}

impl RenderEngine for SkiaRenderEngine {
    fn target_frame_view_kind(&self) -> RenderFrameViewKind {
        RenderFrameViewKind::DrawContext2D
    }

    fn render_frame_to_target(
        &self,
        composition: &Composition,
        frame_index: u32,
        session: &mut RenderSession,
        target: &mut RenderTargetHandle,
    ) -> Result<()> {
        target.require_frame_view_kind(RenderFrameViewKind::DrawContext2D)?;
        let frame_surface = target.begin_frame_surface(composition.width, composition.height)?;
        let frame_view = target.resolve_frame_view(frame_surface)?;
        let render_result = crate::runtime::pipeline::render_frame_on_surface(
            composition,
            frame_index,
            session,
            frame_view,
        );
        let end_result = target.end_frame();
        render_result.and(end_result)
    }

    fn render_frame_rgba(
        &self,
        composition: &Composition,
        frame_index: u32,
        session: &mut RenderSession,
    ) -> Result<Vec<u8>> {
        match self.frame_surface {
            SkiaFrameSurface::Raster => render_frame_rgba_raster(composition, frame_index, session),
            SkiaFrameSurface::MetalOffscreen => {
                #[cfg(target_os = "macos")]
                {
                    let mut bridge = MetalEncodeBridge::new(composition.width, composition.height)?;
                    bridge.render_frame_rgba(composition, frame_index, session)
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Err(anyhow!(
                        "accelerated render backend is only available on macOS"
                    ))
                }
            }
        }
    }

    fn draw_scene_snapshot(
        &self,
        snapshot: &SceneSnapshot,
        frame_view: RenderFrameView,
    ) -> Result<()> {
        let canvas = skia_canvas(frame_view)?;
        if snapshot.cull_rect().is_empty() {
            return Err(anyhow!("scene snapshot has empty bounds"));
        }
        canvas.draw_picture(snapshot, None, None);
        Ok(())
    }

    fn record_display_tree_snapshot(
        &self,
        runtime: &mut SceneRenderContext<'_>,
        display_tree: &AnnotatedDisplayTree,
    ) -> Result<SceneSnapshot> {
        let snapshot = skia::record_display_tree_snapshot(
            display_tree,
            runtime.width,
            runtime.height,
            runtime.assets,
            runtime.cache_registry.image_cache(),
            runtime.cache_registry.glyph_path_cache(),
            runtime.cache_registry.glyph_image_cache(),
            runtime.cache_registry.item_picture_cache(),
            runtime.cache_registry.subtree_snapshot_cache(),
            runtime.cache_registry.subtree_image_cache(),
            Some(&mut *runtime.media_ctx),
            runtime.frame_ctx,
        )?;
        Ok(snapshot)
    }

    fn draw_ordered_scene(
        &self,
        runtime: &mut SceneRenderContext<'_>,
        display_tree: &AnnotatedDisplayTree,
        ordered_scene: &OrderedSceneProgram,
        frame_view: RenderFrameView,
    ) -> Result<()> {
        let direct_draw_span =
            span!(target: "render.backend", Level::TRACE, "display_tree_direct_draw");
        let _profile_span = direct_draw_span.enter();
        let canvas = skia_canvas(frame_view)?;
        skia::draw_ordered_scene_cached(
            display_tree,
            ordered_scene,
            canvas,
            runtime.assets,
            runtime.cache_registry.image_cache(),
            runtime.cache_registry.glyph_path_cache(),
            runtime.cache_registry.glyph_image_cache(),
            runtime.cache_registry.item_picture_cache(),
            runtime.cache_registry.subtree_snapshot_cache(),
            runtime.cache_registry.subtree_image_cache(),
            Some(&mut *runtime.media_ctx),
            runtime.frame_ctx,
        )?;
        Ok(())
    }
}

fn render_frame_rgba_raster(
    composition: &Composition,
    frame_index: u32,
    session: &mut RenderSession,
) -> Result<Vec<u8>> {
    let mut surface = surfaces::raster_n32_premul((composition.width, composition.height))
        .ok_or_else(|| anyhow!("failed to create skia raster surface"))?;
    let frame_view = RenderFrameView::new(
        RenderFrameViewKind::DrawContext2D,
        surface.canvas() as *const _ as *mut c_void,
    )?;
    crate::runtime::pipeline::render_frame_on_surface(
        composition,
        frame_index,
        session,
        frame_view,
    )?;
    let image = surface.image_snapshot();
    let image_info = ImageInfo::new(
        (composition.width, composition.height),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );

    let mut rgba = vec![0_u8; (composition.width as usize) * (composition.height as usize) * 4];
    let read_ok = image.read_pixels(
        &image_info,
        rgba.as_mut_slice(),
        (composition.width as usize) * 4,
        (0, 0),
        CachingHint::Allow,
    );

    if !read_ok {
        return Err(anyhow!("failed to read pixels from skia surface"));
    }

    Ok(rgba)
}

fn skia_canvas(frame_view: RenderFrameView) -> Result<&'static Canvas> {
    if frame_view.kind() != RenderFrameViewKind::DrawContext2D {
        return Err(anyhow!(
            "render frame view {:?} is not compatible with skia renderer",
            frame_view.kind()
        ));
    }
    // SAFETY: Skia backend only accepts Canvas surface views and the raw pointer is owned by the
    // active target or raster surface for the duration of the call chain.
    Ok(unsafe { &*(frame_view.raw() as *const Canvas) })
}

// ---------------------------------------------------------------------------
// Core trait implementations — bridge to existing Skia canvas functions.
// ---------------------------------------------------------------------------

use crate::resource::asset_catalog::AssetCatalog;
use crate::resource::media::MediaContext;
use opencat_core::platform::backend::BackendTypes;
use opencat_core::platform::render_engine::{
    FrameView, RecordCtx, RenderCtx, RenderEngine as CoreRenderEngine,
};
use std::any::Any;

/// Bundle passed through `platform_data` from engine driver to SkiaRenderEngine core trait impls.
/// Allows the core pipeline to render via existing canvas functions without canvas knowing about core types.
pub struct SkiaRenderData<'a> {
    pub assets: &'a AssetCatalog,
    pub media_ctx: &'a mut MediaContext,
}

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
        let snapshot = skia::record_display_tree_snapshot(
            display_tree,
            ctx.frame_ctx.width as i32,
            ctx.frame_ctx.height as i32,
            data.assets,
            ctx.cache.image_cache(),
            ctx.cache.glyph_path_cache(),
            ctx.cache.glyph_image_cache(),
            ctx.cache.item_picture_cache(),
            ctx.cache.subtree_snapshot_cache(),
            ctx.cache.subtree_image_cache(),
            Some(&mut *data.media_ctx),
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
        skia::draw_ordered_scene_cached(
            ctx.display_tree,
            ctx.ordered_scene,
            canvas,
            data.assets,
            ctx.cache.image_cache(),
            ctx.cache.glyph_path_cache(),
            ctx.cache.glyph_image_cache(),
            ctx.cache.item_picture_cache(),
            ctx.cache.subtree_snapshot_cache(),
            ctx.cache.subtree_image_cache(),
            Some(&mut *data.media_ctx),
            ctx.frame_ctx,
        )?;
        Ok(())
    }
}

fn skia_canvas_from_core(frame_view: FrameView<'_>) -> Result<&'static Canvas> {
    use opencat_core::platform::render_engine::FrameViewKind;
    let kind = match &frame_view.kind {
        FrameViewKind::Opaque(any) => any,
    };
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
