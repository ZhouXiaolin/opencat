use crate::{
    backend::resource_cache::BackendResourceCache,
    layout::LayoutSession,
    resource::{
        assets::AssetsMap,
        media::{MediaContext, VideoPreviewQuality},
    },
    runtime::{
        audio::{AudioIntervalCache, DecodedAudioCache},
        cache::CacheCaps,
        fingerprint::CompositeSig,
        policy::cache::{SceneSlot, SceneSnapshotCache},
        profile::RenderProfiler,
        render_engine::SharedRenderEngine,
        render_registry,
        text_engine::SharedTextEngine,
    },
    scene::script::ScriptRuntimeCache,
};

#[derive(Default)]
struct CompositeHistoryCache {
    scene: Vec<CompositeSig>,
    transition_from: Vec<CompositeSig>,
    transition_to: Vec<CompositeSig>,
}

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
    pub(crate) audio_interval_cache: AudioIntervalCache,
    pub(crate) text_engine: SharedTextEngine,
    pub(crate) render_engine: SharedRenderEngine,
    composite_history: CompositeHistoryCache,
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
            backend_resources: BackendResourceCache::new(cache_caps),
            script_runtime: ScriptRuntimeCache::default(),
            scene_layout: LayoutSession::new(),
            transition_from_layout: LayoutSession::new(),
            transition_to_layout: LayoutSession::new(),
            profiler: RenderProfiler::default(),
            prepared_root_ptr: None,
            audio_decode_cache: DecodedAudioCache::default(),
            audio_interval_cache: AudioIntervalCache::default(),
            text_engine,
            render_engine,
            composite_history: CompositeHistoryCache::default(),
        }
    }

    pub fn print_profile_summary(&self) {
        self.profiler.print_summary();
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

    pub(crate) fn mark_display_tree_composite_dirty(
        &mut self,
        slot: SceneSlot,
        display_tree: &mut crate::display::tree::DisplayTree,
        structure_rebuild: bool,
    ) {
        let previous = if structure_rebuild {
            &[][..]
        } else {
            self.composite_history_for_slot(slot)
        };
        let mut next = Vec::new();
        let mut index = 0;
        mark_display_node_composite_dirty(&mut display_tree.root, previous, &mut index, &mut next);
        *self.composite_history_for_slot_mut(slot) = next;
    }

    fn composite_history_for_slot(&self, slot: SceneSlot) -> &[CompositeSig] {
        match slot {
            SceneSlot::Scene => &self.composite_history.scene,
            SceneSlot::TransitionFrom => &self.composite_history.transition_from,
            SceneSlot::TransitionTo => &self.composite_history.transition_to,
        }
    }

    fn composite_history_for_slot_mut(&mut self, slot: SceneSlot) -> &mut Vec<CompositeSig> {
        match slot {
            SceneSlot::Scene => &mut self.composite_history.scene,
            SceneSlot::TransitionFrom => &mut self.composite_history.transition_from,
            SceneSlot::TransitionTo => &mut self.composite_history.transition_to,
        }
    }
}

impl Default for RenderSession {
    fn default() -> Self {
        Self::new()
    }
}

fn mark_display_node_composite_dirty(
    node: &mut crate::display::tree::DisplayNode,
    previous: &[CompositeSig],
    index: &mut usize,
    next: &mut Vec<CompositeSig>,
) -> bool {
    let current_index = *index;
    *index += 1;

    let current_sig = CompositeSig::from_node(node);
    let composite_dirty = previous
        .get(current_index)
        .is_some_and(|previous_sig| *previous_sig != current_sig);
    next.push(current_sig);
    node.composite_dirty = composite_dirty;

    let mut subtree_contains_dynamic = node.paint_variance
        == crate::runtime::fingerprint::PaintVariance::TimeVariant
        || composite_dirty;
    for child in &mut node.children {
        subtree_contains_dynamic |= mark_display_node_composite_dirty(child, previous, index, next);
    }
    node.subtree_contains_dynamic = subtree_contains_dynamic;
    subtree_contains_dynamic
}
