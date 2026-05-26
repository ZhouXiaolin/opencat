use std::collections::HashSet;

use crate::ir::draw_frame::DrawOpFrame;
use crate::ir::draw_op::DrawOp;
use crate::ir::draw_types::{DrawOpRange, ImageRef, RuntimeEffectChildRef, SubtreeId};
use crate::ir::media_plan::FrameMediaPlan;

/// Extract all media references from a DrawOpFrame and build a FrameMediaPlan.
/// Deduplicates references so each image/effect appears only once.
pub fn build_media_plan(frame: &DrawOpFrame) -> FrameMediaPlan {
    let mut images: Vec<ImageRef> = Vec::new();
    let mut seen_images: HashSet<ImageRef> = HashSet::new();
    let mut visited_ranges: HashSet<DrawOpRange> = HashSet::new();
    let mut visited_subtrees: HashSet<SubtreeId> = HashSet::new();

    collect_ops(
        frame,
        &frame.ops,
        &mut images,
        &mut seen_images,
        &mut visited_ranges,
        &mut visited_subtrees,
    );

    FrameMediaPlan {
        images,
        runtime_effects: frame.effects.clone(),
    }
}

fn collect_ops(
    frame: &DrawOpFrame,
    ops: &[DrawOp],
    images: &mut Vec<ImageRef>,
    seen_images: &mut HashSet<ImageRef>,
    visited_ranges: &mut HashSet<DrawOpRange>,
    visited_subtrees: &mut HashSet<SubtreeId>,
) {
    for op in ops {
        match op {
            DrawOp::Image { image, .. } | DrawOp::ImageRect { image, .. } => {
                push_image(image, images, seen_images);
            }
            DrawOp::RuntimeEffect { children, .. } => {
                collect_child_range(
                    frame,
                    *children,
                    images,
                    seen_images,
                    visited_ranges,
                    visited_subtrees,
                );
            }
            DrawOp::ReplayRange { range } => {
                collect_range(
                    frame,
                    *range,
                    images,
                    seen_images,
                    visited_ranges,
                    visited_subtrees,
                );
            }
            DrawOp::ReplaySubtreePicture { subtree, .. } => {
                collect_subtree(
                    frame,
                    *subtree,
                    images,
                    seen_images,
                    visited_ranges,
                    visited_subtrees,
                );
            }
            _ => {}
        }
    }
}

fn collect_child_range(
    frame: &DrawOpFrame,
    children: crate::ir::draw_types::ChildRange,
    images: &mut Vec<ImageRef>,
    seen_images: &mut HashSet<ImageRef>,
    visited_ranges: &mut HashSet<DrawOpRange>,
    visited_subtrees: &mut HashSet<SubtreeId>,
) {
    let start = children.start as usize;
    let end = start.saturating_add(children.len as usize);
    let Some(child_refs) = frame.children.get(start..end) else {
        return;
    };
    for child in child_refs {
        match child {
            RuntimeEffectChildRef::Image(image_ref) => {
                push_image(image_ref, images, seen_images);
            }
            RuntimeEffectChildRef::Picture(range) => {
                collect_range(
                    frame,
                    *range,
                    images,
                    seen_images,
                    visited_ranges,
                    visited_subtrees,
                );
            }
            RuntimeEffectChildRef::SubtreePicture(subtree) => {
                collect_subtree(
                    frame,
                    *subtree,
                    images,
                    seen_images,
                    visited_ranges,
                    visited_subtrees,
                );
            }
            RuntimeEffectChildRef::Shader(_) => {}
        }
    }
}

fn collect_range(
    frame: &DrawOpFrame,
    range: DrawOpRange,
    images: &mut Vec<ImageRef>,
    seen_images: &mut HashSet<ImageRef>,
    visited_ranges: &mut HashSet<DrawOpRange>,
    visited_subtrees: &mut HashSet<SubtreeId>,
) {
    if !visited_ranges.insert(range) {
        return;
    }
    let start = range.start_op as usize;
    let end = start.saturating_add(range.op_len as usize);
    let Some(ops) = frame.ops.get(start..end) else {
        return;
    };
    collect_ops(
        frame,
        ops,
        images,
        seen_images,
        visited_ranges,
        visited_subtrees,
    );
}

fn collect_subtree(
    frame: &DrawOpFrame,
    subtree: SubtreeId,
    images: &mut Vec<ImageRef>,
    seen_images: &mut HashSet<ImageRef>,
    visited_ranges: &mut HashSet<DrawOpRange>,
    visited_subtrees: &mut HashSet<SubtreeId>,
) {
    if !visited_subtrees.insert(subtree) {
        return;
    }
    let Some(ops) = frame.subtrees.get(subtree.0 as usize) else {
        return;
    };
    collect_ops(
        frame,
        ops,
        images,
        seen_images,
        visited_ranges,
        visited_subtrees,
    );
}

