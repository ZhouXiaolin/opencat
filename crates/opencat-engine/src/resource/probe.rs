use crate::resource::media::MediaContext;
use crate::resource::path_store::AssetPathStore;
use anyhow::Result;
use opencat_core::resource::asset_id::AssetId;
use opencat_core::resource::catalog::{ResourceCatalog, VideoInfoMeta};
use opencat_core::resource::hash_map_catalog::HashMapResourceCatalog;
use std::path::Path;

pub fn probe_video(
    catalog: &mut HashMapResourceCatalog,
    path_store: &mut AssetPathStore,
    path: &Path,
    media: &mut MediaContext,
) -> Result<VideoInfoMeta> {
    let info = media.video_info(path)?;
    let meta = VideoInfoMeta {
        width: info.width,
        height: info.height,
        duration_secs: info.duration_secs,
    };
    let locator = path.to_string_lossy();
    catalog.register_video_dimensions(&locator, meta.width, meta.height, meta.duration_secs);
    let id = AssetId(locator.into_owned());
    path_store.insert(id, path.to_path_buf());
    Ok(meta)
}
