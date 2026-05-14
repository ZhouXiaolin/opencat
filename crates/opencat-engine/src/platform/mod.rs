//! Engine-side Platform impl: aggregates quickjs script runtime, ffmpeg/skia
//! media context, Skia render engine, asset path table, audio caches.

pub mod audio_runtime;

use std::path::PathBuf;
use std::sync::Arc;

use opencat_core::Platform;
use opencat_core::resource::asset_id::AssetId;
use opencat_core::resource::preload::preload_all;
use opencat_core::scene::node::{Node, NodeKind};
use opencat_core::scene::path_bounds::PathBoundsComputer;
use opencat_core::scene::primitives::ImageSource;
use opencat_core::scene::time::FrameState;

use crate::backend::skia::renderer::{SkiaRenderData, SkiaRenderEngine};
use crate::resource::fetch::{build_preload_runtime, fetch_openverse_token};
use crate::resource::media::MediaContext;
use crate::resource::path_store::AssetPathStore;
use crate::resource::resolver::EngineAssetResolver;
use crate::runtime::audio::{AudioIntervalCache, DecodedAudioCache};
use crate::runtime::cache::CacheCaps;
use crate::runtime::path_bounds::SkiaPathBounds;
use crate::script::ScriptRuntimeCache;

pub use audio_runtime::AudioRuntime;

/// Engine platform implementation.
pub struct EnginePlatform {
    pub backend: Arc<SkiaRenderEngine>,
    pub script: ScriptRuntimeCache,
    pub video: MediaContext,
    pub asset_paths: AssetPathStore,
    pub audio: AudioRuntime,
    pub path_bounds: SkiaPathBounds,
    /// Decoded audio cache for streaming playback.
    pub audio_decode_cache: DecodedAudioCache,
    /// Audio interval cache for composition-level audio scheduling.
    pub audio_interval_cache: AudioIntervalCache,
    /// Last preflight root pointer to skip duplicate preflight runs.
    pub prepared_root_ptr: Option<usize>,
}

impl EnginePlatform {
    pub fn new(backend: Arc<SkiaRenderEngine>) -> Self {
        Self::with_cache_caps(backend, CacheCaps::default())
    }

    pub fn with_cache_caps(backend: Arc<SkiaRenderEngine>, _caps: CacheCaps) -> Self {
        Self {
            backend,
            script: ScriptRuntimeCache::default(),
            video: MediaContext::new(),
            asset_paths: AssetPathStore::new(),
            audio: AudioRuntime::new(),
            path_bounds: SkiaPathBounds,
            audio_decode_cache: DecodedAudioCache::default(),
            audio_interval_cache: AudioIntervalCache::default(),
            prepared_root_ptr: None,
        }
    }

    pub fn set_video_preview_quality(
        &mut self,
        quality: crate::resource::media::VideoPreviewQuality,
    ) {
        self.video.set_video_preview_quality(quality);
    }

    pub fn preflight(
        &mut self,
        composition: &opencat_core::scene::composition::Composition,
        catalog: &mut opencat_core::resource::hash_map_catalog::HashMapResourceCatalog,
    ) -> anyhow::Result<()> {
        let req = opencat_core::runtime::preflight_collect::collect_resource_requests(composition);

        let cache_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".opencat")
            .join("assets");
        std::fs::create_dir_all(&cache_dir).ok();

        let rt = build_preload_runtime("preflight")?;
        let token = rt.block_on(fetch_openverse_token(None))?;
        {
            let mut resolver = EngineAssetResolver::new(
                &mut self.asset_paths,
                &mut self.video,
                cache_dir,
                token,
            )?;
            rt.block_on(preload_all(req, &mut resolver, catalog))?;
        }

        self.video.set_composition_fps(composition.fps);

        // Register canvas asset aliases in both catalogs so that canvas
        // rendering functions can resolve them by alias.
        for frame in 0..composition.frames.max(1) {
            let frame_ctx = opencat_core::frame_ctx::FrameCtx {
                frame,
                fps: composition.fps,
                width: composition.width,
                height: composition.height,
                frames: composition.frames,
            };
            let root = composition.root_node(&frame_ctx);
            let state = opencat_core::scene::time::frame_state_for_root(&root, &frame_ctx);
            register_canvas_asset_aliases(&state, &frame_ctx, catalog, &mut self.asset_paths);
        }

        Ok(())
    }
}

impl Platform for EnginePlatform {
    type Backend = SkiaRenderEngine;
    type Script = ScriptRuntimeCache;
    type Video = MediaContext;

