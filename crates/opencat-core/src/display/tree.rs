use crate::core::display::list::{DisplayClip, DisplayItem, DisplayTransform};
use crate::core::element::tree::ElementId;

#[derive(Clone, Debug)]
pub struct DisplayTree {
    pub root: DisplayNode,
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
}
