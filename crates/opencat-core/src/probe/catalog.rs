use std::collections::{HashMap, HashSet};

use crate::parse::primitives::{AudioSource, ImageSource, SrtEntry, SubtitleSource};
pub use crate::parse::primitives::VideoSource;
pub use crate::resource::asset_id::AssetId;

#[derive(Default, Clone, Debug)]
pub struct ResourceRequests {
    pub images: HashSet<ImageSource>,
    pub videos: HashSet<VideoSource>,
    pub audios: HashSet<AudioSource>,
    pub subtitles: HashSet<SubtitleSource>,
}

#[derive(Default, Clone, Debug)]
pub struct ResourceCatalog {
    pub images: HashMap<AssetId, ImageMeta>,
    pub videos: HashMap<AssetId, VideoInfoMeta>,
    pub audios: HashSet<AssetId>,
    pub subtitles: HashMap<AssetId, Vec<SrtEntry>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageMeta {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoInfoMeta {
    pub width: u32,
    pub height: u32,
    pub duration_ms: Option<u64>,
}

#[derive(Default, Clone, Debug)]
pub struct AudioPlan {
    pub segments: Vec<AudioSegment>,
}

#[derive(Clone, Debug)]
pub struct AudioSegment {
    pub asset: AssetId,
    pub start_ms: u64,
    pub end_ms: u64,
}
