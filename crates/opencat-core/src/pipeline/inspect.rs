//! Layout inspection from the real core pipeline resolve/layout pass.
//!
//! Inspection must not re-open a second resolve/layout session with its own
//! catalog or script runtime. Callers obtain [`FrameElementRect`]s from a
//! [`super::DefaultPipeline`] (or any host that already prepared catalog/fonts
//! and opened a pipeline) so rects share state with [`RenderFrame`].

use std::collections::HashMap;

use anyhow::{Result, anyhow};

use crate::layout::tree::{LayoutNode, LayoutTree};
use crate::parse::node::{Node, NodeKind};
use crate::parse::primitives::{ImageSource, LottieSource, VideoSource};
use crate::parse::time::TimelineSegment;
use crate::resolve::tree::{ElementKind, ElementNode};
use crate::style::NodeStyle;

/// One laid-out element in draw order for the evaluated frame.
#[derive(Clone, Debug, PartialEq)]
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
    pub kind: String,
    pub text_content: Option<String>,
    pub media_source: Option<String>,
    pub icon_name: Option<String>,
    pub script_source: Option<String>,
    pub canvas_command_count: Option<u32>,
}

#[derive(Clone, Debug, Default)]
struct SourceNodeMeta {
    kind: Option<String>,
    text_content: Option<String>,
    media_source: Option<String>,
    icon_name: Option<String>,
    script_source: Option<String>,
}

/// Collect layout rects from the resolved element tree and its layout tree.
///
/// `source_root` is the composition root for the same frame (for diagnostic
/// metadata such as lucide icon names and script source). Rect geometry and
/// draw order come only from `element_root` / `layout_tree`.
///
/// Crate-private: hosts call [`super::DefaultPipeline::inspect_frame`].
pub(crate) fn collect_frame_element_rects(
    source_root: &Node,
    element_root: &ElementNode,
    layout_tree: &LayoutTree,
) -> Result<Vec<FrameElementRect>> {
    let mut source_meta_by_id = HashMap::<String, SourceNodeMeta>::new();
    collect_source_metadata(source_root, &mut source_meta_by_id);

    let mut rects = Vec::new();
    let mut draw_order = 0_u32;
    collect_rects_in_draw_order(
        element_root,
        &layout_tree.root,
        0.0,
        0.0,
        0,
        None,
        &source_meta_by_id,
        &mut draw_order,
        &mut rects,
    )?;
    Ok(rects)
}

#[allow(clippy::too_many_arguments)]
fn collect_rects_in_draw_order(
    element: &ElementNode,
    layout: &LayoutNode,
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
    let media_source = source_meta
        .and_then(|meta| meta.media_source.clone())
        .or_else(|| media_source_from_element(element));
    let icon_name = source_meta.and_then(|meta| meta.icon_name.clone());
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
        ElementKind::Timeline(_) => "timeline",
        ElementKind::Text(_) => "text",
        ElementKind::Bitmap(ref bitmap) if bitmap.video_timing.is_some() => "video",
        ElementKind::Bitmap(_) => "bitmap",
        ElementKind::Canvas(_) => "canvas",
        ElementKind::SvgPath(_) => "svg-path",
        ElementKind::Lottie(_) => "lottie",
    }
}

fn media_source_from_element(element: &ElementNode) -> Option<String> {
    match &element.kind {
        ElementKind::Bitmap(bitmap) => Some(bitmap.asset_id.0.clone()),
        ElementKind::Lottie(lottie) => Some(lottie.bundle_id.0.clone()),
        _ => None,
    }
}

