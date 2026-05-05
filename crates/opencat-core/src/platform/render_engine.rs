//! Core-level RenderEngine trait + per-frame borrow contexts.

use std::any::Any;

use anyhow::Result;

use crate::frame_ctx::FrameCtx;
use crate::platform::backend::BackendTypes;
use crate::resource::hash_map_catalog::HashMapResourceCatalog;
use crate::runtime::annotation::AnnotatedDisplayTree;
use crate::runtime::compositor::OrderedSceneProgram;

/// Backend-agnostic frame view.
pub struct FrameView<'a> {
    pub width: u32,
    pub height: u32,
    pub kind: FrameViewKind<'a>,
}

pub enum FrameViewKind<'a> {
    /// Platform native drawing handle.
    Opaque(&'a mut dyn Any),
}

/// Borrow-bundle for backend.record_* operations.
pub struct RecordCtx<'a, B: BackendTypes + ?Sized> {
    pub catalog: &'a HashMapResourceCatalog,
    pub frame_ctx: &'a FrameCtx,
    pub width: i32,
    pub height: i32,
    pub _phantom: std::marker::PhantomData<&'a B>,
}

/// Borrow-bundle for backend.draw_ordered_scene.
pub struct RenderCtx<'a, B: BackendTypes + ?Sized> {
    pub catalog: &'a HashMapResourceCatalog,
    pub frame_ctx: &'a FrameCtx,
    pub display_tree: &'a AnnotatedDisplayTree,
    pub ordered_scene: &'a OrderedSceneProgram,
    pub width: i32,
    pub height: i32,
    pub _phantom: std::marker::PhantomData<&'a B>,
}

/// Backend-only render engine surface.
pub trait RenderEngine: BackendTypes + Send + Sync {
    fn target_frame_view_kind(&self) -> &'static str;

    fn draw_scene_snapshot(
        &self,
        snapshot: &Self::Picture,
        frame_view: FrameView<'_>,
    ) -> Result<()>;

    fn record_display_tree_snapshot(
        &self,
        ctx: &mut RecordCtx<'_, Self>,
        display_tree: &AnnotatedDisplayTree,
    ) -> Result<Self::Picture>;

    fn draw_ordered_scene(
        &self,
        ctx: &mut RenderCtx<'_, Self>,
        frame_view: FrameView<'_>,
    ) -> Result<()>;
}
