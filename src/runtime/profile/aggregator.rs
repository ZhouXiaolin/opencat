use std::collections::BTreeMap;

use super::{
    BackendProfile, BackendProfileReport, BackendSpanAggregate, BackendSpanKey, FrameProfile,
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
        match (event.target, event.kind, event.name, event.result) {
            ("render.cache", "cache", "subtree_snapshot", "hit") => {
                frame.backend.subtree_snapshot_cache_hits += event.amount;
            }
            ("render.cache", "cache", "subtree_snapshot", "miss") => {
                frame.backend.subtree_snapshot_cache_misses += event.amount;
            }
            ("render.draw", "draw", "rect", "count") => {
                frame.backend.draw_rect_count += event.amount;
            }
            ("render.draw", "draw", "text", "count") => {
                frame.backend.draw_text_count += event.amount;
            }
            ("render.layer", "layer", "save_layer", "count") => {
                frame.backend.save_layer_count += event.amount;
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

        let summary = aggregator.finish();
        let frame = summary.frames.get(&3).expect("frame summary should exist");

        assert_eq!(frame.backend.subtree_snapshot_cache_hits, 2);
        assert_eq!(frame.backend.draw_rect_count, 4);
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
}
