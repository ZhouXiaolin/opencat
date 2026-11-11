#[derive(Clone, Copy, Debug)]
pub struct LayoutRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// 行内格式化里跨行文本 run 的一行片段（`layout::inline`）。
#[derive(Clone, Copy, Debug)]
pub struct TextFragment {
    /// 文本元素原文的字节区间 `[start, end)`。
    pub start: usize,
    pub end: usize,
    /// 片段行盒，相对所属文本节点 rect 原点：x/width 是前进区间，
    /// y/height 是该 run 自身行高的行盒（单行绘制时基线随之对齐）。
    pub rect: LayoutRect,
}

#[derive(Clone, Debug)]
pub struct LayoutNode {
    pub id: String,
    pub rect: LayoutRect,
    pub output_fingerprint: LayoutOutputFingerprint,
    pub children: Vec<LayoutNode>,
    /// 文本 run 在行内流里跨行时的逐行片段；`None` = 整段单行/自身折行。
    pub inline_fragments: Option<std::sync::Arc<[TextFragment]>>,
    /// 末行基线（相对 rect 顶）。行内流容器与跨行文本 run 记录；其余节点
    /// 由消费方按需推导（原子行内盒的基线对齐）。
    pub baseline: Option<f32>,
    /// 文本叶自身折行的基准宽：取整前的盒宽（非文本节点 `None`）。布局按
    /// 取整前的宽量行数；`rect.width` 取整后可能短不足 1px，拿它折行会把
    /// 恰好放下的末词挤到下一行。渲染必须与测量同口径。
    pub wrap_width: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct LayoutTree {
    pub root: LayoutNode,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LayoutOutputFingerprint {
    pub local: u64,
    pub subtree: u64,
    pub record_size: u64,
}
