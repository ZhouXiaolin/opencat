//! Shared per-frame executor for pipeline implementations.

use std::sync::Arc;

use anyhow::Result;

#[cfg(feature = "profile")]
use tracing::{Level, event, span};

use crate::analyze::annotation::{annotate_display_tree, compute_display_tree_fingerprints};
use crate::analyze::compositor::OrderedSceneProgram;
use crate::display::build::DisplayBuildSession;
use crate::frame_ctx::{FrameCtx, ScriptFrameCtx};
use crate::ir::{DrawOpFrame, FrameMediaPlan};
use crate::layout::LayoutSession;
use crate::parse::composition::Composition;
use crate::probe::catalog::PreparedResourceCatalog;
use crate::render::RenderCtx;
use crate::render::builder::DrawOpBuilder;
use crate::render::media_plan::build_media_plan;

/// The document's author-asserted video color-matrix overrides
/// (`data-color-matrix`), keyed by canonical asset id — the media-plan input
/// that routes host video decoding to an asset's real YUV→RGB encoding.
fn video_color_matrices(
    catalog: &PreparedResourceCatalog,
) -> std::collections::BTreeMap<String, crate::media::VideoColorMatrix> {
    catalog
        .videos
        .iter()
        .filter_map(|(id, meta)| meta.color_matrix.map(|m| (id.key.clone(), m)))
        .collect()
}
use crate::resolve::resolve::resolve_ui_tree_with_script_cache;
use crate::script::ScriptHost;
use crate::text::DefaultFontProvider;

/// Intermediate trees produced by the shared resolve + layout pass that both
/// [`render_frame_with_state`] and inspection consume. Geometry here is exactly
/// what the real pipeline uses for the given frame.
///
/// Crate-private: hosts consume [`crate::ir::RenderFrame`] and
/// [`super::DefaultPipeline::inspect_frame`], not this intermediate type.
pub(crate) struct FrameEvaluation {
    pub frame_ctx: FrameCtx,
    /// Composition root node for this frame (source tree before resolve).
    pub source_root: crate::parse::node::Node,
    pub element_root: crate::resolve::tree::ElementNode,
    pub layout_tree: crate::layout::tree::LayoutTree,
}

