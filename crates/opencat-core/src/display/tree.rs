use crate::display::list::{DisplayClip, DisplayItem, DisplayTransform, DrawScriptDisplayItem};
use crate::layout::tree::LayoutOutputFingerprint;
use crate::resolve::tree::ElementId;
use crate::semantic::fingerprint::ElementInputFingerprints;
use crate::canvas::paint::ImageFilterSpec;

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
    pub paint_clip: Option<DisplayClip>,
    pub clip: Option<DisplayClip>,
    pub item: DisplayItem,
    pub children: Vec<DisplayNode>,
    pub draw_slot: Option<DrawScriptDisplayItem>,
    pub hidden_subtree: Vec<HiddenChildDisplayNode>,
}
