use crate::canvas::paint::PaintSpec;
use crate::ir::draw_frame::DrawOpFrame;
use crate::ir::draw_op::{DrawOp, F32Range};
use crate::ir::draw_types::*;
use std::collections::HashMap;

/// Immediate-mode paint state for script canvas tracking.
/// Mirrors the HTML5 CanvasRenderingContext2D state for paint properties.
/// Currently populated by script canvas recording (to be integrated in Chunk 7).
#[derive(Default)]
#[allow(dead_code)]
struct DrawScriptPaintState {
    fill_color: Option<crate::ir::draw_op::ColorU8>,
    stroke_color: Option<crate::ir::draw_op::ColorU8>,
    line_width: f32,
    line_cap: crate::ir::draw_op::LineCap,
    line_join: crate::ir::draw_op::LineJoin,
    line_dash: Option<(Vec<f32>, f32)>,
    global_alpha: f32,
    anti_alias: bool,
}

/// Frame-local builder that all rendering functions write into.
/// The single point of interning for paint/path/string data.
#[derive(Default)]
pub struct DrawOpBuilder {
    ops: Vec<DrawOp>,
    subtrees: Vec<Vec<DrawOp>>,
    paints: Vec<PaintSpec>,
    paint_dedup: HashMap<PaintSpec, PaintId>,
    paths: Vec<EncodedPath>,
    children: Vec<RuntimeEffectChildRef>,
    strings: Vec<String>,
    string_dedup: HashMap<String, StringId>,
    bytes: Vec<u8>,
    byte_ranges: Vec<TableRange>,
    f32_pool: Vec<f32>,
    ranges: Vec<DrawOpRange>,
    resources: Vec<ResourceRef>,
    effects: Vec<EffectRef>,
    #[allow(dead_code)]
    paint_state: DrawScriptPaintState,
}

impl DrawOpBuilder {
    /// Append a fully-formed DrawOp directly.
    pub fn push(&mut self, op: DrawOp) {
        self.ops.push(op);
    }

    /// Intern a PaintSpec, returning a deduplicated PaintId.
    pub fn intern_paint(&mut self, spec: PaintSpec) -> PaintId {
        if let Some(&id) = self.paint_dedup.get(&spec) {
            return id;
        }
        let id = PaintId(self.paints.len() as u32);
        self.paints.push(spec.clone());
        self.paint_dedup.insert(spec, id);
        id
    }

    /// Intern a string, returning a deduplicated StringId.
    pub fn intern_string(&mut self, s: &str) -> StringId {
        if let Some(&id) = self.string_dedup.get(s) {
            return id;
        }
        let id = StringId(self.strings.len() as u32);
        self.strings.push(s.to_string());
        self.string_dedup.insert(s.to_string(), id);
        id
    }

    /// Intern a slice of f32 values, returning an F32Range pointing into the pool.
    pub fn intern_f32_range(&mut self, values: &[f32]) -> crate::ir::draw_op::F32Range {
        let start = self.f32_pool.len() as u32;
        self.f32_pool.extend_from_slice(values);
        crate::ir::draw_op::F32Range {
            start,
            len: values.len() as u32,
        }
    }

    /// Intern an EncodedPath, returning a PathId.
    pub fn intern_path(&mut self, path: EncodedPath) -> PathId {
        let id = PathId(self.paths.len() as u32);
        self.paths.push(path);
        id
    }

    /// Begin a range marker. Returns a token for `end_range`.
    pub fn begin_range(&mut self) -> RangeMarker {
        RangeMarker {
            start: self.ops.len() as u32,
        }
    }

    /// End a range, recording a DrawOpRange for cache tracking.
    pub fn end_range(&mut self, marker: RangeMarker) -> DrawOpRange {
        let range = DrawOpRange {
            start_op: marker.start,
            op_len: (self.ops.len() as u32).saturating_sub(marker.start),
        };
        self.ranges.push(range);
        range
    }