    fn render_engine(&self) -> &Self::Backend {
        &self.backend
    }
    fn script_host(&mut self) -> &mut Self::Script {
        &mut self.script
    }
    fn video_source(&mut self) -> &mut Self::Video {
        &mut self.video
    }
    fn path_bounds(&self) -> &dyn PathBoundsComputer {
        &self.path_bounds
    }

    /// Provide render context to the backend.
    fn with_render_context<R>(
        &mut self,
        f: impl FnOnce(&mut Self::Video, &Self::Backend, &mut dyn std::any::Any) -> R,
    ) -> R {
        let this = self as *mut Self;
        let video = unsafe { &mut *this }.video_source();
        let backend = unsafe { &*this }.render_engine();
        let media_ctx_ptr = video as *mut MediaContext;

        let render_data = Box::new(SkiaRenderData {
            asset_paths: unsafe { &(*this).asset_paths },
            media_ctx: media_ctx_ptr,
        });

        // Extend the lifetime of render_data for the duration of the call
        let extended_data: &'static mut SkiaRenderData = unsafe {
            std::mem::transmute::<Box<SkiaRenderData>, &'static mut SkiaRenderData>(render_data)
        };

        let result = f(video, backend, extended_data);

        // Clean up the extended reference
        let _ = unsafe { Box::from_raw(extended_data as *mut SkiaRenderData) };

        result
    }
}

/// Walk the scene tree and register canvas asset aliases in both catalogs.
fn register_canvas_asset_aliases(
    state: &FrameState,
    frame_ctx: &opencat_core::frame_ctx::FrameCtx,
    catalog: &mut opencat_core::resource::hash_map_catalog::HashMapResourceCatalog,
    path_store: &mut crate::resource::path_store::AssetPathStore,
) {
    match state {
        FrameState::Scene { scene, .. } => {
            register_canvas_aliases_from_node(scene, frame_ctx, catalog, path_store);
        }
        FrameState::Transition { from, to, .. } => {
            register_canvas_aliases_from_node(from, frame_ctx, catalog, path_store);
            register_canvas_aliases_from_node(to, frame_ctx, catalog, path_store);
        }
    }
}

fn register_canvas_aliases_from_node(
    node: &Node,
    frame_ctx: &opencat_core::frame_ctx::FrameCtx,
    catalog: &mut opencat_core::resource::hash_map_catalog::HashMapResourceCatalog,
    path_store: &mut crate::resource::path_store::AssetPathStore,
) {
    use opencat_core::resource::catalog::ResourceCatalog;
    match node.kind() {
        NodeKind::Component(component) => {
            let rendered = component.render(frame_ctx);
            register_canvas_aliases_from_node(&rendered, frame_ctx, catalog, path_store);
        }
        NodeKind::Div(div) => {
            for child in div.children_ref() {
                register_canvas_aliases_from_node(child, frame_ctx, catalog, path_store);
            }
        }
        NodeKind::Canvas(canvas) => {
            for asset in canvas.assets_ref() {
                if let ImageSource::Path(ref path) = asset.source {
                    let target = catalog
                        .resolve_image(&ImageSource::Path(path.clone()))
                        .unwrap();
                    let alias_id = AssetId(asset.asset_id.clone());
                    let _ = catalog.alias(alias_id.clone(), &target);
                    if let Some(_target_path) = path_store.path(&target) {
                        let _ = path_store.alias(alias_id, &target);
                    }
                }
            }
        }
        NodeKind::Timeline(timeline) => {
            for segment in timeline.segments() {
                match segment {
                    opencat_core::scene::time::TimelineSegment::Scene { scene, .. } => {
                        register_canvas_aliases_from_node(scene, frame_ctx, catalog, path_store);
                    }
                    opencat_core::scene::time::TimelineSegment::Transition { from, to, .. } => {
                        register_canvas_aliases_from_node(from, frame_ctx, catalog, path_store);
                        register_canvas_aliases_from_node(to, frame_ctx, catalog, path_store);
                    }
                }
            }
        }
        NodeKind::Image(_)
        | NodeKind::Video(_)
        | NodeKind::Text(_)
        | NodeKind::Lucide(_)
        | NodeKind::Path(_)
        | NodeKind::Caption(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencat_core::Platform;

    #[test]
    fn engine_platform_constructs() {
        let backend = crate::backend::skia::renderer::shared_raster_engine_typed();
        let mut platform = EnginePlatform::new(backend);
        let _engine: &SkiaRenderEngine = platform.render_engine();
        let _script = platform.script_host();
        let _video = platform.video_source();
        let _bounds = platform.path_bounds();
    }
}
