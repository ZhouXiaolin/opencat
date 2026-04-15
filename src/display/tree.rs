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

impl DisplayNode {
    pub fn layer_bounds(&self) -> crate::display::list::DisplayRect {
        let mut bounds = self.item.visual_bounds();
        for child in &self.children {
            let child_bounds = child
                .layer_bounds()
                .translate(child.transform.translation_x, child.transform.translation_y);
            bounds = bounds.union(child_bounds);
        }
        bounds
    }
}
