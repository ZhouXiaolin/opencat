use std::collections::HashMap;

use anyhow::{Result, anyhow};

use crate::{
    analyze::fingerprint::display_recorded_subtree_fingerprint,
    display::{
        list::{
            BitmapDisplayItem, BitmapPaintStyle, DisplayClip, DisplayItem, DisplayRect,
            DisplayTransform, DrawScriptDisplayItem, LottieDisplayItem, RectDisplayItem,
            RectPaintStyle, SvgPathDisplayItem, SvgPathPaintStyle, TextDisplayItem,
            TimelineDisplayItem, TimelineTransitionDisplay,
        },
        tree::{DisplayNode, DisplayTree, HiddenChildDisplayNode},
    },
    frame_ctx::FrameCtx,
    layout::tree::{LayoutNode, LayoutTree},
    parse::transition::TransitionKind,
    resolve::tree::{ElementId, ElementKind, ElementNode},
    style::{ClipPath, LengthPercentage, Position},
};

/// L3 子树 merkle 缓存。命中条件：
/// - element.fingerprints.paint_input_subtree —— 子树内所有 paint 输入
/// - layout.output_fingerprint.subtree —— 子树内所有 rect (x/y/w/h)
/// 以上两轴 + element_id + node_count 全等 -> 命中缓存。
///
/// 命中后分两条路：
/// - apply_input_subtree 也相同 → 完整命中，直接克隆复用。
/// - apply_input_subtree 不同 → 仅 apply 字段（opacity/transforms/backdrop_blur）
///   变化，走 patch 路径，原地更新 apply 字段，跳过 DisplayItem 重建。
///
/// 任一轴不等就在该节点失效，但 children 仍可按各自键继续命中。结构变化（child_count
/// 不等 / element_id 不等）则整棵替换，避免错位复用。
pub struct DisplayBuildSession {
    root: Option<CachedDisplayNode>,
}

struct CachedDisplayNode {
    element_id: ElementId,
    paint_input_subtree: u64,
    apply_input_subtree: u64,
    layout_output_subtree: u64,
    node_count: usize,
    node: DisplayNode,
    children: Vec<CachedDisplayNode>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DisplayBuildStats {
    pub structure_rebuilds: usize,
    pub subtree_full_hit_subtrees: usize,
    pub subtree_full_hit_nodes: usize,
    pub rebuilt_nodes: usize,
    pub apply_only_nodes: usize,
}

impl DisplayBuildSession {
    pub fn new() -> Self {
        Self { root: None }
    }

