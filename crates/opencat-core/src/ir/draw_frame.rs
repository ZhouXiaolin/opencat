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
/// runtime effects, **and full generated-image RGBA**). All halves are a pure
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
    /// GPU 命令流段（`getContext('webgl2')` 录制面产出；占位 DrawOp 的像素
    /// 由执行宿主渲染流后回填 media plan——契约见 `ir::gpu_stream`）。
    pub gpu: Option<crate::ir::gpu_stream::GpuSection>,
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