    /// Record a hidden subtree as an isolated draw program.
    ///
    /// The closure shares all side tables with the main builder, but its ops
    /// are captured into DrawOpFrame.subtrees instead of being appended to the
    /// main program.
    pub fn record_subtree<F, E>(&mut self, f: F) -> Result<SubtreeId, E>
    where
        F: FnOnce(&mut Self) -> Result<(), E>,
    {
        let main_ops = std::mem::take(&mut self.ops);
        let result = f(self);
        let subtree_ops = std::mem::replace(&mut self.ops, main_ops);
        match result {
            Ok(()) => {
                let id = SubtreeId(self.subtrees.len() as u32);
                self.subtrees.push(subtree_ops);
                Ok(id)
            }
            Err(err) => Err(err),
        }
    }

    /// Intern an effect (hash + SkSL), returning a deduplicated EffectId.
    pub fn intern_effect(&mut self, hash: u64, sksl: &str) -> EffectId {
        if let Some(pos) = self.effects.iter().position(|e| e.hash == hash) {
            #[cfg(debug_assertions)]
            debug_assert_eq!(
                self.effects[pos].sksl, sksl,
                "effect hash collision: same hash={hash:#x} but different sksl"
            );
            return EffectId(pos as u32);
        }
        let id = EffectId(self.effects.len() as u32);
        self.effects.push(EffectRef {
            hash,
            sksl: sksl.to_string(),
        });
        id
    }

    /// Intern raw bytes, returning a BytesRangeId into the byte table.
    pub fn intern_bytes(&mut self, data: &[u8]) -> BytesRangeId {
        let id = BytesRangeId(self.byte_ranges.len() as u32);
        let start = self.bytes.len() as u32;
        self.bytes.extend_from_slice(data);
        self.byte_ranges.push(TableRange {
            start,
            len: data.len() as u32,
        });
        id
    }

    /// Push a child reference into the children table, returning its index.
    pub fn push_child(&mut self, child: RuntimeEffectChildRef) -> u32 {
        let idx = self.children.len() as u32;
        self.children.push(child);
        idx
    }

    /// Current size of the child table; used by callers that need to record
    /// the start index of a new `ChildRange` before pushing children.
    pub fn children_len(&self) -> usize {
        self.children.len()
    }

    /// Import a cached segment into the current builder, remapping all id
    /// offsets so they index correctly into this builder's side tables.
    pub fn import_segment(&mut self, segment: &crate::ir::cache::CachedDrawSegment) -> DrawOpRange {
        let paint_offset = self.paints.len() as u32;
        let path_offset = self.paths.len() as u32;
        let string_offset = self.strings.len() as u32;
        let child_offset = self.children.len() as u32;
        let resource_offset = self.resources.len() as u32;
        let effects_offset = self.effects.len() as u32;
        let f32_pool_off = self.f32_pool.len() as u32;
        let byte_ranges_off = self.byte_ranges.len() as u32;
        let ops_off = self.ops.len() as u32;

        // Append side-table data from the segment, remapping child references
        // that contain frame-local ranges (e.g., Picture(DrawOpRange))
        self.paints.extend(segment.paints.iter().cloned());
        self.paths.extend(segment.paths.iter().cloned());
        self.strings.extend(segment.strings.iter().cloned());
        for child in &segment.children {
            self.children.push(match child {
                RuntimeEffectChildRef::Picture(range) => {
                    RuntimeEffectChildRef::Picture(DrawOpRange {
                        start_op: range.start_op + ops_off,
                        op_len: range.op_len,
                    })
                }
                other => other.clone(),
            });
        }
        self.bytes.extend_from_slice(&segment.bytes);
        self.byte_ranges.extend(segment.byte_ranges.iter().cloned());
        self.f32_pool.extend_from_slice(&segment.f32_pool);
        self.resources.extend(segment.resources.iter().cloned());
        self.effects.extend(segment.effects.iter().cloned());

        for op in &segment.ops {
            self.ops.push(remap_op(
                op,
                paint_offset,
                path_offset,
                string_offset,
                child_offset,
                resource_offset,
                effects_offset,
                f32_pool_off,
                byte_ranges_off,
                ops_off,
            ));
        }
        let range = DrawOpRange {
            start_op: ops_off,
            op_len: segment.ops.len() as u32,
        };
        self.ranges.push(range);
        range
    }

