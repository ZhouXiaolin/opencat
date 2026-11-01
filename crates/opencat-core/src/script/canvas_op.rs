//! Canvas 2D 录制面命令（脚本面 → 翻译层 → 纯 `DrawOp` 流）。
//!
//! [`DrawOp`] 是渲染 IR，只保留近 Skia 语义的原语。canvas 2D 的浏览器
//! 语义（shadow 状态机、putImageData 直写、运行时效果）与录制面无法
//! intern 的裸 payload（图像字节、f32 stop 序列）在这里表达；帧构建期
//! 由 `lower_canvas_op`（render::helpers）翻译进 `DrawOpBuilder`
//! （intern + 展开），渲染面（engine replay / 二进制编码器 / web 解码器）
//! 只见 canonical `DrawOp`——枚举本身永远不带 canvas 语义。

use crate::ir::draw_op::{ColorU8, DrawOp, Rect4};
use crate::ir::draw_types::{ImageRef, ScriptRuntimeEffectChild};

#[derive(Clone, Debug, PartialEq)]
pub enum CanvasOp {
    /// canonical op 透传（绘制原语、save/restore、路径、渐变/虚线的
    /// canonical 形态……）。占绝大多数，直通 `execute_draw_op`。
    Op(DrawOp),

    /// `ctx.drawImage` 的编码图像字节：录制面没有帧字节表，翻译层
    /// intern 进 `ImageRef::Encoded` 后发标准 `ImageRect`。
    Image {
        bytes: Vec<u8>,
        src: Option<Rect4>,
        dst: Rect4,
        /// 浏览器 `globalAlpha` 在 drawImage 时刻的快照（ heatmap 逐点
        /// 图章这类调用依赖它做 alpha 合成）。
        alpha: f32,
    },

    /// `putImageData`：像素直写。翻译层 intern `PaintSpec{blend_mode: Src}`
    /// （替换而非叠加，防底色透出）后发标准 `ImageRect`。
    PutImage {
        image: ImageRef,
        dst: Rect4,
    },

    /// `setLineDash`：录制面没有 f32 池，翻译层 intern 区间后发标准
    /// `SetLineDash`（空列表的「实线」语义由绑定层转 `ClearLineDash`）。
    SetLineDash {
        intervals: Vec<f32>,
        phase: f32,
    },

    /// `createLinearGradient` 的裸 stops（平铺 offset,r,g,b,a 五元组），
    /// 翻译层 intern 后发标准 `SetStyleLinearGradient`。
    SetStyleLinearGradient {
        stroke: bool,
        start: [f32; 2],
        end: [f32; 2],
        stops: Vec<f32>,
    },

    /// `createRadialGradient` 的裸 stops，同上发 `SetStyleRadialGradient`。
    SetStyleRadialGradient {
        stroke: bool,
        start: [f32; 3],
        end: [f32; 3],
        stops: Vec<f32>,
    },

    /// 运行时效果（SKSL）：翻译层 intern effect/uniforms/children 后发
    /// canonical `RuntimeEffect`；`PictureSubtree` 子项需要 display 树，
    /// 由 render 循环就地解析（离屏路径直接跳过）。
    RuntimeEffect {
        sksl: String,
        uniforms_bytes: Vec<u8>,
        children: Vec<ScriptRuntimeEffectChild>,
        dst: Rect4,
    },

    /// canvas 阴影状态机（sticky）：翻译层记入漆状态，把后续
    /// fill/stroke 包进 `SaveLayer(DropShadow)`。
    SetShadow {
        color: ColorU8,
        blur: f32,
        dx: f32,
        dy: f32,
    },

    /// 组级阴影层（canvas pattern 填充）：翻译层展开为
    /// `Save + [Translate] + SaveLayer(DropShadow)`，调用方配平 Restore。
    BeginShadowLayer,
}

impl std::hash::Hash for CanvasOp {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            CanvasOp::Op(op) => {
                0_u8.hash(state);
                op.hash(state);
            }
            CanvasOp::Image { bytes, src, dst, alpha } => {
                1_u8.hash(state);
                bytes.hash(state);
                src.hash(state);
                dst.hash(state);
                alpha.to_bits().hash(state);
            }
            CanvasOp::PutImage { image, dst } => {
                2_u8.hash(state);
                image.hash(state);
                dst.hash(state);
            }
            CanvasOp::SetLineDash { intervals, phase } => {
                3_u8.hash(state);
                intervals.iter().for_each(|v| v.to_bits().hash(state));
                phase.to_bits().hash(state);
            }
            CanvasOp::SetStyleLinearGradient {
                stroke,
                start,
                end,
                stops,
            } => {
                4_u8.hash(state);
                stroke.hash(state);
                start.map(f32::to_bits).hash(state);
                end.map(f32::to_bits).hash(state);
                stops.iter().for_each(|v| v.to_bits().hash(state));
            }
            CanvasOp::SetStyleRadialGradient {
                stroke,
                start,
                end,
                stops,
            } => {
                5_u8.hash(state);
                stroke.hash(state);
                start.map(f32::to_bits).hash(state);
                end.map(f32::to_bits).hash(state);
                stops.iter().for_each(|v| v.to_bits().hash(state));
            }
            CanvasOp::RuntimeEffect {
                sksl,
                uniforms_bytes,
                children,
                dst,
            } => {
                6_u8.hash(state);
                sksl.hash(state);
                uniforms_bytes.hash(state);
                children.hash(state);
                dst.hash(state);
            }
            CanvasOp::SetShadow {
                color,
                blur,
                dx,
                dy,
            } => {
                7_u8.hash(state);
                color.hash(state);
                blur.to_bits().hash(state);
                dx.to_bits().hash(state);
                dy.to_bits().hash(state);
            }
            CanvasOp::BeginShadowLayer => {
                8_u8.hash(state);
            }
        }
    }
}
