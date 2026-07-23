use super::draw_op::DrawOp;
use super::media_plan::FrameMediaPlan;

/// Typed in-memory render frame consumed by platform executors directly.
/// Contains all side-table data that DrawOp IDs reference.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DrawOpFrame {
    pub ops: Vec<DrawOp>,
    pub subtrees: Vec<Vec<DrawOp>>,
    pub paints: Vec<crate::canvas::paint::PaintSpec>,
    pub paths: Vec<super::draw_types::EncodedPath>,
    pub children: Vec<super::draw_types::RuntimeEffectChildRef>,
    pub strings: Vec<String>,
    pub bytes: Vec<u8>,
    pub byte_ranges: Vec<super::draw_types::TableRange>,
    pub f32_pool: Vec<f32>,
    pub ranges: Vec<super::draw_types::DrawOpRange>,
    pub resources: Vec<super::draw_types::ResourceRef>,
    pub effects: Vec<super::draw_types::EffectRef>,
}

/// The single deterministic per-frame output contract of the render pipeline.
///
/// `draw` is the precise draw-IR for this frame; `media` is the host-facing
/// media preparation plan for this frame (images, video frames, Lottie bundles,
/// runtime effects, **and full generated-image RGBA**). Both halves are a pure
/// function of the composition and the requested `frame_index`: rendering the
/// same frame on the same pipeline — directly, out of order, or repeatedly —
/// must yield field-by-field identical results regardless of call history.
///
/// This is the sole core→host current-frame render contract. Hosts consume
/// `RenderFrame` directly and must not reach into pipeline-internal resource
/// tables for generated images. Hosts implement fetch, decode, cache, seek,
/// prefetch, and export independently.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderFrame {
    pub draw: DrawOpFrame,
    pub media: FrameMediaPlan,
}

/// Reusable scratch buffers for binary encoding.
/// Cleared and reused each frame to avoid allocation.
#[derive(Default)]
pub struct DrawFrameScratch {
    pub ops: Vec<DrawOp>,
    pub subtrees: Vec<Vec<DrawOp>>,
    pub encoded_ops: Vec<u8>,
    pub encoded_subtrees: Vec<u8>,
    pub children: Vec<super::draw_types::RuntimeEffectChildRef>,
    pub encoded_children: Vec<u8>,
    pub f32_pool: Vec<f32>,
    pub bytes: Vec<u8>,
    pub byte_ranges: Vec<super::draw_types::TableRange>,
    pub strings_utf8: Vec<u8>,
    pub string_ranges: Vec<super::draw_types::TableRange>,
}

impl DrawFrameScratch {
    /// Clear all buffers for reuse.
    pub fn clear(&mut self) {
        self.ops.clear();
        self.subtrees.clear();
        self.encoded_ops.clear();
        self.encoded_subtrees.clear();
        self.children.clear();
        self.encoded_children.clear();
        self.f32_pool.clear();
        self.bytes.clear();
        self.byte_ranges.clear();
        self.strings_utf8.clear();
        self.string_ranges.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_frame_has_empty_tables() {
        let frame = DrawOpFrame::default();
        assert!(frame.ops.is_empty());
        assert!(frame.subtrees.is_empty());
        assert!(frame.paints.is_empty());
        assert!(frame.paths.is_empty());
        assert!(frame.children.is_empty());
        assert!(frame.strings.is_empty());
        assert!(frame.bytes.is_empty());
        assert!(frame.byte_ranges.is_empty());
        assert!(frame.f32_pool.is_empty());
        assert!(frame.ranges.is_empty());
        assert!(frame.resources.is_empty());
        assert!(frame.effects.is_empty());
    }

    #[test]
    fn frame_from_builder_roundtrips() {
        let mut builder = crate::render::builder::DrawOpBuilder::default();
        builder.push(DrawOp::Save);
        builder.push(DrawOp::Restore);
        let frame = builder.finish();
        assert_eq!(frame.ops.len(), 2);
    }

    #[test]
    fn scratch_is_empty_on_create() {
        let scratch = DrawFrameScratch::default();
        assert!(scratch.ops.is_empty());
        assert!(scratch.subtrees.is_empty());
        assert!(scratch.encoded_ops.is_empty());
        assert!(scratch.encoded_subtrees.is_empty());
    }

    #[test]
    fn scratch_can_be_cleared_and_reused() {
        let mut scratch = DrawFrameScratch::default();
        scratch.ops.push(DrawOp::Save);
        scratch.subtrees.push(vec![DrawOp::Restore]);
        scratch.clear();
        assert!(scratch.ops.is_empty());
        assert!(scratch.subtrees.is_empty());
    }
}
