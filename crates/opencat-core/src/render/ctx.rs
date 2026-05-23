//! Render context — borrow-bundle passed to render functions.
//!
//! Carries scene data, frame metadata, and the `DrawOpBuilder` that
//! all render functions write their draw-ops into.

use crate::frame_ctx::FrameCtx;
use crate::render::builder::DrawOpBuilder;
use crate::resource::blob_store::BlobStore;
use crate::resource::hash_map_catalog::HashMapResourceCatalog;
use crate::analyze::annotation::AnnotatedDisplayTree;
use crate::analyze::compositor::OrderedSceneProgram;

/// Rendering context passed to all render functions.
///
/// Carries scene data, frame metadata, and the `DrawOpBuilder`
/// that all render functions append `DrawOp`s into.
pub struct RenderCtx<'a> {
    /// Asset catalog for resolving ImageRef asset_ids to binary data.
    pub catalog: &'a HashMapResourceCatalog,
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
