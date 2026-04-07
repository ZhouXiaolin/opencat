use crate::display::list::{DisplayClip, DisplayItem, DisplayTransform};

#[derive(Clone, Debug)]
pub struct DisplayTree {
    pub root: DisplayNode,
}

#[derive(Clone, Debug)]
pub struct DisplayNode {
    pub transform: DisplayTransform,
    pub opacity: f32,
    pub clip: Option<DisplayClip>,
    pub item: DisplayItem,
    pub children: Vec<DisplayNode>,
}
