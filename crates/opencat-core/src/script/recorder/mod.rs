//! Mutation recorder trait + default `MutationStore` implementation.
//!
//! 统一的脚本→宿主 mutation 录入面（JS binding 桥与 OMP 命令流共用）。
//! **style 键不走这里**：JS 全走 `write_style_value` 字符串通用口（store 词
//! 表收归 CSS 标准，见 M2-②），类型化 style record_* 搬运层已随 M2-①
//! 退役（`run_frame` 直接返回 `StyleMutations` 层序列，合并统一在 resolve
//! 侧 `apply_mutation_stack`）。本 trait 只承载：canvas / child_override
//! 命令面 + 帧状态与查询面。

mod store;

pub use store::MutationStore;

use crate::ir::draw_op::DrawOp;
use crate::script::ScriptTextSource;
use crate::script::mutations::StyleMutations;

/// Recorder receives mutation calls from the JS bridge layer.
///
/// The default implementation [`MutationStore`] accumulates everything in
/// HashMaps; platform-specific impls (e.g. wasm-bindgen wrapper) typically
/// delegate to the default impl after argument conversion.
pub trait MutationRecorder {
    /// canonical DrawOp 直录（绝大多数绑定走这里；内部包成
    /// [`CanvasOp::Op`] 透传）。
    fn record_draw_op(&mut self, id: &str, cmd: DrawOp) {
        self.record_canvas_op(id, crate::script::canvas_op::CanvasOp::Op(cmd));
    }

    /// canvas 2D 录制面命令：canonical 透传之外，还有翻译层才消化的
    /// 浏览器语义 / 裸 payload 形态（shadow 状态机、putImageData、
    /// 预驻留渐变等）。见 `script::canvas_op`。
    fn record_canvas_op(&mut self, id: &str, cmd: crate::script::canvas_op::CanvasOp);

    /// canvas 位图重置（HTML：`canvas.width/height` 赋值清空位图）：丢弃该
    /// 节点本帧已录的全部命令。默认 no-op：不录 canvas 命令的录制器无需关心。
    fn reset_canvas(&mut self, id: &str) {
        let _ = id;
    }

    fn record_draw_picture(&mut self, target_id: &str, owner_id: &str, x: f32, y: f32);

    /// 本帧某个 canvas 节点录到了 SVG markup（g-svg 渲染器产 `<svg>` 子树而
    /// 不产 DrawOp）。录进 canvas 条目的 `svg` 槽，resolve 时该节点按内联
    /// SVG 解析。默认 no-op：不消费 SVG 的录制器无需关心。
    fn record_canvas_svg(&mut self, canvas_id: &str, markup: String) {
        let _ = (canvas_id, markup);
    }

    /// 运行期 `appendChild` 上报的宿主子树覆盖（html 通道）。默认 no-op：
    /// 不承载 DOM 结构的录制器（布局探针、预计算快照）无需关心。
    fn record_child_override(
        &mut self,
        _host_id: &str,
        _override: crate::script::mutations::ChildOverride,
    ) {
    }

    fn record_canvas_runtime_effect(
        &mut self,
        id: &str,
        sksl: String,
        uniforms_bytes: Vec<u8>,
        children: Vec<crate::ir::draw_types::ScriptRuntimeEffectChild>,
        dst: crate::ir::draw_op::Rect4,
    );

    fn reset_for_frame(&mut self, current_frame: u32, fps: u32);
    fn snapshot_mutations(&self) -> StyleMutations;

    fn read_style_value(&self, id: &str, property: &str) -> Option<serde_json::Value>;
    fn write_style_value(&mut self, id: &str, property: &str, value: serde_json::Value);

    fn register_text_source(&mut self, id: &str, source: ScriptTextSource);
    fn clear_text_sources(&mut self);
    fn get_text_source(&self, id: &str) -> Option<&ScriptTextSource>;

    /// Script-side DOM overlay (dynamic nodes + per-frame child views).
    /// Recorders that do not model a tree return `None` and the tree bindings
    /// report that they are unavailable.
    fn tree_overlay(&self) -> Option<&crate::script::tree::TreeOverlay> {
        None
    }
    fn tree_overlay_mut(&mut self) -> Option<&mut crate::script::tree::TreeOverlay> {
        None
    }

    /// Resolved text style per declared node, from the declarative tree.
    /// `getBoundingClientRect` shapes text to answer a box, and shaping needs
    /// the style; recorders that do not shape keep the default no-op.
    fn set_text_styles(
        &mut self,
        _styles: &std::collections::HashMap<String, crate::style::ComputedTextStyle>,
    ) {
    }
    fn text_style(&self, _id: &str) -> Option<&crate::style::ComputedTextStyle> {
        None
    }
    fn effective_text_style(&self, _id: &str) -> crate::style::ComputedTextStyle {
        Default::default()
    }

    /// 合成帧视口（`cqw`/`cqh` 写入折 px 的基准）。每帧随 frame_ctx 注入；
    /// 不 shape 的 recorder 保持默认 no-op。
    fn set_viewport(&mut self, _width: f32, _height: f32) {}

    /// 声明文本节点下所有动态叶子的字节区间（含祖先链）。渲染桥据此把
    /// wrapper 上的样式变更换算成该元素的 `TextUnitOverrideBatch`。
    fn text_leaf_ranges(&mut self, _id: &str) -> Vec<(Vec<String>, usize, usize)> {
        Vec::new()
    }
    fn current_frame(&self) -> u32;
    /// Frame used to evaluate tween progress: the scene-local override when set
    /// (timeline windows), otherwise the composition frame.
    fn script_frame(&self) -> u32;
    fn set_frame_override(&mut self, frame: Option<u32>);
    /// Time in seconds used to evaluate tween progress. Kept separate from the
    /// frame because tween progress advances by elapsed time, and a `<timeline>` scene
    /// window can start between frames.
    fn script_time_secs(&self) -> f64;
    fn set_time_override_secs(&mut self, secs: Option<f64>);
}
