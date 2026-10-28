//! **Canonical composition-level audio schedule** derived by core.
//!
//! [`AudioPlan`] is the **sole** canonical audio output at the composition
//! level. Core derives it purely from the parsed composition tree
//! (timeline/scene/transition offsets, explicit duration trims) via
//! [`collect_audio_plan`]. The result lives on
//! [`CompositionInfo::audio_plan`](crate::ir::CompositionInfo::audio_plan)
//! for the lifetime of the pipeline.
//!
//! **Hosts (Engine / Web) must not re-walk the composition tree** to produce a
//! second set of audio offsets. They decode, mix, preview, and export using
//! the canonical `AudioPlan` only. Core reads no bytes, opens no files, and
//! performs no audio decode — every conversion uses typed microsecond
//! arithmetic from the composition's frame count and frame rate.

use crate::frame_ctx::FrameCtx;
use crate::ir::asset_id::{AssetId, asset_id_for_audio};
use crate::parse::composition::{AudioAttachment, Composition};
use crate::parse::node::{Node, NodeKind};
use crate::parse::time::TimelineSegment;
use crate::time::{
    DurationMicros, DurationRange, FrameIndex, RationalFrameRate, TimestampMicros,
    duration_secs_to_frames, frames_to_timestamp_micros, secs_to_micros,
};

/// Whole-composition audio schedule: one segment per attached source.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct AudioPlan {
    pub segments: Vec<AudioSegment>,
}

/// One scheduled audio clip on the composition timeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioSegment {
    /// Canonical audio [`AssetId`] (same identity as resource requirements).
    pub asset: AssetId,
    /// Half-open composition range `[start, start + duration)`.
    pub range: DurationRange,
}

impl AudioSegment {
    pub fn start_micros(&self) -> TimestampMicros {
        self.range.start
    }

    pub fn end_micros(&self) -> TimestampMicros {
        self.range.end().unwrap_or(self.range.start)
    }

    pub fn duration_micros(&self) -> DurationMicros {
        self.range.duration.unwrap_or(DurationMicros::ZERO)
    }
}

/// Derive the audio schedule from a composition: timeline attachment spans the
/// full composition duration; scene attachment uses timeline segment offsets
/// (including intervening transitions); optional explicit `duration_secs` trims
/// the segment from its start.
///
/// Pure function of composition structure and typed time conversion — no host
/// audio metadata is required (missing probe fields must not block the plan).
pub fn collect_audio_plan(comp: &Composition) -> AudioPlan {
    let fps = comp.fps.max(1);
    let rate = RationalFrameRate::integer(fps);
    let total_range = composition_duration_range(comp, rate);
    let mut segments = Vec::new();

    for source in comp.audio_sources() {
        let Some(asset) = asset_id_for_audio(&source.source) else {
            continue;
        };

        let range = match &source.attach {
            AudioAttachment::Timeline => {
                apply_explicit_duration(TimestampMicros::ZERO, source.duration_secs, total_range)
            }
            AudioAttachment::Scene { scene_id } => match find_scene_timing(comp, scene_id) {
                Some((start_frame, scene_frames)) => {
                    let start = frames_to_timestamp_micros(FrameIndex(start_frame), rate);
                    let scene_end = frames_to_timestamp_micros(
                        FrameIndex(start_frame.saturating_add(scene_frames)),
                        rate,
                    );
                    let scene_range = DurationRange::with_duration(
                        start,
                        DurationMicros(scene_end.0.saturating_sub(start.0)),
                    );
                    apply_explicit_duration(start, source.duration_secs, scene_range)
                }
                None => {
                    // Unknown scene id: fall back to composition start with
                    // explicit duration or full composition length.
                    apply_explicit_duration(
                        TimestampMicros::ZERO,
                        source.duration_secs,
                        total_range,
                    )
                }
            },
        };

        segments.push(AudioSegment { asset, range });
    }

    AudioPlan { segments }
}

fn composition_duration_range(comp: &Composition, rate: RationalFrameRate) -> DurationRange {
    // Prefer frame-count → micros so the plan aligns with visual frame timing;
    // fall back to authored seconds when frames is zero.
    if comp.frames > 0 {
        let end = frames_to_timestamp_micros(FrameIndex(comp.frames), rate);
        DurationRange::with_duration(TimestampMicros::ZERO, DurationMicros(end.0))
    } else {
        DurationRange::with_duration(
            TimestampMicros::ZERO,
            DurationMicros(secs_to_micros(comp.duration.max(0.0))),
        )
    }
}

fn apply_explicit_duration(
    start: TimestampMicros,
    duration_secs: Option<f64>,
    fallback: DurationRange,
) -> DurationRange {
    match duration_secs {
        Some(secs) if secs.is_finite() && secs > 0.0 => {
            DurationRange::with_duration(start, DurationMicros(secs_to_micros(secs)))
        }
        _ => {
            // Keep fallback start if it already begins at `start`; otherwise
            // re-anchor duration from `start` using fallback's end when known.
            if fallback.start == start {
                fallback
            } else if let Some(end) = fallback.end() {
                DurationRange::with_duration(start, DurationMicros(end.0.saturating_sub(start.0)))
            } else {
                DurationRange::from_start(start)
            }
        }
    }
}

fn find_scene_timing(comp: &Composition, scene_id: &str) -> Option<(u32, u32)> {
    let probe_ctx = FrameCtx {
        frame: 0,
        fps: comp.fps.max(1),
        width: comp.width,
        height: comp.height,
        frames: comp.frames,
    };
    let root = comp.root_node(&probe_ctx);
    find_scene_timing_in_node(&root, scene_id, &probe_ctx)
}

fn find_scene_timing_in_node(node: &Node, scene_id: &str, ctx: &FrameCtx) -> Option<(u32, u32)> {
    match node.kind() {
        NodeKind::Timeline(tl) => {
            if tl.style_ref().id == scene_id {
                return Some((0, tl.duration_in_frames(ctx)));
            }
            // Boundaries are the ceiling of the running seconds cursor, so a
            // scene's frame window matches where the renderer places it.
            let bounds = tl.segment_frame_bounds(ctx);
            for (index, segment) in tl.segments().iter().enumerate() {
                if let TimelineSegment::Scene { scene, .. } = segment {
                    if scene.style_ref().id == scene_id {
                        let start = bounds[index];
                        return Some((start, bounds[index + 1].saturating_sub(start)));
                    }
                }
            }
            None
        }
        NodeKind::Div(div) => {
            for child in div.children_ref() {
                if let Some(result) = find_scene_timing_in_node(child, scene_id, ctx) {
                    return Some(result);
                }
            }
            None
        }
        NodeKind::Video(video) => {
            for child in video.children_ref() {
                if let Some(result) = find_scene_timing_in_node(child, scene_id, ctx) {
                    return Some(result);
                }
            }
            None
        }
        _ => None,
    }
}


