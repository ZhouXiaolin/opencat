use anyhow::{Result, anyhow};

use crate::{
    Composition, FrameCtx,
    element::{resolve::resolve_ui_tree_with_script_cache, tree::ElementNode},
    frame_ctx::ScriptFrameCtx,
    layout::tree::LayoutNode,
    runtime::{policy::cache::SceneSlot, session::RenderSession},
    scene::{
        node::Node,
        time::{FrameState, frame_state_for_root},
    },
};

#[derive(Clone, Debug)]
pub struct FrameElementRect {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub z_index: i32,
    pub depth: u32,
    pub draw_order: u32,
    pub slot: FrameElementSlot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameElementSlot {
    Scene,
    TransitionFrom,
    TransitionTo,
}

pub fn collect_frame_layout_rects(
    composition: &Composition,
    frame_index: u32,
    session: &mut RenderSession,
) -> Result<Vec<FrameElementRect>> {
    let max_frame = composition.frames.max(1).saturating_sub(1);
    let clamped_frame = frame_index.min(max_frame);
    let frame_ctx = FrameCtx {
        frame: clamped_frame,
        fps: composition.fps,
        width: composition.width,
        height: composition.height,
        frames: composition.frames.max(1),
    };

    let root = composition.root_node(&frame_ctx);
    let frame_state = frame_state_for_root(&root, &frame_ctx);

    let mut rects = Vec::new();
    let mut draw_order = 0_u32;

    match frame_state {
        FrameState::Scene {
            scene,
            script_frame_ctx,
        } => {
            collect_scene_rects(
                &scene,
                &frame_ctx,
                &script_frame_ctx,
                session,
                SceneSlot::Scene,
                FrameElementSlot::Scene,
                &mut draw_order,
                &mut rects,
            )?;
        }
        FrameState::Transition {
            from,
            to,
            from_script_frame_ctx,
            to_script_frame_ctx,
            ..
        } => {
            collect_scene_rects(
                &from,
                &frame_ctx,
                &from_script_frame_ctx,
                session,
                SceneSlot::TransitionFrom,
                FrameElementSlot::TransitionFrom,
                &mut draw_order,
                &mut rects,
            )?;
            collect_scene_rects(
                &to,
                &frame_ctx,
                &to_script_frame_ctx,
                session,
                SceneSlot::TransitionTo,
                FrameElementSlot::TransitionTo,
                &mut draw_order,
                &mut rects,
            )?;
        }
    }

    Ok(rects)
}

fn collect_scene_rects(
    scene: &Node,
    frame_ctx: &FrameCtx,
    script_frame_ctx: &ScriptFrameCtx,
    session: &mut RenderSession,
    scene_slot: SceneSlot,
    output_slot: FrameElementSlot,
    draw_order: &mut u32,
    out: &mut Vec<FrameElementRect>,
) -> Result<()> {
    let element_root = resolve_ui_tree_with_script_cache(
        scene,
        frame_ctx,
        script_frame_ctx,
        &mut session.media_ctx,
        &mut session.assets,
        None,
        &mut session.script_runtime,
    )?;

    let text_engine = session.text_engine_handle();
    let (layout_tree, _) = session
        .layout_session_mut(scene_slot)
        .compute_layout_with_text_engine(&element_root, frame_ctx, text_engine.as_ref())?;

    collect_rects_in_draw_order(
        &element_root,
        &layout_tree.root,
        output_slot,
        0.0,
        0.0,
        0,
        draw_order,
        out,
    )
}

fn collect_rects_in_draw_order(
    element: &ElementNode,
    layout: &LayoutNode,
    slot: FrameElementSlot,
    parent_x: f32,
    parent_y: f32,
    depth: u32,
    draw_order: &mut u32,
    out: &mut Vec<FrameElementRect>,
) -> Result<()> {
    if element.children.len() != layout.children.len() {
        return Err(anyhow!(
            "element/layout child count mismatch while collecting frame rects"
        ));
    }

    let x = parent_x + layout.rect.x;
    let y = parent_y + layout.rect.y;

    if layout.rect.width > 0.0 && layout.rect.height > 0.0 {
        out.push(FrameElementRect {
            id: element.style.id.clone(),
            x,
            y,
            width: layout.rect.width,
            height: layout.rect.height,
            z_index: element.style.layout.z_index,
            depth,
            draw_order: *draw_order,
            slot,
        });
    }

    *draw_order = draw_order.saturating_add(1);

    let mut child_pairs = element
        .children
        .iter()
        .zip(layout.children.iter())
        .collect::<Vec<_>>();
    child_pairs.sort_by_key(|(child, _)| child.style.layout.z_index);

    for (child, child_layout) in child_pairs {
        collect_rects_in_draw_order(
            child,
            child_layout,
            slot,
            x,
            y,
            depth.saturating_add(1),
            draw_order,
            out,
        )?;
    }

    Ok(())
}
