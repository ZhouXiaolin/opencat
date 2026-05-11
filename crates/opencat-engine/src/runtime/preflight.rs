use std::sync::Arc;

use anyhow::Result;

use crate::platform::EnginePlatform;
use opencat_core::runtime::preflight_collect::collect_resource_requests;
use opencat_core::scene::composition::Composition;
use opencat_core::scene::node::{Node, NodeKind};
use opencat_core::scene::primitives::ImageSource;
use opencat_core::scene::time::{FrameState, frame_state_for_root};
use opencat_core::resource::asset_id::AssetId;

pub(crate) fn ensure_assets_preloaded(
    composition: &Composition,
    session: &mut opencat_core::runtime::session::RenderSession<EnginePlatform>,
) -> Result<()> {
    let root_ptr = Arc::as_ptr(&composition.root) as *const () as usize;
    if session.platform.prepared_root_ptr == Some(root_ptr) {
        return Ok(());
    }

    let req = collect_resource_requests(composition);

    crate::resource::fetch::preload_image_sources(&mut session.platform.assets, req.image_sources)?;
    crate::resource::fetch::preload_audio_sources(&mut session.platform.assets, req.audio_sources)?;

    for path in req.video_paths {
        let _ = crate::resource::probe::probe_video(
            &mut session.platform.assets,
            &path,
            &mut session.platform.video,
        );
    }

    // Register canvas asset aliases in the engine's AssetCatalog so that canvas
    // rendering functions can resolve them by alias. The core pipeline's
    // HashMapResourceCatalog also registers these, but the Skia canvas functions
    // read from AssetCatalog through SkiaRenderData.
    for frame in 0..composition.frames.max(1) {
        let frame_ctx = opencat_core::frame_ctx::FrameCtx {
            frame,
            fps: composition.fps,
            width: composition.width,
            height: composition.height,
            frames: composition.frames,
        };
        let root = composition.root_node(&frame_ctx);
        let state = frame_state_for_root(&root, &frame_ctx);
        register_canvas_asset_aliases(&state, &frame_ctx, &mut session.platform.assets);
    }

    session.platform.prepared_root_ptr = Some(root_ptr);
    Ok(())
}

/// Walk the scene tree and register canvas asset aliases in the engine's AssetCatalog.
fn register_canvas_asset_aliases(
    state: &FrameState,
    frame_ctx: &opencat_core::frame_ctx::FrameCtx,
    assets: &mut crate::resource::asset_catalog::AssetCatalog,
) {
    match state {
        FrameState::Scene { scene, .. } => {
            register_canvas_aliases_from_node(scene, frame_ctx, assets);
        }
        FrameState::Transition { from, to, .. } => {
            register_canvas_aliases_from_node(from, frame_ctx, assets);
            register_canvas_aliases_from_node(to, frame_ctx, assets);
        }
    }
}

fn register_canvas_aliases_from_node(
    node: &Node,
    frame_ctx: &opencat_core::frame_ctx::FrameCtx,
    assets: &mut crate::resource::asset_catalog::AssetCatalog,
) {
    match node.kind() {
        NodeKind::Component(component) => {
            let rendered = component.render(frame_ctx);
            register_canvas_aliases_from_node(&rendered, frame_ctx, assets);
        }
        NodeKind::Div(div) => {
            for child in div.children_ref() {
                register_canvas_aliases_from_node(child, frame_ctx, assets);
            }
        }
        NodeKind::Canvas(canvas) => {
            for asset in canvas.assets_ref() {
                if let ImageSource::Path(ref path) = asset.source {
                    let target = assets.register(path);
                    let _ = assets.alias(AssetId(asset.asset_id.clone()), &target);
                }
            }
        }
        NodeKind::Timeline(timeline) => {
            for segment in timeline.segments() {
                match segment {
                    opencat_core::scene::time::TimelineSegment::Scene { scene, .. } => {
                        register_canvas_aliases_from_node(scene, frame_ctx, assets);
                    }
                    opencat_core::scene::time::TimelineSegment::Transition { from, to, .. } => {
                        register_canvas_aliases_from_node(from, frame_ctx, assets);
                        register_canvas_aliases_from_node(to, frame_ctx, assets);
                    }
                }
            }
        }
        NodeKind::Image(_)
        | NodeKind::Video(_)
        | NodeKind::Text(_)
        | NodeKind::Lucide(_)
        | NodeKind::Path(_)
        | NodeKind::Caption(_) => {}
    }
}
