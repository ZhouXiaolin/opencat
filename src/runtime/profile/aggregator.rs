use std::collections::BTreeMap;

use super::{BackendSpanKey, FrameProfile};

#[derive(Clone, Copy, Debug)]
pub(crate) struct CompletedProfileSpan {
    pub frame: u32,
    pub target: &'static str,
    pub name: &'static str,
    pub parent: Option<&'static str>,
    pub inclusive_ms: f64,
    pub exclusive_ms: f64,
    /// render.backend span tree 内的深度；非 backend span 为 None。
    /// 0 表示该 backend span 没有 render.backend 祖先（frame / transition 祖先不算）。
    pub backend_depth: Option<usize>,
    /// 仅对 render.transition::draw_transition span 有意义；其他 span 为 None。
    /// 取值与 canvas.rs 中 transition_kind 字段一致："slide" | "light_leak" | "gltransition" | "other"。
    pub transition_kind: Option<&'static str>,
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
            // 所有 root backend span（没有 render.backend 祖先）累加进 backend_ms。
            // 嵌套 backend span 的 inclusive 已经被其 root 覆盖，不重复计入。
            if depth == 0 {
                frame.backend_ms += span.inclusive_ms;
            }
            match span.name {
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
            ("cache", "subtree_snapshot_composite_dirty", "hit") => {
                frame.backend.subtree_snapshot_composite_dirty_hits += event.amount;
            }
            ("cache", "subtree_snapshot_composite_dirty", "miss") => {
                frame.backend.subtree_snapshot_composite_dirty_misses += event.amount;
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
        RenderProfileSummary {
            frames: self.frames,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BackendSpanKey, CompletedProfileSpan, ProfileCountEvent, RenderProfileAggregator};

    #[test]
    fn backend_ms_sums_all_root_backend_spans() {
        // 契约：backend_ms 应该是 *所有* backend_depth == 0 的 render.backend span 的
        // inclusive_ms 之和，不再局限于 display_tree_direct_draw / display_tree_snapshot_record
        // 两个硬编码 name。嵌套 backend span 不重复计入（inclusive 已经被 root 包含）。
        let mut aggregator = RenderProfileAggregator::default();
        let frame_id = 1;

        let roots = [
            ("display_tree_direct_draw", 10.0_f64, 2.0_f64),
            ("scene_snapshot_present", 5.0, 5.0),
            ("light_leak_mask", 3.0, 3.0),
        ];
        for (name, inclusive, exclusive) in roots {
            aggregator.record_span(CompletedProfileSpan {
                frame: frame_id,
                target: "render.backend",
                name,
                parent: None,
                inclusive_ms: inclusive,
                exclusive_ms: exclusive,
                backend_depth: Some(0),
                transition_kind: None,
            });
        }

        aggregator.record_span(CompletedProfileSpan {
            frame: frame_id,
            target: "render.backend",
            name: "subtree_snapshot_record",
            parent: Some("display_tree_direct_draw"),
            inclusive_ms: 4.0,
            exclusive_ms: 4.0,
            backend_depth: Some(1),
            transition_kind: None,
        });

        let summary = aggregator.finish();
        let frame = summary.frames.get(&frame_id).expect("frame must exist");
        assert!(
            (frame.backend_ms - 18.0).abs() < 1e-9,
            "backend_ms = 10 + 5 + 3 = 18, got {}",
            frame.backend_ms
        );
    }

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
            backend_depth: Some(0),
            transition_kind: None,
        });
        aggregator.record_span(CompletedProfileSpan {
            frame: 12,
            target: "render.backend",
            name: "subtree_snapshot_record",
            parent: Some("display_tree_direct_draw"),
            inclusive_ms: 74.0,
            exclusive_ms: 71.0,
            backend_depth: Some(1),
            transition_kind: None,
        });

