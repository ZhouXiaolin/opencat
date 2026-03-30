use std::path::{Path, PathBuf};

use crate::style::{impl_node_style_api, NodeStyle};

#[derive(Clone)]
pub struct Image {
    source: PathBuf,
    pub(crate) style: NodeStyle,
}

impl Image {
    pub fn source(&self) -> &Path {
        &self.source
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }
}

pub fn image(path: impl AsRef<Path>) -> Image {
    Image {
        source: path.as_ref().to_path_buf(),
        style: NodeStyle::default(),
    }
}

impl_node_style_api!(Image);
