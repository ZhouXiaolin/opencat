use std::{collections::HashSet, sync::Arc};

use anyhow::Result;

use crate::{
    frame_ctx::FrameCtx,
    runtime::session::RenderSession,
    scene::{
        composition::Composition,
        node::{Node, NodeKind},
        primitives::{AudioSource, ImageSource},
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
    let mut audio_sources = composition
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
        match frame_state_for_root(&root, &frame_ctx) {
            FrameState::Scene { scene, .. } => {
                collect_sources(&scene, &frame_ctx, &mut image_sources, &mut audio_sources);
            }
            FrameState::Transition { from, to, .. } => {
                collect_sources(&from, &frame_ctx, &mut image_sources, &mut audio_sources);
                collect_sources(&to, &frame_ctx, &mut image_sources, &mut audio_sources);
            }
        }
    }

    session.assets.preload_image_sources(image_sources)?;
    session.assets.preload_audio_sources(audio_sources)?;
    session.prepared_root_ptr = Some(root_ptr);
    Ok(())
}

pub(crate) fn collect_sources(
    node: &Node,
    frame_ctx: &FrameCtx,
    image_sources: &mut HashSet<ImageSource>,
    audio_sources: &mut HashSet<AudioSource>,
) {
    match node.kind() {
        NodeKind::Component(component) => {
            let rendered = component.render(frame_ctx);
            collect_sources(&rendered, frame_ctx, image_sources, audio_sources);
        }
        NodeKind::Div(div) => {
            for child in div.children_ref() {
                collect_sources(child, frame_ctx, image_sources, audio_sources);
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
        NodeKind::Audio(_) => {}
        NodeKind::Timeline(_) => match frame_state_for_root(node, frame_ctx) {
            FrameState::Scene { scene, .. } => {
                collect_sources(&scene, frame_ctx, image_sources, audio_sources);
            }
            FrameState::Transition { from, to, .. } => {
                collect_sources(&from, frame_ctx, image_sources, audio_sources);
                collect_sources(&to, frame_ctx, image_sources, audio_sources);
            }
        },
        NodeKind::Text(_) | NodeKind::Lucide(_) | NodeKind::Video(_) => {}
    }
}
