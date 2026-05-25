use crate::frame_ctx::FrameCtx;
use crate::parse::composition::Composition;
use crate::parse::node::{Node, NodeKind};
use crate::parse::primitives::{AudioSource, ImageSource, SubtitleSource, VideoSource};
use crate::parse::time::{FrameState, frame_state_for_root};
use crate::probe::catalog::ResourceRequests;

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

pub fn collect_audio_plan(comp: &Composition) -> crate::probe::catalog::AudioPlan {
    use crate::ir::asset_id::{AssetId, asset_id_for_audio_url};
    use crate::probe::catalog::{AudioPlan, AudioSegment};

    let fps = comp.fps.max(1) as u64;
    let ms_per_frame = 1000 / fps;
    let total_ms = (comp.frames as u64) * ms_per_frame;
    let mut segments = Vec::new();

    for s in comp.audio_sources() {
        let asset = match &s.source {
            AudioSource::Unset => continue,
            AudioSource::Url(u) => asset_id_for_audio_url(u),
            AudioSource::Path(p) => AssetId(format!("audio:path:{}", p.to_string_lossy())),
        };
        let (start_ms, end_ms) = match &s.attach {
            crate::parse::composition::AudioAttachment::Timeline => (0, total_ms),
            crate::parse::composition::AudioAttachment::Scene { .. } => {
                let dur_ms = s
                    .duration
                    .map(|d| d as u64 * ms_per_frame)
                    .unwrap_or(total_ms);
                (0, dur_ms)
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
        NodeKind::Component(component) => {
            let rendered = component.render(frame_ctx);
            collect_sources(&rendered, frame_ctx, req);
        }
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
        NodeKind::Video(video) => {
            req.videos.insert(video.source().clone());
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::parse::{
        composition::Composition,
        primitives::{div, image, video, video_url},
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
            .frames(5)
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
            .frames(5)
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
}
