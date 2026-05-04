use std::sync::Arc;

use anyhow::Result;

use crate::resource::catalog::VideoInfoMeta;
use crate::runtime::{preflight_collect::collect_resource_requests, session::RenderSession};
use crate::scene::composition::Composition;

pub(crate) fn ensure_assets_preloaded(
    composition: &Composition,
    session: &mut RenderSession,
) -> Result<()> {
    let root_ptr = Arc::as_ptr(&composition.root) as *const () as usize;
    if session.prepared_root_ptr == Some(root_ptr) {
        return Ok(());
    }

    let req = collect_resource_requests(composition);

    crate::resource::assets::preload_image_sources(&mut session.assets, req.image_sources)?;
    crate::resource::assets::preload_audio_sources(&mut session.assets, req.audio_sources)?;

    for path in req.video_paths {
        if let Ok(info) = session.media_ctx.video_info(&path) {
            session
                .assets
                .register_video_info(&path, VideoInfoMeta::from(&info));
        }
    }

    session.prepared_root_ptr = Some(root_ptr);
    Ok(())
}