        let summary = aggregator.finish();
        let frame = summary.frames.get(&12).expect("frame summary should exist");
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
            backend_depth: None,
            transition_kind: None,
        });
        aggregator.record_span(CompletedProfileSpan {
            frame: 5,
            target: "render.scene",
            name: "resolve_ui_tree",
            parent: Some("build_scene_display_list"),
            inclusive_ms: 10.0,
            exclusive_ms: 10.0,
            backend_depth: None,
            transition_kind: None,
        });
        aggregator.record_span(CompletedProfileSpan {
            frame: 5,
            target: "render.scene",
            name: "compute_layout",
            parent: Some("build_scene_display_list"),
            inclusive_ms: 2.0,
            exclusive_ms: 2.0,
            backend_depth: None,
            transition_kind: None,
        });

        let summary = aggregator.finish();
        let frame = summary.frames.get(&5).expect("frame summary should exist");

        assert_eq!(frame.frame_state_ms, 1.5);
        assert_eq!(frame.resolve_ms, 10.0);
        assert_eq!(frame.layout_ms, 2.0);
    }

    #[test]
    fn transition_span_writes_per_kind_breakdown() {
        // 契约：render.transition::draw_transition span 必须按 transition_kind 拆分到
        // slide_transition_* / light_leak_transition_* 字段；同时 transition_ms 总和不变。
        // 这是修复"transition avg ms/active-frame: ... 0 frames"bug 的核心写入路径。
        let mut aggregator = RenderProfileAggregator::default();

        aggregator.record_span(CompletedProfileSpan {
            frame: 100,
            target: "render.transition",
            name: "draw_transition",
            parent: Some("frame"),
            inclusive_ms: 12.5,
            exclusive_ms: 12.5,
            backend_depth: None,
            transition_kind: Some("slide"),
        });
        aggregator.record_span(CompletedProfileSpan {
            frame: 101,
            target: "render.transition",
            name: "draw_transition",
            parent: Some("frame"),
            inclusive_ms: 30.0,
            exclusive_ms: 30.0,
            backend_depth: None,
            transition_kind: Some("light_leak"),
        });
        aggregator.record_span(CompletedProfileSpan {
            frame: 101,
            target: "render.transition",
            name: "draw_transition",
            parent: Some("frame"),
            inclusive_ms: 20.0,
            exclusive_ms: 20.0,
            backend_depth: None,
            transition_kind: Some("light_leak"),
        });
        // "other" / fade / 未知 kind 不拆分，但仍计入总 transition_ms。
        aggregator.record_span(CompletedProfileSpan {
            frame: 102,
            target: "render.transition",
            name: "draw_transition",
            parent: Some("frame"),
            inclusive_ms: 5.0,
            exclusive_ms: 5.0,
            backend_depth: None,
            transition_kind: Some("other"),
        });

        let summary = aggregator.finish();

        let slide_frame = summary.frames.get(&100).expect("slide frame exists");
        assert_eq!(slide_frame.slide_transition_ms, 12.5);
        assert_eq!(slide_frame.slide_transition_frames, 1);
        assert_eq!(slide_frame.light_leak_transition_ms, 0.0);
        assert_eq!(slide_frame.light_leak_transition_frames, 0);
        assert_eq!(slide_frame.transition_ms, 12.5);

        let leak_frame = summary.frames.get(&101).expect("light_leak frame exists");
        assert_eq!(leak_frame.light_leak_transition_ms, 50.0);
        assert_eq!(leak_frame.light_leak_transition_frames, 2);
        assert_eq!(leak_frame.slide_transition_frames, 0);
        assert_eq!(leak_frame.transition_ms, 50.0);

        let other_frame = summary.frames.get(&102).expect("other frame exists");
        assert_eq!(other_frame.transition_ms, 5.0);
        assert_eq!(other_frame.slide_transition_frames, 0);
        assert_eq!(other_frame.light_leak_transition_frames, 0);

        // average_light_leak_transition_ms 应基于真实计数：50.0 / 2 = 25.0
        assert!((summary.average_light_leak_transition_ms() - 25.0).abs() < 1e-9);
    }
}
