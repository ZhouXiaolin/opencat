use std::collections::HashSet;
use std::path::PathBuf;

use crate::core::frame_ctx::FrameCtx;
use crate::core::scene::composition::Composition;
use crate::core::scene::node::{Node, NodeKind};
use crate::core::scene::primitives::{AudioSource, ImageSource};
use crate::core::scene::time::{FrameState, frame_state_for_root};

#[derive(Default, Debug)]
pub struct ResourceRequests {
    pub image_sources: HashSet<ImageSource>,
    pub audio_sources: HashSet<AudioSource>,
    pub video_paths: HashSet<PathBuf>,
}

pub fn collect_resource_requests(composition: &Composition) -> ResourceRequests {
    let mut req = ResourceRequests::default();
    req.audio_sources.extend(
        composition
            .audio_sources()
            .iter()
            .map(|a| a.source.clone()),
    );

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

pub(crate) fn collect_sources(
    node: &Node,
    frame_ctx: &FrameCtx,
    req: &mut ResourceRequests,
) {
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
                    req.image_sources.insert(asset.source.clone());
                }
            }
        }
        NodeKind::Image(image) => {
            if !matches!(image.source(), ImageSource::Unset) {
                req.image_sources.insert(image.source().clone());
            }
        }
        NodeKind::Video(video) => {
            req.video_paths.insert(video.source().to_path_buf());
        }
        NodeKind::Timeline(_) => collect_sources_from_frame_state(
            &frame_state_for_root(node, frame_ctx),
            frame_ctx,
            req,
        ),
        NodeKind::Text(_)
        | NodeKind::Lucide(_)
        | NodeKind::Path(_)
        | NodeKind::Caption(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::core::scene::{composition::Composition, primitives::{div, image, video}};

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
        assert_eq!(req.image_sources.len(), 1);
        assert_eq!(req.video_paths.len(), 1);
    }
}
