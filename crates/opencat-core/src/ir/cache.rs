use super::draw_frame::DrawOpFrame;
use super::draw_op::DrawOp;
use super::draw_types::*;
use crate::cache::lru::BoundedLruCache;
use crate::canvas::paint::PaintSpec;
use crate::display::list::DisplayRect;
use crate::display::tree::DisplayRecordedSubtreeFingerprint;

/// IR-based render cache. Stores DrawOp IR segments instead of backend-specific
/// objects, making cache data portable across platforms.
pub struct RenderCache {
    /// Subtree snapshot entries keyed by fingerprint primary hash.
    pub subtree_snapshots: BoundedLruCache<u64, CachedSubtreeIr>,
    /// Cached IR segments keyed by segment_key.
    pub segments: BoundedLruCache<u64, CachedDrawSegment>,
    /// Item-level cached ranges keyed by item_paint_fingerprint.
    pub item_ranges: BoundedLruCache<u64, CachedDrawRange>,
    /// Most-recent scene-level snapshot. Reused on the next frame when
    /// `SceneRenderPlan::allows_scene_snapshot_cache` is true and the
    /// composition viewport is unchanged.
    pub last_scene_snapshot: Option<SceneSnapshotEntry>,
}

/// A cached whole-frame DrawOp recording paired with the viewport metadata
/// and root subtree fingerprint required to validate reuse. The fingerprint
/// captures every change in the draw program — paint, composite, structure,
/// and per-frame item content such as transition progress — so a cached
/// entry is only reusable when the entire scene tree fingerprints identically.
#[derive(Clone, Debug)]
pub struct SceneSnapshotEntry {
    pub frame: DrawOpFrame,
    pub width: i32,
    pub height: i32,
    pub root_fingerprint: DisplayRecordedSubtreeFingerprint,
}

impl RenderCache {
    /// Create a new RenderCache with the given capacities.
    pub fn new(subtree_snapshot_cap: usize, segment_cap: usize, item_range_cap: usize) -> Self {
        Self {
            subtree_snapshots: BoundedLruCache::new(subtree_snapshot_cap),
            segments: BoundedLruCache::new(segment_cap),
            item_ranges: BoundedLruCache::new(item_range_cap),
            last_scene_snapshot: None,
        }
    }
}

/// Subtree cache entry. Core tracks eligibility and hit count;
/// platform executors compile the segment into native objects.
#[derive(Clone, Debug)]
pub struct CachedSubtreeIr {
    pub segment_key: u64,
    pub consecutive_hits: usize,
    pub recorded_bounds: DisplayRect,
}

/// DrawOp range metadata for executor compilation.
#[derive(Clone, Debug)]
pub struct CachedDrawRange {
    pub segment_range: DrawOpRange,
    pub fingerprint: u64,
    pub bounds: DisplayRect,
    pub segment_key: u64,
}

/// A cached IR segment — the pure IR data for a subtree.
/// On cache hit, imported into current DrawOpBuilder via import_segment().
#[derive(Clone, Debug, Default)]
pub struct CachedDrawSegment {
    pub ops: Vec<DrawOp>,
    pub paints: Vec<PaintSpec>,
    pub paths: Vec<EncodedPath>,
    pub children: Vec<RuntimeEffectChildRef>,
    pub strings: Vec<String>,
    pub bytes: Vec<u8>,
    pub byte_ranges: Vec<TableRange>,
    pub f32_pool: Vec<f32>,
    pub resources: Vec<ResourceRef>,
    pub effects: Vec<EffectRef>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::list::DisplayRect;
    use crate::ir::draw_op::DrawOp;

    #[test]
    fn cached_draw_segment_holds_ir_data() {
        let segment = CachedDrawSegment {
            ops: vec![DrawOp::Save, DrawOp::Restore],
            paints: Vec::new(),
            paths: Vec::new(),
            children: Vec::new(),
            strings: Vec::new(),
            bytes: Vec::new(),
            byte_ranges: Vec::new(),
            f32_pool: Vec::new(),
            resources: Vec::new(),
            effects: Vec::new(),
        };
        assert_eq!(segment.ops.len(), 2);
    }

    #[test]
    fn cached_subtree_ir_tracks_hits() {
        let entry = CachedSubtreeIr {
            segment_key: 42,
            consecutive_hits: 0,
            recorded_bounds: DisplayRect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
        };
        assert_eq!(entry.segment_key, 42);
        assert_eq!(entry.consecutive_hits, 0);
    }

    #[test]
    fn render_cache_can_insert_and_lookup() {
        use crate::cache::lru::BoundedLruCache;

        let mut cache = RenderCache {
            subtree_snapshots: BoundedLruCache::new(16),
            segments: BoundedLruCache::new(16),
            item_ranges: BoundedLruCache::new(64),
            last_scene_snapshot: None,
        };

        let segment = CachedDrawSegment::default();
        let entry = CachedSubtreeIr {
            segment_key: 1,
            consecutive_hits: 0,
            recorded_bounds: DisplayRect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
        };

        cache.segments.insert(1, segment);
        cache.subtree_snapshots.insert(1, entry);

        assert!(cache.segments.get_cloned(&1).is_some());
        assert!(cache.subtree_snapshots.get_cloned(&1).is_some());
    }
}
