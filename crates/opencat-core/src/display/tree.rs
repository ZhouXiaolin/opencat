use crate::display::list::{
    DisplayClip, DisplayItem, DisplayTransform, DrawScriptDisplayItem,
};
use crate::resolve::tree::ElementId;

#[derive(Clone, Debug)]
pub struct DisplayTree {
    pub root: DisplayNode,
}

#[derive(Clone, Debug)]
pub struct HiddenChildDisplayNode {
    pub node: DisplayNode,
    pub owner_id: String,
}

#[derive(Clone, Debug)]
pub struct DisplayNode {
    pub element_id: ElementId,
    pub transform: DisplayTransform,
    pub opacity: f32,
    pub backdrop_blur_sigma: Option<f32>,
    pub clip: Option<DisplayClip>,
    pub item: DisplayItem,
    pub children: Vec<DisplayNode>,
    pub draw_slot: Option<DrawScriptDisplayItem>,
    pub hidden_subtree: Vec<HiddenChildDisplayNode>,
}
