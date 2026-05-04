//! src/host/resource/probe.rs
use std::path::Path;
use anyhow::Result;
use crate::core::resource::{AssetCatalog, catalog::VideoInfoMeta};
use crate::host::resource::media::MediaContext;

pub fn probe_video(catalog: &mut AssetCatalog, path: &Path, media: &mut MediaContext) -> Result<VideoInfoMeta> {
    let info = media.video_info(path)?;
    let meta = VideoInfoMeta { width: info.width, height: info.height, duration_secs: info.duration_secs };
    catalog.register_video_info(path, meta);
    Ok(meta)
}
