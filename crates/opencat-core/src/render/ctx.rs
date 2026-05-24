//! Render context — borrow-bundle passed to render functions.
//!
//! Carries scene data, frame metadata, and the `DrawOpBuilder` that
//! all render functions write their draw-ops into.

use crate::analyze::annotation::AnnotatedDisplayTree;
use crate::analyze::compositor::OrderedSceneProgram;
use crate::canvas::paint::{PaintSpec, PaintStyle, PathEffectSpec, StrokeCap, StrokeJoin};
use crate::frame_ctx::FrameCtx;
use crate::render::builder::DrawOpBuilder;
use crate::resource::blob_store::BlobStore;
use crate::resource::catalog::ResourceCatalog;

/// Rendering context passed to all render functions.
///
/// Carries scene data, frame metadata, and the `DrawOpBuilder`
/// that all render functions append `DrawOp`s into.
pub struct RenderCtx<'a> {
    /// Asset catalog for resolving ImageRef asset_ids to binary data.
    pub catalog: &'a dyn ResourceCatalog,
    /// Frame-level metadata (canvas size, mouse position, time, etc.).
    pub frame_ctx: &'a FrameCtx,
    /// The annotated display tree for this frame.
    pub display_tree: &'a AnnotatedDisplayTree,
    /// Precomputed scene program (order of display items to render).
    pub ordered_scene: &'a OrderedSceneProgram,
    /// The DrawOp builder — all render functions append ops here.
    pub builder: &'a mut DrawOpBuilder,
    /// Optional blob store for reading cached binary data.
    pub blob_store: Option<&'a dyn BlobStore>,
}

/// Mutable drawing state carried through a draw-script execution.
#[derive(Clone, Debug)]
pub struct DrawScriptPaintState {
    pub fill_style: PaintSpec,
    pub stroke_style: PaintSpec,
    pub line_width: f32,
    pub global_alpha: f32,
    pub anti_alias: bool,
    pub line_cap: StrokeCap,
    pub line_join: StrokeJoin,
    pub line_dash: Option<Vec<f32>>,
    pub line_dash_phase: f32,
}

impl Default for DrawScriptPaintState {
    fn default() -> Self {
        Self {
            fill_style: PaintSpec::default(),
            stroke_style: PaintSpec::default(),
            line_width: 1.0,
            global_alpha: 1.0,
            anti_alias: true,
            line_cap: StrokeCap::Butt,
            line_join: StrokeJoin::Miter,
            line_dash: None,
            line_dash_phase: 0.0,
        }
    }
}

impl DrawScriptPaintState {
    pub fn fill_paint_spec(&self) -> PaintSpec {
        let mut spec = self.fill_style.clone();
        spec.style = PaintStyle::Fill;
        spec.anti_alias = self.anti_alias;
        spec
    }

    pub fn stroke_paint_spec(&self) -> PaintSpec {
        let mut spec = self.stroke_style.clone();
        spec.style = PaintStyle::Stroke;
        spec.anti_alias = self.anti_alias;
        let mut s = spec.stroke.unwrap_or_default();
        s.width = self.line_width.max(0.0);
        s.cap = self.line_cap;
        s.join = self.line_join;
        spec.stroke = Some(s);
        if let Some(ref intervals) = self.line_dash {
            spec.path_effect = Some(PathEffectSpec::Dash {
                intervals: intervals.clone(),
                phase: self.line_dash_phase,
            });
        }
        spec
    }
}
