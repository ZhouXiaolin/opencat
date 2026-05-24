use std::path::{Path, PathBuf};
use std::sync::Arc;

use opencat_core::cache::lru::BoundedLruCache;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VideoFrameKey {
    path: PathBuf,
    pts_quantized: u64,
    target_size: Option<(u32, u32)>,
}

pub struct VideoFrameCache {
    entries: BoundedLruCache<VideoFrameKey, Arc<Vec<u8>>>,
}

impl VideoFrameCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: BoundedLruCache::new(capacity),
        }
    }

    pub fn get(
        &mut self,
        path: &Path,
        time_secs: f64,
        target_size: Option<(u32, u32)>,
    ) -> Option<Arc<Vec<u8>>> {
        self.entries.get_cloned(&VideoFrameKey::new(
            path,
            quantize_pts(time_secs),
            target_size,
        ))
    }

    pub fn insert(
        &mut self,
        path: &Path,
        time_secs: f64,
        target_size: Option<(u32, u32)>,
        frame: Arc<Vec<u8>>,
    ) {
        self.entries.insert(
            VideoFrameKey::new(path, quantize_pts(time_secs), target_size),
            frame,
        );
    }
}

impl VideoFrameKey {
    fn new(path: &Path, pts_quantized: u64, target_size: Option<(u32, u32)>) -> Self {
        Self {
            path: path.to_path_buf(),
            pts_quantized,
            target_size,
        }
    }
}

pub fn quantize_pts(time_secs: f64) -> u64 {
    (time_secs.max(0.0) * 10_000.0).round() as u64
}
