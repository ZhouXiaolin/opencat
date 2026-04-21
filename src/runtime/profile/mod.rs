mod aggregator;
mod layer;
mod output;

use std::collections::BTreeMap;

use crate::layout::LayoutPassStats;

pub(crate) use aggregator::{
    CompletedProfileSpan, ProfileCountEvent, RenderProfileAggregator, RenderProfileSummary,
};
pub(crate) use layer::{ProfileConfig, ProfileOutputFormat, profile_render};
pub(crate) use output::print_profile_summary;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct BackendSpanKey {
    pub depth: usize,
    pub parent: Option<&'static str>,
    pub name: &'static str,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BackendSpanAggregate {
    pub inclusive_ms: f64,
    pub exclusive_ms: f64,
    pub count: usize,
}

impl BackendSpanAggregate {
    pub(crate) fn record(&mut self, inclusive_ms: f64, exclusive_ms: f64) {
        self.inclusive_ms += inclusive_ms;
        self.exclusive_ms += exclusive_ms;
        self.count += 1;
    }

    pub(crate) fn merge(&mut self, other: &BackendSpanAggregate) {
        self.inclusive_ms += other.inclusive_ms;
        self.exclusive_ms += other.exclusive_ms;
        self.count += other.count;
    }
}

#[derive(Clone, Debug, Default)]
pub struct BackendProfile {
    pub subtree_snapshot_record_ms: f64,
    pub subtree_snapshot_draw_ms: f64,
    pub subtree_image_rasterize_ms: f64,
    pub subtree_image_draw_ms: f64,
    pub light_leak_mask_ms: f64,
    pub light_leak_composite_ms: f64,
    pub scene_snapshot_cache_hits: usize,
    pub scene_snapshot_cache_misses: usize,
    pub subtree_snapshot_cache_hits: usize,
    pub subtree_snapshot_cache_misses: usize,
    pub subtree_snapshot_collision_rejected: usize,
    pub subtree_image_cache_hits: usize,
    pub subtree_image_cache_misses: usize,
    pub subtree_image_promotions: usize,
    pub text_cache_hits: usize,
    pub text_cache_misses: usize,
    pub item_picture_cache_hits: usize,
    pub item_picture_cache_misses: usize,
    pub image_cache_hits: usize,
    pub image_cache_misses: usize,
    pub video_frame_cache_hits: usize,
    pub video_frame_cache_misses: usize,
    pub video_frame_decodes: usize,
    pub draw_rect_count: usize,
    pub draw_text_count: usize,
    pub draw_bitmap_count: usize,
    pub draw_script_count: usize,
    pub save_layer_count: usize,
    pub text_cache_evictions: usize,
    pub text_cache_record_repeats: usize,
    pub text_cache_capacity_utilization: usize,
    pub item_picture_cache_evictions: usize,
    pub item_picture_cache_record_repeats: usize,
    pub item_picture_cache_capacity_utilization: usize,
    pub subtree_snapshot_cache_evictions: usize,
    pub subtree_snapshot_cache_record_repeats: usize,
    pub subtree_snapshot_cache_capacity_utilization: usize,
    pub subtree_image_cache_evictions: usize,
    pub subtree_image_cache_record_repeats: usize,
    pub subtree_image_cache_capacity_utilization: usize,
    pub image_cache_evictions: usize,
    pub image_cache_record_repeats: usize,
    pub image_cache_capacity_utilization: usize,
}

#[derive(Default)]
pub(crate) struct SceneBuildStats {
    pub resolve_ms: f64,
    pub layout_ms: f64,
    pub display_ms: f64,
    pub layout_pass: LayoutPassStats,
    pub contains_time_variant_paint: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct FrameProfile {
    pub script_ms: f64,
    pub frame_state_ms: f64,
    pub resolve_ms: f64,
    pub layout_ms: f64,
    pub display_ms: f64,
    pub backend_ms: f64,
    pub transition_ms: f64,
    pub slide_transition_ms: f64,
    pub light_leak_transition_ms: f64,
    pub slide_transition_frames: usize,
    pub light_leak_transition_frames: usize,
    pub reused_nodes: usize,
    pub layout_dirty_nodes: usize,
    pub raster_dirty_nodes: usize,
    pub composite_dirty_nodes: usize,
    pub structure_rebuilds: usize,
    pub backend: BackendProfile,
    pub backend_spans: BTreeMap<BackendSpanKey, BackendSpanAggregate>,
}