fn collect_source_metadata(node: &Node, out: &mut HashMap<String, SourceNodeMeta>) {
    match node.kind() {
        NodeKind::Div(div) => {
            let entry = upsert_style_meta(div.style_ref(), "div", out);
            if let Some(entry) = entry {
                entry.media_source = None;
            }
            for child in div.children_ref() {
                collect_source_metadata(child, out);
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
        NodeKind::Path(path) => {
            upsert_style_meta(path.style_ref(), "path", out);
        }
        NodeKind::Video(video) => {
            let entry = upsert_style_meta(video.style_ref(), "video", out);
            if let Some(entry) = entry {
                let source_str = match video.source() {
                    VideoSource::Path(p) => p.clone(),
                    VideoSource::Url(u) => format!("video:url:{u}"),
                };
                entry.media_source = Some(source_str);
            }
            for child in video.children_ref() {
                collect_source_metadata(child, out);
            }
        }
        NodeKind::Timeline(timeline) => {
            let _ = upsert_style_meta(timeline.style_ref(), "timeline", out);
            for segment in timeline.segments() {
                match segment {
                    TimelineSegment::Scene { scene, .. } => {
                        collect_source_metadata(scene, out);
                    }
                    TimelineSegment::Transition { from, to, .. } => {
                        collect_source_metadata(from, out);
                        collect_source_metadata(to, out);
                    }
                }
            }
        }
        NodeKind::Caption(caption) => {
            let entry = upsert_style_meta(caption.style_ref(), "caption", out);
            if let Some(entry) = entry {
                entry.media_source = match caption.source() {
                    crate::parse::primitives::SubtitleSource::Path(p) => {
                        Some(p.to_string_lossy().to_string())
                    }
                    crate::parse::primitives::SubtitleSource::Url(u) => Some(u.clone()),
                };
            }
        }
        NodeKind::Lottie(lottie) => {
            let entry = upsert_style_meta(lottie.style_ref(), "lottie", out);
            if let Some(entry) = entry {
                let source_str = match lottie.source() {
                    LottieSource::Unset => "unset".to_string(),
                    LottieSource::Path(p) => p.clone(),
                    LottieSource::Url(u) => format!("lottie:url:{u}"),
                };
                entry.media_source = Some(source_str);
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
        .map(|driver| driver.source.to_string());
    Some(entry)
}

fn format_image_source(source: &ImageSource) -> String {
    match source {
        ImageSource::Unset => "unset".to_string(),
        ImageSource::Path(path) => path.clone(),
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
mod tests {
    use super::*;
    use crate::layout::tree::{LayoutNode, LayoutOutputFingerprint, LayoutRect, LayoutTree};
    use crate::parse::primitives::{div, text};
    use crate::resolve::resolve::resolve_ui_tree;
    use crate::test_support::{MockScriptHost, TestCatalog};
    use crate::FrameCtx;

    #[test]
    fn collect_rects_matches_layout_geometry_and_draw_order() {
        let source: Node = div()
            .id("root")
            .w(100.0)
            .h(50.0)
            .child(text("hello").id("label").w(40.0).h(20.0))
            .into();

        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 100,
            height: 50,
            frames: 1,
        };
        let mut catalog = TestCatalog::new();
        let mut scripts = MockScriptHost::default();
        let element_root =
            resolve_ui_tree(&source, &frame_ctx, &mut catalog, None, &mut scripts)
                .expect("resolve");

        // Hand-built layout tree with absolute coordinates matching what
        // layout would produce for fixed-size children.
        let layout_tree = LayoutTree {
            root: LayoutNode {
                id: "root".into(),
                rect: LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 50.0,
                },
                output_fingerprint: LayoutOutputFingerprint::default(),
                children: vec![LayoutNode {
                    id: "label".into(),
                    rect: LayoutRect {
                        x: 10.0,
                        y: 5.0,
                        width: 40.0,
                        height: 20.0,
                    },
                    output_fingerprint: LayoutOutputFingerprint::default(),
                    children: vec![],
                }],
            },
        };

        let rects = collect_frame_element_rects(&source, &element_root, &layout_tree)
            .expect("collect rects");
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].id, "root");
        assert_eq!(rects[0].draw_order, 0);
        assert_eq!(rects[0].width, 100.0);
        assert_eq!(rects[1].id, "label");
        assert_eq!(rects[1].x, 10.0);
        assert_eq!(rects[1].y, 5.0);
        assert_eq!(rects[1].draw_order, 1);
        assert_eq!(rects[1].parent_draw_order, Some(0));
        assert_eq!(rects[1].kind, "text");
        assert_eq!(rects[1].text_content.as_deref(), Some("hello"));
    }
}