    pub fn build_with_cache(
        &mut self,
        element_root: &ElementNode,
        layout_tree: &LayoutTree,
        frame_ctx: &FrameCtx,
    ) -> Result<(DisplayTree, DisplayBuildStats)> {
        let mut stats = DisplayBuildStats::default();

        if element_root.children.len() != layout_tree.root.children.len() {
            return Err(anyhow!(
                "element/layout child count mismatch while building display tree"
            ));
        }

        let (node, cached) = update_or_build(
            element_root,
            &layout_tree.root,
            frame_ctx,
            self.root.take(),
            &mut stats,
        )?;

        self.root = Some(cached);
        Ok((DisplayTree { root: node }, stats))
    }
}

impl Default for DisplayBuildSession {
    fn default() -> Self {
        Self::new()
    }
}

fn cached_matches(cached: &CachedDisplayNode, element: &ElementNode, layout: &LayoutNode) -> bool {
    cached.element_id == element.id
        && cached.paint_input_subtree == element.fingerprints.paint_input_subtree
        && cached.layout_output_subtree == layout.output_fingerprint.subtree
        && cached.node_count == element.fingerprints.node_count
}

fn refresh_paint_epochs(node: &mut DisplayNode, frame: u64) {
    match &mut node.item {
        DisplayItem::Bitmap(bitmap) => {
            if bitmap.video_timing.is_some() {
                bitmap.paint_epoch = frame;
            }
        }
        DisplayItem::Lottie(lottie) => {
            lottie.paint_epoch = frame;
        }
        _ => {}
    }
    for child in &mut node.children {
        refresh_paint_epochs(child, frame);
    }
    for hidden in &mut node.hidden_subtree {
        refresh_paint_epochs(&mut hidden.node, frame);
    }
    node.recorded_subtree_fingerprint = display_recorded_subtree_fingerprint(node);
}

fn patch_cached_subtree_apply(
    mut cached: CachedDisplayNode,
    element: &ElementNode,
    layout: &LayoutNode,
    frame_ctx: &FrameCtx,
) -> CachedDisplayNode {
    let mut child_map: HashMap<ElementId, CachedDisplayNode> = cached
        .children
        .into_iter()
        .map(|c| (c.element_id, c))
        .collect();

    let child_pairs: Vec<_> = element
        .children
        .iter()
        .zip(layout.children.iter())
        .collect();
    let mut built_children: Vec<DisplayNode> = Vec::with_capacity(child_pairs.len());
    let mut cached_children: Vec<CachedDisplayNode> = Vec::with_capacity(child_pairs.len());

    for (child_elem, child_layout) in child_pairs {
        let mut prev = child_map
            .remove(&child_elem.id)
            .expect("structure must match when cached_matches passed");
        if prev.apply_input_subtree == child_elem.fingerprints.apply_input_subtree {
            refresh_paint_epochs(&mut prev.node, frame_ctx.frame as u64);
            built_children.push(prev.node.clone());
            cached_children.push(prev);
        } else {
            let patched = patch_cached_subtree_apply(prev, child_elem, child_layout, frame_ctx);
            built_children.push(patched.node.clone());
            cached_children.push(patched);
        }
    }

    let node = assemble_display_node(element, layout, frame_ctx, built_children);

    cached.apply_input_subtree = element.fingerprints.apply_input_subtree;
    cached.node = node;
    cached.children = cached_children;
    cached
}

fn update_or_build(
    element: &ElementNode,
    layout: &LayoutNode,
    frame_ctx: &FrameCtx,
    cached: Option<CachedDisplayNode>,
    stats: &mut DisplayBuildStats,
) -> Result<(DisplayNode, CachedDisplayNode)> {
    if let Some(entry) = cached.as_ref() {
        if cached_matches(entry, element, layout) {
            if entry.apply_input_subtree == element.fingerprints.apply_input_subtree {
                stats.subtree_full_hit_subtrees += 1;
                stats.subtree_full_hit_nodes += entry.node_count;
                let mut cached = cached.expect("just checked Some");
                let mut node = cached.node.clone();
                refresh_paint_epochs(&mut node, frame_ctx.frame as u64);
                cached.node = node;
                return Ok((cached.node.clone(), cached));
            }

            stats.apply_only_nodes += entry.node_count;
            let entry = cached.expect("just checked Some");
            let patched = patch_cached_subtree_apply(entry, element, layout, frame_ctx);
            return Ok((patched.node.clone(), patched));
        }
    }

    if element.children.len() != layout.children.len() {
        return Err(anyhow!(
            "element/layout child count mismatch while building display tree"
        ));
    }

    let mut cached_children_by_id: std::collections::HashMap<ElementId, CachedDisplayNode> = cached
        .map(|c| {
            if c.element_id != element.id {
                stats.structure_rebuilds += 1;
            }
            c.children
                .into_iter()
                .map(|child| (child.element_id, child))
                .collect()
        })
        .unwrap_or_default();

    let mut child_pairs: Vec<_> = element
        .children
        .iter()
        .zip(layout.children.iter())
        .collect();
    child_pairs.sort_by_key(|(child, _)| child.style.layout.z_index);

    let mut built_children: Vec<DisplayNode> = Vec::with_capacity(child_pairs.len());
    let mut cached_children_next: Vec<CachedDisplayNode> = Vec::with_capacity(child_pairs.len());

    for (child_element, child_layout) in child_pairs {
        let prev = cached_children_by_id.remove(&child_element.id);
        let (child_node, child_cached) =
            update_or_build(child_element, child_layout, frame_ctx, prev, stats)?;
        built_children.push(child_node);
        cached_children_next.push(child_cached);
    }

    let node = assemble_display_node(element, layout, frame_ctx, built_children);
    stats.rebuilt_nodes += 1;

    let cached_entry = CachedDisplayNode {
        element_id: element.id,
        paint_input_subtree: element.fingerprints.paint_input_subtree,
        apply_input_subtree: element.fingerprints.apply_input_subtree,
        layout_output_subtree: layout.output_fingerprint.subtree,
        node_count: element.fingerprints.node_count,
        node: node.clone(),
        children: cached_children_next,
    };
    Ok((node, cached_entry))
}

pub fn build_display_tree(
    element_root: &ElementNode,
    layout_tree: &LayoutTree,
    frame_ctx: &FrameCtx,
) -> Result<DisplayTree> {
    Ok(DisplayTree {
        root: build_display_node(element_root, &layout_tree.root, frame_ctx)?,
    })
}

fn build_display_node(
    element: &ElementNode,
    layout: &LayoutNode,
    frame_ctx: &FrameCtx,
) -> Result<DisplayNode> {
    if element.children.len() != layout.children.len() {
        return Err(anyhow!(
            "element/layout child count mismatch while building display tree"
        ));
    }

    let mut child_pairs = element
        .children
        .iter()
        .zip(layout.children.iter())
        .collect::<Vec<_>>();
    child_pairs.sort_by_key(|(child, _)| child.style.layout.z_index);

    let built_children = child_pairs
        .into_iter()
        .map(|(child, child_layout)| build_display_node(child, child_layout, frame_ctx))
        .collect::<Result<Vec<_>>>()?;

    Ok(assemble_display_node(
        element,
        layout,
        frame_ctx,
        built_children,
    ))
}

/// 把已经构建好（按 z_index 排序）的 `built_children` 装配为父节点。
///
/// 抽出来给两条路径共享：
/// - `build_display_node`：无缓存的全量构建
/// - `DisplayBuildSession::update_or_build`：每帧 merkle 失效后的局部重建
fn assemble_display_node(
    element: &ElementNode,
    layout: &LayoutNode,
    frame_ctx: &FrameCtx,
    built_children: Vec<DisplayNode>,
) -> DisplayNode {
    let bounds = DisplayRect {
        x: 0.0,
        y: 0.0,
        width: layout.rect.width,
        height: layout.rect.height,
    };

    let visual = &element.style.visual;
    let uniform_border = visual.border_width.unwrap_or(0.0);
    let border_top_w = visual.border_top_width.unwrap_or(uniform_border);
    let border_right_w = visual.border_right_width.unwrap_or(uniform_border);
    let border_bottom_w = visual.border_bottom_width.unwrap_or(uniform_border);
    let border_left_w = visual.border_left_width.unwrap_or(uniform_border);

    let clip = if visual.clip_contents {
        let inner_bounds = DisplayRect {
            x: bounds.x + border_left_w,
            y: bounds.y + border_top_w,
            width: (bounds.width - border_left_w - border_right_w).max(0.0),
            height: (bounds.height - border_top_w - border_bottom_w).max(0.0),
        };
        let outer_radius = visual.border_radius;
        let inner_radius = crate::style::BorderRadius {
            top_left: (outer_radius.top_left - border_top_w.max(border_left_w)).max(0.0),
            top_right: (outer_radius.top_right - border_top_w.max(border_right_w)).max(0.0),
            bottom_right: (outer_radius.bottom_right - border_bottom_w.max(border_right_w))
                .max(0.0),
            bottom_left: (outer_radius.bottom_left - border_bottom_w.max(border_left_w)).max(0.0),
        };
        Some(DisplayClip {
            bounds: inner_bounds,
            border_radius: inner_radius,
        })
    } else {
        None
    };
    let paint_clip = visual
        .clip_path
        .map(|clip_path| display_clip_for_clip_path(clip_path, bounds));

    let (children, hidden_subtree) = if matches!(&element.kind, ElementKind::Canvas(_)) {
        let owner_id = element.style.id.clone();
        let hidden_subtree = built_children
            .into_iter()
            .map(|node| HiddenChildDisplayNode {
                node,
                owner_id: owner_id.clone(),
            })
            .collect::<Vec<_>>();
        (Vec::new(), hidden_subtree)
    } else {
        (built_children, Vec::new())
    };

    let hidden_is_empty = hidden_subtree.is_empty();
    let item = display_item_for_node(
        element,
        bounds,
        frame_ctx,
        if hidden_is_empty {
            Vec::new()
        } else {
            hidden_subtree.clone()
        },
    );

    let draw_slot = if element.draw_slot.commands.is_empty() {
        None
    } else {
        Some(DrawScriptDisplayItem {
            bounds,
            commands: element.draw_slot.commands.clone(),
            drop_shadow: Vec::new(),
            hidden_subtree: if hidden_is_empty {
                Vec::new()
            } else {
                hidden_subtree.clone()
            },
        })
    };

    let mut node = DisplayNode {
        input_fingerprints: element.fingerprints,
        layout_output_fingerprint: layout.output_fingerprint,
        recorded_subtree_fingerprint: Default::default(),
        transform: DisplayTransform {
            translation_x: layout.rect.x,
            translation_y: layout.rect.y,
            bounds,
            transforms: element.style.visual.transforms.clone(),
        },
        element_id: element.id,
        opacity: element.style.visual.opacity,
        css_filter: element.style.visual.css_filter.clone(),
        backdrop_blur_sigma: element.style.visual.backdrop_blur_sigma,
        paint_clip,
        clip,
        item,
        children,
        draw_slot,
        hidden_subtree,
    };
    node.recorded_subtree_fingerprint = display_recorded_subtree_fingerprint(&node);
    node
}

fn display_clip_for_clip_path(clip_path: ClipPath, bounds: DisplayRect) -> DisplayClip {
    match clip_path {
        ClipPath::Inset(inset) => {
            let left = resolve_clip_length(inset.left, bounds.width);
            let top = resolve_clip_length(inset.top, bounds.height);
            let right = resolve_clip_length(inset.right, bounds.width);
            let bottom = resolve_clip_length(inset.bottom, bounds.height);
            DisplayClip {
                bounds: DisplayRect {
                    x: bounds.x + left,
                    y: bounds.y + top,
                    width: (bounds.width - left - right).max(0.0),
                    height: (bounds.height - top - bottom).max(0.0),
                },
                border_radius: Default::default(),
            }
        }
    }
}

fn resolve_clip_length(value: LengthPercentage, axis: f32) -> f32 {
    match value {
        LengthPercentage::Length(value) => value,
        LengthPercentage::Percent(value) => value * axis,
    }
}

fn display_item_for_node(
    element: &ElementNode,
    bounds: DisplayRect,
    frame_ctx: &FrameCtx,
    hidden_subtree: Vec<HiddenChildDisplayNode>,
) -> DisplayItem {
    match &element.kind {
        ElementKind::Div(_) => DisplayItem::Rect(RectDisplayItem {
            bounds,
            paint: RectPaintStyle {
                background: element.style.visual.background.clone(),
                border_radius: element.style.visual.border_radius,
                border_width: element.style.visual.border_width,
                border_top_width: element.style.visual.border_top_width,
                border_right_width: element.style.visual.border_right_width,
                border_bottom_width: element.style.visual.border_bottom_width,
                border_left_width: element.style.visual.border_left_width,
                border_color: element.style.visual.border_color,
                border_style: element.style.visual.border_style,
                box_shadow: element.style.visual.box_shadow.clone(),
                inset_shadow: element.style.visual.inset_shadow.clone(),
                drop_shadow: element.style.visual.drop_shadow.clone(),
                backdrop_blur_sigma: element.style.visual.backdrop_blur_sigma,
            },
        }),
        ElementKind::Timeline(timeline) => DisplayItem::Timeline(TimelineDisplayItem {
            bounds,
            paint: RectPaintStyle {
                background: element.style.visual.background.clone(),
                border_radius: element.style.visual.border_radius,
                border_width: element.style.visual.border_width,
                border_top_width: element.style.visual.border_top_width,
                border_right_width: element.style.visual.border_right_width,
                border_bottom_width: element.style.visual.border_bottom_width,
                border_left_width: element.style.visual.border_left_width,
                border_color: element.style.visual.border_color,
                border_style: element.style.visual.border_style,
                box_shadow: element.style.visual.box_shadow.clone(),
                inset_shadow: element.style.visual.inset_shadow.clone(),
                drop_shadow: element.style.visual.drop_shadow.clone(),
                backdrop_blur_sigma: element.style.visual.backdrop_blur_sigma,
            },
            transition: timeline.transition.as_ref().map(|transition| {
                let mut kind = transition.kind.clone();
                if let TransitionKind::Gl(ref mut gl) = kind {
                    gl.fill_sksl();
                }
                TimelineTransitionDisplay {
                    progress: transition.progress,
                    kind,
                }
            }),
        }),
        ElementKind::Text(text) => {
            let (visual_expand_x, visual_expand_y) = conservative_text_visual_expansion(
                text.text_unit_overrides.as_ref(),
                text.text_style.text_px,
            );
            DisplayItem::Text(TextDisplayItem {
                bounds,
                text: text.text.clone(),
                style: text.text_style.clone(),
                allow_wrap: text_element_allows_wrap(element),
                truncate: element.style.layout.truncate,
                drop_shadow: element.style.visual.drop_shadow.clone(),
                text_shadows: element.style.visual.text_shadows.clone(),
                text_unit_overrides: text.text_unit_overrides.clone(),
                visual_expand_x,
                visual_expand_y,
                glyphs: None,
            })
        }
        ElementKind::Bitmap(bitmap) => DisplayItem::Bitmap(BitmapDisplayItem {
            bounds,
            asset_id: bitmap.asset_id.clone(),
            width: bitmap.width,
            height: bitmap.height,
            video_timing: bitmap.video_timing,
            paint_epoch: bitmap.video_timing.map_or(0, |_| frame_ctx.frame as u64),
            object_fit: element.style.visual.object_fit,
            paint: BitmapPaintStyle {
                background: element.style.visual.background.clone(),
                border_radius: element.style.visual.border_radius,
                border_width: element.style.visual.border_width,
                border_top_width: element.style.visual.border_top_width,
                border_right_width: element.style.visual.border_right_width,
                border_bottom_width: element.style.visual.border_bottom_width,
                border_left_width: element.style.visual.border_left_width,
                border_color: element.style.visual.border_color,
                border_style: element.style.visual.border_style,
                box_shadow: element.style.visual.box_shadow.clone(),
                inset_shadow: element.style.visual.inset_shadow.clone(),
                drop_shadow: element.style.visual.drop_shadow.clone(),
            },
        }),
        ElementKind::Lottie(lottie) => DisplayItem::Lottie(LottieDisplayItem {
            bounds,
            bundle_id: lottie.bundle_id.clone(),
            width: lottie.width,
            height: lottie.height,
            fps: lottie.fps,
            in_frame: lottie.in_frame,
            out_frame: lottie.out_frame,
            timing: lottie.timing,
            paint_epoch: frame_ctx.frame as u64,
            object_fit: element.style.visual.object_fit,
            paint: BitmapPaintStyle {
                background: element.style.visual.background.clone(),
                border_radius: element.style.visual.border_radius,
                border_width: element.style.visual.border_width,
                border_top_width: element.style.visual.border_top_width,
                border_right_width: element.style.visual.border_right_width,
                border_bottom_width: element.style.visual.border_bottom_width,
                border_left_width: element.style.visual.border_left_width,
                border_color: element.style.visual.border_color,
                border_style: element.style.visual.border_style,
                box_shadow: element.style.visual.box_shadow.clone(),
                inset_shadow: element.style.visual.inset_shadow.clone(),
                drop_shadow: element.style.visual.drop_shadow.clone(),
            },
        }),
        ElementKind::Canvas(canvas) => DisplayItem::DrawScript(DrawScriptDisplayItem {
            bounds,
            commands: canvas.commands.clone(),
            drop_shadow: element.style.visual.drop_shadow.clone(),
            hidden_subtree,
        }),
        ElementKind::SvgPath(svg) => DisplayItem::SvgPath(SvgPathDisplayItem {
            bounds,
            path_data: svg.path_data.clone(),
            paint: SvgPathPaintStyle {
                fill: element.style.visual.fill.clone(),
                stroke_width: element.style.visual.stroke_width,
                stroke_color: element.style.visual.stroke_color,
                drop_shadow: element.style.visual.drop_shadow.clone(),
                stroke_dasharray: element.style.visual.stroke_dasharray,
                stroke_dashoffset: element.style.visual.stroke_dashoffset,
            },
            view_box: svg.view_box,
        }),
    }
}

fn text_element_allows_wrap(element: &ElementNode) -> bool {
    if element.style.layout.truncate {
        return false;
    }

    let has_definite_width = element.style.layout.width.is_some()
        || element.style.layout.width_percent.is_some()
        || element.style.layout.width_full;

    if element.style.layout.position == Position::Absolute && !has_definite_width {
        return false;
    }

    element.style.text.wrap_text || has_definite_width
}

fn conservative_text_visual_expansion(
    batch: Option<&crate::script::TextUnitOverrideBatch>,
    text_px: f32,
) -> (f32, f32) {
    let Some(batch) = batch else {
        return (0.0, 0.0);
    };
    let mut max_x = 0.0_f32;
    let mut max_y = 0.0_f32;
    let base = text_px.max(1.0);
    for unit in &batch.overrides {
        max_x = max_x.max(unit.translate_x.unwrap_or(0.0).abs());
        max_y = max_y.max(unit.translate_y.unwrap_or(0.0).abs());
        let scale = unit.scale.unwrap_or(1.0);
        if scale > 1.0 {
            max_x = max_x.max((scale - 1.0) * base);
            max_y = max_y.max((scale - 1.0) * base);
        }
        if unit.rotation_deg.unwrap_or(0.0) != 0.0 {
            max_x = max_x.max(base * 0.5);
            max_y = max_y.max(base * 0.5);
        }
    }
    (max_x.ceil(), max_y.ceil())
}

#[cfg(test)]
mod tests {
    use super::{DisplayBuildSession, build_display_tree};
    use crate::{
        FrameCtx,
        analyze::annotation::{annotate_display_tree, compute_display_tree_fingerprints},
        parse,
        parse::primitives::{div, lucide},
        resolve::{resolve::resolve_ui_tree, tree::ElementNode},
        style::{ColorToken, ObjectFit},
        test_support::MockScriptHost,
        probe::catalog::PreparedResourceCatalog,
    };
    use crate::{
        display::list::DisplayItem,
        layout::tree::{LayoutNode, LayoutRect, LayoutTree},
    };

