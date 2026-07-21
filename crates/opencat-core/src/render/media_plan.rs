use std::collections::HashSet;
use std::sync::Arc;

use crate::ir::draw_frame::DrawOpFrame;
use crate::ir::draw_op::DrawOp;
use crate::ir::draw_types::{DrawOpRange, ImageRef, RuntimeEffectChildRef, SubtreeId};
use crate::ir::generated_image::{GeneratedImageId, GeneratedImageTable};
use crate::ir::media_plan::{FrameGeneratedImage, FrameMediaPlan};

/// Mutable accumulator for building a `FrameMediaPlan`. Each bucket keeps its
/// own dedup set so that static images, video frames, generated images, and
/// Lottie bundles never collide with each other.
struct MediaCollector {
    images: Vec<ImageRef>,
    video_frames: Vec<ImageRef>,
    /// Deduped generated ids in first-seen order; RGBA is resolved after the walk.
    generated_ids: Vec<GeneratedImageId>,
    lottie_bundles: Vec<String>,
    seen_images: HashSet<ImageRef>,
    seen_generated: HashSet<GeneratedImageId>,
    seen_lottie: HashSet<String>,
    visited_ranges: HashSet<DrawOpRange>,
    visited_subtrees: HashSet<SubtreeId>,
}

impl MediaCollector {
    fn new() -> Self {
        Self {
            images: Vec::new(),
            video_frames: Vec::new(),
            generated_ids: Vec::new(),
            lottie_bundles: Vec::new(),
            seen_images: HashSet::new(),
            seen_generated: HashSet::new(),
            seen_lottie: HashSet::new(),
            visited_ranges: HashSet::new(),
            visited_subtrees: HashSet::new(),
        }
    }

    /// Partition an image reference into the static, video, or generated-image
    /// bucket, deduped within its own bucket. Each category is distinct: a
    /// static image, a video frame, and a generated glyph sharing an asset id
    /// are all kept independently.
    fn push_image(&mut self, image: &ImageRef) {
        match image {
            ImageRef::Static { .. } => {
                if self.seen_images.insert(image.clone()) {
                    self.images.push(image.clone());
                }
            }
            ImageRef::VideoFrame { .. } => {
                if self.seen_images.insert(image.clone()) {
                    self.video_frames.push(image.clone());
                }
            }
            ImageRef::Generated { id } => {
                if self.seen_generated.insert(*id) {
                    self.generated_ids.push(*id);
                }
            }
        }
    }

    fn push_lottie(&mut self, bundle_id: &str) {
        if self.seen_lottie.insert(bundle_id.to_owned()) {
            self.lottie_bundles.push(bundle_id.to_owned());
        }
    }
}

/// Extract all media references from a DrawOpFrame and build a FrameMediaPlan.
///
/// Each category (external images, video frames, Lottie bundles, runtime
/// effects, generated images) is deduplicated independently. Generated image
/// entries include full RGBA looked up from `generated_table` — the host-facing
/// contract never requires reading the pipeline table.
///
/// The walk follows the same op structure the renderer emits, including
/// runtime-effect child tables and replayed ranges/subtrees, so a host preparing
/// media for the plan never misses a reference hidden behind a replay or shader
/// child.
///
/// # Panics
///
/// Panics if a `DrawOp` references a [`GeneratedImageId`] that is missing from
/// `generated_table`. Core always inserts before emitting
/// `ImageRef::Generated`; a missing entry is a core invariant violation and
/// must not silently drop RGBA from the host contract.
pub fn build_media_plan(
    frame: &DrawOpFrame,
    generated_table: &GeneratedImageTable,
) -> FrameMediaPlan {
    let mut collector = MediaCollector::new();
    collect_ops(frame, &frame.ops, &mut collector);

    let mut generated_images = Vec::with_capacity(collector.generated_ids.len());
    for id in collector.generated_ids {
        let entry = generated_table.get(&id).unwrap_or_else(|| {
            panic!(
                "DrawOp references GeneratedImageId {id:?} missing from table; \
                 core must insert RGBA before emitting ImageRef::Generated"
            )
        });
        generated_images.push(FrameGeneratedImage {
            id,
            width: entry.width,
            height: entry.height,
            rgba: Arc::clone(&entry.rgba),
        });
    }

    FrameMediaPlan {
        images: collector.images,
        video_frames: collector.video_frames,
        generated_images,
        lottie_bundles: collector.lottie_bundles,
        // Runtime effects are interned in a side table; expose all of them
        // deduplicated by effect id (the table itself is already unique).
        runtime_effects: frame.effects.clone(),
    }
}

