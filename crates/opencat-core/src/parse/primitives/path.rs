use crate::style::{NodeStyle, impl_node_style_api};

#[derive(Clone)]
pub struct Path {
    pub(crate) data: String,
    /// Explicit SVG viewBox `[min_x, min_y, width, height]`. When set, the path
    /// is fitted into the node box through this view box instead of the
    /// per-path bounds, so several sibling paths can share one coordinate
    /// space (the `view-box="minx,miny,w,h"` XML attribute).
    pub(crate) view_box: Option<[f32; 4]>,
    pub(crate) style: NodeStyle,
}

impl Path {
    pub fn data(&self) -> &str {
        &self.data
    }

    pub fn view_box(&self) -> Option<[f32; 4]> {
        self.view_box
    }

    pub fn set_view_box(&mut self, view_box: [f32; 4]) {
        self.view_box = Some(view_box);
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }
}

pub fn path(data: impl Into<String>) -> Path {
    Path {
        data: data.into(),
        view_box: None,
        style: NodeStyle::default(),
    }
}

impl_node_style_api!(Path);