    /// Capture a DrawOpRange as a CachedDrawSegment for cache storage.
    /// Clones all side-table data referenced by ops in the given range,
    /// compacting indices to 0-based and remapping all IDs in the cloned ops.
    pub fn snapshot_range(&self, range: DrawOpRange) -> crate::ir::cache::CachedDrawSegment {
        use crate::ir::cache::CachedDrawSegment;
        let start = range.start_op as usize;
        let end = start + range.op_len as usize;
        let ops_slice = &self.ops[start..end];

        let mut paint_ids: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        let mut path_ids: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        let mut child_ranges: Vec<ChildRange> = Vec::new();
        let mut resource_ids: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        let mut effect_ids: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();

        collect_references(
            ops_slice,
            &mut paint_ids,
            &mut path_ids,
            &mut child_ranges,
            &mut resource_ids,
            &mut effect_ids,
        );

        // Build old→new index maps for compacted side-table entries
        let paint_map: std::collections::BTreeMap<u32, u32> = paint_ids
            .iter()
            .enumerate()
            .map(|(new_idx, &old_id)| (old_id, new_idx as u32))
            .collect();
        let path_map: std::collections::BTreeMap<u32, u32> = path_ids
            .iter()
            .enumerate()
            .map(|(new_idx, &old_id)| (old_id, new_idx as u32))
            .collect();
        let effect_map: std::collections::BTreeMap<u32, u32> = effect_ids
            .iter()
            .enumerate()
            .map(|(new_idx, &old_id)| (old_id, new_idx as u32))
            .collect();

        let paints: Vec<_> = paint_ids
            .iter()
            .map(|&id| self.paints[id as usize].clone())
            .collect();

        let paths: Vec<_> = path_ids
            .iter()
            .map(|&id| self.paths[id as usize].clone())
            .collect();

        let mut children = Vec::new();
        let mut flat_children: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        for cr in &child_ranges {
            for i in cr.start..cr.start + cr.len {
                flat_children.insert(i);
            }
        }
        let child_map: std::collections::BTreeMap<u32, u32> = flat_children
            .iter()
            .enumerate()
            .map(|(new_idx, &old_id)| (old_id, new_idx as u32))
            .collect();
        for idx in &flat_children {
            if (*idx as usize) < self.children.len() {
                children.push(self.children[*idx as usize].clone());
            }
        }

        let resources: Vec<_> = resource_ids
            .iter()
            .map(|&id| self.resources[id as usize].clone())
            .collect();

        let effects: Vec<_> = effect_ids
            .iter()
            .map(|&id| self.effects[id as usize].clone())
            .collect();

        // Remap all IDs in cloned ops to match compacted side-tables
        let ops: Vec<DrawOp> = ops_slice
            .iter()
            .map(|op| remap_op_snapshot(op, &paint_map, &path_map, &effect_map, &child_map))
            .collect();

        CachedDrawSegment {
            ops,
            paints,
            paths,
            children,
            strings: self.strings.clone(),
            bytes: self.bytes.clone(),
            byte_ranges: self.byte_ranges.clone(),
            f32_pool: self.f32_pool.clone(),
            resources,
            effects,
        }
    }

    /// Consume the builder and produce a DrawOpFrame.
    pub fn finish(self) -> DrawOpFrame {
        DrawOpFrame {
            ops: self.ops,
            subtrees: self.subtrees,
            paints: self.paints,
            paths: self.paths,
            children: self.children,
            strings: self.strings,
            bytes: self.bytes,
            byte_ranges: self.byte_ranges,
            f32_pool: self.f32_pool,
            ranges: self.ranges,
            resources: self.resources,
            effects: self.effects,
        }
    }
}

/// Opaque token returned by `begin_range`.
pub struct RangeMarker {
    start: u32,
}

