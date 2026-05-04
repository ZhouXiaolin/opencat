//! opencat-web — WASM/Web rendering target for opencat-core.
//!
//! 占位 crate，后续实现 Web 端的 RenderEngine。当前仅提供编译期可见的 API 形状。

use opencat_core::scene::path_bounds::{DefaultPathBounds, PathBoundsComputer};

/// Web 渲染引擎占位：未来挂上 RenderEngine trait 实现。
pub struct WebRenderEngine {
    path_bounds: Box<dyn PathBoundsComputer>,
}

impl Default for WebRenderEngine {
    fn default() -> Self {
        Self {
            path_bounds: Box::new(DefaultPathBounds),
        }
    }
}

impl WebRenderEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn path_bounds(&self) -> &dyn PathBoundsComputer {
        self.path_bounds.as_ref()
    }
}
