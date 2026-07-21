use crate::style::{NodeStyle, impl_node_style_api};
use crate::{Node, media::VideoFrameTiming};

/// Video source locator. Paths are **logical** (document-relative strings), not
/// host filesystem paths — core never joins a base directory or stores `PathBuf`.
/// Hosts interpret `Path` against their own document base (FS, VFS, URL).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum VideoSource {
    /// Logical locator (e.g. `"clips/a.mp4"`). Not a resolved filesystem path.
    Path(String),
    Url(String),
}

#[derive(Clone)]
pub struct Video {
    source: VideoSource,
    timing: VideoFrameTiming,
    children: Vec<Node>,
    pub(crate) style: NodeStyle,
}

impl Video {
    pub fn child<T: Into<Node>>(mut self, child: T) -> Self {
        self.children.push(child.into());
        self
    }

    pub fn children_ref(&self) -> &[Node] {
        &self.children
    }

    pub(crate) fn set_children(&mut self, children: Vec<Node>) {
        self.children = children;
    }

    pub fn source(&self) -> &VideoSource {
        &self.source
    }

    pub fn timing(&self) -> VideoFrameTiming {
        self.timing
    }

    pub fn media_offset_secs(mut self, offset_secs: f64) -> Self {
        self.timing.media_start_secs = offset_secs.max(0.0);
        self
    }

    pub fn with_timing(mut self, timing: VideoFrameTiming) -> Self {
        self.timing = timing;
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

/// Build a video node from a logical path locator.
pub fn video(path: impl Into<String>) -> Video {
    Video {
        source: VideoSource::Path(path.into()),
        timing: VideoFrameTiming::default(),
        children: Vec::new(),
        style: NodeStyle::default(),
    }
}

pub fn video_url(url: impl Into<String>) -> Video {
    Video {
        source: VideoSource::Url(url.into()),
        timing: VideoFrameTiming::default(),
        children: Vec::new(),
        style: NodeStyle::default(),
    }
}

impl_node_style_api!(Video);
