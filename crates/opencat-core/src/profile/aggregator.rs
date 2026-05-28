use std::collections::BTreeMap;

use super::{BackendSpanKey, FrameProfile};

#[derive(Clone, Copy, Debug)]
pub struct CompletedProfileSpan {
    pub frame: u32,
    pub target: &'static str,
    pub name: &'static str,
    pub parent: Option<&'static str>,
    pub inclusive_ms: f64,
    pub exclusive_ms: f64,
    pub backend_depth: Option<usize>,
    pub transition_kind: Option<&'static str>,
}

#[derive(Clone, Copy, Debug)]
pub struct ProfileCountEvent {
    pub frame: u32,
    pub kind: &'static str,
    pub name: &'static str,
    pub result: &'static str,
    pub amount: usize,
}

#[derive(Clone, Debug, Default)]
pub struct RenderProfileSummary {
    pub frames: BTreeMap<u32, FrameProfile>,
}

impl RenderProfileSummary {
    #[allow(dead_code)]
    pub fn average_light_leak_transition_ms(&self) -> f64 {
        let total_count = self
            .frames
            .values()
            .map(|frame| frame.light_leak_transition_frames)
            .sum::<usize>();
        if total_count == 0 {
            return 0.0;
        }
        self.frames
            .values()
            .map(|frame| frame.light_leak_transition_ms)
            .sum::<f64>()
            / total_count as f64
    }
}

#[derive(Default)]
pub struct RenderProfileAggregator {
    frames: BTreeMap<u32, FrameProfile>,
}

impl RenderProfileAggregator {
    pub fn frame_mut(&mut self, frame: u32) -> &mut FrameProfile {
        self.frames.entry(frame).or_default()
    }

    pub fn record_span(&mut self, span: CompletedProfileSpan) {
        match (span.target, span.name) {
            ("render.pipeline", "frame_state") => {
                self.frame_mut(span.frame).frame_state_ms += span.inclusive_ms;
                return;
            }
            ("render.pipeline", "script") => {
                self.frame_mut(span.frame).script_ms += span.inclusive_ms;
                return;
            }
            ("render.scene", "resolve_ui_tree") => {
                self.frame_mut(span.frame).resolve_ms += span.inclusive_ms;
                return;
            }
            ("render.scene", "compute_layout") => {
                self.frame_mut(span.frame).layout_ms += span.inclusive_ms;
                return;
            }
            ("render.scene", "build_display_tree") => {
                self.frame_mut(span.frame).display_ms += span.inclusive_ms;
                return;
            }
            ("render.scene", "layout_structure_update") => {
                self.frame_mut(span.frame).layout_ms += span.inclusive_ms;
                return;
            }
            ("render.scene", "layout_resolve") => {
                self.frame_mut(span.frame).layout_ms += span.inclusive_ms;
                return;
            }
            ("render.transition", "draw_transition") => {
                let frame_state = self.frame_mut(span.frame);
                frame_state.transition_ms += span.inclusive_ms;
                match span.transition_kind {
                    Some("slide") => {
                        frame_state.slide_transition_ms += span.inclusive_ms;
                        frame_state.slide_transition_frames += 1;
                    }
                    Some("light_leak") => {
                        frame_state.light_leak_transition_ms += span.inclusive_ms;
                        frame_state.light_leak_transition_frames += 1;
                    }
                    _ => {}
                }
                return;
            }
            _ => {}
        }

        let frame = self.frame_mut(span.frame);
        if span.target == "render.backend" {
            let depth = span.backend_depth.unwrap_or(0);
            frame
                .backend_spans
                .entry(BackendSpanKey {
                    depth,
                    parent: span.parent,
                    name: span.name,
                })
                .or_default()
                .record(span.inclusive_ms, span.exclusive_ms);
            if depth == 0 {
                frame.backend_ms += span.inclusive_ms;
            }
            match span.name {
                "node_own_segment_record" => {
                    frame.backend.node_own_segment_record_ms += span.inclusive_ms;
                }
                "subtree_image_rasterize" => {
                    frame.backend.subtree_image_rasterize_ms += span.inclusive_ms;
                }
                "subtree_image_draw" => {
                    frame.backend.subtree_image_draw_ms += span.inclusive_ms;
                }
                "light_leak_mask" => {
                    frame.backend.light_leak_mask_ms += span.inclusive_ms;
                }
                "light_leak_composite" => {
                    frame.backend.light_leak_composite_ms += span.inclusive_ms;
                }
                _ => {}
            }
        }
    }

