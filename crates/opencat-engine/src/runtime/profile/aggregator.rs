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
    #[allow(dead_code)]
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
            _ => {}
        }
    }

    pub(crate) fn finish(self) -> RenderProfileSummary {
        RenderProfileSummary {
            frames: self.frames,
        }
    }
}