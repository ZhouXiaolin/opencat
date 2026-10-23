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

/// 一帧 3D 场景 IR 的一次提交：canvas id + 该帧的 [`Scene3`](super::scene3::Scene3)
/// 快照 + draw 流里占位 `DrawOp::Image { ImageRef::Generated }` 引用的生成图 id。
///
/// 执行契约：宿主按 `image_id` 把光栅化结果回填进
/// [`FrameMediaPlan::generated_images`](super::media_plan::FrameMediaPlan)
/// 的同名占位条目（CPU 像素或 GPU 句柄），占位 `DrawOp` 的 z 序即 3D 层的
/// 合成位置——engine 用 opencat-three 执行，web 用真 three.js 执行。
#[derive(Clone, Debug, PartialEq)]
pub struct Scene3Commit {
    pub image_id: crate::ir::GeneratedImageId,
    pub target_id: String,
    pub scene: super::scene3::Scene3,
}

/// `RenderFrame` 的 3D 段（与 2D `draw` / `media` 并列的跨边界 IR）。
///
/// `geometries` 是 realm 级几何注册表的轻量快照（`BTreeMap<String, Arc>`
/// 克隆只做指针自增），`scene` 里 `Geometry3::Asset { id }` 的解析来源；
/// 重网格（GLB、高细分球）不逐帧克隆。占位生成图的像素不在本段——本段是
/// 纯声明 IR，像素由执行器（engine / web facade）产出。
#[derive(Clone, Debug, PartialEq)]
pub struct Scene3Section {
    pub commits: Vec<Scene3Commit>,
    pub geometries: super::scene3::GeometryRegistry,
}

/// The single deterministic per-frame output contract of the render pipeline.
///
/// `draw` is the precise draw-IR for this frame; `media` is the host-facing
/// media preparation plan for this frame (images, video frames, Lottie bundles,
/// runtime effects, **and full generated-image RGBA**); `scene3` is the 3D
/// scene IR section (declared snapshots + placeholder generated-image ids the
/// executing host rasterizes into the plan), `shader` is the shaders-library
/// section (declared layer stacks + placeholder ids the host bakes). All halves are a pure
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
    pub scene3: Option<Scene3Section>,
    pub shader: Option<super::shader::ShaderSection>,
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
