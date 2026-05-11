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
    if session.prepared_root_ptr == Some(root_ptr) {
        return Ok(());
    }

    let req = collect_resource_requests(composition);

    crate::resource::fetch::preload_image_sources(
        &mut session.catalog,
        &mut session.platform.asset_paths,
        req.image_sources,
    )?;
    crate::resource::fetch::preload_audio_sources(
        &mut session.catalog,
        &mut session.platform.asset_paths,
        req.audio_sources,
    )?;

    for path in req.video_paths {
        let _ = crate::resource::probe::probe_video(
            &mut session.catalog,
            &mut session.platform.asset_paths,
            &path,
            &mut session.platform.video,
        );
    }

    session.platform.video.set_composition_fps(composition.fps as u32);

    // Register canvas asset aliases in both catalogs so that canvas
    // rendering functions can resolve them by alias.
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
        register_canvas_asset_aliases(&state, &frame_ctx, &mut session.catalog, &mut session.platform.asset_paths);
    }

    session.prepared_root_ptr = Some(root_ptr);
    Ok(())
}

/// Walk the scene tree and register canvas asset aliases in both catalogs.
fn register_canvas_asset_aliases(
    state: &FrameState,
    frame_ctx: &opencat_core::frame_ctx::FrameCtx,
    catalog: &mut opencat_core::resource::hash_map_catalog::HashMapResourceCatalog,
    path_store: &mut crate::resource::path_store::AssetPathStore,
) {
    match state {
        FrameState::Scene { scene, .. } => {
            register_canvas_aliases_from_node(scene, frame_ctx, catalog, path_store);
        }
        FrameState::Transition { from, to, .. } => {
            register_canvas_aliases_from_node(from, frame_ctx, catalog, path_store);
            register_canvas_aliases_from_node(to, frame_ctx, catalog, path_store);
        }
    }
}

fn register_canvas_aliases_from_node(
    node: &Node,
    frame_ctx: &opencat_core::frame_ctx::FrameCtx,
    catalog: &mut opencat_core::resource::hash_map_catalog::HashMapResourceCatalog,
    path_store: &mut crate::resource::path_store::AssetPathStore,
) {
    use opencat_core::resource::catalog::ResourceCatalog;
    match node.kind() {
        NodeKind::Component(component) => {
            let rendered = component.render(frame_ctx);
            register_canvas_aliases_from_node(&rendered, frame_ctx, catalog, path_store);
        }
        NodeKind::Div(div) => {
            for child in div.children_ref() {
                register_canvas_aliases_from_node(child, frame_ctx, catalog, path_store);
            }
        }
        NodeKind::Canvas(canvas) => {
            for asset in canvas.assets_ref() {
                if let ImageSource::Path(ref path) = asset.source {
                    let target = catalog.resolve_image(&ImageSource::Path(path.clone())).unwrap();
                    let alias_id = AssetId(asset.asset_id.clone());
                    let _ = catalog.alias(alias_id.clone(), &target);
                    if let Some(_target_path) = path_store.path(&target) {
                        let _ = path_store.alias(alias_id, &target);
                    }
                }
            }
        }
        NodeKind::Timeline(timeline) => {
            for segment in timeline.segments() {
                match segment {
                    opencat_core::scene::time::TimelineSegment::Scene { scene, .. } => {
                        register_canvas_aliases_from_node(scene, frame_ctx, catalog, path_store);
                    }
                    opencat_core::scene::time::TimelineSegment::Transition { from, to, .. } => {
                        register_canvas_aliases_from_node(from, frame_ctx, catalog, path_store);
                        register_canvas_aliases_from_node(to, frame_ctx, catalog, path_store);
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