    pub fn record_count(&mut self, event: ProfileCountEvent) {
        let frame = self.frame_mut(event.frame);
        match (event.kind, event.name, event.result) {
            ("cache", "subtree_snapshot_request_after_analyze", "fresh") => {
                frame.backend.subtree_snapshot_request_after_analyze_fresh += event.amount;
            }
            ("cache", "subtree_snapshot_request_after_analyze", "reused") => {
                frame.backend.subtree_snapshot_request_after_analyze_reused += event.amount;
            }
            ("cache", "subtree_snapshot_request_after_analyze", "composite_blocked") => {
                frame
                    .backend
                    .subtree_snapshot_request_after_analyze_composite_blocked += event.amount;
            }
            ("cache", "scene_snapshot", "hit") => {
                frame.backend.scene_snapshot_cache_hits += event.amount;
            }
            ("cache", "scene_snapshot", "miss") => {
                frame.backend.scene_snapshot_cache_misses += event.amount;
            }
            ("cache", "scene_snapshot_miss", "plan_blocked") => {
                frame.backend.scene_snapshot_miss_plan_blocked += event.amount;
            }
            ("cache", "scene_snapshot_miss", "empty") => {
                frame.backend.scene_snapshot_miss_empty += event.amount;
            }
            ("cache", "scene_snapshot_miss", "viewport_changed") => {
                frame.backend.scene_snapshot_miss_viewport_changed += event.amount;
            }
            ("cache", "scene_snapshot_miss", "root_fingerprint_changed") => {
                frame.backend.scene_snapshot_miss_root_fingerprint_changed += event.amount;
            }
            ("cache", "scene_snapshot_plan_blocked", "structure") => {
                frame.backend.scene_snapshot_plan_blocked_by_structure += event.amount;
            }
            ("cache", "scene_snapshot_plan_blocked", "layout") => {
                frame.backend.scene_snapshot_plan_blocked_by_layout += event.amount;
            }
            ("cache", "scene_snapshot_plan_blocked", "raster") => {
                frame.backend.scene_snapshot_plan_blocked_by_raster += event.amount;
            }
            ("cache", "scene_snapshot_plan_blocked", "apply_change") => {
                frame.backend.scene_snapshot_plan_blocked_by_apply_change += event.amount;
            }
            ("cache", "subtree_image", "hit") => {
                frame.backend.subtree_image_cache_hits += event.amount;
            }
            ("cache", "subtree_image", "miss") => {
                frame.backend.subtree_image_cache_misses += event.amount;
            }
            ("cache", "subtree_image", "promote") => {
                frame.backend.subtree_image_promotions += event.amount;
            }
            ("cache", "glyph_path", "hit") => {
                frame.backend.glyph_path_cache_hits += event.amount;
            }
            ("cache", "glyph_path", "miss") => {
                frame.backend.glyph_path_cache_misses += event.amount;
            }
            ("cache", "glyph_image", "hit") => {
                frame.backend.glyph_image_cache_hits += event.amount;
            }
            ("cache", "glyph_image", "miss") => {
                frame.backend.glyph_image_cache_misses += event.amount;
            }
            ("cache", "item_picture", "hit") => {
                frame.backend.item_picture_cache_hits += event.amount;
            }
            ("cache", "item_picture", "miss") => {
                frame.backend.item_picture_cache_misses += event.amount;
            }
            ("cache", "image", "hit") => {
                frame.backend.image_cache_hits += event.amount;
            }
            ("cache", "image", "miss") => {
                frame.backend.image_cache_misses += event.amount;
            }
            ("cache", "video_frame", "hit") => {
                frame.backend.video_frame_cache_hits += event.amount;
            }
            ("cache", "video_frame", "miss") => {
                frame.backend.video_frame_cache_misses += event.amount;
            }
            ("cache", "video_frame", "decode") => {
                frame.backend.video_frame_decodes += event.amount;
            }
            ("draw", "rect", "count") => {
                frame.backend.draw_rect_count += event.amount;
            }
            ("draw", "text", "count") => {
                frame.backend.draw_text_count += event.amount;
            }
            ("draw", "bitmap", "count") => {
                frame.backend.draw_bitmap_count += event.amount;
            }
            ("draw", "script", "count") => {
                frame.backend.draw_script_count += event.amount;
            }
            ("layer", "save_layer", "count") => {
                frame.backend.save_layer_count += event.amount;
            }
            ("eviction", "glyph_path", "count") => {
                frame.backend.glyph_path_cache_evictions += event.amount;
            }
            ("repeat", "glyph_path", "count") => {
                frame.backend.glyph_path_cache_record_repeats += event.amount;
            }
            ("utilization", "glyph_path", "count") => {
                frame.backend.glyph_path_cache_capacity_utilization = frame
                    .backend
                    .glyph_path_cache_capacity_utilization
                    .max(event.amount);
            }
            ("eviction", "item_picture", "count") => {
                frame.backend.item_picture_cache_evictions += event.amount;
            }
            ("repeat", "item_picture", "count") => {
                frame.backend.item_picture_cache_record_repeats += event.amount;
            }
            ("utilization", "item_picture", "count") => {
                frame.backend.item_picture_cache_capacity_utilization = frame
                    .backend
                    .item_picture_cache_capacity_utilization
                    .max(event.amount);
            }
            ("eviction", "subtree_image", "count") => {
                frame.backend.subtree_image_cache_evictions += event.amount;
            }
            ("repeat", "subtree_image", "count") => {
                frame.backend.subtree_image_cache_record_repeats += event.amount;
            }
            ("utilization", "subtree_image", "count") => {
                frame.backend.subtree_image_cache_capacity_utilization = frame
                    .backend
                    .subtree_image_cache_capacity_utilization
                    .max(event.amount);
            }
            ("eviction", "image", "count") => {
                frame.backend.image_cache_evictions += event.amount;
            }
            ("repeat", "image", "count") => {
                frame.backend.image_cache_record_repeats += event.amount;
            }
            ("utilization", "image", "count") => {
                frame.backend.image_cache_capacity_utilization = frame
                    .backend
                    .image_cache_capacity_utilization
                    .max(event.amount);
            }
            ("layout", "reused_nodes", "count") => {
                frame.reused_nodes += event.amount;
            }
            ("layout", "input_merkle_full_hit_subtrees", "count") => {
                frame.input_merkle_full_hit_subtrees += event.amount;
            }
            ("layout", "input_merkle_full_hit_nodes", "count") => {
                frame.input_merkle_full_hit_nodes += event.amount;
            }
            ("layout", "layout_merkle_skipped_subtrees", "count") => {
                frame.layout_merkle_skipped_subtrees += event.amount;
            }
            ("layout", "layout_merkle_skipped_nodes", "count") => {
                frame.layout_merkle_skipped_nodes += event.amount;
            }
            ("layout", "layout_dirty", "count") => {
                frame.layout_dirty_nodes += event.amount;
            }
            ("layout", "raster_dirty", "count") => {
                frame.raster_dirty_nodes += event.amount;
            }
            ("layout", "structure_rebuild", "count") => {
                frame.structure_rebuilds += event.amount;
            }
            ("display", "display_recorded_subtree_identical_subtrees", "count") => {
                frame.display_recorded_subtree_identical_subtrees += event.amount;
            }
            ("display", "display_recorded_subtree_identical_nodes", "count") => {
                frame.display_recorded_subtree_identical_nodes += event.amount;
            }
            ("display", "display_merkle_skipped_subtrees", "count") => {
                frame.display_merkle_skipped_subtrees += event.amount;
            }
            ("display", "display_merkle_skipped_nodes", "count") => {
                frame.display_merkle_skipped_nodes += event.amount;
            }
            ("display", "display_rebuilt_nodes", "count") => {
                frame.display_rebuilt_nodes += event.amount;
            }
            ("display", "display_apply_only_nodes", "count") => {
                frame.display_apply_only_nodes += event.amount;
            }
            ("analyze", "analyze_merkle_skipped_subtrees", "count") => {
                frame.analyze_merkle_skipped_subtrees += event.amount;
            }
            ("analyze", "analyze_merkle_skipped_nodes", "count") => {
                frame.analyze_merkle_skipped_nodes += event.amount;
            }
            ("analyze", "analyze_recorded_hit_subtrees", "count") => {
                frame.analyze_recorded_hit_subtrees += event.amount;
            }
            ("analyze", "analyze_recorded_hit_nodes", "count") => {
                frame.analyze_recorded_hit_nodes += event.amount;
            }
            ("analyze", "analyze_snapshot_eligibility_hit_subtrees", "count") => {
                frame.analyze_snapshot_eligibility_hit_subtrees += event.amount;
            }
            ("analyze", "analyze_snapshot_eligibility_hit_nodes", "count") => {
                frame.analyze_snapshot_eligibility_hit_nodes += event.amount;
            }
            ("analyze", "analyze_composite_blocked_subtrees", "count") => {
                frame.analyze_composite_blocked_subtrees += event.amount;
            }
            ("analyze", "analyze_composite_blocked_nodes", "count") => {
                frame.analyze_composite_blocked_nodes += event.amount;
            }
            ("analyze", "analyze_apply_changed_nodes", "count") => {
                frame.analyze_apply_changed_nodes += event.amount;
            }
            ("cache", "node_own_segment", "hit") => {
                frame.backend.node_own_segment_hits += event.amount;
            }
            ("cache", "node_own_segment", "record") => {
                frame.backend.node_own_segment_records += event.amount;
            }
            ("cache", "node_own_segment", "replaced") => {
                frame.backend.node_own_segment_replaced += event.amount;
            }
            ("cache", "apply_segment", "hit") => {
                frame.backend.apply_segment_hits += event.amount;
            }
            ("cache", "apply_segment", "miss") => {
                frame.backend.apply_segment_misses += event.amount;
            }
            ("eviction", "apply", "count") => {
                frame.backend.apply_cache_evictions += event.amount;
            }
            ("repeat", "apply", "count") => {
                frame.backend.apply_cache_record_repeats += event.amount;
            }
            ("utilization", "apply", "count") => {
                frame.backend.apply_cache_capacity_utilization =
                    frame.backend.apply_cache_capacity_utilization.max(event.amount);
            }
            ("eviction", "node_own", "count") => {
                frame.backend.node_own_cache_evictions += event.amount;
            }
            ("repeat", "node_own", "count") => {
                frame.backend.node_own_cache_record_repeats += event.amount;
            }
            ("utilization", "node_own", "count") => {
                frame.backend.node_own_cache_capacity_utilization = frame
                    .backend
                    .node_own_cache_capacity_utilization
                    .max(event.amount);
            }
            _ => {}
        }
    }

