use std::collections::HashSet;

use crate::draw::frame::DrawOpFrame;
use crate::draw::op::DrawOp;
use crate::draw::types::{EffectId, ImageRef, RuntimeEffectChildRef};
use crate::platform::media::FrameMediaPlan;

/// Extract all media references from a DrawOpFrame and build a FrameMediaPlan.
/// Deduplicates references so each image/effect appears only once.
pub fn build_media_plan(frame: &DrawOpFrame) -> FrameMediaPlan {
    let mut images: Vec<ImageRef> = Vec::new();
    let mut seen_images: HashSet<ImageRef> = HashSet::new();
    let mut effect_ids: Vec<EffectId> = Vec::new();
    let mut seen_effects: HashSet<EffectId> = HashSet::new();

    for op in &frame.ops {
        match op {
            DrawOp::Image { image, .. } | DrawOp::ImageRect { image, .. } => {
                let img = image.clone();
                if seen_images.insert(img.clone()) {
                    images.push(img);
                }
            }
            DrawOp::RuntimeEffect {
                effect, children, ..
            } => {
                if seen_effects.insert(*effect) {
                    effect_ids.push(*effect);
                }
                let start = children.start as usize;
                let end = start + children.len as usize;
                for child in &frame.children[start..end] {
                    if let RuntimeEffectChildRef::Image(image_ref) = child {
                        let img = image_ref.clone();
                        if seen_images.insert(img.clone()) {
                            images.push(img);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    FrameMediaPlan {
        images,
        runtime_effects: effect_ids,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::builder::DrawOpBuilder;
    use crate::draw::frame::DrawOpFrame;
    use crate::draw::op::{DrawOp, Rect4};
    use crate::draw::types::{BytesRangeId, ChildRange, EffectId, ImageRef, RuntimeEffectChildRef};

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
        assert_eq!(plan.runtime_effects.len(), 1);
        assert_eq!(plan.runtime_effects[0], EffectId(0));
    }

    #[test]
    fn runtime_effect_children_images_are_collected() {
        let mut frame = DrawOpFrame::default();
        let child_img = ImageRef::Static {
            asset_id: "child.png".into(),
        };
        frame
            .children
            .push(RuntimeEffectChildRef::Image(child_img.clone()));
        frame.ops.push(DrawOp::RuntimeEffect {
            effect: EffectId(0),
            uniforms: BytesRangeId(0),
            children: ChildRange { start: 0, len: 1 },
            dst: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
        });
        let plan = build_media_plan(&frame);

        assert_eq!(plan.images.len(), 1);
        assert_eq!(plan.images[0], child_img);
    }

    #[test]
    fn runtime_effect_child_images_deduplicate_with_direct_images() {
        let mut frame = DrawOpFrame::default();
        let img = ImageRef::Static {
            asset_id: "shared.png".into(),
        };
        frame
            .children
            .push(RuntimeEffectChildRef::Image(img.clone()));
        frame.ops.push(DrawOp::Image {
            image: img.clone(),
            x: 0.0,
            y: 0.0,
            paint: None,
        });
        frame.ops.push(DrawOp::RuntimeEffect {
            effect: EffectId(0),
            uniforms: BytesRangeId(0),
            children: ChildRange { start: 0, len: 1 },
            dst: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
        });
        let plan = build_media_plan(&frame);

        assert_eq!(plan.images.len(), 1);
    }

    #[test]
    fn effect_ids_are_deduplicated() {
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
        builder.push(DrawOp::RuntimeEffect {
            effect: EffectId(0),
            uniforms: BytesRangeId(1),
            children: ChildRange { start: 0, len: 0 },
            dst: Rect4 {
                x: 50.0,
                y: 50.0,
                width: 100.0,
                height: 100.0,
            },
        });
        let frame = builder.finish();
        let plan = build_media_plan(&frame);

        assert_eq!(plan.runtime_effects.len(), 1);
    }
}