    fn simple_layout(id: &str, rect: LayoutRect, children: Vec<LayoutNode>) -> LayoutNode {
        LayoutNode {
            id: id.to_string(),
            rect,
            output_fingerprint: Default::default(),
            children,
        }
    }

    #[test]
    fn bitmap_display_item_preserves_object_fit() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = PreparedResourceCatalog::default();
        let element = div()
            .id("root")
            .child(
                crate::parse::primitives::image()
                    .id("bitmap")
                    .path("/tmp/test-display-bitmap.png")
                    .size(2.0, 2.0)
                    .cover(),
            )
            .into();
        let resolved = resolve_ui_tree(
            &element,
            &frame_ctx,
            &mut assets,
            None,
            &mut MockScriptHost::default(),
        )
        .expect("tree should resolve");
        let layout_tree = LayoutTree {
            root: simple_layout(
                "root",
                LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 320.0,
                    height: 180.0,
                },
                vec![simple_layout(
                    "bitmap",
                    LayoutRect {
                        x: 0.0,
                        y: 0.0,
                        width: 2.0,
                        height: 2.0,
                    },
                    Vec::new(),
                )],
            ),
        };

        let tree = build_display_tree(&resolved, &layout_tree, &frame_ctx)
            .expect("display tree should build");
        let DisplayItem::Bitmap(bitmap) = &tree.root.children[0].item else {
            panic!("expected bitmap draw item");
        };