    pub fn finish(self) -> RenderProfileSummary {
        RenderProfileSummary {
            frames: self.frames,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ProfileCountEvent, RenderProfileAggregator};

    #[test]
    fn count_events_record_input_merkle_full_hit_subtrees() {
        let mut aggregator = RenderProfileAggregator::default();

        aggregator.record_count(ProfileCountEvent {
            frame: 7,
            kind: "layout",
            name: "input_merkle_full_hit_subtrees",
            result: "count",
            amount: 3,
        });

        let summary = aggregator.finish();
        assert_eq!(summary.frames[&7].input_merkle_full_hit_subtrees, 3);
    }

    #[test]
    fn count_events_record_layout_merkle_skipped_nodes() {
        let mut aggregator = RenderProfileAggregator::default();

        aggregator.record_count(ProfileCountEvent {
            frame: 7,
            kind: "layout",
            name: "layout_merkle_skipped_nodes",
            result: "count",
            amount: 12,
        });

        let summary = aggregator.finish();
        assert_eq!(summary.frames[&7].layout_merkle_skipped_nodes, 12);
    }

    #[test]
    fn count_events_record_analyze_merkle_skipped_nodes() {
        let mut aggregator = RenderProfileAggregator::default();

        aggregator.record_count(ProfileCountEvent {
            frame: 7,
            kind: "analyze",
            name: "analyze_merkle_skipped_nodes",
            result: "count",
            amount: 12,
        });

        let summary = aggregator.finish();
        assert_eq!(summary.frames[&7].analyze_merkle_skipped_nodes, 12);
    }

    #[test]
    fn count_events_record_analyze_recorded_hit_nodes() {
        let mut aggregator = RenderProfileAggregator::default();

        aggregator.record_count(ProfileCountEvent {
            frame: 7,
            kind: "analyze",
            name: "analyze_recorded_hit_nodes",
            result: "count",
            amount: 12,
        });

        let summary = aggregator.finish();
        assert_eq!(summary.frames[&7].analyze_recorded_hit_nodes, 12);
    }

    #[test]
    fn count_events_record_display_recorded_subtree_identical() {
        let mut aggregator = RenderProfileAggregator::default();

        aggregator.record_count(ProfileCountEvent {
            frame: 7,
            kind: "display",
            name: "display_recorded_subtree_identical_subtrees",
            result: "count",
            amount: 3,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 7,
            kind: "display",
            name: "display_recorded_subtree_identical_nodes",
            result: "count",
            amount: 15,
        });

        let summary = aggregator.finish();
        assert_eq!(
            summary.frames[&7].display_recorded_subtree_identical_subtrees,
            3
        );
        assert_eq!(
            summary.frames[&7].display_recorded_subtree_identical_nodes,
            15
        );
    }

    #[test]
    fn count_events_subtree_snapshot_request_after_analyze_reasons() {
        let mut aggregator = RenderProfileAggregator::default();

        aggregator.record_count(ProfileCountEvent {
            frame: 7,
            kind: "cache",
            name: "subtree_snapshot_request_after_analyze",
            result: "fresh",
            amount: 4,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 7,
            kind: "cache",
            name: "subtree_snapshot_request_after_analyze",
            result: "reused",
            amount: 6,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 7,
            kind: "cache",
            name: "subtree_snapshot_request_after_analyze",
            result: "composite_blocked",
            amount: 2,
        });

        let summary = aggregator.finish();
        let frame = &summary.frames[&7];
        assert_eq!(
            frame.backend.subtree_snapshot_request_after_analyze_fresh,
            4
        );
        assert_eq!(
            frame.backend.subtree_snapshot_request_after_analyze_reused,
            6
        );
        assert_eq!(
            frame
                .backend
                .subtree_snapshot_request_after_analyze_composite_blocked,
            2
        );
    }

    #[test]
    fn count_events_record_node_own_segment_activity() {
        let mut aggregator = RenderProfileAggregator::default();

        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "cache",
            name: "node_own_segment",
            result: "hit",
            amount: 2,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "cache",
            name: "node_own_segment",
            result: "record",
            amount: 3,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "cache",
            name: "node_own_segment",
            result: "replaced",
            amount: 1,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "eviction",
            name: "node_own",
            result: "count",
            amount: 4,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "repeat",
            name: "node_own",
            result: "count",
            amount: 5,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "utilization",
            name: "node_own",
            result: "count",
            amount: 6,
        });

