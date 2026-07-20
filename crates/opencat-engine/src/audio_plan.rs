//! Engine-owned audio schedule derivation.
//!
//! `AudioPlan` / `AudioSegment` previously lived in core's `probe::catalog`
//! and were carried on `CompositionInfo.audio_plan`. Per issue #2 / #11, audio
//! execution (decode / mix / preview / export) is a host responsibility, so
//! the schedule host needs is now derived by the host itself, straight from
//! the parsed composition. This module is that derivation for the engine.
//!
//! It is a pure function of `Composition` — the same inputs produce the same
//! segments regardless of pipeline state.

use opencat_core::frame_ctx::{FrameCtx, duration_secs_to_frames};
use opencat_core::ir::asset_id::{AssetId, asset_id_for_audio};
use opencat_core::parse::composition::{AudioAttachment, Composition};
use opencat_core::parse::node::{Node, NodeKind};
use opencat_core::parse::time::TimelineSegment;

#[derive(Default, Clone, Debug)]
pub struct AudioPlan {
    pub segments: Vec<AudioSegment>,
}

#[derive(Clone, Debug)]
pub struct AudioSegment {
    pub asset: AssetId,
    pub start_ms: u64,
    pub end_ms: u64,
}

/// Derive the engine audio schedule from a composition: one segment per
/// attached audio source, with start/end milliseconds resolved against the
/// scene/timeline it is attached to.
pub fn collect_audio_plan(comp: &Composition) -> AudioPlan {
    let fps = comp.fps.max(1) as f64;
    let frame_to_ms = |frame: u32| ((frame as f64 / fps) * 1000.0).round() as u64;
    let duration_to_ms = |duration_secs: f64| (duration_secs * 1000.0).round().max(0.0) as u64;
    let total_ms = duration_to_ms(comp.duration);
    let mut segments = Vec::new();

    for s in comp.audio_sources() {
        let asset = match asset_id_for_audio(&s.source) {
            Some(id) => id,
            None => continue,
        };
        let (start_ms, end_ms) = match &s.attach {
            AudioAttachment::Timeline => (0, total_ms),
            AudioAttachment::Scene { scene_id } => {
                let timing = find_scene_timing(comp, scene_id);
                match timing {
                    Some((start_frame, scene_duration)) => {
                        let start_ms = frame_to_ms(start_frame);
                        let end_ms = s
                            .duration_secs
                            .map(|duration| start_ms + duration_to_ms(duration))
                            .unwrap_or_else(|| {
                                frame_to_ms(start_frame.saturating_add(scene_duration))
                            });
                        (start_ms, end_ms)
                    }
                    None => {
                        let dur_ms = s.duration_secs.map(duration_to_ms).unwrap_or(total_ms);
                        (0, dur_ms)
                    }
                }
            }
        };
        segments.push(AudioSegment {
            asset,
            start_ms,
            end_ms,
        });
    }

    AudioPlan { segments }
}

fn find_scene_timing(comp: &Composition, scene_id: &str) -> Option<(u32, u32)> {
    let probe_ctx = FrameCtx {
        frame: 0,
        fps: comp.fps,
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
            let mut cursor_frame = 0;
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
    use opencat_core::parse::composition::CompositionAudioSource;
    use opencat_core::parse::easing::Easing;
    use opencat_core::parse::primitives::{AudioSource, div};
    use opencat_core::parse::transition::{fade, timeline};
    use std::sync::Arc;

    #[test]
    fn scene_audio_uses_correct_offset() {
        let root_node: Node = timeline()
            .sequence(10.0 / 30.0, div().id("scene-a").into())
            .transition(fade().timing(Easing::Linear, 5.0 / 30.0))
            .sequence(20.0 / 30.0, div().id("scene-b").into())
            .into();

        let root = Arc::new(move |_ctx: &FrameCtx| root_node.clone());
        let comp = Composition::new("test")
            .size(100, 100)
            .fps(30)
            .duration(35.0 / 30.0)
            .root(move |ctx| root(ctx))
            .audio_sources(vec![
                CompositionAudioSource::scene(
                    "audio-a",
                    AudioSource::Url("a.mp3".into()),
                    "scene-a",
                ),
                CompositionAudioSource::scene(
                    "audio-b",
                    AudioSource::Url("b.mp3".into()),
                    "scene-b",
                ),
            ])
            .build()
            .unwrap();

        let plan = collect_audio_plan(&comp);
        assert_eq!(plan.segments.len(), 2);

        // scene-a starts at frame 0, duration 10 frames at 30fps => 0ms to 333ms
        assert_eq!(plan.segments[0].start_ms, 0);
        assert_eq!(plan.segments[0].end_ms, 333);

        // scene-b starts at frame 15 (10 scene + 5 transition), duration 20 frames => 500ms to 1167ms
        assert_eq!(plan.segments[1].start_ms, 500);
        assert_eq!(plan.segments[1].end_ms, 1167);
    }

    #[test]
    fn timeline_audio_uses_full_duration() {
        let root_node: Node = timeline()
            .sequence(10.0 / 30.0, div().id("scene-a").into())
            .transition(fade().timing(Easing::Linear, 5.0 / 30.0))
            .sequence(20.0 / 30.0, div().id("scene-b").into())
            .into();

        let root = Arc::new(move |_ctx: &FrameCtx| root_node.clone());
        let comp = Composition::new("test")
            .size(100, 100)
            .fps(30)
            .duration(35.0 / 30.0)
            .root(move |ctx| root(ctx))
            .audio_sources(vec![CompositionAudioSource {
                id: "bgm".into(),
                source: AudioSource::Url("bgm.mp3".into()),
                attach: AudioAttachment::Timeline,
                duration_secs: None,
            }])
            .build()
            .unwrap();

        let plan = collect_audio_plan(&comp);
        assert_eq!(plan.segments.len(), 1);
        assert_eq!(plan.segments[0].start_ms, 0);
        assert_eq!(plan.segments[0].end_ms, 1167);
    }
}
