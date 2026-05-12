//! Cache hit/miss decision functions shared by all backends.

use crate::display::list::{DisplayItem, DisplayRect};
use crate::runtime::fingerprint::SubtreeSnapshotFingerprint;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubtreeSnapshotResolution {
    Hit,
    Miss,
    CollisionRejected,
}

pub fn resolve_subtree_snapshot_lookup(
    query_fingerprint: SubtreeSnapshotFingerprint,
    cached_secondary: Option<u64>,
) -> SubtreeSnapshotResolution {
    match cached_secondary {
        None => SubtreeSnapshotResolution::Miss,
        Some(secondary) if secondary == query_fingerprint.secondary => {
            SubtreeSnapshotResolution::Hit
        }
        Some(_) => SubtreeSnapshotResolution::CollisionRejected,
    }
}

pub const SUBTREE_IMAGE_PROMOTION_HITS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CachedSubtreeRenderMode {
    DrawImage,
    DrawPicture,
    PromoteToImage,
}

pub fn resolve_cached_subtree_render_mode(
    has_cached_image: bool,
    consecutive_hits: usize,
    recorded_bounds: DisplayRect,
    current_bounds: DisplayRect,
    has_non_unit_scale: bool,
) -> CachedSubtreeRenderMode {
    if has_cached_image && !has_non_unit_scale {
        return CachedSubtreeRenderMode::DrawImage;
    }
    if should_promote_snapshot_to_image(
        consecutive_hits,
        recorded_bounds,
        current_bounds,
        has_non_unit_scale,
    ) {
        CachedSubtreeRenderMode::PromoteToImage
    } else {
        CachedSubtreeRenderMode::DrawPicture
    }
}

pub fn should_promote_snapshot_to_image(
    consecutive_hits: usize,
    recorded_bounds: DisplayRect,
    current_bounds: DisplayRect,
    has_non_unit_scale: bool,
) -> bool {
    consecutive_hits >= SUBTREE_IMAGE_PROMOTION_HITS
        && !has_non_unit_scale
        && recorded_bounds.x.to_bits() == current_bounds.x.to_bits()
        && recorded_bounds.y.to_bits() == current_bounds.y.to_bits()
        && recorded_bounds.width.to_bits() == current_bounds.width.to_bits()
        && recorded_bounds.height.to_bits() == current_bounds.height.to_bits()
}

pub fn should_cache_item_picture(item: &DisplayItem) -> bool {
    matches!(
        item,
        DisplayItem::Bitmap(_) | DisplayItem::DrawScript(_) | DisplayItem::SvgPath(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(width: f32, height: f32) -> DisplayRect {
        DisplayRect {
            x: 0.0,
            y: 0.0,
            width,
            height,
        }
    }

    #[test]
    fn snapshot_lookup_hit_when_secondary_matches() {
        let fp = SubtreeSnapshotFingerprint {
            primary: 1,
            secondary: 100,
        };
        assert_eq!(
            resolve_subtree_snapshot_lookup(fp, Some(100)),
            SubtreeSnapshotResolution::Hit,
        );
    }

    #[test]
    fn snapshot_lookup_collision_when_secondary_differs() {
        let fp = SubtreeSnapshotFingerprint {
            primary: 1,
            secondary: 100,
        };
        assert_eq!(
            resolve_subtree_snapshot_lookup(fp, Some(999)),
            SubtreeSnapshotResolution::CollisionRejected,
        );
    }

    #[test]
    fn snapshot_lookup_miss_when_nothing_cached() {
        let fp = SubtreeSnapshotFingerprint {
            primary: 1,
            secondary: 100,
        };
        assert_eq!(
            resolve_subtree_snapshot_lookup(fp, None),
            SubtreeSnapshotResolution::Miss,
        );
    }

    #[test]
    fn promotion_requires_threshold_and_matching_bounds() {
        let recorded = bounds(320.0, 180.0);
        assert!(!should_promote_snapshot_to_image(
            SUBTREE_IMAGE_PROMOTION_HITS - 1,
            recorded,
            recorded,
            false,
        ));
        assert!(should_promote_snapshot_to_image(
            SUBTREE_IMAGE_PROMOTION_HITS,
            recorded,
            recorded,
            false,
        ));
    }

    #[test]
    fn promotion_rejects_scale_and_bounds_changes() {
        let recorded = bounds(320.0, 180.0);
        let resized = bounds(640.0, 360.0);
        assert!(!should_promote_snapshot_to_image(
            SUBTREE_IMAGE_PROMOTION_HITS,
            recorded,
            resized,
            false,
        ));
        assert!(!should_promote_snapshot_to_image(
            SUBTREE_IMAGE_PROMOTION_HITS,
            recorded,
            recorded,
            true,
        ));
    }

    #[test]
    fn render_mode_draw_image_wins_when_no_scale() {
        let recorded = bounds(320.0, 180.0);
        assert_eq!(
            resolve_cached_subtree_render_mode(true, 0, recorded, recorded, false),
            CachedSubtreeRenderMode::DrawImage,
        );
    }

    #[test]
    fn render_mode_promotes_once_threshold_reached() {
        let recorded = bounds(320.0, 180.0);
        assert_eq!(
            resolve_cached_subtree_render_mode(
                false,
                SUBTREE_IMAGE_PROMOTION_HITS,
                recorded,
                recorded,
                false,
            ),
            CachedSubtreeRenderMode::PromoteToImage,
        );
    }

    #[test]
    fn render_mode_scale_forces_picture_fallback() {
        let recorded = bounds(320.0, 180.0);
        assert_eq!(
            resolve_cached_subtree_render_mode(true, 99, recorded, recorded, true),
            CachedSubtreeRenderMode::DrawPicture,
        );
    }
}
