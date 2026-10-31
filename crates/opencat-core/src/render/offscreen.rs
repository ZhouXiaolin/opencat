//! 离屏 canvas 的脚本期光栅化准备。
//!
//! `document.createElement('canvas')` 的 2D 绘制由 context2d.js 录成标准
//! `DrawOp` 流（与 DOM canvas 走同一套 `__canvas_*` 绑定），存进
//! `MutationStore::canvases`（未 resolve 的 id 就停在那里）。消费点
//! （getImageData / toDataURL / createPattern / drawImage(canvas)）在脚本期
//! 同步要像素，此时走本模块：把录制的哨兵 paint 命令流折叠成规范
//! `DrawOpFrame`（独立 `DrawOpBuilder`，`execute_draw_op` 与主帧同一条折叠
//! 路径），engine 侧再用 Skia `surfaces::raster` 重放 + read_pixels 回读。
//!
//! 零新 IR op：主帧只见标准 ImageRect{Generated} / FillPattern。

use crate::render::builder::DrawOpBuilder;
use crate::render::helpers::{execute_draw_op, LocalPaintState};
use crate::script::mutations::CanvasMutations;

/// 折叠离屏 canvas 命令流为规范 [`DrawOpFrame`]。
///
/// 输入是 `CanvasMutations` 的原始脚本命令（含哨兵 paint、ScriptImage、
/// ScriptSetLineDash 等 pre-intern 形态）；输出可直接交给 engine 的
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
    // （无 Clear 时 Save+AddRect+ClipPath；脚本 getImageData 语义是
    // "读回你画的东西"，画布边界外浏览器补透明）。
    let needs_alpha_layer = canvas
        .commands
        .iter()
        .any(|cmd| matches!(cmd, crate::ir::draw_op::DrawOp::Clear { .. }));
    if needs_alpha_layer {
        builder.push(crate::ir::draw_op::DrawOp::SaveLayer {
            bounds: Some(crate::ir::draw_op::Rect4 {
                x: 0.0,
                y: 0.0,
                width,
                height,
            }),
            paint: None,
            alpha: 1.0,
            backdrop: false,
        });
    } else {
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
    }

    for command in &canvas.commands {
        match command {
            // 离屏 2D 流不含子树/运行时效果（那些来自 DOM 树展开）。
            // ScriptImage 在这里就地翻译（主帧由 execute_draw_op 的
            // intern_bytes 路径翻译，离屏没有 RenderCtx/字节表，走 Skia
            // replay 的 Encoded 路径需要 frame 自带字节段）。
            crate::ir::draw_op::DrawOp::DrawSubtreePicture { .. }
            | crate::ir::draw_op::DrawOp::ScriptRuntimeEffect { .. } => {}
            _ => execute_draw_op(&mut builder, command, &mut state)?,
        }
    }

    builder.push(crate::ir::draw_op::DrawOp::Restore);
    Ok(builder.finish())
}
