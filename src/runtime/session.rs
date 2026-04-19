use crate::{
    layout::LayoutSession,
    resource::{
        assets::AssetsMap,
        media::{MediaContext, VideoPreviewQuality},
    },
    runtime::{
        audio::{AudioIntervalCache, DecodedAudioCache},
        cache::{CacheCaps, CacheRegistry},
        compositor::{SceneSlot, SceneSnapshotCache},
        invalidation::CompositeHistory,
        render_engine::SharedRenderEngine,
        render_registry,
        text_engine::SharedTextEngine,
    },
    scene::script::ScriptRuntimeCache,
};

pub struct RenderSession {
    pub(crate) media_ctx: MediaContext,
    pub(crate) assets: AssetsMap,
    pub(crate) scene_snapshots: SceneSnapshotCache,
    pub(crate) cache_registry: CacheRegistry,
    pub(crate) script_runtime: ScriptRuntimeCache,
    pub(crate) scene_layout: LayoutSession,
    pub(crate) transition_from_layout: LayoutSession,
    pub(crate) transition_to_layout: LayoutSession,
    pub(crate) prepared_root_ptr: Option<usize>,
    pub(crate) audio_decode_cache: DecodedAudioCache,
    pub(crate) audio_interval_cache: AudioIntervalCache,
    pub(crate) text_engine: SharedTextEngine,
    pub(crate) render_engine: SharedRenderEngine,
    composite_history: CompositeHistory,
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
        let text_engine = render_engine.text_engine();
        Self {
            media_ctx: MediaContext::with_cache_caps(cache_caps),
            assets: AssetsMap::new(),
            scene_snapshots: SceneSnapshotCache::new(),
            cache_registry: CacheRegistry::new(cache_caps),
            script_runtime: ScriptRuntimeCache::default(),
            scene_layout: LayoutSession::new(),
            transition_from_layout: LayoutSession::new(),
            transition_to_layout: LayoutSession::new(),
            prepared_root_ptr: None,
            audio_decode_cache: DecodedAudioCache::default(),
            audio_interval_cache: AudioIntervalCache::default(),
            text_engine,
            render_engine,
            composite_history: CompositeHistory::default(),
        }
    }

    pub fn set_video_preview_quality(&mut self, quality: VideoPreviewQuality) {
        self.media_ctx.set_video_preview_quality(quality);
    }

    pub(crate) fn layout_session_mut(&mut self, slot: SceneSlot) -> &mut LayoutSession {
        match slot {
            SceneSlot::Scene => &mut self.scene_layout,
            SceneSlot::TransitionFrom => &mut self.transition_from_layout,
            SceneSlot::TransitionTo => &mut self.transition_to_layout,
        }
    }

    pub(crate) fn text_engine_handle(&self) -> SharedTextEngine {
        self.text_engine.clone()
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
