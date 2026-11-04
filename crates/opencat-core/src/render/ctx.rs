//! Render context — borrow-bundle passed to render functions.
//!
//! Carries scene data, frame metadata, and the `DrawOpBuilder` that
//! all render functions write their draw-ops into.

use crate::analyze::annotation::AnnotatedDisplayTree;
use crate::analyze::compositor::OrderedSceneProgram;
use crate::frame_ctx::FrameCtx;
use crate::ir::GeneratedImageTable;
use crate::probe::catalog::PreparedResourceCatalog;
use crate::render::builder::DrawOpBuilder;

/// Rendering context passed to all render functions.
///
/// Carries scene data, frame metadata, and the `DrawOpBuilder`
/// that all render functions append `DrawOp`s into.
pub struct RenderCtx<'a> {
    pub catalog: &'a PreparedResourceCatalog,
    pub frame_ctx: &'a FrameCtx,
    pub display_tree: &'a AnnotatedDisplayTree,
    pub ordered_scene: &'a OrderedSceneProgram,
    pub builder: &'a mut DrawOpBuilder,
    pub font_db: &'a fontdb::Database,
    pub hidden_picture_stack: Vec<String>,
    /// Table for core-rasterized images (color-emoji glyphs).
    /// Text rendering inserts RGBA here and emits `ImageRef::Generated`; hosts
    /// resolve the id back to a bitmap from this table.
    pub generated_images: &'a mut GeneratedImageTable,
    /// Accumulated 4×4 of the 3-D context we are currently inside. Kept here
    /// because rendering is the only place that knows a node's position in the
    /// 3-D tree, and it changes every frame.
    pub space3: crate::render::dispatch::Space3,
}

