//! Render context — borrow-bundle passed to render functions.
//!
//! Generic over any `Canvas2D` backend so that core render helpers can
//! operate without knowing the concrete Skia / CPU / GPU implementation.

use std::cell::RefCell;
use std::marker::PhantomData;

use crate::canvas::Canvas2D;
use crate::frame_ctx::FrameCtx;
use crate::resource::AssetPathStore;
use crate::resource::hash_map_catalog::HashMapResourceCatalog;
use crate::runtime::annotation::AnnotatedDisplayTree;
use crate::runtime::compositor::ordered_scene::OrderedSceneProgram;
use crate::platform::video::VideoFrameProvider;

/// All data needed for rendering a single frame.
///
/// The `'a` lifetime ties every borrow to the driver stack frame.
/// `C: Canvas2D` is the abstract drawing backend (Skia, mock, …).
pub struct RenderCtx<'a, C: Canvas2D> {
    /// Catalog with metadata (URL, dimensions, etc.).
    pub catalog: &'a HashMapResourceCatalog,
    /// Per-frame timing / dimensions.
    pub frame_ctx: &'a FrameCtx,
    /// The display tree to render.
    pub display_tree: &'a AnnotatedDisplayTree,
    /// Ordered scene program (painter's order).
    pub ordered_scene: &'a OrderedSceneProgram,
    /// Video frame decoder (platform-supplied).
    /// Wrapped in RefCell so render functions can call `frame_rgba`
    /// (which takes `&mut self`) through a shared `&RenderCtx`.
    pub video: RefCell<&'a mut dyn VideoFrameProvider>,
    /// Physical path table for bitmap loading.
    pub asset_paths: Option<&'a AssetPathStore>,
    /// Platform-specific userdata (e.g. engine's MediaContext).
    pub platform_data: &'a mut dyn std::any::Any,
    /// Phantom marker for the canvas2d backend type.
    #[doc(hidden)]
    pub _phantom: PhantomData<C>,
}
