use std::sync::Arc;

use anyhow::Result;

use opencat_core::runtime::preflight_collect::collect_resource_requests;
use crate::host::runtime::session::RenderSession;
use opencat_core::scene::composition::Composition;

pub(crate) fn ensure_assets_preloaded(
    composition: &Composition,
    session: &mut RenderSession,
) -> Result<()> {
    let root_ptr = Arc::as_ptr(&composition.root) as *const () as usize;
    if session.prepared_root_ptr == Some(root_ptr) {
        return Ok(());
    }

    let req = collect_resource_requests(composition);

    crate::host::resource::fetch::preload_image_sources(&mut session.assets, req.image_sources)?;
    crate::host::resource::fetch::preload_audio_sources(&mut session.assets, req.audio_sources)?;

    for path in req.video_paths {
        let _ = crate::host::resource::probe::probe_video(&mut session.assets, &path, &mut session.media_ctx);
    }

    session.prepared_root_ptr = Some(root_ptr);
    Ok(())
}
