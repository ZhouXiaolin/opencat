use std::path::{Path, PathBuf};

use crate::style::{NodeStyle, impl_node_style_api};

#[derive(Clone)]
pub struct Video {
    source: PathBuf,
    pub(crate) style: NodeStyle,
}

impl Video {
    pub fn source(&self) -> &Path {
        &self.source
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }
}

pub fn video(path: impl AsRef<Path>) -> Video {
    Video {
        source: path.as_ref().to_path_buf(),
        style: NodeStyle::default(),
    }
}

impl_node_style_api!(Video);
