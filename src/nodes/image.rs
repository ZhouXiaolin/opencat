use std::any::Any;
use std::path::{Path, PathBuf};

use crate::{
    ViewNode,
    style::{NodeStyle, impl_node_style_api},
};

#[derive(Clone)]
pub struct Image {
    source: PathBuf,
    pub(crate) style: NodeStyle,
}

impl Image {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            source: path.as_ref().to_path_buf(),
            style: NodeStyle::default(),
        }
    }

    pub fn source(&self) -> &Path {
        &self.source
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }
}

impl_node_style_api!(Image);

impl ViewNode for Image {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn style_ref(&self) -> &NodeStyle {
        &self.style
    }
}