fn collect_references(
    ops: &[DrawOp],
    paint_ids: &mut std::collections::BTreeSet<u32>,
    path_ids: &mut std::collections::BTreeSet<u32>,
    child_ranges: &mut Vec<ChildRange>,
    _resource_ids: &mut std::collections::BTreeSet<u32>,
    effect_ids: &mut std::collections::BTreeSet<u32>,
) {
    for op in ops {
        match op {
            DrawOp::Rect { paint, .. }
            | DrawOp::RRect { paint, .. }
            | DrawOp::DRRect { paint, .. }
            | DrawOp::Oval { paint, .. }
            | DrawOp::Circle { paint, .. }
            | DrawOp::Arc { paint, .. }
            | DrawOp::Line { paint, .. }
            | DrawOp::Points { paint, .. }
            | DrawOp::Paint { paint } => {
                paint_ids.insert(paint.0);
            }
            DrawOp::DrawPath { path, paint } => {
                path_ids.insert(path.0);
                paint_ids.insert(paint.0);
            }
            DrawOp::Image { paint, .. } | DrawOp::ImageRect { paint, .. } => {
                if let Some(p) = paint {
                    paint_ids.insert(p.0);
                }
            }
            DrawOp::SaveLayer { paint, .. } => {
                if let Some(p) = paint {
                    paint_ids.insert(p.0);
                }
            }
            DrawOp::RuntimeEffect {
                effect, children, ..
            } => {
                effect_ids.insert(effect.0);
                child_ranges.push(*children);
            }
            DrawOp::ReplayRange { .. } => {}
            _ => {}
        }
    }
}

/// Remap id offsets when importing a cached segment.
/// Every side-table id (PaintId, PathId, etc.) and range (F32Range,
/// DrawOpRange, ChildRange, BytesRangeId) needs its start offset shifted
/// so it indexes into the current builder's tables instead of the segment's.
fn remap_op(
    op: &DrawOp,
    paint_off: u32,
    path_off: u32,
    _string_off: u32,
    child_off: u32,
    _resource_off: u32,
    effects_off: u32,
    f32_pool_off: u32,
    byte_ranges_off: u32,
    ops_off: u32,
) -> DrawOp {
    match op {
        DrawOp::Rect { rect, paint } => DrawOp::Rect {
            rect: *rect,
            paint: PaintId(paint.0 + paint_off),
        },
        DrawOp::RRect { rect, radii, paint } => DrawOp::RRect {
            rect: *rect,
            radii: *radii,
            paint: PaintId(paint.0 + paint_off),
        },
        DrawOp::DRRect {
            outer,
            inner,
            paint,
        } => DrawOp::DRRect {
            outer: *outer,
            inner: *inner,
            paint: PaintId(paint.0 + paint_off),
        },
        DrawOp::Oval { rect, paint } => DrawOp::Oval {
            rect: *rect,
            paint: PaintId(paint.0 + paint_off),
        },
        DrawOp::Circle {
            cx,
            cy,
            radius,
            paint,
        } => DrawOp::Circle {
            cx: *cx,
            cy: *cy,
            radius: *radius,
            paint: PaintId(paint.0 + paint_off),
        },
        DrawOp::Arc {
            rect,
            start,
            sweep,
            use_center,
            paint,
        } => DrawOp::Arc {
            rect: *rect,
            start: *start,
            sweep: *sweep,
            use_center: *use_center,
            paint: PaintId(paint.0 + paint_off),
        },
        DrawOp::Line {
            x0,
            y0,
            x1,
            y1,
            paint,
        } => DrawOp::Line {
            x0: *x0,
            y0: *y0,
            x1: *x1,
            y1: *y1,
            paint: PaintId(paint.0 + paint_off),
        },
        DrawOp::Points {
            mode,
            points,
            paint,
        } => DrawOp::Points {
            mode: *mode,
            points: F32Range {
                start: points.start + f32_pool_off,
                len: points.len,
            },
            paint: PaintId(paint.0 + paint_off),
        },
        DrawOp::SetLineDash { intervals, phase } => DrawOp::SetLineDash {
            intervals: F32Range {
                start: intervals.start + f32_pool_off,
                len: intervals.len,
            },
            phase: *phase,
        },
        DrawOp::DrawPath { path, paint } => DrawOp::DrawPath {
            path: PathId(path.0 + path_off),
            paint: PaintId(paint.0 + paint_off),
        },
        DrawOp::Paint { paint } => DrawOp::Paint {
            paint: PaintId(paint.0 + paint_off),
        },
        DrawOp::SaveLayer {
            bounds,
            paint,
            alpha,
        } => DrawOp::SaveLayer {
            bounds: *bounds,
            paint: paint.map(|p| PaintId(p.0 + paint_off)),
            alpha: *alpha,
        },
        DrawOp::RuntimeEffect {
            effect,
            uniforms,
            children,
            dst,
        } => DrawOp::RuntimeEffect {
            effect: EffectId(effect.0 + effects_off),
            uniforms: BytesRangeId(uniforms.0 + byte_ranges_off),
            children: ChildRange {
                start: children.start + child_off,
                len: children.len,
            },
            dst: *dst,
        },
        DrawOp::ReplayRange { range } => DrawOp::ReplayRange {
            range: DrawOpRange {
                start_op: range.start_op + ops_off,
                op_len: range.op_len,
            },
        },
        DrawOp::Image { image, x, y, paint } => DrawOp::Image {
            image: image.clone(),
            x: *x,
            y: *y,
            paint: paint.map(|p| PaintId(p.0 + paint_off)),
        },
        DrawOp::ImageRect {
            image,
            src,
            dst,
            paint,
        } => DrawOp::ImageRect {
            image: image.clone(),
            src: *src,
            dst: *dst,
            paint: paint.map(|p| PaintId(p.0 + paint_off)),
        },
        // Identity ops — no IDs to remap
        _ => op.clone(),
    }
}

