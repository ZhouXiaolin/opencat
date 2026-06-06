use crate::frame_ctx::FrameCtx;
use crate::media::{VideoFrameRequest, VideoPreviewQuality};
use crate::parse::composition::Composition;
use crate::parse::node::{Node, NodeKind};
use crate::parse::primitives::{AudioSource, ImageSource, Video};
use crate::parse::time::{FrameState, frame_state_for_root};
use crate::probe::catalog::ResourceRequests;
use crate::resource::fonts::FontManifest;
use crate::resource::manifest::{ExternalResourceManifest, build_manifest};

pub fn collect_resource_requests(composition: &Composition) -> ResourceRequests {
    let mut req = ResourceRequests::default();
    req.audios
        .extend(composition.audio_sources().iter().map(|a| a.source.clone()));

    for frame in 0..composition.frames {
        let frame_ctx = FrameCtx {
            frame,
            fps: composition.fps,
            width: composition.width,
            height: composition.height,
            frames: composition.frames,
        };
        let root = composition.root_node(&frame_ctx);
        collect_sources_from_frame_state(
            &frame_state_for_root(&root, &frame_ctx),
            &frame_ctx,
            &mut req,
        );
    }
    req
}

/// Preflight: resource requests + unified external manifest (OpenCat + fonts + future Lottie).
pub fn collect_external_manifest(
    composition: &Composition,
    font_manifest: &FontManifest,
) -> (ResourceRequests, ExternalResourceManifest) {
    let req = collect_resource_requests(composition);
    let manifest = build_manifest(&req, font_manifest);
    (req, manifest)
}

