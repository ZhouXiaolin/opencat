use std::path::PathBuf;

use crate::style::{NodeStyle, impl_node_style_api};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LottieSource {
    Unset,
    Path(PathBuf),
    Url(String),
}

#[derive(Clone)]
pub struct Lottie {
    source: LottieSource,
    pub(crate) style: NodeStyle,
}

impl Lottie {
    pub fn source(&self) -> &LottieSource {
        &self.source
    }

    pub fn path(mut self, path: impl AsRef<std::path::Path>) -> Self {
        self.source = LottieSource::Path(path.as_ref().to_path_buf());
        self
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.source = LottieSource::Url(url.into());
        self
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }
}

impl_node_style_api!(Lottie);

pub fn lottie() -> Lottie {
    Lottie {
        source: LottieSource::Unset,
        style: NodeStyle::default(),
    }
}