use std::time::Instant;

use anyhow::Result;

use crate::display::list::{DisplayRect, DisplayTransform};
use crate::runtime::{
    annotation::{AnnotatedDisplayTree, AnnotatedNodeHandle},
    fingerprint::CompositeSig,
    frame_view::RenderFrameView,
    profile::{BackendDurationMetric, backend_span, record_backend_elapsed},
    render_engine::{SceneRenderContext, SceneSnapshot, SharedRenderEngine},
};
use crate::display::list::DisplayClip;

pub(crate) struct DynamicLayer {
    pub root: AnnotatedNodeHandle,
    pub composite: CompositeSig,
    pub transform_chain: Vec<DisplayTransform>,
    pub opacity: f32,
    pub backdrop_blur_sigma: Option<f32>,
    pub clip: Option<DisplayClip>,
    pub bounds: DisplayRect,
}

impl DynamicLayer {
    fn draw(
        &self,
        runtime: &mut SceneRenderContext<'_>,
        render_engine: SharedRenderEngine,
        display_tree: &AnnotatedDisplayTree,
        frame_view: RenderFrameView,
    ) -> Result<()> {
        render_engine.draw_dynamic_layer(runtime, display_tree, self, frame_view)
    }
}

pub(crate) struct LayeredScene {
    pub static_layer: Option<SceneSnapshot>,
    pub dynamic: Vec<DynamicLayer>,
    pub bounds: DisplayRect,
}

impl LayeredScene {
    pub fn compose(
        &self,
        runtime: &mut SceneRenderContext<'_>,
        render_engine: SharedRenderEngine,
        display_tree: &AnnotatedDisplayTree,
        frame_view: RenderFrameView,
    ) -> Result<()> {
        if self.bounds.width <= 0.0 || self.bounds.height <= 0.0 {
            return Ok(());
        }
        if let Some(static_layer) = &self.static_layer {
            let _profile_span = backend_span("scene_static_draw");
            let static_draw_started = Instant::now();
            render_engine.draw_scene_snapshot(static_layer, frame_view)?;
            record_backend_elapsed(BackendDurationMetric::SceneStaticDraw, static_draw_started);
        }

        for layer in &self.dynamic {
            let _profile_span = backend_span("scene_dynamic_draw");
            let dynamic_draw_started = Instant::now();
            layer.draw(runtime, render_engine.clone(), display_tree, frame_view)?;
            record_backend_elapsed(
                BackendDurationMetric::SceneDynamicDraw,
                dynamic_draw_started,
            );
        }
        Ok(())
    }
}
