//! Coarse export contracts shared by native and web exporters.

use anyhow::Result;

use crate::media::codec::VideoFrameRgba;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportKind {
    Mp4,
    PngSequence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExportJob {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub frames: u32,
    pub kind: ExportKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExportProgress {
    pub completed_frames: u32,
    pub total_frames: u32,
}

impl ExportProgress {
    pub fn ratio(&self) -> f64 {
        if self.total_frames == 0 {
            1.0
        } else {
            self.completed_frames as f64 / self.total_frames as f64
        }
    }
}

pub trait ExportFrameSource {
    fn frame_rgba(&mut self, frame_index: u32) -> Result<VideoFrameRgba>;
}