fn push_image(
    image: &ImageRef,
    images: &mut Vec<ImageRef>,
    seen_images: &mut HashSet<ImageRef>,
) {
    let img = image.clone();
    if seen_images.insert(img.clone()) {
        images.push(img);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::draw_frame::DrawOpFrame;
    use crate::ir::draw_op::{DrawOp, Rect4};
    use crate::ir::draw_types::{
        BytesRangeId, ChildRange, EffectId, EffectRef, ImageRef, RuntimeEffectChildRef, SubtreeId,
    };
    use crate::render::builder::DrawOpBuilder;

    #[test]
    fn empty_frame_produces_empty_plan() {
        let frame = DrawOpFrame::default();
        let plan = build_media_plan(&frame);
        assert!(plan.images.is_empty());
        assert!(plan.runtime_effects.is_empty());
    }

    #[test]
    fn preserves_runtime_effect_table_order() {
        let mut frame = DrawOpFrame::default();
        frame.effects.push(EffectRef {
            hash: 0xAA01,
            sksl: "half4 main(float2 uv) { return half4(1); }".into(),
        });
        frame.effects.push(EffectRef {
            hash: 0xAA02,
            sksl: "half4 main(float2 uv) { return half4(0); }".into(),
        });
        frame.ops.push(DrawOp::RuntimeEffect {
            effect: EffectId(1),
            uniforms: BytesRangeId(0),
            children: ChildRange { start: 0, len: 0 },
            dst: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
        });

        let plan = build_media_plan(&frame);

        assert_eq!(plan.runtime_effects.len(), 2);
        assert_eq!(plan.runtime_effects[0].hash, 0xAA01);
        assert_eq!(plan.runtime_effects[1].hash, 0xAA02);
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
        let effect_id = builder.intern_effect(0xAA01, "half4 main(float2 uv) {}");
        assert_eq!(effect_id, EffectId(0));
        builder.push(DrawOp::RuntimeEffect {
            effect: effect_id,
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
        assert_eq!(plan.runtime_effects[0].hash, 0xAA01);
    }

    #[test]
    fn runtime_effect_child_images_are_collected() {
        let mut frame = DrawOpFrame::default();
        let child_img = ImageRef::Static {
            asset_id: "child.png".into(),
        };
        frame.effects.push(EffectRef {
            hash: 0xAA01,
            sksl: "half4 main(float2 uv) {}".into(),
        });
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
        frame.effects.push(EffectRef {
            hash: 0xAA01,
            sksl: "half4 main(float2 uv) {}".into(),
        });
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
    fn subtree_images_are_collected() {
        let mut frame = DrawOpFrame::default();
        let img = ImageRef::Static {
            asset_id: "hidden.png".into(),
        };
        frame.subtrees.push(vec![DrawOp::Image {
            image: img.clone(),
            x: 0.0,
            y: 0.0,
            paint: None,
        }]);
        frame.ops.push(DrawOp::ReplaySubtreePicture {
            subtree: SubtreeId(0),
            x: 4.0,
            y: 5.0,
        });

        let plan = build_media_plan(&frame);

        assert_eq!(plan.images, vec![img]);
    }

    #[test]
    fn runtime_effect_subtree_picture_images_are_collected() {
        let mut frame = DrawOpFrame::default();
        let img = ImageRef::Static {
            asset_id: "shader-child.png".into(),
        };
        frame.effects.push(EffectRef {
            hash: 0xAA01,
            sksl: "half4 main(float2 uv) {}".into(),
        });
        frame
            .byte_ranges
            .push(crate::ir::draw_types::TableRange { start: 0, len: 0 });
        frame.subtrees.push(vec![DrawOp::ImageRect {
            image: img.clone(),
            src: None,
            dst: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            paint: None,
        }]);
        frame
            .children
            .push(RuntimeEffectChildRef::SubtreePicture(SubtreeId(0)));
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

        assert_eq!(plan.images, vec![img]);
    }

    #[test]
    fn effect_ids_are_deduplicated() {
        let mut builder = DrawOpBuilder::default();
        let effect_id = builder.intern_effect(0xAA01, "half4 main(float2 uv) {}");
        builder.push(DrawOp::RuntimeEffect {
            effect: effect_id,
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
            effect: effect_id,
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
