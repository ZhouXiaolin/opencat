use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::host::runtime::cache::lru::BoundedLruCache;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct VideoFrameKey {
    path: PathBuf,
    pts_quantized: u64,
    target_size: Option<(u32, u32)>,
}

pub(crate) struct VideoFrameCache {
    entries: BoundedLruCache<VideoFrameKey, Arc<Vec<u8>>>,
}

impl VideoFrameCache {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            entries: BoundedLruCache::new(capacity),
        }
    }

    pub(crate) fn get(
        &mut self,
        path: &Path,
        time_secs: f64,
        target_size: Option<(u32, u32)>,
    ) -> Option<Arc<Vec<u8>>> {
        self.entries
            .get_cloned(&VideoFrameKey::new(path, quantize_pts(time_secs), target_size))
    }

    pub(crate) fn insert(
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

/// 将连续时间量化为 1/10000 秒精度的离散 tick。
///
/// 用于:
/// - `VideoFrameCache` 的 key 量化(本文件)
/// - `item_paint_fingerprint` 对 Video Bitmap 的 fingerprint 量化(`fingerprint/mod.rs`)
///
/// 两处**必须**共用同一函数,避免 fingerprint 与解码缓存对"同一帧"的判定错位。
pub(crate) fn quantize_pts(time_secs: f64) -> u64 {
    (time_secs.max(0.0) * 10_000.0).round() as u64
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use super::VideoFrameCache;

    #[test]
    fn reuses_same_quantized_video_frame() {
        let mut cache = VideoFrameCache::new(4);
        let path = Path::new("/tmp/demo.mp4");
        let frame = Arc::new(vec![1, 2, 3, 4]);

        cache.insert(path, 1.234_560_1, None, frame.clone());

        assert_eq!(
            cache.get(path, 1.234_560_2, None).as_deref(),
            Some(frame.as_ref())
        );
    }

    #[test]
    fn evicts_oldest_video_frame_when_capacity_is_exceeded() {
        let mut cache = VideoFrameCache::new(2);
        let path = Path::new("/tmp/demo.mp4");

        cache.insert(path, 0.0, None, Arc::new(vec![0]));
        cache.insert(path, 1.0, None, Arc::new(vec![1]));
        cache.insert(path, 2.0, None, Arc::new(vec![2]));

        assert!(cache.get(path, 0.0, None).is_none());
        assert_eq!(
            cache
                .get(path, 1.0, None)
                .as_deref()
                .map(|bytes| bytes.as_slice()),
            Some(&[1][..])
        );
        assert_eq!(
            cache
                .get(path, 2.0, None)
                .as_deref()
                .map(|bytes| bytes.as_slice()),
            Some(&[2][..])
        );
    }
}
