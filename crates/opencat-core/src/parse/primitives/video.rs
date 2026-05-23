use std::path::{Path, PathBuf};

use crate::resource::types::VideoFrameTiming;
use crate::style::{NodeStyle, impl_node_style_api};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum VideoSource {
    Path(PathBuf),
    Url(String),
}

#[derive(Clone)]
pub struct Video {
    source: VideoSource,
    timing: VideoFrameTiming,
    pub(crate) style: NodeStyle,
}

impl Video {
    pub fn source(&self) -> &VideoSource {
        &self.source
    }

    pub fn timing(&self) -> VideoFrameTiming {
        self.timing
    }

    pub fn media_offset_secs(mut self, offset_secs: f64) -> Self {
        self.timing.media_offset_secs = offset_secs.max(0.0);
        self
    }

    pub fn playback_rate(mut self, playback_rate: f64) -> Self {
        self.timing.playback_rate = playback_rate.max(0.000_001);
        self
    }

    pub fn looping(mut self, looping: bool) -> Self {
        self.timing.looping = looping;
        self
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }
}

pub fn video(path: impl AsRef<Path>) -> Video {
    Video {
        source: VideoSource::Path(path.as_ref().to_path_buf()),
        timing: VideoFrameTiming::default(),
        style: NodeStyle::default(),
    }
}

pub fn video_url(url: impl Into<String>) -> Video {
    Video {
        source: VideoSource::Url(url.into()),
        timing: VideoFrameTiming::default(),
        style: NodeStyle::default(),
    }
}

impl_node_style_api!(Video);
