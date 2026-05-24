//! Backend-neutral video seek and decode planning strategy.

use crate::media::types::VideoPreviewQuality;

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

    let (index, _) = cursors
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            (left.current_pts_secs - target_secs)
                .abs()
                .partial_cmp(&(right.current_pts_secs - target_secs).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .expect("non-empty cursor list should produce a nearest lane");
    DecoderLaneSelection::Reuse(index)
}

pub fn decode_slice_end_index(
    chunks: &[DecodeSlicePlan],
    start_idx: usize,
    target_us: u64,
    margin_us: u64,
    lookahead_chunks: usize,
) -> Option<usize> {
    if chunks.is_empty() {
        return None;
    }

    let start = start_idx.min(chunks.len() - 1);
    let stop_pts_us = target_us.saturating_add(margin_us);
    let mut end_idx = start;
    let mut max_seen_pts_us = 0;
    let mut covered_at_idx = None;

    for (i, chunk) in chunks.iter().enumerate().skip(start) {
        end_idx = i;
        max_seen_pts_us = max_seen_pts_us.max(chunk.timestamp_us);
        if i > start && max_seen_pts_us >= stop_pts_us && covered_at_idx.is_none() {
            covered_at_idx = Some(i);
        }
        if let Some(covered) = covered_at_idx
            && i.saturating_sub(covered) >= lookahead_chunks
        {
            break;
        }
    }

    Some(end_idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_seek_strategy_is_backend_neutral() {
        let keyframes = [0, 500_000, 1_200_000, 2_400_000];

        assert_eq!(nearest_keyframe_before(&keyframes, 100_000), 0);
        assert_eq!(nearest_keyframe_before(&keyframes, 1_800_000), 1_200_000);
        assert_eq!(
            previous_keyframe_before(&keyframes, 1_200_000),
            Some(500_000)
        );
        assert_eq!(previous_keyframe_before(&keyframes, 0), None);

        assert_eq!(seek_feed_margin_us(VideoPreviewQuality::Scrubbing), 120_000);
        assert_eq!(seek_feed_margin_us(VideoPreviewQuality::Realtime), 350_000);
        assert_eq!(seek_feed_margin_us(VideoPreviewQuality::Exact), 500_000);
        assert!((seek_threshold_secs(VideoPreviewQuality::Exact) - 1.5).abs() < 1e-6);
    }

    #[test]
    fn media_decoder_lane_selection_matches_preview_strategy() {
        let lanes = [
            DecoderCursor {
                has_frame: true,
                current_pts_secs: 1.0,
            },
            DecoderCursor {
                has_frame: true,
                current_pts_secs: 8.0,
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
        assert_eq!(
            select_decoder_lane(&lanes, 6.9, VideoPreviewQuality::Exact, 2),
            DecoderLaneSelection::Reuse(1)
        );
    }

    #[test]
    fn media_decode_slice_plans_decode_order_covering_target_margin() {
        let chunks = [
            DecodeSlicePlan {
                timestamp_us: 0,
                is_key: true,
            },
            DecodeSlicePlan {
                timestamp_us: 900_000,
                is_key: false,
            },
            DecodeSlicePlan {
                timestamp_us: 300_000,
                is_key: false,
            },
            DecodeSlicePlan {
                timestamp_us: 700_000,
                is_key: false,
            },
        ];

        assert_eq!(
            decode_slice_end_index(&chunks, 0, 250_000, 120_000, 1),
            Some(2)
        );
    }
}
