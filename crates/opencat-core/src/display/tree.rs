use crate::analyze::fingerprint::SubtreeSnapshotFingerprint;
use crate::canvas::paint::ImageFilterSpec;
use crate::display::list::{DisplayClip, DisplayItem, DisplayRect, DisplayTransform, DrawScriptDisplayItem};
use crate::layout::tree::LayoutOutputFingerprint;
use crate::resolve::tree::ElementId;
use crate::semantic::fingerprint::ElementInputFingerprints;

#[derive(Clone, Debug)]
pub struct DisplayTree {
    pub root: DisplayNode,
}

#[derive(Clone, Debug)]
pub struct HiddenChildDisplayNode {
    pub node: DisplayNode,
    pub owner_id: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DisplayRecordedSubtreeFingerprint(pub u64);

#[derive(Clone, Debug)]
pub struct DisplayNode {
    pub element_id: ElementId,
    pub input_fingerprints: ElementInputFingerprints,
    pub layout_output_fingerprint: LayoutOutputFingerprint,
    pub recorded_subtree_fingerprint: DisplayRecordedSubtreeFingerprint,
    pub transform: DisplayTransform,
    pub opacity: f32,
    /// `filter` 折算产物（build 期经 `css_filter_image_filter`）；
    /// display 层不携带 style 词汇（H1）。
    pub css_filter_image: Option<ImageFilterSpec>,
    pub backdrop_blur_sigma: Option<f32>,
    /// CSS `backdrop-filter` 非 blur 算子的折算产物（blur 走
    /// `backdrop_blur_sigma`）。
    pub backdrop_filter_image: Option<ImageFilterSpec>,
    /// CSS `mask-image` 折算出的渐变填充；绘制期以 DstIn 罩回本元素层。
    pub mask_image: Option<crate::style::BackgroundFill>,
    pub paint_clip: Option<DisplayClip>,
    pub clip: Option<DisplayClip>,
    pub item: DisplayItem,
    /// 合成层 bounds（build 期折算）：自身 visual bounds ∪ children 平移后
    /// bounds，再按 `filter: drop-shadow()` 外扩（extent = σ·3）。渲染期
    /// SaveLayer 用；3-D 子树在渲染端降级为无界。
    pub layer_bounds: DisplayRect,
    /// 子树 paint 指纹（渲染期自底向上填充，见
    /// `analyze::fingerprint::compute_display_tree_fingerprints`）。
    pub paint_fingerprint: Option<u64>,
    /// 子树 picture 缓存键指纹（与 paint_fingerprint 同轮填充）。
    pub snapshot_fingerprint: Option<SubtreeSnapshotFingerprint>,
    pub children: Vec<DisplayNode>,
    pub draw_slot: Option<DrawScriptDisplayItem>,
    pub hidden_subtree: Vec<HiddenChildDisplayNode>,
}
