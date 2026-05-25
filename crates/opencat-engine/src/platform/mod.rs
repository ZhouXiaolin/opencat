//! Engine-side runtime state: quickjs script runtime, ffmpeg/skia media
//! context, asset path table, and audio caches.

pub mod audio_runtime;

use std::path::PathBuf;

use opencat_core::parse::node::{Node, NodeKind};
use opencat_core::parse::primitives::ImageSource;
use opencat_core::parse::time::FrameState;
use opencat_core::resource::asset_id::AssetId;
use opencat_core::resource::preload::preload_all;

use crate::media::{AudioIntervalCache, DecodedAudioCache, MediaContext, VideoPreviewQuality};
use crate::resource::AssetPathStore;
use crate::resource::fetch::build_preload_runtime;
use crate::resource::resolver::EngineAssetResolver;
use crate::script::ScriptRuntimeCache;

pub use audio_runtime::AudioRuntime;

/// Engine runtime services owned by the engine render session.
pub struct EnginePlatform {
    pub script: ScriptRuntimeCache,
    pub video: MediaContext,
    pub asset_paths: AssetPathStore,
    pub audio: AudioRuntime,
    /// Decoded audio cache for streaming playback.
    pub audio_decode_cache: DecodedAudioCache,
    /// Audio interval cache for composition-level audio scheduling.
    pub audio_interval_cache: AudioIntervalCache,
}

impl EnginePlatform {
    pub fn new() -> Self {
        Self {
            script: ScriptRuntimeCache::default(),
            video: MediaContext::new(),
            asset_paths: AssetPathStore::new(),
            audio: AudioRuntime::new(),
            audio_decode_cache: DecodedAudioCache::default(),
            audio_interval_cache: AudioIntervalCache::default(),
        }
    }

    pub fn set_video_preview_quality(&mut self, quality: VideoPreviewQuality) {
        self.video.set_video_preview_quality(quality);
    }

    pub fn preflight(
        &mut self,
        composition: &opencat_core::parse::composition::Composition,
        catalog: &mut opencat_core::resource::hash_map_catalog::HashMapResourceCatalog,
    ) -> anyhow::Result<()> {
        let req = opencat_core::parse::preflight::collect_resource_requests(composition);

        let cache_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".opencat")
            .join("assets");
        std::fs::create_dir_all(&cache_dir).ok();

        let rt = build_preload_runtime("preflight")?;
        {
            let mut resolver = EngineAssetResolver::new(&mut self.asset_paths, cache_dir)?;
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
            let state = opencat_core::parse::time::frame_state_for_root(&root, &frame_ctx);
            register_canvas_asset_aliases(&state, &frame_ctx, catalog, &mut self.asset_paths);
        }

        Ok(())
    }
}

/// Walk the scene tree and register canvas asset aliases in both catalogs.
fn register_canvas_asset_aliases(
    state: &FrameState,
    frame_ctx: &opencat_core::frame_ctx::FrameCtx,
    catalog: &mut opencat_core::resource::hash_map_catalog::HashMapResourceCatalog,
    path_store: &mut crate::resource::AssetPathStore,
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
    path_store: &mut crate::resource::AssetPathStore,
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
            for child in canvas.hidden_children_ref() {
                register_canvas_aliases_from_node(child, frame_ctx, catalog, path_store);
            }
        }
        NodeKind::Timeline(timeline) => {
            for segment in timeline.segments() {
                match segment {
                    opencat_core::parse::time::TimelineSegment::Scene { scene, .. } => {
                        register_canvas_aliases_from_node(scene, frame_ctx, catalog, path_store);
                    }
                    opencat_core::parse::time::TimelineSegment::Transition { from, to, .. } => {
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

    #[test]
    fn engine_platform_constructs_runtime_services() {
        let platform = EnginePlatform::new();
        assert!(platform.asset_paths.entries.is_empty());
    }
}