/// Like remap_op but for snapshot compaction: maps old builder indices
/// to new compacted indices using the provided maps.
fn remap_op_snapshot(
    op: &DrawOp,
    paint_map: &std::collections::BTreeMap<u32, u32>,
    path_map: &std::collections::BTreeMap<u32, u32>,
    effect_map: &std::collections::BTreeMap<u32, u32>,
    child_map: &std::collections::BTreeMap<u32, u32>,
) -> DrawOp {
    let remap_paint = |p: &PaintId| PaintId(*paint_map.get(&p.0).unwrap_or(&0));
    let remap_path = |p: &PathId| PathId(*path_map.get(&p.0).unwrap_or(&0));
    let remap_effect = |e: &EffectId| EffectId(*effect_map.get(&e.0).unwrap_or(&0));
    let remap_child = |c: &ChildRange| ChildRange {
        start: *child_map.get(&c.start).unwrap_or(&0),
        len: c.len,
    };

    match op {
        DrawOp::Rect { rect, paint } => DrawOp::Rect {
            rect: *rect,
            paint: remap_paint(paint),
        },
        DrawOp::RRect { rect, radii, paint } => DrawOp::RRect {
            rect: *rect,
            radii: *radii,
            paint: remap_paint(paint),
        },
        DrawOp::DRRect {
            outer,
            inner,
            paint,
        } => DrawOp::DRRect {
            outer: *outer,
            inner: *inner,
            paint: remap_paint(paint),
        },
        DrawOp::Oval { rect, paint } => DrawOp::Oval {
            rect: *rect,
            paint: remap_paint(paint),
        },
        DrawOp::Circle {
            cx,
            cy,
            radius,
            paint,
        } => DrawOp::Circle {
            cx: *cx,
            cy: *cy,
            radius: *radius,
            paint: remap_paint(paint),
        },
        DrawOp::Arc {
            rect,
            start,
            sweep,
            use_center,
            paint,
        } => DrawOp::Arc {
            rect: *rect,
            start: *start,
            sweep: *sweep,
            use_center: *use_center,
            paint: remap_paint(paint),
        },
        DrawOp::Line {
            x0,
            y0,
            x1,
            y1,
            paint,
        } => DrawOp::Line {
            x0: *x0,
            y0: *y0,
            x1: *x1,
            y1: *y1,
            paint: remap_paint(paint),
        },
        DrawOp::Points {
            mode,
            points,
            paint,
        } => DrawOp::Points {
            mode: *mode,
            points: *points,
            paint: remap_paint(paint),
        },
        DrawOp::DrawPath { path, paint } => DrawOp::DrawPath {
            path: remap_path(path),
            paint: remap_paint(paint),
        },
        DrawOp::Paint { paint } => DrawOp::Paint {
            paint: remap_paint(paint),
        },
        DrawOp::RuntimeEffect {
            effect,
            uniforms,
            children,
            dst,
        } => DrawOp::RuntimeEffect {
            effect: remap_effect(effect),
            uniforms: *uniforms,
            children: remap_child(children),
            dst: *dst,
        },
        DrawOp::SaveLayer {
            bounds,
            paint,
            alpha,
        } => DrawOp::SaveLayer {
            bounds: *bounds,
            paint: paint.as_ref().map(&remap_paint),
            alpha: *alpha,
        },
        DrawOp::Image { image, x, y, paint } => DrawOp::Image {
            image: image.clone(),
            x: *x,
            y: *y,
            paint: paint.as_ref().map(&remap_paint),
        },
        DrawOp::ImageRect {
            image,
            src,
            dst,
            paint,
        } => DrawOp::ImageRect {
            image: image.clone(),
            src: *src,
            dst: *dst,
            paint: paint.as_ref().map(remap_paint),
        },
        DrawOp::ReplayRange { range } => DrawOp::ReplayRange { range: *range },
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::paint::{FillSpec, PaintStyle};

    #[test]
    fn builder_push_and_finish_produces_empty_frame() {
        let builder = DrawOpBuilder::default();
        let frame = builder.finish();
        assert!(frame.ops.is_empty());
    }

    #[test]
    fn builder_pushes_direct_op() {
        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::Save);
        builder.push(DrawOp::Restore);
        let frame = builder.finish();
        assert_eq!(frame.ops.len(), 2);
        assert_eq!(frame.ops[0], DrawOp::Save);
        assert_eq!(frame.ops[1], DrawOp::Restore);
    }

    #[test]
    fn builder_interns_paint_and_returns_paint_id() {
        let mut builder = DrawOpBuilder::default();
        let spec = PaintSpec {
            style: PaintStyle::Fill,
            fill: FillSpec::Solid([1.0, 0.0, 0.0, 1.0]),
            ..Default::default()
        };
        let id = builder.intern_paint(spec.clone());
        assert_eq!(id, PaintId(0));
        // Same spec should return same id
        let id2 = builder.intern_paint(spec);
        assert_eq!(id2, PaintId(0));
        // Different spec returns different id
        let spec2 = PaintSpec {
            fill: FillSpec::Solid([0.0, 1.0, 0.0, 1.0]),
            ..Default::default()
        };
        let id3 = builder.intern_paint(spec2);
        assert_eq!(id3, PaintId(1));
    }

    #[test]
    fn builder_interns_string() {
        let mut builder = DrawOpBuilder::default();
        let id1 = builder.intern_string("hello");
        let id2 = builder.intern_string("world");
        let id3 = builder.intern_string("hello");
        assert_eq!(id1, StringId(0));
        assert_eq!(id2, StringId(1));
        assert_eq!(id3, StringId(0)); // dedup
    }

    #[test]
    fn builder_begin_end_range() {
        let mut builder = DrawOpBuilder::default();
        let marker = builder.begin_range();
        builder.push(DrawOp::Save);
        builder.push(DrawOp::Restore);
        let range = builder.end_range(marker);
        assert_eq!(range.start_op, 0);
        assert_eq!(range.op_len, 2);
    }

    #[test]
    fn builder_import_segment_remaps_paint_ids() {
        use crate::ir::cache::CachedDrawSegment;
        use crate::ir::draw_op::Rect4;

        // Create a cached segment with a paint-referencing op
        let mut seg_builder = DrawOpBuilder::default();
        let paint_id = seg_builder.intern_paint(PaintSpec {
            style: PaintStyle::Fill,
            fill: FillSpec::Solid([1.0, 0.0, 0.0, 1.0]),
            ..Default::default()
        });
        seg_builder.push(DrawOp::Rect {
            rect: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            paint: paint_id,
        });
        let segment = CachedDrawSegment {
            ops: seg_builder.ops.clone(),
            paints: seg_builder.paints.clone(),
            ..Default::default()
        };

        // Import into a fresh builder that already has some paints
        let mut builder = DrawOpBuilder::default();
        // Add a pre-existing paint so the imported segment's offsets shift
        builder.intern_paint(PaintSpec {
            style: PaintStyle::Stroke,
            fill: FillSpec::Solid([0.0, 0.0, 1.0, 1.0]),
            ..Default::default()
        });
        let range = builder.import_segment(&segment);

        assert_eq!(range.start_op, 0);
        assert_eq!(range.op_len, 1);

        // The imported op's paint_id should have been remapped: PaintId(0) -> PaintId(1)
        let frame = builder.finish();
        if let DrawOp::Rect { paint, .. } = &frame.ops[0] {
            assert_eq!(paint.0, 1, "paint_id should be remapped from 0 to 1");
        } else {
            panic!("expected Rect op");
        }
    }

    #[test]
    fn builder_import_empty_segment_is_noop() {
        use crate::ir::cache::CachedDrawSegment;

        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::Save);

        let segment = CachedDrawSegment::default();
        let range = builder.import_segment(&segment);

        assert_eq!(range.start_op, 1); // after the existing Save
        assert_eq!(range.op_len, 0);

        let frame = builder.finish();
        assert_eq!(frame.ops.len(), 1); // only the original Save
    }

    #[test]
    fn snapshot_range_import_segment_roundtrip() {
        use crate::ir::draw_op::Rect4;

        let mut builder = DrawOpBuilder::default();
        let spec = PaintSpec {
            style: PaintStyle::Fill,
            fill: FillSpec::Solid([1.0, 0.0, 0.0, 1.0]),
            ..Default::default()
        };
        let paint_id = builder.intern_paint(spec);

        let marker = builder.begin_range();
        builder.push(DrawOp::Save);
        builder.push(DrawOp::Rect {
            rect: Rect4 {
                x: 10.0,
                y: 20.0,
                width: 100.0,
                height: 50.0,
            },
            paint: paint_id,
        });
        builder.push(DrawOp::Restore);
        let range = builder.end_range(marker);

        // Snapshot
        let segment = builder.snapshot_range(range);

        // New builder, import segment
        let mut builder2 = DrawOpBuilder::default();
        let _imported = builder2.import_segment(&segment);

        let frame = builder2.finish();
        // Should have 3 ops, with paint id remapped to 0
        assert_eq!(frame.ops.len(), 3);
        if let DrawOp::Rect { paint, .. } = &frame.ops[1] {
            assert_eq!(paint.0, 0);
        }
    }

    #[test]
    fn builder_children_len_tracks_pushes() {
        use crate::ir::draw_types::{ImageRef, RuntimeEffectChildRef};
        let mut b = DrawOpBuilder::default();
        assert_eq!(b.children_len(), 0);
        b.push_child(RuntimeEffectChildRef::Image(ImageRef::Static {
            asset_id: "x".into(),
        }));
        assert_eq!(b.children_len(), 1);
    }

    #[test]
    fn record_subtree_keeps_subtree_ops_out_of_main_program() {
        use crate::ir::draw_types::SubtreeId;

        let mut builder = DrawOpBuilder::default();
        builder.push(DrawOp::Save);
        let subtree = builder
            .record_subtree(|b| {
                b.push(DrawOp::Translate { x: 10.0, y: 20.0 });
                b.push(DrawOp::Restore);
                Ok::<(), ()>(())
            })
            .expect("subtree records");
        builder.push(DrawOp::ReplaySubtreePicture {
            subtree,
            x: 4.0,
            y: 5.0,
        });

        let frame = builder.finish();
        assert_eq!(subtree, SubtreeId(0));
        assert_eq!(frame.ops.len(), 2);
        assert_eq!(frame.subtrees.len(), 1);
        assert_eq!(frame.subtrees[0].len(), 2);
        assert!(matches!(frame.ops[0], DrawOp::Save));
        assert!(matches!(
            frame.ops[1],
            DrawOp::ReplaySubtreePicture {
                subtree: SubtreeId(0),
                x: 4.0,
                y: 5.0,
            }
        ));
    }
}
