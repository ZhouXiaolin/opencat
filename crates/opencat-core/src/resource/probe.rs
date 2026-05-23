pub use crate::probe::probe::{probe_image as probe_image_dims, probe_video};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageDims {
    pub width: u32,
    pub height: u32,
}

impl From<crate::probe::catalog::ImageMeta> for ImageDims {
    fn from(m: crate::probe::catalog::ImageMeta) -> Self {
        Self {
            width: m.width,
            height: m.height,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoProbe {
    pub width: u32,
    pub height: u32,
    pub duration_secs: Option<f64>,
}

impl From<crate::probe::catalog::VideoInfoMeta> for VideoProbe {
    fn from(m: crate::probe::catalog::VideoInfoMeta) -> Self {
        Self {
            width: m.width,
            height: m.height,
            duration_secs: m.duration_ms.map(|ms| ms as f64 / 1000.0),
        }
    }
}
