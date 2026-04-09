use std::sync::Arc;

use anyhow::{Result, anyhow};

use crate::{frame_ctx::FrameCtx, scene::{node::Node, primitives::AudioSource}};

type RootComponent = dyn Fn(&FrameCtx) -> Node + Send + Sync;

#[derive(Clone)]
pub struct Composition {
    pub id: String,
    pub width: i32,
    pub height: i32,
    pub fps: u32,
    pub frames: u32,
    pub(crate) root: Arc<RootComponent>,
    pub(crate) global_audio_sources: Arc<Vec<AudioSource>>,
}

pub struct CompositionBuilder {
    id: String,
    width: i32,
    height: i32,
    fps: u32,
    frames: Option<u32>,
    root: Option<Arc<RootComponent>>,
    global_audio_sources: Vec<AudioSource>,
}

impl Composition {
    pub fn new(id: impl Into<String>) -> CompositionBuilder {
        CompositionBuilder {
            id: id.into(),
            width: 1920,
            height: 1080,
            fps: 30,
            frames: None,
            root: None,
            global_audio_sources: Vec::new(),
        }
    }

    pub fn root_node(&self, ctx: &FrameCtx) -> Node {
        (self.root)(ctx)
    }

    pub(crate) fn global_audio_sources(&self) -> &[AudioSource] {
        self.global_audio_sources.as_ref()
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
            frames: self.frames,
            root: self.root.clone(),
            global_audio_sources: self.global_audio_sources.clone(),
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

    pub fn frames(mut self, frames: u32) -> Self {
        self.frames = Some(frames);
        self
    }

    pub fn root<F>(mut self, root: F) -> Self
    where
        F: Fn(&FrameCtx) -> Node + Send + Sync + 'static,
    {
        self.root = Some(Arc::new(root));
        self
    }

    pub fn global_audio_sources<I>(mut self, sources: I) -> Self
    where
        I: IntoIterator<Item = AudioSource>,
    {
        self.global_audio_sources = sources.into_iter().collect();
        self
    }

    pub fn build(self) -> Result<Composition> {
        let root = self
            .root
            .ok_or_else(|| anyhow!("composition root is required"))?;

        let frames = if let Some(frames) = self.frames {
            frames
        } else {
            let probe_ctx = FrameCtx {
                frame: 0,
                fps: self.fps,
                width: self.width,
                height: self.height,
                frames: 0,
            };

            root(&probe_ctx)
                .duration_in_frames(&probe_ctx)
                .unwrap_or(150)
        };

        Ok(Composition {
            id: self.id,
            width: self.width,
            height: self.height,
            fps: self.fps,
            frames,
            root,
            global_audio_sources: Arc::new(self.global_audio_sources),
        })
    }
}

fn align_to_even(value: i32) -> i32 {
    value + (value & 1)
}
