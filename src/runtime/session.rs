use crate::{
    backend::resource_cache::BackendResourceCache,
    layout::LayoutSession,
    resource::{assets::AssetsMap, media::MediaContext},
    runtime::{
        audio::DecodedAudioCache,
        policy::cache::{SceneSlot, SceneSnapshotCache},
        profile::RenderProfiler,
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
    pub(crate) backend_resources: BackendResourceCache,
    pub(crate) script_runtime: ScriptRuntimeCache,
    pub(crate) scene_layout: LayoutSession,
    pub(crate) transition_from_layout: LayoutSession,
    pub(crate) transition_to_layout: LayoutSession,
    pub(crate) profiler: RenderProfiler,
    pub(crate) prepared_root_ptr: Option<usize>,
    pub(crate) audio_decode_cache: DecodedAudioCache,
    pub(crate) text_engine: SharedTextEngine,
    pub(crate) render_engine: SharedRenderEngine,
}

impl RenderSession {
    pub fn new() -> Self {
        Self::with_render_engine(render_registry::default_render_engine())
    }

    pub(crate) fn with_render_engine(render_engine: SharedRenderEngine) -> Self {
        let text_engine = render_engine.text_engine();
        Self {
            media_ctx: MediaContext::new(),
            assets: AssetsMap::new(),
            scene_snapshots: SceneSnapshotCache::new(),
            backend_resources: BackendResourceCache::new(),
            script_runtime: ScriptRuntimeCache::default(),
            scene_layout: LayoutSession::new(),
            transition_from_layout: LayoutSession::new(),
            transition_to_layout: LayoutSession::new(),
            profiler: RenderProfiler::default(),
            prepared_root_ptr: None,
            audio_decode_cache: DecodedAudioCache::default(),
            text_engine,
            render_engine,
        }
    }

    pub fn print_profile_summary(&self) {
        self.profiler.print_summary();
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
}

impl Default for RenderSession {
    fn default() -> Self {
        Self::new()
    }
}
