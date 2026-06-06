use std::hash::{Hash, Hasher};
use std::sync::Arc;

use anyhow::{Result, anyhow};

use crate::{
    frame_ctx::{FrameCtx, duration_secs_to_frames, frames_to_duration_secs},
    parse::{node::Node, primitives::AudioSource},
};

type RootComponent = dyn Fn(&FrameCtx) -> Node + Send + Sync;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AudioAttachment {
    Timeline,
    Scene { scene_id: String },
}

#[derive(Clone, Debug)]
pub struct CompositionAudioSource {
    pub id: String,
    pub source: AudioSource,
    pub attach: AudioAttachment,
    pub duration_secs: Option<f64>,
}

impl PartialEq for CompositionAudioSource {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.source == other.source
            && self.attach == other.attach
            && self.duration_secs.map(f64::to_bits) == other.duration_secs.map(f64::to_bits)
    }
}

impl Eq for CompositionAudioSource {}

impl Hash for CompositionAudioSource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.source.hash(state);
        self.attach.hash(state);
        self.duration_secs.map(f64::to_bits).hash(state);
    }
}

impl CompositionAudioSource {
    pub fn timeline(id: impl Into<String>, source: AudioSource) -> Self {
        Self {
            id: id.into(),
            source,
            attach: AudioAttachment::Timeline,
            duration_secs: None,
        }
    }

    pub fn scene(id: impl Into<String>, source: AudioSource, scene_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            source,
            attach: AudioAttachment::Scene {
                scene_id: scene_id.into(),
            },
            duration_secs: None,
        }
    }

    pub fn with_duration(mut self, duration_secs: Option<f64>) -> Self {
        self.duration_secs = duration_secs;
        self
    }
}

#[derive(Clone)]
pub struct Composition {
    pub id: String,
    pub width: i32,
    pub height: i32,
    pub fps: u32,
    pub duration: f64,
    pub frames: u32,
    pub root: Arc<RootComponent>,
    pub audio_sources: Arc<Vec<CompositionAudioSource>>,
}

enum DurationSpec {
    Seconds(f64),
    Frames(u32),
}

pub struct CompositionBuilder {
    id: String,
    width: i32,
    height: i32,
    fps: u32,
    duration: Option<DurationSpec>,
    root: Option<Arc<RootComponent>>,
    audio_sources: Vec<CompositionAudioSource>,
}

impl Composition {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(id: impl Into<String>) -> CompositionBuilder {
        CompositionBuilder {
            id: id.into(),
            width: 1920,
            height: 1080,
            fps: 30,
            duration: None,
            root: None,
            audio_sources: Vec::new(),
        }
    }

    pub fn root_node(&self, ctx: &FrameCtx) -> Node {
        (self.root)(ctx)
    }

    pub fn has_audio_sources(&self) -> bool {
        !self.audio_sources.is_empty()
    }

    pub fn audio_sources(&self) -> &[CompositionAudioSource] {
        self.audio_sources.as_ref()
    }

    pub fn aligned_for_video_encoding(&self) -> Composition {
        let aligned_width = align_to_even(self.width.max(1));
        let aligned_height = align_to_even(self.height.max(1));
        if aligned_width == self.width && aligned_height == self.height {
            return self.clone();
        }

        Composition {
            id: self.id.clone(),
            width: aligned_width,
            height: aligned_height,
            fps: self.fps,
            duration: self.duration,
            frames: self.frames,
            root: self.root.clone(),
            audio_sources: self.audio_sources.clone(),
        }
    }
}

impl CompositionBuilder {
    pub fn size(mut self, width: i32, height: i32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn fps(mut self, fps: u32) -> Self {
        self.fps = fps;
        self
    }

    pub fn duration(mut self, duration_secs: f64) -> Self {
        self.duration = Some(DurationSpec::Seconds(duration_secs));
        self
    }

    pub fn frames(mut self, frames: u32) -> Self {
        self.duration = Some(DurationSpec::Frames(frames));
        self
    }

    pub fn root<F>(mut self, root: F) -> Self
    where
        F: Fn(&FrameCtx) -> Node + Send + Sync + 'static,
    {
        self.root = Some(Arc::new(root));
        self
    }

    pub fn audio_sources<I>(mut self, sources: I) -> Self
    where
        I: IntoIterator<Item = CompositionAudioSource>,
    {
        self.audio_sources = sources.into_iter().collect();
        self
    }

    pub fn global_audio_sources<I>(mut self, sources: I) -> Self
    where
        I: IntoIterator<Item = AudioSource>,
    {
        self.audio_sources = sources
            .into_iter()
            .enumerate()
            .map(|(index, source)| {
                CompositionAudioSource::timeline(format!("audio-{index}"), source)
            })
            .collect();
        self
    }

    pub fn build(self) -> Result<Composition> {
        let root = self
            .root
            .ok_or_else(|| anyhow!("composition root is required"))?;

        let (duration, frames) = match self.duration {
            Some(DurationSpec::Seconds(duration)) => {
                (duration, duration_secs_to_frames(duration, self.fps))
            }
            Some(DurationSpec::Frames(frames)) => (frames_to_duration_secs(frames, self.fps), frames),
            None => {
                let probe_ctx = FrameCtx {
                    frame: 0,
                    fps: self.fps,
                    width: self.width,
                    height: self.height,
                    frames: 0,
                };

                let frames = root(&probe_ctx)
                    .duration_in_frames(&probe_ctx)
                    .unwrap_or(150);
                (frames_to_duration_secs(frames, self.fps), frames)
            }
        };

        Ok(Composition {
            id: self.id,
            width: self.width,
            height: self.height,
            fps: self.fps,
            duration,
            frames,
            root,
            audio_sources: Arc::new(self.audio_sources),
        })
    }
}

fn align_to_even(value: i32) -> i32 {
    value + (value & 1)
}
