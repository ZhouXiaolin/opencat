use crate::canvas::paint::PaintSpec;
use crate::ir::draw_frame::DrawOpFrame;
use crate::ir::draw_op::DrawOp;
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

    /// Debug dump of the op stream (OC_SVG_DEBUG diagnostics only).
    pub fn ops_debug(&self) -> &[DrawOp] {
        &self.ops
    }

    /// Debug dump of the interned paints (OC_SVG_DEBUG diagnostics only).
    pub fn paints_debug(&self) -> &[PaintSpec] {
        &self.paints
    }

    /// Debug dump of the interned path table (OC_SVG_DEBUG diagnostics only).
    pub fn paths_debug(&self) -> &[EncodedPath] {
        &self.paths
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