/// Resolve scripts + layout for one frame using the pipeline's live sessions.
///
/// This is the single resolve/layout path shared by render and inspection so
/// diagnostic rects cannot drift from the trees that produce [`RenderFrame`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_frame_layout(
    composition: &Composition,
    frame_index: u32,
    layout_session: &mut LayoutSession,
    font_db: &Arc<fontdb::Database>,
    catalog: &mut PreparedResourceCatalog,
    script: &mut dyn ScriptHost,
) -> Result<FrameEvaluation> {
    #[cfg(feature = "profile")]
    let _resolve_span = span!(target: "render.scene", Level::TRACE, "resolve_ui_tree").entered();
    let frame_ctx = FrameCtx {
        frame: frame_index,
        fps: composition.fps,
        width: composition.width,
        height: composition.height,
        frames: composition.frames,
    };
    let script_frame_ctx = ScriptFrameCtx::global(&frame_ctx);
    let source_root = composition.root_node(&frame_ctx);
    #[cfg(feature = "profile")]
    let _script_span = span!(target: "render.pipeline", Level::TRACE, "script").entered();
    let element_root = crate::text::scope_font_db(font_db, || {
        resolve_ui_tree_with_script_cache(
            &source_root,
            &frame_ctx,
            &script_frame_ctx,
            catalog,
            None,
            script,
        )
    })?;
    #[cfg(feature = "profile")]
    drop(_script_span);
    #[cfg(feature = "profile")]
    drop(_resolve_span);

    let provider = DefaultFontProvider::from_arc(font_db.clone());
    let (layout_tree, layout_pass) =
        layout_session.compute_layout_with_provider(&element_root, &frame_ctx, &provider)?;

    #[cfg(feature = "profile")]
    {
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "reused_nodes", result = "count", amount = layout_pass.reused_nodes as u64);
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "input_merkle_full_hit_subtrees", result = "count", amount = layout_pass.input_merkle_full_hit_subtrees as u64);
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "input_merkle_full_hit_nodes", result = "count", amount = layout_pass.input_merkle_full_hit_nodes as u64);
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "layout_merkle_skipped_subtrees", result = "count", amount = layout_pass.layout_merkle_skipped_subtrees as u64);
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "layout_merkle_skipped_nodes", result = "count", amount = layout_pass.layout_merkle_skipped_nodes as u64);
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "layout_dirty", result = "count", amount = layout_pass.layout_dirty_nodes as u64);
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "raster_dirty", result = "count", amount = layout_pass.raster_dirty_nodes as u64);
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "structure_rebuild", result = "count", amount = layout_pass.structure_rebuild as u64);
    }

    Ok(FrameEvaluation {
        frame_ctx,
        source_root,
        element_root,
        layout_tree,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn render_frame_with_state(
    composition: &Composition,
    frame_index: u32,
    layout_session: &mut LayoutSession,
    display_build_session: &mut DisplayBuildSession,
    font_db: &Arc<fontdb::Database>,
    catalog: &mut PreparedResourceCatalog,
    last_ordered_scene: &mut OrderedSceneProgram,
    script: &mut dyn ScriptHost,
    generated_images: &mut crate::ir::GeneratedImageTable,
) -> Result<(
    DrawOpFrame,
    FrameMediaPlan,
    Option<crate::ir::draw_frame::Scene3Section>,
    Option<crate::ir::shader::ShaderSection>,
)> {
    #[cfg(feature = "profile")]
    let _frame_span = span!(
        target: "render.pipeline",
        Level::TRACE,
        "frame",
        frame = frame_index,
        width = composition.width as i64,
        height = composition.height as i64,
        fps = composition.fps as i64,
        mode = "scene"
    )
    .entered();

    // Retire entries no frame has refreshed. Declared at the top of the frame
    // so every insert below is stamped with the composition frame index that
    // asked for it — that is what makes out-of-order and repeated renders
    // (preview scrubbing) expire correctly instead of by call count.
    generated_images.advance_frame(frame_index as u64);

    let evaluation = evaluate_frame_layout(
        composition,
        frame_index,
        layout_session,
        font_db,
        catalog,
        script,
    )?;
    // Scripts may have generated RGBA images this frame (canvas putImageData
    // / surface auto-bake path). Land them in the pipeline table BEFORE
    // the media plan is built so hosts receive the payloads for this frame.
    for image in script.take_frame_generated_images() {
        // GPU-resident entries carry no CPU pixels (see `FrameGeneratedImage`);
        // scene3 placeholders ship none either — the executing host backfills
        // both from this frame's render payload before drawing.
        match (image.gpu, image.rgba.is_empty()) {
            (Some(gpu), _) => {
                generated_images.insert_gpu(image.id, image.width, image.height, gpu)?
            }
            (None, true) => {
                generated_images.insert_placeholder(image.id, image.width, image.height)?
            }
            (None, false) => {
                generated_images.insert(image.id, image.width, image.height, image.rgba)?
            }
        }
    }
    // 3D scene IR section (`ctx.three` commits + placeholder DrawOps already
    // in the draw stream). Rides `RenderFrame.scene3` to the executing host.
    let scene3 = script.take_scene3_commits();
    // shaders 库段（`<Shader>` 层栈 + 占位 id）。同 scene3，纯声明。
    let shader = script.take_shader_section();
    let frame_ctx = evaluation.frame_ctx;
    let element_root = evaluation.element_root;
    let layout_tree = evaluation.layout_tree;

    #[cfg(feature = "profile")]
    let _display_span = span!(target: "render.scene", Level::TRACE, "build_display_tree").entered();
    let (display_tree, display_stats) =
        display_build_session.build_with_cache(&element_root, &layout_tree, &frame_ctx)?;
    #[cfg(feature = "profile")]
    {
        event!(target: "render.display", Level::TRACE, kind = "display", name = "display_merkle_skipped_subtrees", result = "count", amount = display_stats.subtree_full_hit_subtrees as u64);
        event!(target: "render.display", Level::TRACE, kind = "display", name = "display_merkle_skipped_nodes", result = "count", amount = display_stats.subtree_full_hit_nodes as u64);
        event!(target: "render.display", Level::TRACE, kind = "display", name = "display_rebuilt_nodes", result = "count", amount = display_stats.rebuilt_nodes as u64);
        event!(target: "render.display", Level::TRACE, kind = "display", name = "display_apply_only_nodes", result = "count", amount = display_stats.apply_only_nodes as u64);
    }
    #[cfg(not(feature = "profile"))]
    let _ = display_stats;
    let mut annotated = annotate_display_tree(&display_tree);
    compute_display_tree_fingerprints(&mut annotated);
    #[cfg(feature = "profile")]
    drop(_display_span);

    let ordered_scene = OrderedSceneProgram::build(&annotated);
    *last_ordered_scene = ordered_scene.clone();

    let mut builder = DrawOpBuilder::default();
    let mut ctx = RenderCtx {
        catalog: &*catalog,
        frame_ctx: &frame_ctx,
        display_tree: &annotated,
        ordered_scene: &ordered_scene,
        builder: &mut builder,
        font_db: font_db.as_ref(),
        hidden_picture_stack: Vec::new(),
        generated_images,
    };

    crate::text::scope_font_db(font_db, || {
        crate::render::dispatch::render_display_tree(&mut ctx, &annotated)
    })?;

    let frame = builder.finish();
    if std::env::var_os("OC_DBG_STACK").is_some() {
        debug_stack_balance(&frame.ops);
    }
    let media_plan = build_media_plan(&frame, generated_images, &video_color_matrices(catalog));
    Ok((frame, media_plan, scene3, shader))
}

/// Debug-only (`OC_DBG_STACK=1`): report the save/restore balance of a
/// finished frame. A non-zero final depth means a composite/tree Paint leaked
/// its canvas transform into following siblings.
fn debug_stack_balance(ops: &[crate::ir::draw_op::DrawOp]) {
    use crate::ir::draw_op::DrawOp;
    let mut depth: i64 = 0;
    let mut min_depth: i64 = 0;
    let mut first_neg: Option<usize> = None;
    for (i, op) in ops.iter().enumerate() {
        match op {
            DrawOp::Save | DrawOp::SaveLayer { .. } | DrawOp::SaveBackdropLayer { .. } => {
                depth += 1;
            }
            DrawOp::Restore => {
                depth -= 1;
                if depth < min_depth {
                    min_depth = depth;
                    if first_neg.is_none() {
                        first_neg = Some(i);
                    }
                }
            }
            _ => {}
        }
    }
    eprintln!(
        "[stack] ops={} final_depth={} min_depth={} first_negative_op={:?}",
        ops.len(),
        depth,
        min_depth,
        first_neg
    );
    if depth != 0 || min_depth < 0 {
        report_first_negative(ops, first_neg);
    }
}

fn report_first_negative(ops: &[crate::ir::draw_op::DrawOp], first_neg: Option<usize>) {
    if let Some(i) = first_neg {
        let lo = i.saturating_sub(6);
        for (j, op) in ops[lo..=i].iter().enumerate() {
            eprintln!("[stack]   op[{}] = {:?}", lo + j, op);
        }
    }
}
