use std::sync::Arc;

use anyhow::{anyhow, Result};

use crate::{FrameCtx, Node};

type RootComponent = dyn Fn(&FrameCtx) -> Node + Send + Sync;

pub struct Composition {
    pub id: String,
    pub width: i32,
    pub height: i32,
    pub fps: u32,
    pub frames: u32,
    pub(crate) root: Arc<RootComponent>,
}

pub struct CompositionBuilder {
    id: String,
    width: i32,
    height: i32,
    fps: u32,
    frames: Option<u32>,
    root: Option<Arc<RootComponent>>,
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
        }
    }

    pub fn root_node(&self, ctx: &FrameCtx) -> Node {
        (self.root)(ctx)
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
        })
    }
}