fn collect_ops(frame: &DrawOpFrame, ops: &[DrawOp], collector: &mut MediaCollector) {
    for op in ops {
        match op {
            DrawOp::Image { image, .. } | DrawOp::ImageRect { image, .. } => {
                collector.push_image(image);
            }
            DrawOp::LottieRect { bundle_id, .. } => {
                collector.push_lottie(bundle_id);
            }
            DrawOp::RuntimeEffect { children, .. } => {
                collect_child_range(frame, *children, collector);
            }
            DrawOp::ReplayRange { range } => {
                collect_range(frame, *range, collector);
            }
            DrawOp::ReplaySubtreePicture { subtree, .. } => {
                collect_subtree(frame, *subtree, collector);
            }
            _ => {}
        }
    }
}

fn collect_child_range(
    frame: &DrawOpFrame,
    children: crate::ir::draw_types::ChildRange,
    collector: &mut MediaCollector,
) {
    let start = children.start as usize;
    let end = start.saturating_add(children.len as usize);
    let Some(child_refs) = frame.children.get(start..end) else {
        return;
    };
    for child in child_refs {
        match child {
            RuntimeEffectChildRef::Image(image_ref) => {
                collector.push_image(image_ref);
            }
            RuntimeEffectChildRef::Picture(range) => {
                collect_range(frame, *range, collector);
            }
            RuntimeEffectChildRef::SubtreePicture(subtree) => {
                collect_subtree(frame, *subtree, collector);
            }
            RuntimeEffectChildRef::Shader(_) => {}
        }
    }
}

fn collect_range(frame: &DrawOpFrame, range: DrawOpRange, collector: &mut MediaCollector) {
    if !collector.visited_ranges.insert(range) {
        return;
    }
    let start = range.start_op as usize;
    let end = start.saturating_add(range.op_len as usize);
    let Some(ops) = frame.ops.get(start..end) else {
        return;
    };
    collect_ops(frame, ops, collector);
}

