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

    pub fn record_count(&mut self, event: ProfileCountEvent) {
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
                frame.backend.glyph_path_cache_capacity_utilization += event.amount;
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
            ("layout", "reused_nodes", "count") => {
                frame.reused_nodes += event.amount;
            }
            ("layout", "merkle_skipped_subtrees", "count") => {
                frame.merkle_skipped_subtrees += event.amount;
            }
            ("layout", "layout_dirty", "count") => {
                frame.layout_dirty_nodes += event.amount;
            }
            ("layout", "raster_dirty", "count") => {
                frame.raster_dirty_nodes += event.amount;
            }
            ("layout", "composite_dirty", "count") => {
                frame.composite_dirty_nodes += event.amount;
            }
            ("layout", "structure_rebuild", "count") => {
                frame.structure_rebuilds += event.amount;
            }
            ("consecutive", "subtree_snapshot", "count") => {
                frame.backend.subtree_snapshot_consecutive_hits_total += event.amount;
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
    fn count_events_record_merkle_skipped_subtrees() {
        let mut aggregator = RenderProfileAggregator::default();

        aggregator.record_count(ProfileCountEvent {
            frame: 7,
            kind: "layout",
            name: "merkle_skipped_subtrees",
            result: "count",
            amount: 3,
        });

        let summary = aggregator.finish();
        assert_eq!(summary.frames[&7].merkle_skipped_subtrees, 3);
    }
}