pub fn collect_audio_plan(comp: &Composition) -> crate::probe::catalog::AudioPlan {
    use crate::ir::asset_id::{AssetId, asset_id_for_audio_url};
    use crate::parse::composition::AudioAttachment;
    use crate::probe::catalog::{AudioPlan, AudioSegment};

    let fps = comp.fps.max(1) as f64;
    let frame_to_ms = |frame: u32| ((frame as f64 / fps) * 1000.0).round() as u64;
    let duration_to_ms = |duration_secs: f64| (duration_secs * 1000.0).round().max(0.0) as u64;
    let total_ms = duration_to_ms(comp.duration);
    let mut segments = Vec::new();

    for s in comp.audio_sources() {
        let asset = match &s.source {
            AudioSource::Unset => continue,
            AudioSource::Url(u) => asset_id_for_audio_url(u),
            AudioSource::Path(p) => AssetId(format!("audio:path:{}", p.to_string_lossy())),
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
    use crate::parse::time::TimelineSegment;

    match node.kind() {
        NodeKind::Timeline(tl) => {
            if tl.style_ref().id == scene_id {
                return Some((0, tl.duration_in_frames(ctx)));
            }
            let mut cursor_frame = 0;
            for segment in tl.segments() {
                let duration_in_frames =
                    crate::frame_ctx::duration_secs_to_frames(segment.duration_secs(), ctx.fps);
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

pub(crate) fn collect_sources_from_frame_state(
    state: &FrameState,
    frame_ctx: &FrameCtx,
    req: &mut ResourceRequests,
) {
    match state {
        FrameState::Scene { scene, .. } => {
            collect_sources(scene, frame_ctx, req);
        }
        FrameState::Transition { from, to, .. } => {
            collect_sources(from, frame_ctx, req);
            collect_sources(to, frame_ctx, req);
        }
    }
}

pub(crate) fn collect_sources(node: &Node, frame_ctx: &FrameCtx, req: &mut ResourceRequests) {
    match node.kind() {
        NodeKind::Div(div) => {
            for child in div.children_ref() {
                collect_sources(child, frame_ctx, req);
            }
        }
        NodeKind::Canvas(canvas) => {
            for asset in canvas.assets_ref() {
                if !matches!(asset.source, ImageSource::Unset) {
                    req.images.insert(asset.source.clone());
                }
            }
            for child in canvas.hidden_children_ref() {
                collect_sources(child, frame_ctx, req);
            }
        }
        NodeKind::Image(image) => {
            if !matches!(image.source(), ImageSource::Unset) {
                req.images.insert(image.source().clone());
            }
        }
        NodeKind::Lottie(lottie) => {
            if !lottie_visible_at_frame(lottie, frame_ctx) {
                return;
            }
            if !matches!(
                lottie.source(),
                crate::parse::primitives::LottieSource::Unset
            ) {
                let id = lottie.style_ref().id.clone();
                if !id.is_empty() {
                    req.lotties.insert(crate::probe::catalog::LottieRequest {
                        element_id: id,
                        source: lottie.source().clone(),
                    });
                }
            }
        }
        NodeKind::Video(video) => {
            if !video_visible_at_frame(video, frame_ctx) {
                return;
            }
            req.videos.insert(video.source().clone());
            for child in video.children_ref() {
                collect_sources(child, frame_ctx, req);
            }
        }
        NodeKind::Timeline(_) => {
            collect_sources_from_frame_state(&frame_state_for_root(node, frame_ctx), frame_ctx, req)
        }
        NodeKind::Text(_) | NodeKind::Lucide(_) | NodeKind::Path(_) => {}
        NodeKind::Caption(caption) => {
            req.subtitles.insert(caption.source().clone());
        }
    }
}

fn lottie_visible_at_frame(
    lottie: &crate::parse::primitives::Lottie,
    frame_ctx: &FrameCtx,
) -> bool {
    VideoFrameRequest {
        composition_time_secs: frame_ctx.frame as f64 / frame_ctx.fps.max(1) as f64,
        timing: lottie.timing(),
        quality: VideoPreviewQuality::Exact,
        target_size: None,
    }
    .is_visible()
}

fn video_visible_at_frame(video: &Video, frame_ctx: &FrameCtx) -> bool {
    VideoFrameRequest {
        composition_time_secs: frame_ctx.frame as f64 / frame_ctx.fps.max(1) as f64,
        timing: video.timing(),
        quality: VideoPreviewQuality::Exact,
        target_size: None,
    }
    .is_visible()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::parse::{
        composition::{AudioAttachment, Composition, CompositionAudioSource},
        primitives::{AudioSource, VideoSource, div, image, video, video_url},
        transition::{fade, timeline},
    };

    #[test]
    fn collects_image_audio_video_distinctly() {
        let root_node = div()
            .id("r")
            .child(image().id("i").url("https://example.com/a.png"))
            .child(video("/t.mp4").id("v"));

        let root = Arc::new(move |_ctx: &FrameCtx| root_node.clone().into());
        let comp = Composition::new("test")
            .size(100, 100)
            .fps(30)
            .duration(5.0 / 30.0)
            .root(move |ctx| root(ctx))
            .build()
            .unwrap();

        let req = collect_resource_requests(&comp);
        assert_eq!(req.images.len(), 1);
        assert_eq!(req.videos.len(), 1);
    }

    #[test]
    fn collects_video_url_separately() {
        let root_node = div()
            .id("r")
            .child(video_url("https://example.com/v.mp4").id("v"));

        let root = Arc::new(move |_ctx: &FrameCtx| root_node.clone().into());
        let comp = Composition::new("test")
            .size(100, 100)
            .fps(30)
            .duration(5.0 / 30.0)
            .root(move |ctx| root(ctx))
            .build()
            .unwrap();

        let req = collect_resource_requests(&comp);
        assert_eq!(req.videos.len(), 1);
        assert!(
            req.videos
                .contains(&VideoSource::Url("https://example.com/v.mp4".to_string()))
        );
    }

    #[test]
    fn scene_audio_uses_correct_offset() {
        let root_node: Node = timeline()
            .sequence(10.0 / 30.0, div().id("scene-a").into())
            .transition(fade().timing(crate::parse::easing::Easing::Linear, 5.0 / 30.0))
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
            .transition(fade().timing(crate::parse::easing::Easing::Linear, 5.0 / 30.0))
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
