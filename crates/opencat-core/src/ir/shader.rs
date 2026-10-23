//! Shader IR —— `shaders` 库组件的帧级声明（与 [`scene3`](super::scene3) 并列）。
//!
//! 分工与 three 完全对称：脚本侧只声明「哪块 canvas 上有哪些 shader 层、每层
//! 是哪个组件、props 是什么」；`props → WGSL` 的烘焙由 `opencat-shaders`
//! （engine 侧）做，web 侧直接引 shaders 原库跑。所以本段**不携带 WGSL**，
//! 只携带组件名 + props + 合成顺序。
//!
//! 像素仍走 2D 指令集：core 为每个 target 录一条空 RGBA 的
//! `DrawOp::Image { ImageRef::Generated }` 占位（内容寻址 id
//! `shader:{canvas}:{hash}`），宿主把光栅化结果回填进
//! [`FrameMediaPlan::generated_images`](super::media_plan::FrameMediaPlan)
//! 的同名条目——z 序即占位 op 在 canvas 命令流里的位置。

use serde::{Deserialize, Serialize};

/// 一帧的 shader 段（可能有多个 canvas 各自有 shader 层）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct ShaderSection {
    pub targets: Vec<ShaderTarget>,
}

/// 一块 canvas 上的 shader 合成。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShaderTarget {
    /// 占位生成图 id（`shader:{canvas_id}:{hash}`），回填时按它找 media plan 条目。
    pub image_id: crate::ir::GeneratedImageId,
    /// 承载它的 canvas 节点 id（决定占位 `DrawOp::Image` 的合成位置）。
    pub canvas_id: String,
    /// 渲染目标尺寸（px）。
    pub width: u32,
    pub height: u32,
    /// Shader clock for this frame (`f(t)`) — the clock of the hook that
    /// declared the stack, not the composition clock. Bakers turn it into
    /// each layer's `_animTime` via that layer's `speed`.
    pub time: f32,
    /// 合成顺序：先画的在前，依次叠加（`Vec` 顺序即 z 序，没有单独字段）。
    pub layers: Vec<ShaderLayer>,
}

/// 一层 shader。
///
/// `kind` 决定它怎么吃输入：
/// - `generator`：无输入，自己产像素（Aurora、MeshGradient）。
/// - `filter`：吃它前面的兄弟层合成结果（VHS）。
/// - `warp`：同时吃兄弟层与自身的 UV 弯折（ZoomBlur）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShaderLayer {
    /// shaders 库里的组件名（`Aurora`、`MeshGradient`、`VHS`、`ZoomBlur`…）。
    pub component: String,
    /// 逐帧驱动的 props（颜色、尺寸这类静态 prop 也在这里——IR 只带值，
    /// 不带「哪些是 compileTime」的区别）。
    pub props: serde_json::Value,
}

impl ShaderSection {
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
}