fn collect_subtree(frame: &DrawOpFrame, subtree: SubtreeId, collector: &mut MediaCollector) {
    if !collector.visited_subtrees.insert(subtree) {
        return;
    }
    let Some(ops) = frame.subtrees.get(subtree.0 as usize) else {
        return;
    };
    collect_ops(frame, ops, collector);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::draw_frame::DrawOpFrame;
    use crate::ir::draw_op::{DrawOp, Rect4};
    use crate::ir::draw_types::{
        BytesRangeId, ChildRange, EffectId, EffectRef, ImageRef, RuntimeEffectChildRef, SubtreeId,
    };
    use crate::ir::generated_image::{GeneratedImageId, GeneratedImageTable};
    use crate::render::builder::DrawOpBuilder;

    fn empty_table() -> GeneratedImageTable {
        GeneratedImageTable::new()
    }

    fn rgba(value: u8, w: u32, h: u32) -> Arc<[u8]> {
        Arc::from(vec![value; w as usize * h as usize * 4])
    }

    #[test]
    fn empty_frame_produces_empty_plan() {
        let frame = DrawOpFrame::default();
        let plan = build_media_plan(&frame, &empty_table());
        assert!(plan.images.is_empty());
        assert!(plan.video_frames.is_empty());
        assert!(plan.lottie_bundles.is_empty());
        assert!(plan.runtime_effects.is_empty());
        assert!(plan.generated_images.is_empty());
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

        let plan = build_media_plan(&frame, &empty_table());

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
                time_micros: 166_667,
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
        let plan = build_media_plan(&frame, &empty_table());

        // Static images and video frames are distinct categories, each kept.
        assert_eq!(plan.images.len(), 1);
        assert_eq!(plan.video_frames.len(), 1);
        let ImageRef::VideoFrame {
            asset_id,
            time_micros,
        } = &plan.video_frames[0]
        else {
            panic!("expected video frame ref in video_frames bucket");
        };
        assert_eq!(asset_id, "clip.mp4");
        assert_eq!(*time_micros, 166_667);
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
        let plan = build_media_plan(&frame, &empty_table());

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
        let plan = build_media_plan(&frame, &empty_table());

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
        let plan = build_media_plan(&frame, &empty_table());

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
        let plan = build_media_plan(&frame, &empty_table());

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

        let plan = build_media_plan(&frame, &empty_table());

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

        let plan = build_media_plan(&frame, &empty_table());

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
        let plan = build_media_plan(&frame, &empty_table());

        assert_eq!(plan.runtime_effects.len(), 1);
    }

    #[test]
    fn lottie_bundles_are_collected_and_deduplicated() {
        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::LottieRect {
            bundle_id: "lottie:hero".into(),
            frame: 0.0,
            dst: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
        });
        // Same bundle emitted again at a different frame — must dedup.
        builder.push(DrawOp::LottieRect {
            bundle_id: "lottie:hero".into(),
            frame: 12.0,
            dst: Rect4 {
                x: 10.0,
                y: 10.0,
                width: 80.0,
                height: 80.0,
            },
        });
        builder.push(DrawOp::LottieRect {
            bundle_id: "lottie:badge".into(),
            frame: 0.0,
            dst: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            },
        });
        let frame = builder.finish();
        let plan = build_media_plan(&frame, &empty_table());

        // Lottie bundles are NOT images; they have their own bucket and do not
        // leak into the image or video categories.
        assert!(plan.images.is_empty());
        assert!(plan.video_frames.is_empty());
        assert_eq!(plan.lottie_bundles, vec!["lottie:hero", "lottie:badge"]);
    }

    #[test]
    fn lottie_bundles_in_subtrees_are_collected() {
        let mut frame = DrawOpFrame::default();
        frame.subtrees.push(vec![DrawOp::LottieRect {
            bundle_id: "lottie:hidden".into(),
            frame: 5.0,
            dst: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
        }]);
        frame.ops.push(DrawOp::ReplaySubtreePicture {
            subtree: SubtreeId(0),
            x: 0.0,
            y: 0.0,
        });

        let plan = build_media_plan(&frame, &empty_table());

        assert_eq!(plan.lottie_bundles, vec!["lottie:hidden"]);
    }

    #[test]
    fn generated_images_carry_full_rgba_and_dedupe() {
        let id = GeneratedImageId(0xDEAD_BEEF);
        let mut table = GeneratedImageTable::new();
        table
            .insert(id, 2, 2, rgba(0xAB, 2, 2))
            .expect("insert glyph");

        let mut builder = DrawOpBuilder::default();
        let img = ImageRef::Generated { id };
        // Same generated id twice — plan must carry one entry with full RGBA.
        builder.push(DrawOp::Image {
            image: img.clone(),
            x: 0.0,
            y: 0.0,
            paint: None,
        });
        builder.push(DrawOp::ImageRect {
            image: img,
            src: None,
            dst: Rect4 {
                x: 1.0,
                y: 1.0,
                width: 2.0,
                height: 2.0,
            },
            paint: None,
        });
        let frame = builder.finish();
        let plan = build_media_plan(&frame, &table);

        assert!(plan.images.is_empty());
        assert!(plan.video_frames.is_empty());
        assert_eq!(plan.generated_images.len(), 1);
        let g = &plan.generated_images[0];
        assert_eq!(g.id, id);
        assert_eq!(g.width, 2);
        assert_eq!(g.height, 2);
        assert_eq!(g.rgba.as_ref(), rgba(0xAB, 2, 2).as_ref());
        assert_eq!(g.rgba.len(), 2 * 2 * 4);
    }

    #[test]
    fn generated_image_ids_preserve_first_seen_order() {
        let id_a = GeneratedImageId(1);
        let id_b = GeneratedImageId(2);
        let mut table = GeneratedImageTable::new();
        table.insert(id_a, 1, 1, rgba(1, 1, 1)).unwrap();
        table.insert(id_b, 1, 1, rgba(2, 1, 1)).unwrap();

        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::Image {
            image: ImageRef::Generated { id: id_b },
            x: 0.0,
            y: 0.0,
            paint: None,
        });
        builder.push(DrawOp::Image {
            image: ImageRef::Generated { id: id_a },
            x: 0.0,
            y: 0.0,
            paint: None,
        });
        let plan = build_media_plan(&builder.finish(), &table);
        assert_eq!(
            plan.generated_images
                .iter()
                .map(|g| g.id)
                .collect::<Vec<_>>(),
            vec![id_b, id_a]
        );
    }
}
