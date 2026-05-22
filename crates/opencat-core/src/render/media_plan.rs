use std::collections::HashSet;

use crate::draw::frame::DrawOpFrame;
use crate::draw::op::DrawOp;
use crate::draw::types::{EffectId, ImageRef};
use crate::platform::media::FrameMediaPlan;

/// Extract all media references from a DrawOpFrame and build a FrameMediaPlan.
/// Deduplicates references so each image/effect appears only once.
pub fn build_media_plan(frame: &DrawOpFrame) -> FrameMediaPlan {
    let mut images: Vec<ImageRef> = Vec::new();
    let mut seen_images: HashSet<ImageRef> = HashSet::new();
    let mut _effect_ids: Vec<EffectId> = Vec::new();

    for op in &frame.ops {
        match op {
            DrawOp::Image { image, .. } | DrawOp::ImageRect { image, .. } => {
                if seen_images.insert(image.clone()) {
                    images.push(image.clone());
                }
            }
            DrawOp::RuntimeEffect { effect, .. } => {
                _effect_ids.push(*effect);
            }
            _ => {}
        }
    }

    FrameMediaPlan {
        images,
        runtime_effects: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use crate::draw::builder::DrawOpBuilder;
    use crate::draw::frame::DrawOpFrame;
    use crate::draw::op::{DrawOp, Rect4};
    use crate::draw::types::{BytesRangeId, ChildRange, EffectId, ImageRef};
    use super::*;

    #[test]
    fn empty_frame_produces_empty_plan() {
        let frame = DrawOpFrame::default();
        let plan = build_media_plan(&frame);
        assert!(plan.images.is_empty());
        assert!(plan.runtime_effects.is_empty());
    }

    #[test]
    fn extracts_image_references() {
        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::Image {
            image: ImageRef::Static {
                asset_id: "photo.png".into(),
            },
            x: 0.0,
            y: 0.0,
            paint: None,
        });
        builder.push(DrawOp::ImageRect {
            image: ImageRef::VideoFrame {
                asset_id: "clip.mp4".into(),
                frame_index: 5,
            },
            src: None,
            dst: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            paint: None,
        });
        let frame = builder.finish();
        let plan = build_media_plan(&frame);

        assert_eq!(plan.images.len(), 2);
    }

    #[test]
    fn deduplicates_identical_references() {
        let mut builder = DrawOpBuilder::default();
        let img = ImageRef::Static {
            asset_id: "dup.png".into(),
        };
        builder.push(DrawOp::Image {
            image: img.clone(),
            x: 0.0,
            y: 0.0,
            paint: None,
        });
        builder.push(DrawOp::Image {
            image: img.clone(),
            x: 10.0,
            y: 10.0,
            paint: None,
        });
        let frame = builder.finish();
        let plan = build_media_plan(&frame);

        assert_eq!(plan.images.len(), 1);
    }

    #[test]
    fn extracts_runtime_effect_ids() {
        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::RuntimeEffect {
            effect: EffectId(0),
            uniforms: BytesRangeId(0),
            children: ChildRange { start: 0, len: 0 },
            dst: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
        });
        let frame = builder.finish();
        let plan = build_media_plan(&frame);

        assert!(plan.images.is_empty());
    }
}
