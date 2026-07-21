use crate::media::VideoFrameTiming;
use crate::style::{NodeStyle, impl_node_style_api};

/// Lottie source locator. Paths are **logical** (document-relative strings), not
/// host filesystem paths — core never joins a base directory or stores `PathBuf`.
/// Hosts interpret `Path` against their own document base (FS, VFS, URL).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LottieSource {
    Unset,
    /// Logical locator (e.g. `"anim/loader.json"`). Not a resolved filesystem path.
    Path(String),
    Url(String),
}

#[derive(Clone)]
pub struct Lottie {
    source: LottieSource,
    timing: VideoFrameTiming,
    pub(crate) style: NodeStyle,
}

impl Lottie {
    pub fn source(&self) -> &LottieSource {
        &self.source
    }

    pub fn timing(&self) -> VideoFrameTiming {
        self.timing
    }

    pub fn with_timing(mut self, timing: VideoFrameTiming) -> Self {
        self.timing = timing;
        self
    }

    pub fn media_offset_secs(mut self, offset_secs: f64) -> Self {
        self.timing.media_start_secs = offset_secs.max(0.0);
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

    /// Set a logical path locator. Accepts any string-like value; does not
    /// resolve against a filesystem base.
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.source = LottieSource::Path(path.into());
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
        timing: VideoFrameTiming::default(),
        style: NodeStyle::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::{LottieSource, lottie};

    #[test]
    fn lottie_path_stores_logical_string() {
        let node = lottie().path("anim/loader.json");
        assert_eq!(
            node.source(),
            &LottieSource::Path("anim/loader.json".to_string())
        );
    }
}
