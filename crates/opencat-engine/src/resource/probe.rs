//! src/host/resource/probe.rs
use crate::resource::asset_catalog::AssetCatalog;
use crate::resource::media::MediaContext;
use anyhow::Result;
use opencat_core::resource::catalog::VideoInfoMeta;
use std::path::Path;

pub fn probe_video(
    catalog: &mut AssetCatalog,
    path: &Path,
    media: &mut MediaContext,
) -> Result<VideoInfoMeta> {
    let info = media.video_info(path)?;
    let meta = VideoInfoMeta {
        width: info.width,
        height: info.height,
        duration_secs: info.duration_secs,
    };
    catalog.register_video_info(path, meta);
    Ok(meta)
}
