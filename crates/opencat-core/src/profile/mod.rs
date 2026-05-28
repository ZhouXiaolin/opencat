pub mod aggregator;
pub mod layer;
pub mod output;

use std::collections::BTreeMap;

use crate::layout::LayoutPassStats;

pub use aggregator::{
    CompletedProfileSpan, ProfileCountEvent, RenderProfileAggregator, RenderProfileSummary,
};
pub use layer::{ProfileConfig, profile_render};
pub use output::print_profile_summary;

pub fn run_from_env<T>(f: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
    let config = ProfileConfig::from_env();
    let (result, summary) = profile_render(&config, f)?;
    if let Some(summary) = summary {
        print_profile_summary(&summary);
    }
    Ok(result)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BackendSpanKey {
    pub depth: usize,
    pub parent: Option<&'static str>,
    pub name: &'static str,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BackendSpanAggregate {
    pub inclusive_ms: f64,
    pub exclusive_ms: f64,
    pub count: usize,
}

impl BackendSpanAggregate {
    pub fn record(&mut self, inclusive_ms: f64, exclusive_ms: f64) {
        self.inclusive_ms += inclusive_ms;
        self.exclusive_ms += exclusive_ms;
        self.count += 1;
    }
}

#[derive(Clone, Debug, Default)]
pub struct BackendProfile {
    pub node_own_segment_record_ms: f64,
    pub subtree_image_rasterize_ms: f64,
    pub subtree_image_draw_ms: f64,
    pub light_leak_mask_ms: f64,
    pub light_leak_composite_ms: f64,
    pub scene_snapshot_cache_hits: usize,
    pub scene_snapshot_cache_misses: usize,
    pub scene_snapshot_miss_plan_blocked: usize,
    pub scene_snapshot_miss_empty: usize,
    pub scene_snapshot_miss_viewport_changed: usize,
    pub scene_snapshot_miss_root_fingerprint_changed: usize,
    pub scene_snapshot_plan_blocked_by_structure: usize,
    pub scene_snapshot_plan_blocked_by_layout: usize,
    pub scene_snapshot_plan_blocked_by_raster: usize,
    pub scene_snapshot_plan_blocked_by_apply_change: usize,
    pub subtree_snapshot_request_after_analyze_fresh: usize,
    pub subtree_snapshot_request_after_analyze_reused: usize,
    pub subtree_snapshot_request_after_analyze_composite_blocked: usize,
    pub subtree_image_cache_hits: usize,
    pub subtree_image_cache_misses: usize,
    pub subtree_image_promotions: usize,
    pub glyph_path_cache_hits: usize,
    pub glyph_path_cache_misses: usize,
    pub glyph_image_cache_hits: usize,
    pub glyph_image_cache_misses: usize,
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
    pub glyph_path_cache_evictions: usize,
    pub glyph_path_cache_record_repeats: usize,
    pub glyph_path_cache_capacity_utilization: usize,
    pub item_picture_cache_evictions: usize,
    pub item_picture_cache_record_repeats: usize,
    pub item_picture_cache_capacity_utilization: usize,
    pub subtree_image_cache_evictions: usize,
    pub subtree_image_cache_record_repeats: usize,
    pub subtree_image_cache_capacity_utilization: usize,
    pub image_cache_evictions: usize,
    pub image_cache_record_repeats: usize,
    pub image_cache_capacity_utilization: usize,
    pub node_own_segment_hits: usize,
    pub node_own_segment_records: usize,
    pub node_own_segment_replaced: usize,
    pub apply_segment_hits: usize,
    pub apply_segment_misses: usize,
    pub apply_cache_evictions: usize,
    pub apply_cache_record_repeats: usize,
    pub apply_cache_capacity_utilization: usize,
    pub node_own_cache_evictions: usize,
    pub node_own_cache_record_repeats: usize,
    pub node_own_cache_capacity_utilization: usize,
}

#[derive(Default)]
pub struct SceneBuildStats {
    pub resolve_ms: f64,
    pub layout_ms: f64,
    pub display_ms: f64,
    pub layout_pass: LayoutPassStats,
}

#[derive(Clone, Debug, Default)]
pub struct FrameProfile {
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
    pub input_merkle_full_hit_subtrees: usize,
    pub input_merkle_full_hit_nodes: usize,
    pub layout_merkle_skipped_subtrees: usize,
    pub layout_merkle_skipped_nodes: usize,
    pub display_recorded_subtree_identical_subtrees: usize,
    pub display_recorded_subtree_identical_nodes: usize,
    pub display_merkle_skipped_subtrees: usize,
    pub display_merkle_skipped_nodes: usize,
    pub display_rebuilt_nodes: usize,
    pub display_apply_only_nodes: usize,
    pub analyze_merkle_skipped_subtrees: usize,
    pub analyze_merkle_skipped_nodes: usize,
    pub analyze_recorded_hit_subtrees: usize,
    pub analyze_recorded_hit_nodes: usize,
    pub analyze_snapshot_eligibility_hit_subtrees: usize,
    pub analyze_snapshot_eligibility_hit_nodes: usize,
    pub analyze_composite_blocked_subtrees: usize,
    pub analyze_composite_blocked_nodes: usize,
    pub analyze_apply_changed_nodes: usize,
    pub layout_dirty_nodes: usize,
    pub raster_dirty_nodes: usize,
    pub structure_rebuilds: usize,
    pub backend: BackendProfile,
    pub backend_spans: BTreeMap<BackendSpanKey, BackendSpanAggregate>,
}
