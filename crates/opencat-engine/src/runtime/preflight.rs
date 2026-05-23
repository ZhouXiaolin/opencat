use crate::render::RenderSession;
use anyhow::Result;
use opencat_core::parse::composition::Composition;
use std::sync::Arc;

pub fn ensure_assets_preloaded(
    composition: &Composition,
    session: &mut RenderSession,
) -> Result<()> {
    let root_ptr = Arc::as_ptr(&composition.root) as *const () as usize;
    if session.core.prepared_root_ptr == Some(root_ptr) {
        return Ok(());
    }
    session
        .platform
        .preflight(composition, &mut session.core.catalog)?;
    session.core.prepared_root_ptr = Some(root_ptr);
    Ok(())
}
