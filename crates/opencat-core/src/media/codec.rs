//! Backend-neutral decoded media buffers and source metadata.

use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub struct VideoSourceMeta {
    pub width: u32,
    pub height: u32,
    pub duration_secs: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct VideoFrameRgba {
    pub data: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioPcm {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl AudioPcm {
    pub fn new(sample_rate: u32, channels: u16, samples: Vec<f32>) -> Self {
        Self {
            sample_rate,
            channels,
            samples,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn sample_frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }
}
