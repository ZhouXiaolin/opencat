use crate::frame_ctx::FrameCtx;
use crate::media::VideoFrameRequest;
use crate::parse::composition::Composition;
use crate::parse::document::ParsedComposition;
use crate::parse::node::{Node, NodeKind};
use crate::parse::primitives::{ImageSource, Video};
use crate::parse::time::{FrameState, TimelineSegment, frame_state_for_root};
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

/// Collect [`ResourceRequests`] directly from a [`ParsedComposition`].
///
/// This is the canonical, host-facing entry point: it walks the *static* node
/// tree once and declares every referenced source. It does **not** iterate
/// composition frames, does **not** evaluate script-driven root closures, and
/// does **not** filter by per-frame visibility -- a declared asset is collected
/// regardless of when (or whether) it is on screen. Collection order is not
/// part of the protocol: callers may fetch/cache the resulting set in any
/// order.
pub fn collect_resource_requests_from_parsed(parsed: &ParsedComposition) -> ResourceRequests {
    let mut req = ResourceRequests::default();
    req.audios
        .extend(parsed.audio_sources.iter().map(|a| a.source.clone()));
    collect_sources_static(&parsed.root, &mut req);
    req
}

/// Static counterpart of [`collect_sources`]: walks a node tree collecting
/// declared sources without a [`FrameCtx`]. Timeline segments are fully
/// expanded (every scene / transition endpoint is visited), so sources that
/// only appear on a single frame are still declared.
fn collect_sources_static(node: &Node, req: &mut ResourceRequests) {
    match node.kind() {
        NodeKind::Div(div) => {
            for child in div.children_ref() {
                collect_sources_static(child, req);
            }
        }
        NodeKind::Canvas(canvas) => {
            for asset in canvas.assets_ref() {
                if !matches!(asset.source, ImageSource::Unset) {
                    req.images.insert(asset.source.clone());
                }
            }
            for child in canvas.hidden_children_ref() {
                collect_sources_static(child, req);
            }
        }
        NodeKind::Image(image) => {
            if !matches!(image.source(), ImageSource::Unset) {
                req.images.insert(image.source().clone());
            }
        }
        NodeKind::Lottie(lottie) => {
            if !matches!(
                lottie.source(),
                crate::parse::primitives::LottieSource::Unset
            ) {
                req.lotties.insert(crate::probe::catalog::LottieRequest {
                    source: lottie.source().clone(),
                });
            }
        }
        NodeKind::Video(video) => {
            req.videos.insert(video.source().clone());
            for child in video.children_ref() {
                collect_sources_static(child, req);
            }
        }
        NodeKind::Timeline(timeline) => {
            for segment in timeline.segments() {
                match segment {
                    TimelineSegment::Scene { scene, .. } => {
                        collect_sources_static(scene, req);
                    }
                    TimelineSegment::Transition { from, to, .. } => {
                        collect_sources_static(from, req);
                        collect_sources_static(to, req);
                    }
                }
            }
        }
        NodeKind::Text(_) | NodeKind::Path(_) | NodeKind::Svg(_) => {}
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
                req.lotties.insert(crate::probe::catalog::LottieRequest {
                    source: lottie.source().clone(),
                });
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
        NodeKind::Text(_) | NodeKind::Path(_) | NodeKind::Svg(_) => {}
    }
}

fn lottie_visible_at_frame(
    lottie: &crate::parse::primitives::Lottie,
    frame_ctx: &FrameCtx,
) -> bool {
    VideoFrameRequest {
        composition_time_secs: frame_ctx.frame as f64 / frame_ctx.fps.max(1) as f64,
        timing: lottie.timing(),
    }
    .is_visible()
}

fn video_visible_at_frame(video: &Video, frame_ctx: &FrameCtx) -> bool {
    VideoFrameRequest {
        composition_time_secs: frame_ctx.frame as f64 / frame_ctx.fps.max(1) as f64,
        timing: video.timing(),
    }
    .is_visible()
}


