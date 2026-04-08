use std::{any::Any, sync::Arc};

use anyhow::Result;
use skia_safe::Canvas;

use crate::{
    backend::resource_cache::BackendResourceCache,
    display::{list::DisplayList, tree::DisplayTree},
    frame_ctx::FrameCtx,
    resource::{assets::AssetsMap, media::MediaContext},
    runtime::{
        policy::snapshot::SceneSnapshotPlan,
        profile::BackendProfile,
        session::RenderSession,
        target::{RenderSurfaceKind, RenderTargetHandle},
        text_engine::SharedTextEngine,
    },
    scene::{composition::Composition, transition::TransitionKind},
};

pub(crate) trait SceneSnapshotHandle: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
}

pub(crate) type SharedSceneSnapshot = Arc<dyn SceneSnapshotHandle>;

pub(crate) struct SceneRenderContext<'a> {
    pub assets: &'a AssetsMap,
    pub backend_resources: &'a BackendResourceCache,
    pub media_ctx: &'a mut MediaContext,
    pub frame_ctx: &'a FrameCtx,
    pub backend_profile: &'a mut BackendProfile,
    pub width: i32,
    pub height: i32,
}

pub(crate) trait RenderEngine: Send + Sync {
    fn target_surface_kind(&self) -> RenderSurfaceKind;
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
        snapshot: &SharedSceneSnapshot,
        canvas: &Canvas,
        profile: Option<&mut BackendProfile>,
    ) -> Result<()>;
    fn record_display_tree_snapshot(
        &self,
        runtime: &mut SceneRenderContext<'_>,
        display_tree: &DisplayTree,
    ) -> Result<SharedSceneSnapshot>;
    fn record_display_list_snapshot(
        &self,
        runtime: &mut SceneRenderContext<'_>,
        display_list: &DisplayList,
    ) -> Result<SharedSceneSnapshot>;
    fn draw_scene_without_snapshot(
        &self,
        runtime: &mut SceneRenderContext<'_>,
        display_tree: &DisplayTree,
        display_list: &DisplayList,
        plan: SceneSnapshotPlan,
        canvas: &Canvas,
    ) -> Result<()>;
    fn draw_transition(
        &self,
        canvas: &Canvas,
        from: &SharedSceneSnapshot,
        to: &SharedSceneSnapshot,
        progress: f32,
        kind: TransitionKind,
        width: i32,
        height: i32,
        profile: Option<&mut BackendProfile>,
    ) -> Result<()>;
}

pub(crate) type SharedRenderEngine = Arc<dyn RenderEngine>;
