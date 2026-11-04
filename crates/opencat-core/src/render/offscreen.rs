//! 离屏 canvas 的脚本期光栅化准备。
//!
//! `document.createElement('canvas')` 的 2D 绘制由 context2d.js 录成标准
//! `DrawOp` 流（与 DOM canvas 走同一套 `__canvas_*` 绑定），存进
//! `MutationStore::canvases`（未 resolve 的 id 就停在那里）。消费点
//! （getImageData / toDataURL / createPattern / drawImage(canvas)）在脚本期
//! 同步要像素，此时走本模块：把录制的 canvas 命令流（`CanvasOp`）经翻译层
//! 折叠成规范 `DrawOpFrame`（独立 `DrawOpBuilder`，`lower_canvas_op` 与主帧
//! 同一条折叠路径），engine 侧再用 Skia `surfaces::raster` 重放 +
//! read_pixels 回读。
//!
//! 零新 IR op：主帧只见标准 ImageRect{Generated} / FillPattern。

use crate::render::builder::DrawOpBuilder;
use crate::render::{LocalPaintState, lower_canvas_op};
use crate::script::mutations::CanvasMutations;

/// 折叠离屏 canvas 命令流为规范 [`DrawOpFrame`]。
///
/// 输入是 `CanvasMutations` 的原始 canvas 命令（含哨兵 paint 与
/// `CanvasOp::Image`/`SetLineDash` 等预驻留形态）；输出可直接交给 engine 的
/// `replay_frame`。外层 clip 用画布自身 bounds（浏览器语义：canvas 元素的
/// 内容区裁剪；脚本绘制可以画出界，读回时界外为透明）。
pub fn compile_offscreen_commands(
    canvas: &CanvasMutations,
    width: f32,
    height: f32,
) -> Result<crate::ir::draw_frame::DrawOpFrame, crate::render::RenderError> {
    let mut builder = DrawOpBuilder::default();
    let mut state = LocalPaintState::default();

    // 内容区裁剪：与 render_draw_script 对 DOM canvas 的处理一致
    // （Save+AddRect+ClipPath；脚本 getImageData 语义是"读回你画的
    // 东西"，画布边界外浏览器补透明）。不用 SaveLayer 包裹：脚本流允许
    // 未配对 save（浏览器 save 栈深度不可观察，d3-cloud 的条带循环
    // break 就会泄漏一条），SaveLayer 依赖结尾配平 Restore 才合成，
    // 泄漏会把层压在栈里永不合成 → 读回全透明。
    builder.push(crate::ir::draw_op::DrawOp::Save);
    builder.push(crate::ir::draw_op::DrawOp::BeginPath);
    builder.push(crate::ir::draw_op::DrawOp::Path(
        crate::ir::draw_types::PathOp::AddRect {
            x: 0.0,
            y: 0.0,
            width,
            height,
        },
    ));
    builder.push(crate::ir::draw_op::DrawOp::ClipPath {
        anti_alias: true,
        fill_type: Default::default(),
    });

    for command in &canvas.commands {
        match command {
            // 离屏 2D 流不含子树/运行时效果（那些来自 DOM 树展开）。
            crate::script::canvas_op::CanvasOp::Op(crate::ir::draw_op::DrawOp::DrawSubtreePicture { .. })
            | crate::script::canvas_op::CanvasOp::RuntimeEffect { .. } => {}
            other => lower_canvas_op(&mut builder, other, &mut state)?,
        }
    }

    // 收尾不用单条 Restore：脚本流的 save 深度不可信（见上），弹回基础
    // 层（count=1）才保证不依赖流自身配平——浏览器读回与 save 栈深度无关。
    builder.push(crate::ir::draw_op::DrawOp::RestoreToCount { count: 1 });
    Ok(builder.finish())
}
