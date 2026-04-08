use std::path::{Path, PathBuf};

use crate::style::{NodeStyle, impl_node_style_api};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AudioSource {
    Unset,
    Path(PathBuf),
    Url(String),
}

#[derive(Clone)]
pub struct Audio {
    source: AudioSource,
    pub(crate) style: NodeStyle,
}

impl Audio {
    pub fn path(mut self, path: impl AsRef<Path>) -> Self {
        self.source = AudioSource::Path(path.as_ref().to_path_buf());
        self
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.source = AudioSource::Url(url.into());
        self
    }

    pub fn source(&self) -> &AudioSource {
        &self.source
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }
}

pub fn audio() -> Audio {
    Audio {
        source: AudioSource::Unset,
        style: NodeStyle::default(),
    }
}

impl_node_style_api!(Audio);

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{AudioSource, audio};

    #[test]
    fn audio_builder_sets_path_source() {
        let audio = audio().path("/tmp/demo.mp3");
        assert_eq!(
            audio.source(),
            &AudioSource::Path(PathBuf::from("/tmp/demo.mp3"))
        );
    }

    #[test]
    fn audio_builder_sets_url_source() {
        let audio = audio().url("https://example.com/demo.mp3");
        assert_eq!(
            audio.source(),
            &AudioSource::Url("https://example.com/demo.mp3".to_string())
        );
    }
}
