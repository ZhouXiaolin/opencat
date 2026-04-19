use std::collections::BTreeMap;

use super::{
    BackendSpanKey, FrameProfile,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct CompletedProfileSpan {
    pub frame: u32,
    pub target: &'static str,
    pub name: &'static str,
    pub parent: Option<&'static str>,
    pub inclusive_ms: f64,
    pub exclusive_ms: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProfileCountEvent {
    pub frame: u32,
    pub target: &'static str,
    pub kind: &'static str,
    pub name: &'static str,
    pub result: &'static str,
    pub amount: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RenderProfileSummary {
    pub frames: BTreeMap<u32, FrameProfile>,
}

impl RenderProfileSummary {
    pub(crate) fn average_light_leak_transition_ms(&self) -> f64 {
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
pub(crate) struct RenderProfileAggregator {
    frames: BTreeMap<u32, FrameProfile>,
}

impl RenderProfileAggregator {
    pub(crate) fn frame_mut(&mut self, frame: u32) -> &mut FrameProfile {
        self.frames.entry(frame).or_default()
    }

    pub(crate) fn record_span(&mut self, span: CompletedProfileSpan) {
        match (span.target, span.name) {
            ("render.pipeline", "frame_state") => {
                self.frame_mut(span.frame).frame_state_ms += span.inclusive_ms;
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
            ("render.transition", "draw_transition") => {
                self.frame_mut(span.frame).transition_ms += span.inclusive_ms;
                return;
            }
            _ => {}
        }

        let frame = self.frame_mut(span.frame);
        if span.target == "render.backend" {
            let depth = usize::from(span.parent.is_some());
            frame
                .backend_spans
                .entry(BackendSpanKey {
                    depth,
                    parent: span.parent,
                    name: span.name,
                })
                .or_default()
                .record(span.inclusive_ms, span.exclusive_ms);
            match span.name {
                "display_tree_direct_draw" | "display_tree_snapshot_record" => {
                    frame.backend_ms += span.inclusive_ms;
                }
                "subtree_snapshot_record" => {
                    frame.backend.subtree_snapshot_record_ms += span.inclusive_ms;
                }
                "subtree_snapshot_draw" => {
                    frame.backend.subtree_snapshot_draw_ms += span.inclusive_ms;
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

    pub(crate) fn record_count(&mut self, event: ProfileCountEvent) {
        let frame = self.frame_mut(event.frame);
        match (event.kind, event.name, event.result) {
            ("cache", "subtree_snapshot", "hit") => {
                frame.backend.subtree_snapshot_cache_hits += event.amount;
            }
            ("cache", "subtree_snapshot", "miss") => {
                frame.backend.subtree_snapshot_cache_misses += event.amount;
            }
            ("cache", "scene_snapshot", "hit") => {
                frame.backend.scene_snapshot_cache_hits += event.amount;
            }
            ("cache", "scene_snapshot", "miss") => {
                frame.backend.scene_snapshot_cache_misses += event.amount;
            }
            ("cache", "subtree_snapshot", "collision_rejected") => {
                frame.backend.subtree_snapshot_collision_rejected += event.amount;
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
            ("cache", "text", "hit") => {
                frame.backend.text_cache_hits += event.amount;
            }
            ("cache", "text", "miss") => {
                frame.backend.text_cache_misses += event.amount;
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
            ("eviction", "text", "count") => {
                frame.backend.text_cache_evictions += event.amount;
            }
            ("repeat", "text", "count") => {
                frame.backend.text_cache_record_repeats += event.amount;
            }
            ("utilization", "text", "count") => {
                frame.backend.text_cache_capacity_utilization += event.amount;
            }
            ("eviction", "item_picture", "count") => {
                frame.backend.item_picture_cache_evictions += event.amount;
            }
            ("repeat", "item_picture", "count") => {
                frame.backend.item_picture_cache_record_repeats += event.amount;
            }
            ("utilization", "item_picture", "count") => {
                frame.backend.item_picture_cache_capacity_utilization += event.amount;
            }
            ("eviction", "subtree_snapshot", "count") => {
                frame.backend.subtree_snapshot_cache_evictions += event.amount;
            }
            ("repeat", "subtree_snapshot", "count") => {
                frame.backend.subtree_snapshot_cache_record_repeats += event.amount;
            }
            ("utilization", "subtree_snapshot", "count") => {
                frame.backend.subtree_snapshot_cache_capacity_utilization += event.amount;
            }
            ("eviction", "subtree_image", "count") => {
                frame.backend.subtree_image_cache_evictions += event.amount;
            }
            ("repeat", "subtree_image", "count") => {
                frame.backend.subtree_image_cache_record_repeats += event.amount;
            }
            ("utilization", "subtree_image", "count") => {
                frame.backend.subtree_image_cache_capacity_utilization += event.amount;
            }
            ("eviction", "image", "count") => {
                frame.backend.image_cache_evictions += event.amount;
            }
            ("repeat", "image", "count") => {
                frame.backend.image_cache_record_repeats += event.amount;
            }
            ("utilization", "image", "count") => {
                frame.backend.image_cache_capacity_utilization += event.amount;
            }
            _ => {}
        }
    }

    pub(crate) fn finish(self) -> RenderProfileSummary {
        RenderProfileSummary { frames: self.frames }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BackendSpanKey, CompletedProfileSpan, FrameProfile, ProfileCountEvent,
        RenderProfileAggregator,
    };

    #[test]
    fn nested_backend_spans_produce_expected_tree_metrics() {
        let mut aggregator = RenderProfileAggregator::default();

        aggregator.record_span(CompletedProfileSpan {
            frame: 12,
            target: "render.backend",
            name: "display_tree_direct_draw",
            parent: None,
            inclusive_ms: 80.0,
            exclusive_ms: 3.0,
        });
        aggregator.record_span(CompletedProfileSpan {
            frame: 12,
            target: "render.backend",
            name: "subtree_snapshot_record",
            parent: Some("display_tree_direct_draw"),
            inclusive_ms: 74.0,
            exclusive_ms: 71.0,
        });

        let summary = aggregator.finish();
        let frame = summary
            .frames
            .get(&12)
            .expect("frame summary should exist");
        let root = frame
            .backend_spans
            .get(&BackendSpanKey {
                depth: 0,
                parent: None,
                name: "display_tree_direct_draw",
            })
            .expect("root backend span should exist");
        let child = frame
            .backend_spans
            .get(&BackendSpanKey {
                depth: 1,
                parent: Some("display_tree_direct_draw"),
                name: "subtree_snapshot_record",
            })
            .expect("child backend span should exist");

        assert_eq!(root.inclusive_ms, 80.0);
        assert_eq!(root.exclusive_ms, 3.0);
        assert_eq!(child.inclusive_ms, 74.0);
        assert_eq!(child.exclusive_ms, 71.0);
    }

    #[test]
    fn cache_and_draw_events_accumulate_into_frame_profile() {
        let mut aggregator = RenderProfileAggregator::default();

        aggregator.record_count(ProfileCountEvent {
            frame: 3,
            target: "render.cache",
            kind: "cache",
            name: "subtree_snapshot",
            result: "hit",
            amount: 2,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 3,
            target: "render.draw",
            kind: "draw",
            name: "rect",
            result: "count",
            amount: 4,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 3,
            target: "render.cache",
            kind: "cache",
            name: "image",
            result: "miss",
            amount: 5,
        });
        aggregator.record_count(ProfileCountEvent {
            frame: 3,
            target: "render.cache",
            kind: "eviction",
            name: "text",
            result: "count",
            amount: 2,
        });

        let summary = aggregator.finish();
        let frame = summary.frames.get(&3).expect("frame summary should exist");

        assert_eq!(frame.backend.subtree_snapshot_cache_hits, 2);
        assert_eq!(frame.backend.draw_rect_count, 4);
        assert_eq!(frame.backend.image_cache_misses, 5);
        assert_eq!(frame.backend.text_cache_evictions, 2);
    }

    #[test]
    fn transition_active_frame_average_uses_counted_frames_only() {
        let mut aggregator = RenderProfileAggregator::default();

        let first = aggregator.frame_mut(0);
        first.light_leak_transition_ms = 8.0;
        first.light_leak_transition_frames = 1;
        let second = aggregator.frame_mut(1);
        second.light_leak_transition_ms = 4.0;
        second.light_leak_transition_frames = 1;

        let summary = aggregator.finish();
        assert_eq!(summary.average_light_leak_transition_ms(), 6.0);
    }

    #[test]
    fn pipeline_stage_spans_fill_stage_totals() {
        let mut aggregator = RenderProfileAggregator::default();

        aggregator.record_span(CompletedProfileSpan {
            frame: 5,
            target: "render.pipeline",
            name: "frame_state",
            parent: Some("frame"),
            inclusive_ms: 1.5,
            exclusive_ms: 1.5,
        });
        aggregator.record_span(CompletedProfileSpan {
            frame: 5,
            target: "render.scene",
            name: "resolve_ui_tree",
            parent: Some("build_scene_display_list"),
            inclusive_ms: 10.0,
            exclusive_ms: 10.0,
        });
        aggregator.record_span(CompletedProfileSpan {
            frame: 5,
            target: "render.scene",
            name: "compute_layout",
            parent: Some("build_scene_display_list"),
            inclusive_ms: 2.0,
            exclusive_ms: 2.0,
        });

        let summary = aggregator.finish();
        let frame = summary.frames.get(&5).expect("frame summary should exist");

        assert_eq!(frame.frame_state_ms, 1.5);
        assert_eq!(frame.resolve_ms, 10.0);
        assert_eq!(frame.layout_ms, 2.0);
    }
}