        let summary = aggregator.finish();
        let backend = &summary.frames[&1].backend;
        assert_eq!(backend.node_own_segment_hits, 2);
        assert_eq!(backend.node_own_segment_records, 3);
        assert_eq!(backend.node_own_segment_replaced, 1);
        assert_eq!(backend.node_own_cache_evictions, 4);
        assert_eq!(backend.node_own_cache_record_repeats, 5);
        assert_eq!(backend.node_own_cache_capacity_utilization, 6);
    }

    #[test]
    fn count_events_record_apply_segment_activity() {
        let mut aggregator = RenderProfileAggregator::default();

        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "cache",
            name: "apply_segment",
            result: "hit",
            amount: 2,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "cache",
            name: "apply_segment",
            result: "miss",
            amount: 3,
        });

        let summary = aggregator.finish();
        let backend = &summary.frames[&1].backend;
        assert_eq!(backend.apply_segment_hits, 2);
        assert_eq!(backend.apply_segment_misses, 3);
    }

    #[test]
    fn count_events_record_apply_cache_pressure() {
        let mut aggregator = RenderProfileAggregator::default();

        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "eviction",
            name: "apply",
            result: "count",
            amount: 4,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "repeat",
            name: "apply",
            result: "count",
            amount: 5,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "utilization",
            name: "apply",
            result: "count",
            amount: 6,
        });

        let summary = aggregator.finish();
        let backend = &summary.frames[&1].backend;
        assert_eq!(backend.apply_cache_evictions, 4);
        assert_eq!(backend.apply_cache_record_repeats, 5);
        assert_eq!(backend.apply_cache_capacity_utilization, 6);
    }

    #[test]
    fn count_events_keep_peak_cache_utilization_per_frame() {
        let mut aggregator = RenderProfileAggregator::default();

        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "utilization",
            name: "apply",
            result: "count",
            amount: 6,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "utilization",
            name: "apply",
            result: "count",
            amount: 9,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "utilization",
            name: "apply",
            result: "count",
            amount: 4,
        });

        let summary = aggregator.finish();
        let backend = &summary.frames[&1].backend;
        assert_eq!(backend.apply_cache_capacity_utilization, 9);
    }

    #[test]
    fn count_events_record_scene_snapshot_miss_reasons() {
        let mut aggregator = RenderProfileAggregator::default();

        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "cache",
            name: "scene_snapshot_miss",
            result: "plan_blocked",
            amount: 2,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "cache",
            name: "scene_snapshot_miss",
            result: "empty",
            amount: 3,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "cache",
            name: "scene_snapshot_miss",
            result: "viewport_changed",
            amount: 4,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "cache",
            name: "scene_snapshot_miss",
            result: "root_fingerprint_changed",
            amount: 5,
        });

        let summary = aggregator.finish();
        let backend = &summary.frames[&1].backend;
        assert_eq!(backend.scene_snapshot_miss_plan_blocked, 2);
        assert_eq!(backend.scene_snapshot_miss_empty, 3);
        assert_eq!(backend.scene_snapshot_miss_viewport_changed, 4);
        assert_eq!(backend.scene_snapshot_miss_root_fingerprint_changed, 5);
    }

    #[test]
    fn count_events_record_scene_snapshot_plan_block_reasons() {
        let mut aggregator = RenderProfileAggregator::default();

        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "cache",
            name: "scene_snapshot_plan_blocked",
            result: "structure",
            amount: 2,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "cache",
            name: "scene_snapshot_plan_blocked",
            result: "layout",
            amount: 3,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "cache",
            name: "scene_snapshot_plan_blocked",
            result: "raster",
            amount: 4,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 1,
            kind: "cache",
            name: "scene_snapshot_plan_blocked",
            result: "apply_change",
            amount: 5,
        });

        let summary = aggregator.finish();
        let backend = &summary.frames[&1].backend;
        assert_eq!(backend.scene_snapshot_plan_blocked_by_structure, 2);
        assert_eq!(backend.scene_snapshot_plan_blocked_by_layout, 3);
        assert_eq!(backend.scene_snapshot_plan_blocked_by_raster, 4);
        assert_eq!(backend.scene_snapshot_plan_blocked_by_apply_change, 5);
    }
}
