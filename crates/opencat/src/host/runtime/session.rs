use std::sync::Arc;

use crate::{
    core::layout::LayoutSession,
    core::resource::asset_catalog::AssetCatalog,
    host::resource::media::{MediaContext, VideoPreviewQuality},
    runtime::{
        audio::{AudioIntervalCache, DecodedAudioCache},
        cache::{CacheCaps, CacheRegistry},
        compositor::SceneSnapshotCache,
        invalidation::CompositeHistory,
        render_engine::SharedRenderEngine,
        render_registry,
    },
    host::script::ScriptRuntimeCache,
    core::text::default_font_db,
};

pub struct RenderSession {
    pub(crate) media_ctx: MediaContext,
    pub(crate) assets: AssetCatalog,
    pub(crate) scene_snapshots: SceneSnapshotCache,
    pub(crate) cache_registry: CacheRegistry,
    pub(crate) script_runtime: ScriptRuntimeCache,
    pub(crate) layout_session: LayoutSession,
    pub(crate) prepared_root_ptr: Option<usize>,
    pub(crate) audio_decode_cache: DecodedAudioCache,
    pub(crate) audio_interval_cache: AudioIntervalCache,
    pub(crate) font_db: Arc<fontdb::Database>,
    pub(crate) render_engine: SharedRenderEngine,
    pub(crate) composite_history: CompositeHistory,
}

impl RenderSession {
    pub fn new() -> Self {
        Self::with_cache_caps(CacheCaps::default())
    }

    pub fn with_cache_caps(caps: CacheCaps) -> Self {
        Self::with_render_engine_and_cache_caps(render_registry::default_render_engine(), caps)
    }

    pub(crate) fn with_render_engine(render_engine: SharedRenderEngine) -> Self {
        Self::with_render_engine_and_cache_caps(render_engine, CacheCaps::default())
    }

    pub(crate) fn with_render_engine_and_cache_caps(
        render_engine: SharedRenderEngine,
        cache_caps: CacheCaps,
    ) -> Self {
        let font_db = Arc::new(default_font_db(&[]));
        Self {
            media_ctx: MediaContext::with_cache_caps(cache_caps),
            assets: AssetCatalog::new(),
            scene_snapshots: SceneSnapshotCache::new(),
            cache_registry: CacheRegistry::new(cache_caps),
            script_runtime: ScriptRuntimeCache::default(),
            layout_session: LayoutSession::new(),
            prepared_root_ptr: None,
            audio_decode_cache: DecodedAudioCache::default(),
            audio_interval_cache: AudioIntervalCache::default(),
            font_db,
            render_engine,
            composite_history: CompositeHistory::default(),
        }
    }

    pub fn set_video_preview_quality(&mut self, quality: VideoPreviewQuality) {
        self.media_ctx.set_video_preview_quality(quality);
    }

    pub(crate) fn layout_session_mut(&mut self) -> &mut LayoutSession {
        &mut self.layout_session
    }

    pub(crate) fn font_db_handle(&self) -> Arc<fontdb::Database> {
        self.font_db.clone()
    }

    pub(crate) fn render_engine_handle(&self) -> SharedRenderEngine {
        self.render_engine.clone()
    }

    pub(crate) fn composite_history_mut(&mut self) -> &mut CompositeHistory {
        &mut self.composite_history
    }
}

impl Default for RenderSession {
    fn default() -> Self {
        Self::new()
    }
}
