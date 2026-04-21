use anyhow::Result;

use crate::{
    frame_ctx::FrameCtx,
    resource::{assets::AssetsMap, media::MediaContext},
    runtime::{
        annotation::AnnotatedDisplayTree,
        backend_object::BackendObject,
        cache::CacheRegistry,
        compositor::OrderedSceneProgram,
        frame_view::RenderFrameView,
        session::RenderSession,
        target::{RenderFrameViewKind, RenderTargetHandle},
        text_engine::SharedTextEngine,
    },
    scene::composition::Composition,
};

pub(crate) type SceneSnapshot = BackendObject;

pub(crate) struct SceneRenderContext<'a> {
    pub assets: &'a AssetsMap,
    pub cache_registry: &'a CacheRegistry,
    pub media_ctx: &'a mut MediaContext,
    pub frame_ctx: &'a FrameCtx,
    pub width: i32,
    pub height: i32,
}

pub(crate) trait RenderEngine: Send + Sync {
    fn target_frame_view_kind(&self) -> RenderFrameViewKind;
    fn text_engine(&self) -> SharedTextEngine;
    fn render_frame_to_target(
        &self,
        composition: &Composition,
        frame_index: u32,
        session: &mut RenderSession,
        target: &mut RenderTargetHandle,
    ) -> Result<()>;
    fn render_frame_rgba(
        &self,
        composition: &Composition,
        frame_index: u32,
        session: &mut RenderSession,
    ) -> Result<Vec<u8>>;
    fn draw_scene_snapshot(
        &self,
        snapshot: &SceneSnapshot,
        frame_view: RenderFrameView,
    ) -> Result<()>;
    fn record_display_tree_snapshot(
        &self,
        runtime: &mut SceneRenderContext<'_>,
        display_tree: &AnnotatedDisplayTree,
    ) -> Result<SceneSnapshot>;
    fn draw_ordered_scene(
        &self,
        runtime: &mut SceneRenderContext<'_>,
        display_tree: &AnnotatedDisplayTree,
        ordered_scene: &OrderedSceneProgram,
        frame_view: RenderFrameView,
    ) -> Result<()>;
}
pub(crate) type SharedRenderEngine = std::sync::Arc<dyn RenderEngine>;
