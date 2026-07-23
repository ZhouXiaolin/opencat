//! Utility functions for asset management.

use std::path::{Path, PathBuf};

use opencat_core::ir::asset_id::{AssetId, ResourceKind, stable_hash};

/// 生成 cache 文件路径。统一 `.bin` 扩展名 —— 运行时不依赖扩展名，
/// 格式由文件内容（image/video container header）决定。
pub fn cache_file_path(cache_dir: &Path, id: &AssetId) -> PathBuf {
    cache_dir.join(format!("{:016x}.bin", stable_hash(&id.key)))
}

/// 为音频本地路径生成 `AssetId`。
pub fn asset_id_for_audio_path(path: &Path) -> AssetId {
    AssetId::new(
        ResourceKind::Audio,
        format!("audio:path:{}", path.to_string_lossy()),
    )
}
