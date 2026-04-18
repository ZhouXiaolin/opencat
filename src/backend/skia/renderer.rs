use std::sync::OnceLock;

use anyhow::{Result, anyhow};
use skia_safe::{AlphaType, Canvas, ColorType, ImageInfo, Picture, image::CachingHint, surfaces};
use std::ffi::c_void;

#[cfg(target_os = "macos")]
use crate::runtime::surface::MetalEncodeBridge;
use crate::{
    runtime::{
        annotation::AnnotatedDisplayTree,
        compositor::OrderedSceneProgram,
        frame_view::RenderFrameView,
        profile::backend_span,
        render_engine::{RenderEngine, SceneRenderContext, SceneSnapshot, SharedRenderEngine},
        session::RenderSession,
        target::{RenderFrameViewKind, RenderTargetHandle},
        text_engine::SharedTextEngine,
    },
    scene::{composition::Composition, transition::TransitionKind},
};

use super::{canvas as skia, text as skia_text, transition as skia_transition};

enum SkiaFrameSurface {
    Raster,
    MetalOffscreen,
}

pub(crate) struct SkiaRenderEngine {
    frame_surface: SkiaFrameSurface,
    text_engine: SharedTextEngine,
}

struct SkiaSceneSnapshot {
    snapshot: Picture,
}

pub(crate) fn shared_raster_engine() -> SharedRenderEngine {
    static ENGINE: OnceLock<SharedRenderEngine> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            std::sync::Arc::new(SkiaRenderEngine {
                frame_surface: SkiaFrameSurface::Raster,
                text_engine: skia_text::shared_text_engine(),
            }) as SharedRenderEngine
        })
        .clone()
}

pub(crate) fn shared_metal_engine() -> SharedRenderEngine {
    static ENGINE: OnceLock<SharedRenderEngine> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            std::sync::Arc::new(SkiaRenderEngine {
                frame_surface: SkiaFrameSurface::MetalOffscreen,
                text_engine: skia_text::shared_text_engine(),
            }) as SharedRenderEngine
        })
        .clone()
}

impl RenderEngine for SkiaRenderEngine {
    fn target_frame_view_kind(&self) -> RenderFrameViewKind {
        RenderFrameViewKind::DrawContext2D
    }

    fn text_engine(&self) -> SharedTextEngine {
        self.text_engine.clone()
    }

    fn render_frame_to_target(
        &self,
        composition: &Composition,
        frame_index: u32,
        session: &mut RenderSession,
        target: &mut RenderTargetHandle,
    ) -> Result<()> {
        target.require_frame_view_kind(self.target_frame_view_kind())?;
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
        let snapshot_picture = skia_snapshot_picture(snapshot)?;
        if snapshot_picture.cull_rect().is_empty() {
            return Err(anyhow!("scene snapshot has empty bounds"));
        }
        canvas.draw_picture(snapshot_picture, None, None);
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
            runtime.cache_registry.text_snapshot_cache(),
            runtime.cache_registry.item_picture_cache(),
            runtime.cache_registry.subtree_snapshot_cache(),
            runtime.cache_registry.subtree_image_cache(),
            Some(&mut *runtime.media_ctx),
            runtime.frame_ctx,
        )?;
        Ok(SceneSnapshot::new(SkiaSceneSnapshot { snapshot }))
    }

    fn draw_ordered_scene(
        &self,
        runtime: &mut SceneRenderContext<'_>,
        display_tree: &AnnotatedDisplayTree,
        ordered_scene: &OrderedSceneProgram,
        frame_view: RenderFrameView,
    ) -> Result<()> {
        let _profile_span = backend_span("display_tree_direct_draw");
        let canvas = skia_canvas(frame_view)?;
        skia::draw_ordered_scene_cached(
            display_tree,
            ordered_scene,
            canvas,
            runtime.assets,
            runtime.cache_registry.image_cache(),
            runtime.cache_registry.text_snapshot_cache(),
            runtime.cache_registry.item_picture_cache(),
            runtime.cache_registry.subtree_snapshot_cache(),
            runtime.cache_registry.subtree_image_cache(),
            Some(&mut *runtime.media_ctx),
            runtime.frame_ctx,
        )?;
        Ok(())
    }

    fn draw_transition(
        &self,
        frame_view: RenderFrameView,
        from: &SceneSnapshot,
        to: &SceneSnapshot,
        progress: f32,
        kind: TransitionKind,
        width: i32,
        height: i32,
    ) -> Result<()> {
        let canvas = skia_canvas(frame_view)?;
        skia_transition::draw_transition(
            canvas,
            skia_snapshot_picture(from)?,
            skia_snapshot_picture(to)?,
            progress,
            kind,
            width,
            height,
        )
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

fn skia_snapshot_picture(snapshot: &SceneSnapshot) -> Result<&Picture> {
    snapshot
        .downcast_ref::<SkiaSceneSnapshot>()
        .map(|snapshot| &snapshot.snapshot)
        .ok_or_else(|| anyhow!("scene snapshot is not compatible with skia renderer"))
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