        assert_eq!(bitmap.object_fit, ObjectFit::Cover);
        assert_eq!(
            bitmap.paint.border_radius,
            crate::style::BorderRadius::default()
        );
    }

    #[test]
    fn absolute_auto_width_text_display_item_does_not_wrap() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = PreparedResourceCatalog::default();
        let parsed = crate::parse(
            r#"{"type":"composition","width":320,"height":180,"fps":30,"duration":0.033333333333}
{"id":"root","parentId":null,"type":"div","className":"relative w-full h-full"}
{"id":"panel","parentId":"root","type":"div","className":"absolute left-[20px] top-[20px] w-[140px] h-[60px]"}
{"id":"label","parentId":"panel","type":"text","className":"absolute left-[12px] top-[10px] text-[13px] tracking-[3px]","text":"SHORTHAND"}"#,
        )
        .expect("jsonl should parse");
        let resolved = resolve_ui_tree(
            &parsed.root,
            &frame_ctx,
            &mut assets,
            None,
            &mut MockScriptHost::default(),
        )
        .expect("tree should resolve");
        let layout_tree = LayoutTree {
            root: simple_layout(
                "root",
                LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 320.0,
                    height: 180.0,
                },
                vec![simple_layout(
                    "panel",
                    LayoutRect {
                        x: 20.0,
                        y: 20.0,
                        width: 140.0,
                        height: 60.0,
                    },
                    vec![simple_layout(
                        "label",
                        LayoutRect {
                            x: 12.0,
                            y: 10.0,
                            width: 112.0,
                            height: 16.0,
                        },
                        Vec::new(),
                    )],
                )],
            ),
        };

        let tree = build_display_tree(&resolved, &layout_tree, &frame_ctx)
            .expect("display tree should build");
        let DisplayItem::Text(text) = &tree.root.children[0].children[0].item else {
            panic!("expected text draw item");
        };
        assert!(
            !text.allow_wrap,
            "absolute auto-width text should rasterize as shrink-to-fit single-line text"
        );
    }

    #[test]
    fn display_tree_sorts_children_by_z_index_for_painting() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = PreparedResourceCatalog::default();
        let parsed = crate::parse(
            r#"{"type":"composition","width":320,"height":180,"fps":30,"duration":0.033333333333}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"front","parentId":"root","type":"text","className":"text-[12px] z-10","text":"front"}
{"id":"back","parentId":"root","type":"text","className":"text-[12px]","text":"back"}"#,
        )
        .expect("jsonl should parse");
        let resolved = resolve_ui_tree(
            &parsed.root,
            &frame_ctx,
            &mut assets,
            None,
            &mut MockScriptHost::default(),
        )
        .expect("tree should resolve");
        let layout_tree = LayoutTree {
            root: simple_layout(
                "root",
                LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 320.0,
                    height: 180.0,
                },
                vec![
                    simple_layout(
                        "front",
                        LayoutRect {
                            x: 0.0,
                            y: 0.0,
                            width: 50.0,
                            height: 20.0,
                        },
                        Vec::new(),
                    ),
                    simple_layout(
                        "back",
                        LayoutRect {
                            x: 0.0,
                            y: 0.0,
                            width: 50.0,
                            height: 20.0,
                        },
                        Vec::new(),
                    ),
                ],
            ),
        };

        let tree = build_display_tree(&resolved, &layout_tree, &frame_ctx)
            .expect("display tree should build");
        let texts = tree
            .root
            .children
            .iter()
            .filter_map(|node| match &node.item {
                DisplayItem::Text(text) => Some(text.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(texts, vec!["back", "front"]);
    }

    #[test]
    fn display_tree_keeps_clip_for_overflow_hidden_nodes() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 40,
            height: 40,
            frames: 1,
        };
        let mut assets = PreparedResourceCatalog::default();
        let element = div()
            .id("root")
            .rounded(12.0)
            .overflow_hidden()
            .child(div().id("child"))
            .into();
        let resolved = resolve_ui_tree(
            &element,
            &frame_ctx,
            &mut assets,
            None,
            &mut MockScriptHost::default(),
        )
        .expect("tree should resolve");
        let layout_tree = LayoutTree {
            root: simple_layout(
                "root",
                LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 40.0,
                    height: 40.0,
                },
                vec![simple_layout(
                    "child",
                    LayoutRect {
                        x: 0.0,
                        y: 0.0,
                        width: 40.0,
                        height: 40.0,
                    },
                    Vec::new(),
                )],
            ),
        };

        let tree = build_display_tree(&resolved, &layout_tree, &frame_ctx)
            .expect("display tree should build");
        let clip = tree.root.clip.as_ref();
        assert!(clip.is_some());
        assert_eq!(
            clip.expect("clip command should exist").border_radius,
            crate::style::BorderRadius {
                top_left: 12.0,
                top_right: 12.0,
                bottom_right: 12.0,
                bottom_left: 12.0,
            }
        );
    }

    #[test]
    fn build_display_tree_preserves_sorted_children() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 100,
            height: 100,
            frames: 1,
        };
        let mut assets = PreparedResourceCatalog::default();
        let parsed = crate::parse(
            r#"{"type":"composition","width":100,"height":100,"fps":30,"duration":0.033333333333}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"late","parentId":"root","type":"text","className":"text-[12px] z-10","text":"late"}
{"id":"early","parentId":"root","type":"text","className":"text-[12px]","text":"early"}"#,
        )
        .expect("jsonl should parse");
        let resolved = resolve_ui_tree(
            &parsed.root,
            &frame_ctx,
            &mut assets,
            None,
            &mut MockScriptHost::default(),
        )
        .expect("tree should resolve");
        let layout_tree = LayoutTree {
            root: simple_layout(
                "root",
                LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                vec![
                    simple_layout(
                        "late",
                        LayoutRect {
                            x: 1.0,
                            y: 0.0,
                            width: 10.0,
                            height: 10.0,
                        },
                        Vec::new(),
                    ),
                    simple_layout(
                        "early",
                        LayoutRect {
                            x: 2.0,
                            y: 0.0,
                            width: 10.0,
                            height: 10.0,
                        },
                        Vec::new(),
                    ),
                ],
            ),
        };

        let tree = build_display_tree(&resolved, &layout_tree, &frame_ctx)
            .expect("display tree should build");
        let texts = tree
            .root
            .children
            .iter()
            .map(|node| match &node.item {
                DisplayItem::Text(text) => text.text.as_str(),
                _ => panic!("expected text item"),
            })
            .collect::<Vec<_>>();

        assert_eq!(texts, vec!["early", "late"]);
    }

    #[test]
    fn build_display_tree_annotates_paint_and_snapshot_fingerprints_separately() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 100,
            height: 100,
            frames: 1,
        };
        let mut assets = PreparedResourceCatalog::default();
        let parsed = crate::parse(
            r#"{"type":"composition","width":100,"height":100,"fps":30,"duration":0.033333333333}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"child","parentId":"root","type":"div","className":"w-[10px] h-[10px] bg-red-500"}"#,
        )
        .expect("jsonl should parse");
        let resolved = resolve_ui_tree(
            &parsed.root,
            &frame_ctx,
            &mut assets,
            None,
            &mut MockScriptHost::default(),
        )
        .expect("tree should resolve");

        let layout_a = LayoutTree {
            root: simple_layout(
                "root",
                LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                vec![simple_layout(
                    "child",
                    LayoutRect {
                        x: 0.0,
                        y: 0.0,
                        width: 10.0,
                        height: 10.0,
                    },
                    Vec::new(),
                )],
            ),
        };
        let layout_b = LayoutTree {
            root: simple_layout(
                "root",
                LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                vec![simple_layout(
                    "child",
                    LayoutRect {
                        x: 24.0,
                        y: 12.0,
                        width: 10.0,
                        height: 10.0,
                    },
                    Vec::new(),
                )],
            ),
        };

        let tree_a = build_display_tree(&resolved, &layout_a, &frame_ctx)
            .expect("display tree should build");
        let tree_b = build_display_tree(&resolved, &layout_b, &frame_ctx)
            .expect("display tree should build");
        let mut annotated_a = annotate_display_tree(&tree_a);
        compute_display_tree_fingerprints(&mut annotated_a);
        let mut annotated_b = annotate_display_tree(&tree_b);
        compute_display_tree_fingerprints(&mut annotated_b);

        let child_a = annotated_a.children(annotated_a.root)[0];
        let child_b = annotated_b.children(annotated_b.root)[0];
        assert_eq!(
            annotated_a.analysis(child_a).paint_fingerprint,
            annotated_b.analysis(child_b).paint_fingerprint
        );
        assert_eq!(
            annotated_a.analysis(child_a).snapshot_fingerprint,
            annotated_b.analysis(child_b).snapshot_fingerprint
        );
        assert_eq!(
            annotated_a.analysis(annotated_a.root).snapshot_fingerprint,
            annotated_b.analysis(annotated_b.root).snapshot_fingerprint,
            "root snapshot_fingerprint ignores descendant position changes"
        );
    }

    #[test]
    fn display_tree_builds_lucide_visuals_from_element_style() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = PreparedResourceCatalog::default();
        let root = div().id("root").child(
            lucide("play")
                .id("icon")
                .size(24.0, 24.0)
                .stroke_color(ColorToken::Blue)
                .stroke_width(3.5)
                .fill_color(ColorToken::Sky200),
        );
        let resolved = resolve_ui_tree(
            &root.into(),
            &frame_ctx,
            &mut assets,
            None,
            &mut MockScriptHost::default(),
        )
        .expect("tree should resolve");
        let layout_tree = LayoutTree {
            root: simple_layout(
                "root",
                LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 320.0,
                    height: 180.0,
                },
                vec![simple_layout(
                    "icon",
                    LayoutRect {
                        x: 0.0,
                        y: 0.0,
                        width: 24.0,
                        height: 24.0,
                    },
                    Vec::new(),
                )],
            ),
        };

        let tree = build_display_tree(&resolved, &layout_tree, &frame_ctx)
            .expect("display tree should build");
        let DisplayItem::SvgPath(svg) = &tree.root.children[0].item else {
            panic!("expected svg path item");
        };
        assert_eq!(svg.paint.stroke_color, Some(ColorToken::Blue));
        assert_eq!(svg.paint.stroke_width, Some(3.5));
        assert_eq!(
            svg.paint.fill,
            Some(crate::style::BackgroundFill::Solid {
                color: ColorToken::Sky200,
            })
        );
        assert_eq!(svg.view_box, [0.0, 0.0, 24.0, 24.0]);
    }

    #[test]
    fn build_display_tree_reports_structure_mismatch() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 100,
            height: 100,
            frames: 1,
        };
        let mut assets = PreparedResourceCatalog::default();
        let parsed = parse(
            r#"{"type":"composition","width":100,"height":100,"fps":30,"duration":0.033333333333}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"child","parentId":"root","type":"text","className":"text-[12px]","text":"A"}"#,
        )
        .expect("jsonl should parse");
        let resolved = resolve_ui_tree(
            &parsed.root,
            &frame_ctx,
            &mut assets,
            None,
            &mut MockScriptHost::default(),
        )
        .expect("tree should resolve");
        let layout_tree = LayoutTree {
            root: simple_layout(
                "root",
                LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                Vec::new(),
            ),
        };

        let err =
            build_display_tree(&resolved, &layout_tree, &frame_ctx).expect_err("expected mismatch");
        assert!(err.to_string().contains("child count mismatch"));
    }

    #[test]
    fn display_tree_builds_path_visuals() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = PreparedResourceCatalog::default();
        let root = div().id("root").child(
            crate::parse::primitives::path("M0 0 L 100 0 L 50 100 Z")
                .id("triangle")
                .size(100.0, 100.0)
                .fill_color(ColorToken::Red500)
                .stroke_color(ColorToken::Blue)
                .stroke_width(2.0),
        );
        let resolved = resolve_ui_tree(
            &root.into(),
            &frame_ctx,
            &mut assets,
            None,
            &mut MockScriptHost::default(),
        )
        .expect("tree should resolve");
        let layout_tree = LayoutTree {
            root: simple_layout(
                "root",
                LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 320.0,
                    height: 180.0,
                },
                vec![simple_layout(
                    "triangle",
                    LayoutRect {
                        x: 0.0,
                        y: 0.0,
                        width: 100.0,
                        height: 100.0,
                    },
                    Vec::new(),
                )],
            ),
        };

        let tree = build_display_tree(&resolved, &layout_tree, &frame_ctx)
            .expect("display tree should build");
        let DisplayItem::SvgPath(svg) = &tree.root.children[0].item else {
            panic!(
                "expected svg path item, got {:?}",
                tree.root.children[0].item
            );
        };
        assert_eq!(svg.path_data, vec!["M0 0 L 100 0 L 50 100 Z"]);
        assert_eq!(svg.view_box, [0.0, 0.0, 100.0, 100.0]);
        assert_eq!(svg.paint.stroke_width, Some(2.0));
        assert_eq!(svg.paint.stroke_color, Some(ColorToken::Blue));
        assert_eq!(
            svg.paint.fill,
            Some(crate::style::BackgroundFill::Solid {
                color: ColorToken::Red500,
            })
        );
    }

    #[test]
    fn canvas_hidden_subtree_preserves_nested_structure_and_layout() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 240,
            frames: 1,
        };
        let mut assets = PreparedResourceCatalog::default();
        let parsed = crate::parse::markup::parse(
            r#"<opencat width="320" height="240" fps="30" duration="0.033333333333">
  <canvas id="stage" class="w-[200px] h-[120px]">
    <div id="card" class="absolute left-[12px] top-[18px] w-[80px] h-[40px] bg-white">
      <text id="label" class="absolute left-[6px] top-[8px] text-[12px] text-black">Hi</text>
    </div>
  </canvas>
</opencat>"#,
        )
        .expect("markup should parse");
        let resolved = resolve_ui_tree(
            &parsed.root,
            &frame_ctx,
            &mut assets,
            None,
            &mut MockScriptHost::default(),
        )
        .expect("tree should resolve");
        let layout_tree = LayoutTree {
            root: simple_layout(
                "stage",
                LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 200.0,
                    height: 120.0,
                },
                vec![simple_layout(
                    "card",
                    LayoutRect {
                        x: 12.0,
                        y: 18.0,
                        width: 80.0,
                        height: 40.0,
                    },
                    vec![simple_layout(
                        "label",
                        LayoutRect {
                            x: 6.0,
                            y: 8.0,
                            width: 20.0,
                            height: 12.0,
                        },
                        Vec::new(),
                    )],
                )],
            ),
        };

        let tree = build_display_tree(&resolved, &layout_tree, &frame_ctx)
            .expect("display tree should build");
        assert_eq!(tree.root.hidden_subtree.len(), 1);

        let hidden = &tree.root.hidden_subtree[0].node;
        assert_eq!(hidden.transform.translation_x, 12.0);
        assert_eq!(hidden.transform.translation_y, 18.0);
        assert_eq!(hidden.children.len(), 1);

        let nested = &hidden.children[0];
        assert_eq!(nested.transform.translation_x, 6.0);
        assert_eq!(nested.transform.translation_y, 8.0);
        let DisplayItem::Text(text) = &nested.item else {
            panic!("expected nested text item");
        };
        assert_eq!(text.text, "Hi");
    }

    #[test]
    #[ignore]
    fn debug_canvas_ripple_card_hidden_layout() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 800,
            height: 700,
            frames: 180,
        };
        let mut assets = PreparedResourceCatalog::default();
        let xml = std::fs::read_to_string(format!(
            "{}/../../json/canvas-ripple-card.xml",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("read xml");
        let parsed = crate::parse::markup::parse(&xml).expect("markup should parse");
        let resolved = resolve_ui_tree(
            &parsed.root,
            &frame_ctx,
            &mut assets,
            None,
            &mut MockScriptHost::default(),
        )
        .expect("tree should resolve");
        let layout_tree =
            crate::layout::compute_layout(&resolved, &frame_ctx).expect("layout should compute");
        let tree = build_display_tree(&resolved, &layout_tree, &frame_ctx)
            .expect("display tree should build");
        let canvas = &tree.root.children[0];

        for hidden in canvas.hidden_subtree.iter().take(12) {
            println!(
                "hidden root tx={} ty={} w={} h={} children={}",
                hidden.node.transform.translation_x,
                hidden.node.transform.translation_y,
                hidden.node.transform.bounds.width,
                hidden.node.transform.bounds.height,
                hidden.node.children.len()
            );
        }
    }

    #[test]
    fn lucide_node_resolves_to_svg_path_with_default_stroke() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = PreparedResourceCatalog::default();
        let root = div().id("root").child(lucide("play").id("icon"));
        let resolved = resolve_ui_tree(
            &root.into(),
            &frame_ctx,
            &mut assets,
            None,
            &mut MockScriptHost::default(),
        )
        .expect("tree should resolve");

        let child = &resolved.children[0];
        let crate::resolve::tree::ElementKind::SvgPath(svg) = &child.kind else {
            panic!("expected SvgPath element, got {:?}", child.kind);
        };
        assert!(!svg.path_data.is_empty());
        assert_eq!(svg.view_box, [0.0, 0.0, 24.0, 24.0]);
        assert_eq!(svg.intrinsic_size, Some((24.0, 24.0)));
    }

    // ── DisplayBuildSession cache tests ───────────────────────────────

    fn fingerprint_only_layout_root(element_root: &ElementNode) -> LayoutTree {
        // Build a layout tree from the element tree with identity rects.
        // We only need stable LayoutOutputFingerprint values, not real layout —
        // the rects feed into output_fingerprint.subtree.
        fn build(element: &ElementNode) -> LayoutNode {
            let children: Vec<_> = element.children.iter().map(build).collect();
            let rect = LayoutRect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            };
            let output_fingerprint = layout_output_fingerprint_for_test(rect, &children);
            LayoutNode {
                id: element.style.id.clone(),
                rect,
                output_fingerprint,
                children,
            }
        }
        LayoutTree {
            root: build(element_root),
        }
    }

    fn layout_output_fingerprint_for_test(
        rect: LayoutRect,
        children: &[LayoutNode],
    ) -> crate::layout::tree::LayoutOutputFingerprint {
        use std::hash::{Hash, Hasher};
        let mut record_size_hasher = ahash::AHasher::default();
        rect.width.to_bits().hash(&mut record_size_hasher);
        rect.height.to_bits().hash(&mut record_size_hasher);
        let record_size = record_size_hasher.finish();
        let mut local_hasher = ahash::AHasher::default();
        rect.x.to_bits().hash(&mut local_hasher);
        rect.y.to_bits().hash(&mut local_hasher);
        rect.width.to_bits().hash(&mut local_hasher);
        rect.height.to_bits().hash(&mut local_hasher);
        let local = local_hasher.finish();
        let mut subtree_hasher = ahash::AHasher::default();
        local.hash(&mut subtree_hasher);
        children.len().hash(&mut subtree_hasher);
        for child in children {
            child.output_fingerprint.subtree.hash(&mut subtree_hasher);
        }
        crate::layout::tree::LayoutOutputFingerprint {
            local,
            subtree: subtree_hasher.finish(),
            record_size,
        }
    }

    fn small_frame_ctx() -> FrameCtx {
        FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 2,
        }
    }

    fn resolve_root(node: crate::Node) -> ElementNode {
        let mut assets = PreparedResourceCatalog::default();
        resolve_ui_tree(
            &node,
            &small_frame_ctx(),
            &mut assets,
            None,
            &mut MockScriptHost::default(),
        )
        .expect("resolve")
    }

    #[test]
    fn session_first_build_misses_cache_and_records_subtree() {
        let element = resolve_root(
            div()
                .id("root")
                .child(crate::parse::primitives::text("A").id("a"))
                .into(),
        );
        let layout = fingerprint_only_layout_root(&element);

        let mut session = DisplayBuildSession::new();
        let (tree, stats) = session
            .build_with_cache(&element, &layout, &small_frame_ctx())
            .expect("build");
        assert_eq!(
            stats.subtree_full_hit_subtrees, 0,
            "first frame cannot hit cache"
        );
        assert!(
            stats.rebuilt_nodes >= 2,
            "all nodes must be built first frame"
        );
        assert_eq!(tree.root.element_id, element.id);
    }

    #[test]
    fn session_second_build_full_hits_when_inputs_unchanged() {
        let element = resolve_root(
            div()
                .id("root")
                .child(crate::parse::primitives::text("A").id("a"))
                .child(crate::parse::primitives::text("B").id("b"))
                .into(),
        );
        let layout = fingerprint_only_layout_root(&element);
        let mut session = DisplayBuildSession::new();
        let _ = session
            .build_with_cache(&element, &layout, &small_frame_ctx())
            .expect("frame 0");
        let (_, stats) = session
            .build_with_cache(&element, &layout, &small_frame_ctx())
            .expect("frame 1");
        assert_eq!(
            stats.subtree_full_hit_subtrees, 1,
            "root subtree full-hit covers entire tree"
        );
        assert_eq!(stats.rebuilt_nodes, 0, "no node rebuild needed");
        assert_eq!(
            stats.subtree_full_hit_nodes, element.fingerprints.node_count,
            "node count must match"
        );
    }

    #[test]
    fn session_paint_change_invalidates_only_changed_subtree() {
        let mut session = DisplayBuildSession::new();

        let first = resolve_root(
            div()
                .id("root")
                .child(crate::parse::primitives::text("A").id("a"))
                .child(crate::parse::primitives::text("stable").id("stable"))
                .into(),
        );
        let layout1 = fingerprint_only_layout_root(&first);
        let _ = session
            .build_with_cache(&first, &layout1, &small_frame_ctx())
            .expect("frame 0");

        // Only "a" changes its text content (paint dimension).
        let second = resolve_root(
            div()
                .id("root")
                .child(crate::parse::primitives::text("A2").id("a"))
                .child(crate::parse::primitives::text("stable").id("stable"))
                .into(),
        );
        let layout2 = fingerprint_only_layout_root(&second);
        let (_, stats) = session
            .build_with_cache(&second, &layout2, &small_frame_ctx())
            .expect("frame 1");

        assert!(
            stats.subtree_full_hit_subtrees >= 1,
            "stable sibling subtree should still hit, got {stats:?}"
        );
        assert!(
            stats.rebuilt_nodes >= 2,
            "root and changed child must rebuild, got {stats:?}"
        );
    }

    #[test]
    fn session_apply_change_uses_patch_path() {
        let mut session = DisplayBuildSession::new();

        let first = resolve_root(
            div()
                .id("root")
                .child(crate::parse::primitives::text("A").id("a").opacity(1.0))
                .into(),
        );
        let layout1 = fingerprint_only_layout_root(&first);
        let _ = session
            .build_with_cache(&first, &layout1, &small_frame_ctx())
            .expect("frame 0");

        let second = resolve_root(
            div()
                .id("root")
                .child(crate::parse::primitives::text("A").id("a").opacity(0.3))
                .into(),
        );
        let layout2 = fingerprint_only_layout_root(&second);
        let (tree, stats) = session
            .build_with_cache(&second, &layout2, &small_frame_ctx())
            .expect("frame 1");

        assert_eq!(
            stats.apply_only_nodes,
            element_node_count(&second),
            "apply-only change should patch the whole subtree"
        );
        assert_eq!(
            stats.rebuilt_nodes, 0,
            "apply-only change should not trigger any rebuild"
        );
        assert_eq!(
            stats.subtree_full_hit_subtrees, 0,
            "apply-only change is not a full hit"
        );
        assert_eq!(
            tree.root.children[0].opacity, 0.3,
            "patched node must reflect new opacity"
        );
    }

    fn element_node_count(element: &ElementNode) -> usize {
        1 + element
            .children
            .iter()
            .map(element_node_count)
            .sum::<usize>()
    }

    #[test]
    fn session_apply_patch_updates_video_paint_epoch() {
        let frame_ctx_0 = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 2,
        };
        let frame_ctx_1 = FrameCtx {
            frame: 1,
            fps: 30,
            width: 320,
            height: 180,
            frames: 2,
        };

        let mut session = DisplayBuildSession::new();

        let first = resolve_root(
            div()
                .id("root")
                .opacity(1.0)
                .child(crate::parse::primitives::video("test.mp4").id("v"))
                .into(),
        );
        let layout1 = fingerprint_only_layout_root(&first);
        let _ = session
            .build_with_cache(&first, &layout1, &frame_ctx_0)
            .expect("frame 0");

        let second = resolve_root(
            div()
                .id("root")
                .opacity(0.5)
                .child(crate::parse::primitives::video("test.mp4").id("v"))
                .into(),
        );
        let layout2 = fingerprint_only_layout_root(&second);
        let (tree, stats) = session
            .build_with_cache(&second, &layout2, &frame_ctx_1)
            .expect("frame 1");

        assert!(
            stats.apply_only_nodes > 0,
            "apply-only patch should be used"
        );
        assert_eq!(stats.rebuilt_nodes, 0, "no rebuilds for apply-only change");

        let video_node = &tree.root.children[0];
        if let DisplayItem::Bitmap(ref bitmap) = video_node.item {
            assert_eq!(
                bitmap.paint_epoch, 1,
                "video paint_epoch should be updated to current frame"
            );
        } else {
            panic!("expected Bitmap display item for video");
        }
    }

    #[test]
    fn session_layout_rect_change_invalidates_cache() {
        let mut session = DisplayBuildSession::new();

        let element = resolve_root(
            div()
                .id("root")
                .child(crate::parse::primitives::text("A").id("a"))
                .into(),
        );
        let layout1 = fingerprint_only_layout_root(&element);
        let _ = session
            .build_with_cache(&element, &layout1, &small_frame_ctx())
            .expect("frame 0");

        // Same element tree, but rebuild layout with shifted rect → output_fingerprint differs.
        let layout2 = LayoutTree {
            root: LayoutNode {
                id: layout1.root.id.clone(),
                rect: LayoutRect {
                    x: 5.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                },
                output_fingerprint: layout_output_fingerprint_for_test(
                    LayoutRect {
                        x: 5.0,
                        y: 0.0,
                        width: 10.0,
                        height: 10.0,
                    },
                    &layout1.root.children,
                ),
                children: layout1.root.children.clone(),
            },
        };
        let (tree, stats) = session
            .build_with_cache(&element, &layout2, &small_frame_ctx())
            .expect("frame 1");

        // Root's rect changed → root's layout_output_subtree fp changed → root cache misses.
        // Child rect unchanged → child cache still hits (only at the leaf level).
        assert!(
            stats.rebuilt_nodes >= 1,
            "root must rebuild when its rect shifts, got {stats:?}"
        );
        assert_eq!(
            tree.root.transform.translation_x, 5.0,
            "rebuilt root must reflect new rect"
        );
        // The leaf subtree still hits because nothing under it changed.
        assert_eq!(
            stats.subtree_full_hit_subtrees, 1,
            "stable leaf subtree should still hit, got {stats:?}"
        );
    }
}
