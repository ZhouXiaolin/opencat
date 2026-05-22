use std::collections::HashMap;
use crate::canvas::paint::PaintSpec;
use super::op::DrawOp;
use super::frame::DrawOpFrame;
use super::types::*;

/// Immediate-mode paint state for script canvas tracking.
/// Mirrors the HTML5 CanvasRenderingContext2D state for paint properties.
#[derive(Default)]
struct DrawScriptPaintState {
    fill_color: Option<super::op::ColorU8>,
    stroke_color: Option<super::op::ColorU8>,
    line_width: f32,
    line_cap: super::op::LineCap,
    line_join: super::op::LineJoin,
    line_dash: Option<(Vec<f32>, f32)>,
    global_alpha: f32,
    anti_alias: bool,
}

/// Frame-local builder that all rendering functions write into.
/// The single point of interning for paint/path/string data.
#[derive(Default)]
pub struct DrawOpBuilder {
    ops: Vec<DrawOp>,
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

    /// Import a cached segment into the current builder, remapping all id
    /// offsets so they index correctly into this builder's side tables.
    pub fn import_segment(&mut self, segment: &super::cache::CachedDrawSegment) -> DrawOpRange {
        let paint_offset = self.paints.len() as u32;
        let path_offset = self.paths.len() as u32;
        let string_offset = self.strings.len() as u32;
        let child_offset = self.children.len() as u32;
        let resource_offset = self.resources.len() as u32;

        // Append side-table data from the segment
        self.paints.extend(segment.paints.iter().cloned());
        self.paths.extend(segment.paths.iter().cloned());
        self.strings.extend(segment.strings.iter().cloned());
        self.children.extend(segment.children.iter().cloned());
        self.bytes.extend_from_slice(&segment.bytes);
        self.byte_ranges.extend(segment.byte_ranges.iter().cloned());
        self.f32_pool.extend_from_slice(&segment.f32_pool);
        self.resources.extend(segment.resources.iter().cloned());

        let start = self.ops.len() as u32;
        for op in &segment.ops {
            self.ops.push(remap_op(
                op,
                paint_offset,
                path_offset,
                string_offset,
                child_offset,
                resource_offset,
            ));
        }
        let range = DrawOpRange {
            start_op: start,
            op_len: segment.ops.len() as u32,
        };
        self.ranges.push(range);
        range
    }

    /// Consume the builder and produce a DrawOpFrame.
    pub fn finish(self) -> DrawOpFrame {
        DrawOpFrame {
            ops: self.ops,
            paints: self.paints,
            paths: self.paths,
            children: self.children,
            strings: self.strings,
            bytes: self.bytes,
            byte_ranges: self.byte_ranges,
            f32_pool: self.f32_pool,
            ranges: self.ranges,
            resources: self.resources,
        }
    }
}

/// Opaque token returned by `begin_range`.
pub struct RangeMarker {
    start: u32,
}

/// Remap PaintId offsets when importing a cached segment.
/// All id types that reference side tables need offset adjustment.
#[allow(unused_variables)]
fn remap_op(
    op: &DrawOp,
    paint_off: u32,
    path_off: u32,
    string_off: u32,
    child_off: u32,
    resource_off: u32,
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
            points: *points,
            paint: PaintId(paint.0 + paint_off),
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
            effect: EffectId(effect.0),
            uniforms: BytesRangeId(uniforms.0 + resource_off),
            children: ChildRange {
                start: children.start + child_off,
                len: children.len,
            },
            dst: *dst,
        },
        DrawOp::ReplayRange { range } => DrawOp::ReplayRange { range: *range },
        DrawOp::Image {
            image,
            x,
            y,
            paint,
        } => DrawOp::Image {
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
}
