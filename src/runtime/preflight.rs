use std::{collections::HashSet, sync::Arc};

use anyhow::Result;

use crate::{
    frame_ctx::FrameCtx,
    runtime::session::RenderSession,
    scene::{
        composition::Composition,
        node::{Node, NodeKind},
        primitives::ImageSource,
        time::{FrameState, frame_state_for_root},
    },
};

pub(crate) fn ensure_assets_preloaded(
    composition: &Composition,
    session: &mut RenderSession,
) -> Result<()> {
    let root_ptr = Arc::as_ptr(&composition.root) as *const () as usize;
    if session.prepared_root_ptr == Some(root_ptr) {
        return Ok(());
    }

    let mut image_sources = HashSet::new();
    let audio_sources = composition
        .audio_sources()
        .iter()
        .map(|audio| audio.source.clone())
        .collect::<HashSet<_>>();
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
            &mut image_sources,
        );
    }

    session.assets.preload_image_sources(image_sources)?;
    session.assets.preload_audio_sources(audio_sources)?;
    session.prepared_root_ptr = Some(root_ptr);
    Ok(())
}

fn collect_sources_from_frame_state(
    frame_state: &FrameState,
    frame_ctx: &FrameCtx,
    image_sources: &mut HashSet<ImageSource>,
) {
    match frame_state {
        FrameState::Scene { scene, .. } => {
            collect_sources(scene, frame_ctx, image_sources);
        }
        FrameState::Transition { from, to, .. } => {
            collect_sources(from, frame_ctx, image_sources);
            collect_sources(to, frame_ctx, image_sources);
        }
    }
}

pub(crate) fn collect_sources(
    node: &Node,
    frame_ctx: &FrameCtx,
    image_sources: &mut HashSet<ImageSource>,
) {
    match node.kind() {
        NodeKind::Component(component) => {
            let rendered = component.render(frame_ctx);
            collect_sources(&rendered, frame_ctx, image_sources);
        }
        NodeKind::Div(div) => {
            for child in div.children_ref() {
                collect_sources(child, frame_ctx, image_sources);
            }
        }
        NodeKind::Canvas(canvas) => {
            for asset in canvas.assets_ref() {
                if !matches!(asset.source, ImageSource::Unset) {
                    image_sources.insert(asset.source.clone());
                }
            }
        }
        NodeKind::Image(image) => {
            if !matches!(image.source(), ImageSource::Unset) {
                image_sources.insert(image.source().clone());
            }
        }
        NodeKind::Timeline(_) => collect_sources_from_frame_state(
            &frame_state_for_root(node, frame_ctx),
            frame_ctx,
            image_sources,
        ),
        NodeKind::Text(_) | NodeKind::Lucide(_) | NodeKind::Video(_) | NodeKind::Caption(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::{
        frame_ctx::FrameCtx,
        scene::{
            primitives::{div, image},
            transition::timeline,
        },
    };

    use super::collect_sources;

    #[test]
    fn collect_sources_walks_div_children_without_layer_nodes() {
        let root = div()
            .id("root")
            .child(image().id("hero").url("https://example.com/a.png"))
            .child(timeline().sequence(10, div().id("scene-a").into()));
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 10,
        };
        let mut image_sources = HashSet::new();

        collect_sources(&root.into(), &frame_ctx, &mut image_sources);

        assert_eq!(image_sources.len(), 1);
    }
}
