use anyhow::Result;

use opencat_core::frame_ctx::FrameCtx;
use crate::resource::asset_catalog::AssetCatalog;
use opencat_core::scene::composition::Composition;

use opencat_core::runtime::annotation::AnnotatedDisplayTree;

use crate::{
    resource::media::MediaContext,
    runtime::{
        backend_object::BackendObject,
        cache::CacheRegistry,
        compositor::OrderedSceneProgram,
        frame_view::RenderFrameView,
        session::RenderSession,
        target::{RenderFrameViewKind, RenderTargetHandle},
    },
};

pub(crate) type SceneSnapshot = BackendObject;

pub(crate) struct SceneRenderContext<'a> {
    pub assets: &'a AssetCatalog,
    pub cache_registry: &'a CacheRegistry,
    pub media_ctx: &'a mut MediaContext,
    pub frame_ctx: &'a FrameCtx,
    pub width: i32,
    pub height: i32,
}

pub(crate) trait RenderEngine: Send + Sync {
    fn target_frame_view_kind(&self) -> RenderFrameViewKind;
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
