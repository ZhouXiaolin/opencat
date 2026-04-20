use std::collections::HashMap;

use anyhow::{Result, anyhow};

use crate::{
    Composition, FrameCtx,
    element::{
        resolve::resolve_ui_tree_with_script_cache,
        tree::{ElementKind, ElementNode},
    },
    frame_ctx::ScriptFrameCtx,
    layout::tree::LayoutNode,
    runtime::{compositor::SceneSlot, session::RenderSession},
    scene::{
        node::{Node, NodeKind},
        primitives::ImageSource,
        time::{FrameState, TimelineSegment, frame_state_for_root},
    },
    style::NodeStyle,
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
    pub parent_draw_order: Option<u32>,
    pub slot: FrameElementSlot,
    pub kind: String,
    pub text_content: Option<String>,
    pub media_source: Option<String>,
    pub icon_name: Option<String>,
    pub script_source: Option<String>,
    pub canvas_command_count: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameElementSlot {
    Scene,
    TransitionFrom,
    TransitionTo,
}

#[derive(Clone, Debug, Default)]
struct SourceNodeMeta {
    kind: Option<String>,
    text_content: Option<String>,
    media_source: Option<String>,
    icon_name: Option<String>,
    script_source: Option<String>,
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
                SceneSlot::root_scene(),
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
                SceneSlot::root_transition_from(),
                FrameElementSlot::TransitionFrom,
                &mut draw_order,
                &mut rects,
            )?;
            collect_scene_rects(
                &to,
                &frame_ctx,
                &to_script_frame_ctx,
                session,
                SceneSlot::root_transition_to(),
                FrameElementSlot::TransitionTo,
                &mut draw_order,
                &mut rects,
            )?;
        }
        FrameState::Layer { children } => {
            for (index, child) in children.iter().enumerate() {
                collect_frame_state_rects(
                    child,
                    &frame_ctx,
                    session,
                    index,
                    &mut draw_order,
                    &mut rects,
                )?;
            }
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
    seed_asset_entries_for_inspect(scene, frame_ctx, &mut session.assets);

    let mut source_meta_by_id = HashMap::<String, SourceNodeMeta>::new();
    collect_source_metadata(scene, frame_ctx, &mut source_meta_by_id);

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
        None,
        &source_meta_by_id,
        draw_order,
        out,
    )
}

fn collect_frame_state_rects(
    frame_state: &FrameState,
    frame_ctx: &FrameCtx,
    session: &mut RenderSession,
    child_index: usize,
    draw_order: &mut u32,
    out: &mut Vec<FrameElementRect>,
) -> Result<()> {
    match frame_state {
        FrameState::Scene {
            scene,
            script_frame_ctx,
        } => {
            collect_scene_rects(
                scene,
                frame_ctx,
                script_frame_ctx,
                session,
                SceneSlot::child_scene(child_index),
                FrameElementSlot::Scene,
                draw_order,
                out,
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
                from,
                frame_ctx,
                from_script_frame_ctx,
                session,
                SceneSlot::child_transition_from(child_index),
                FrameElementSlot::TransitionFrom,
                draw_order,
                out,
            )?;
            collect_scene_rects(
                to,
                frame_ctx,
                to_script_frame_ctx,
                session,
                SceneSlot::child_transition_to(child_index),
                FrameElementSlot::TransitionTo,
                draw_order,
                out,
            )?;
        }
        FrameState::Layer { children } => {
            for (nested_index, child) in children.iter().enumerate() {
                collect_frame_state_rects(
                    child,
                    frame_ctx,
                    session,
                    nested_index,
                    draw_order,
                    out,
                )?;
            }
        }
    }
    Ok(())
}

