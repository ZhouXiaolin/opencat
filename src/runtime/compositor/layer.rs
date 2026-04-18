use std::time::Instant;

use anyhow::Result;

use crate::{
    display::tree::DisplayTree,
    runtime::{
        frame_view::RenderFrameView,
        profile::{BackendDurationMetric, backend_span, record_backend_elapsed},
        render_engine::{SceneRenderContext, SceneSnapshot, SharedRenderEngine},
    },
};

pub(crate) struct LayeredScene {
    pub static_layer: Option<SceneSnapshot>,
}

impl LayeredScene {
    pub fn compose(
        &self,
        runtime: &mut SceneRenderContext<'_>,
        render_engine: SharedRenderEngine,
        display_tree: &DisplayTree,
        frame_view: RenderFrameView,
    ) -> Result<()> {
        if let Some(static_layer) = &self.static_layer {
            let _profile_span = backend_span("scene_static_draw");
            let static_draw_started = Instant::now();
            render_engine.draw_scene_snapshot(static_layer, frame_view)?;
            record_backend_elapsed(BackendDurationMetric::SceneStaticDraw, static_draw_started);
        }

        let _profile_span = backend_span("scene_dynamic_draw");
        let dynamic_draw_started = Instant::now();
        render_engine.draw_display_tree_dynamic(runtime, display_tree, frame_view)?;
        record_backend_elapsed(
            BackendDurationMetric::SceneDynamicDraw,
            dynamic_draw_started,
        );
        Ok(())
    }
}
