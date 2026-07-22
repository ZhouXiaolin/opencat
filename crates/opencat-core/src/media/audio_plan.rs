//! Composition-level audio schedule derived by core.
//!
//! Hosts decode, mix, preview, and export; they must not re-walk the
//! composition tree to invent timeline / scene / transition offsets.

use crate::frame_ctx::FrameCtx;
use crate::ir::asset_id::{asset_id_for_audio, AssetId};
use crate::parse::composition::{AudioAttachment, Composition};
use crate::parse::node::{Node, NodeKind};
use crate::parse::time::TimelineSegment;
use crate::time::{
    duration_secs_to_frames, frames_to_timestamp_micros, secs_to_micros, DurationMicros,
    DurationRange, FrameIndex, RationalFrameRate, TimestampMicros,
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
        self.range
            .end()
            .unwrap_or(self.range.start)
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
            AudioAttachment::Timeline => apply_explicit_duration(
                TimestampMicros::ZERO,
                source.duration_secs,
                total_range,
            ),
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
                DurationRange::with_duration(
                    start,
                    DurationMicros(end.0.saturating_sub(start.0)),
                )
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
            let mut cursor_frame = 0u32;
            for segment in tl.segments() {
                let duration_in_frames = duration_secs_to_frames(segment.duration_secs(), ctx.fps);
                if let TimelineSegment::Scene { scene, .. } = segment {
                    if scene.style_ref().id == scene_id {
                        return Some((cursor_frame, duration_in_frames));
                    }
                }
                cursor_frame = cursor_frame.saturating_add(duration_in_frames);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::composition::CompositionAudioSource;
    use crate::parse::easing::Easing;
    use crate::parse::primitives::{div, AudioSource};
    use crate::parse::transition::{fade, timeline};
    use std::sync::Arc;

    fn two_scene_comp(audio: Vec<CompositionAudioSource>) -> Composition {
        let root_node: Node = timeline()
            .sequence(10.0 / 30.0, div().id("scene-a").into())
            .transition(fade().timing(Easing::Linear, 5.0 / 30.0))
            .sequence(20.0 / 30.0, div().id("scene-b").into())
            .into();

        let root = Arc::new(move |_ctx: &FrameCtx| root_node.clone());
        Composition::new("test")
            .size(100, 100)
            .fps(30)
            .duration(35.0 / 30.0)
            .root(move |ctx| root(ctx))
            .audio_sources(audio)
            .build()
            .unwrap()
    }

    #[test]
    fn scene_audio_includes_transition_offset() {
        let comp = two_scene_comp(vec![
            CompositionAudioSource::scene("audio-a", AudioSource::Url("a.mp3".into()), "scene-a"),
            CompositionAudioSource::scene("audio-b", AudioSource::Url("b.mp3".into()), "scene-b"),
        ]);

        let plan = collect_audio_plan(&comp);
        assert_eq!(plan.segments.len(), 2);

        // scene-a: frames 0..10 @ 30fps → 0 .. 333_333 µs
        assert_eq!(plan.segments[0].asset.key, "audio:url:a.mp3");
        assert_eq!(plan.segments[0].start_micros().0, 0);
        assert_eq!(plan.segments[0].end_micros().0, 333_333);

        // scene-b starts after 10 + 5 transition frames → 500_000 .. 1_166_667 µs
        assert_eq!(plan.segments[1].asset.key, "audio:url:b.mp3");
        assert_eq!(plan.segments[1].start_micros().0, 500_000);
        assert_eq!(plan.segments[1].end_micros().0, 1_166_667);
    }

    #[test]
    fn timeline_audio_uses_full_composition_duration() {
        let comp = two_scene_comp(vec![CompositionAudioSource {
            id: "bgm".into(),
            source: AudioSource::Url("bgm.mp3".into()),
            attach: AudioAttachment::Timeline,
            duration_secs: None,
        }]);

        let plan = collect_audio_plan(&comp);
        assert_eq!(plan.segments.len(), 1);
        assert_eq!(plan.segments[0].start_micros().0, 0);
        assert_eq!(plan.segments[0].end_micros().0, 1_166_667);
    }

    #[test]
    fn explicit_duration_trims_scene_audio() {
        let mut sources = vec![CompositionAudioSource::scene(
            "audio-a",
            AudioSource::Url("a.mp3".into()),
            "scene-a",
        )];
        sources[0].duration_secs = Some(0.1); // 100ms trim inside 333ms scene

        let comp = two_scene_comp(sources);
        let plan = collect_audio_plan(&comp);
        assert_eq!(plan.segments.len(), 1);
        assert_eq!(plan.segments[0].start_micros().0, 0);
        assert_eq!(plan.segments[0].end_micros().0, 100_000);
        assert_eq!(plan.segments[0].duration_micros().0, 100_000);
    }

    #[test]
    fn missing_scene_falls_back_to_composition_start() {
        let comp = two_scene_comp(vec![CompositionAudioSource::scene(
            "orphan",
            AudioSource::Url("x.mp3".into()),
            "no-such-scene",
        )]);
        let plan = collect_audio_plan(&comp);
        assert_eq!(plan.segments.len(), 1);
        assert_eq!(plan.segments[0].start_micros().0, 0);
        assert_eq!(plan.segments[0].end_micros().0, 1_166_667);
    }

    #[test]
    fn unset_audio_source_is_skipped() {
        let comp = two_scene_comp(vec![CompositionAudioSource::timeline(
            "missing",
            AudioSource::Unset,
        )]);
        let plan = collect_audio_plan(&comp);
        assert!(plan.segments.is_empty());
    }
}