fn seed_asset_entries_for_inspect(
    node: &Node,
    frame_ctx: &FrameCtx,
    assets: &mut crate::resource::assets::AssetsMap,
) {
    match node.kind() {
        NodeKind::Component(component) => {
            let rendered = component.render(frame_ctx);
            seed_asset_entries_for_inspect(&rendered, frame_ctx, assets);
        }
        NodeKind::Div(div) => {
            for child in div.children_ref() {
                seed_asset_entries_for_inspect(child, frame_ctx, assets);
            }
        }
        NodeKind::Canvas(canvas) => {
            for asset in canvas.assets_ref() {
                assets.ensure_image_source_entry_for_inspect(&asset.source);
            }
        }
        NodeKind::Image(image) => {
            assets.ensure_image_source_entry_for_inspect(image.source());
        }
        NodeKind::Timeline(timeline) => {
            for segment in timeline.segments() {
                match segment {
                    TimelineSegment::Scene { scene, .. } => {
                        seed_asset_entries_for_inspect(scene, frame_ctx, assets);
                    }
                    TimelineSegment::Transition { from, to, .. } => {
                        seed_asset_entries_for_inspect(from, frame_ctx, assets);
                        seed_asset_entries_for_inspect(to, frame_ctx, assets);
                    }
                }
            }
        }
        NodeKind::Text(_) | NodeKind::Lucide(_) | NodeKind::Video(_) | NodeKind::Caption(_) => {}
        NodeKind::Layer(layer) => {
            for child in layer.children_ref() {
                seed_asset_entries_for_inspect(child, frame_ctx, assets);
            }
        }
    }
}

fn collect_rects_in_draw_order(
    element: &ElementNode,
    layout: &LayoutNode,
    slot: FrameElementSlot,
    parent_x: f32,
    parent_y: f32,
    depth: u32,
    parent_draw_order: Option<u32>,
    source_meta_by_id: &HashMap<String, SourceNodeMeta>,
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
    let current_draw_order = *draw_order;
    *draw_order = draw_order.saturating_add(1);

    let node_id = &element.style.id;
    let source_meta = source_meta_by_id.get(node_id);
    let kind = source_meta
        .and_then(|meta| meta.kind.clone())
        .unwrap_or_else(|| fallback_kind_for_element(element).to_string());
    let text_content = source_meta
        .and_then(|meta| meta.text_content.clone())
        .or_else(|| match &element.kind {
            ElementKind::Text(text) => Some(text.text.clone()),
            _ => None,
        });
    let media_source = source_meta.and_then(|meta| meta.media_source.clone());
    let icon_name = source_meta
        .and_then(|meta| meta.icon_name.clone())
        .or_else(|| match &element.kind {
            ElementKind::Lucide(icon) => Some(icon.icon.clone()),
            _ => None,
        });
    let script_source = source_meta.and_then(|meta| meta.script_source.clone());
    let canvas_command_count = match &element.kind {
        ElementKind::Canvas(canvas) => Some(canvas.commands.len() as u32),
        _ => None,
    };

    let pushed_current_node = layout.rect.width > 0.0 && layout.rect.height > 0.0;
    if pushed_current_node {
        out.push(FrameElementRect {
            id: node_id.clone(),
            x,
            y,
            width: layout.rect.width,
            height: layout.rect.height,
            z_index: element.style.layout.z_index,
            depth,
            draw_order: current_draw_order,
            parent_draw_order,
            slot,
            kind,
            text_content,
            media_source,
            icon_name,
            script_source,
            canvas_command_count,
        });
    }

    let next_parent_draw_order = if pushed_current_node {
        Some(current_draw_order)
    } else {
        parent_draw_order
    };

    let mut ordered_children = element.children.iter().enumerate().collect::<Vec<_>>();
    if element.style.layout.is_flex || element.style.layout.is_grid {
        ordered_children.sort_by_key(|(index, child)| (child.style.layout.order, *index));
    }

    let mut child_pairs = ordered_children
        .into_iter()
        .map(|(_, child)| child)
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
            next_parent_draw_order,
            source_meta_by_id,
            draw_order,
            out,
        )?;
    }

    Ok(())
}

fn fallback_kind_for_element(element: &ElementNode) -> &'static str {
    match element.kind {
        ElementKind::Div(_) => "div",
        ElementKind::Text(_) => "text",
        ElementKind::Bitmap(_) => "bitmap",
        ElementKind::Canvas(_) => "canvas",
        ElementKind::Lucide(_) => "lucide",
    }
}

