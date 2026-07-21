//! Native decoder seek and lane-selection policy.

use crate::media::VideoPreviewQuality;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecoderCursor {
    pub has_frame: bool,
    pub current_pts_secs: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecoderLaneSelection {
    Reuse(usize),
    OpenNew,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodeSlicePlan {
    pub timestamp_us: u64,
    pub is_key: bool,
}

pub fn seek_threshold_secs(quality: VideoPreviewQuality) -> f64 {
    match quality {
        VideoPreviewQuality::Scrubbing => 0.12,
        VideoPreviewQuality::Realtime => 0.35,
        VideoPreviewQuality::Exact => 1.5,
    }
}

pub fn seek_feed_margin_us(quality: VideoPreviewQuality) -> u64 {
    match quality {
        VideoPreviewQuality::Scrubbing => 120_000,
        VideoPreviewQuality::Realtime => 350_000,
        VideoPreviewQuality::Exact => 500_000,
    }
}

pub fn nearest_keyframe_before(keyframes: &[u64], target_us: u64) -> u64 {
    if keyframes.is_empty() {
        return target_us;
    }
    let index = keyframes.partition_point(|&us| us <= target_us.saturating_add(1));
    keyframes[index.saturating_sub(1)]
}

pub fn previous_keyframe_before(keyframes: &[u64], target_us: u64) -> Option<u64> {
    if keyframes.is_empty() {
        return None;
    }
    let index = keyframes.partition_point(|&us| us < target_us);
    index.checked_sub(1).map(|idx| keyframes[idx])
}

pub fn select_decoder_lane(
    cursors: &[DecoderCursor],
    target_secs: f64,
    quality: VideoPreviewQuality,
    max_lanes_per_asset: usize,
) -> DecoderLaneSelection {
    if cursors.is_empty() {
        return DecoderLaneSelection::OpenNew;
    }
    if let Some((index, _)) = cursors.iter().enumerate().find(|(_, cursor)| {
        cursor.has_frame && (cursor.current_pts_secs - target_secs).abs() < 1e-6
    }) {
        return DecoderLaneSelection::Reuse(index);
    }
    if let Some((index, _)) = cursors
        .iter()
        .enumerate()
        .find(|(_, cursor)| !cursor.has_frame)
    {
        return DecoderLaneSelection::Reuse(index);
    }

    let threshold_secs = seek_threshold_secs(quality);
    if let Some((index, _)) = cursors
        .iter()
        .enumerate()
        .filter(|(_, cursor)| {
            cursor.has_frame
                && target_secs + 1e-6 >= cursor.current_pts_secs
                && target_secs - cursor.current_pts_secs <= threshold_secs
        })
        .min_by(|(_, left), (_, right)| {
            (target_secs - left.current_pts_secs)
                .partial_cmp(&(target_secs - right.current_pts_secs))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    {
        return DecoderLaneSelection::Reuse(index);
    }
    if cursors.len() < max_lanes_per_asset.max(1) {
        return DecoderLaneSelection::OpenNew;
    }
    let index = cursors
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            (left.current_pts_secs - target_secs)
                .abs()
                .partial_cmp(&(right.current_pts_secs - target_secs).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(index, _)| index)
        .unwrap_or(0);
    DecoderLaneSelection::Reuse(index)
}

pub fn build_decode_slice_plan(
    keyframes: &[u64],
    target_us: u64,
    quality: VideoPreviewQuality,
) -> Vec<DecodeSlicePlan> {
    let keyframe = nearest_keyframe_before(keyframes, target_us);
    let mut plan = vec![DecodeSlicePlan {
        timestamp_us: keyframe,
        is_key: true,
    }];
    let feed_until = target_us.saturating_add(seek_feed_margin_us(quality));
    if feed_until > keyframe {
        plan.push(DecodeSlicePlan {
            timestamp_us: feed_until,
            is_key: false,
        });
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_quality_uses_wider_seek_window() {
        assert_eq!(seek_feed_margin_us(VideoPreviewQuality::Scrubbing), 120_000);
        assert_eq!(seek_feed_margin_us(VideoPreviewQuality::Exact), 500_000);
        assert!((seek_threshold_secs(VideoPreviewQuality::Exact) - 1.5).abs() < 1e-6);
    }

    #[test]
    fn lane_selection_reuses_nearby_cursor_or_opens_a_lane() {
        let lanes = [
            DecoderCursor {
                has_frame: true,
                current_pts_secs: 1.0,
            },
            DecoderCursor {
                has_frame: true,
                current_pts_secs: 7.0,
            },
        ];
        assert_eq!(
            select_decoder_lane(&lanes, 1.2, VideoPreviewQuality::Realtime, 2),
            DecoderLaneSelection::Reuse(0)
        );
        assert_eq!(
            select_decoder_lane(&lanes[..1], 6.0, VideoPreviewQuality::Exact, 2),
            DecoderLaneSelection::OpenNew
        );
    }

    #[test]
    fn decode_plan_starts_at_nearest_keyframe() {
        let plan = build_decode_slice_plan(
            &[0, 1_000_000, 2_000_000],
            1_400_000,
            VideoPreviewQuality::Realtime,
        );
        assert_eq!(plan[0].timestamp_us, 1_000_000);
        assert!(plan[0].is_key);
        assert_eq!(plan[1].timestamp_us, 1_750_000);
    }
}
