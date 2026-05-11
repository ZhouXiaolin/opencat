//! Utility functions for asset management.

use std::fs;
use std::path::{Path, PathBuf};

use opencat_core::resource::asset_id::{AssetId, stable_hash};

/// Read image dimensions from a file path.
pub fn read_image_dimensions(path: &Path) -> (u32, u32) {
    let Ok(bytes) = fs::read(path) else {
        return (0, 0);
    };
    let Ok(image) = image::load_from_memory(&bytes) else {
        return (0, 0);
    };
    (image.width(), image.height())
}

/// Generate cache file path for an asset.
pub fn cache_file_path(cache_dir: &Path, id: &AssetId, extension: &str) -> PathBuf {
    cache_dir.join(format!("{:016x}.{extension}", stable_hash(&id.0)))
}

/// Generate asset ID for an audio file path.
pub fn asset_id_for_audio_path(path: &Path) -> AssetId {
    AssetId(format!("audio:path:{}", path.to_string_lossy()))
}