fn collect_source_metadata(
    node: &Node,
    frame_ctx: &FrameCtx,
    out: &mut HashMap<String, SourceNodeMeta>,
) {
    match node.kind() {
        NodeKind::Component(component) => {
            let rendered = component.render(frame_ctx);
            collect_source_metadata(&rendered, frame_ctx, out);
        }
        NodeKind::Div(div) => {
            let entry = upsert_style_meta(div.style_ref(), "div", out);
            if let Some(entry) = entry {
                entry.media_source = None;
            }
            for child in div.children_ref() {
                collect_source_metadata(child, frame_ctx, out);
            }
        }
        NodeKind::Canvas(canvas) => {
            let entry = upsert_style_meta(canvas.style_ref(), "canvas", out);
            if let Some(entry) = entry {
                let asset_ids = canvas
                    .assets_ref()
                    .iter()
                    .map(|asset| asset.asset_id.clone())
                    .collect::<Vec<_>>();
                if !asset_ids.is_empty() {
                    entry.media_source = Some(format!("assets: {}", asset_ids.join(", ")));
                }
            }
        }
        NodeKind::Text(text) => {
            let entry = upsert_style_meta(text.style_ref(), "text", out);
            if let Some(entry) = entry {
                entry.text_content = Some(text.content().to_string());
            }
        }
        NodeKind::Image(image) => {
            let entry = upsert_style_meta(image.style_ref(), "image", out);
            if let Some(entry) = entry {
                entry.media_source = Some(format_image_source(image.source()));
            }
        }
        NodeKind::Lucide(icon) => {
            let entry = upsert_style_meta(icon.style_ref(), "lucide", out);
            if let Some(entry) = entry {
                entry.icon_name = Some(icon.icon().to_string());
            }
        }
        NodeKind::Video(video) => {
            let entry = upsert_style_meta(video.style_ref(), "video", out);
            if let Some(entry) = entry {
                entry.media_source = Some(video.source().to_string_lossy().to_string());
            }
        }
        NodeKind::Timeline(timeline) => {
            let _ = upsert_style_meta(timeline.style_ref(), "timeline", out);
            for segment in timeline.segments() {
                match segment {
                    TimelineSegment::Scene { scene, .. } => {
                        collect_source_metadata(scene, frame_ctx, out);
                    }
                    TimelineSegment::Transition { from, to, .. } => {
                        collect_source_metadata(from, frame_ctx, out);
                        collect_source_metadata(to, frame_ctx, out);
                    }
                }
            }
        }
        NodeKind::Caption(caption) => {
            let entry = upsert_style_meta(caption.style_ref(), "caption", out);
            if let Some(entry) = entry {
                entry.text_content = caption
                    .active_text(frame_ctx.frame)
                    .map(|text| text.to_string());
                entry.media_source = Some(caption.path_ref().to_string_lossy().to_string());
            }
        }
        NodeKind::Layer(layer) => {
            let _ = upsert_style_meta(layer.style_ref(), "layer", out);
            for child in layer.children_ref() {
                collect_source_metadata(child, frame_ctx, out);
            }
        }
    }
}

fn upsert_style_meta<'a>(
    style: &NodeStyle,
    kind: &str,
    out: &'a mut HashMap<String, SourceNodeMeta>,
) -> Option<&'a mut SourceNodeMeta> {
    if style.id.is_empty() {
        return None;
    }

    let entry = out.entry(style.id.clone()).or_default();
    entry.kind = Some(kind.to_string());
    entry.script_source = style
        .script_driver
        .as_ref()
        .map(|driver| driver.source().to_string());
    Some(entry)
}

fn format_image_source(source: &ImageSource) -> String {
    match source {
        ImageSource::Unset => "unset".to_string(),
        ImageSource::Path(path) => path.to_string_lossy().to_string(),
        ImageSource::Url(url) => url.clone(),
        ImageSource::Query(query) => {
            let aspect = query.aspect_ratio.as_deref().unwrap_or("-");
            format!(
                "query:{} count:{} aspect:{}",
                query.query, query.count, aspect
            )
        }
    }
}

#[cfg(test)]
mod browser_layout_integration_tests;

#[cfg(test)]
mod browser_layout_tests;
