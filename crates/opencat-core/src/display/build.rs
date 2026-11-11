use std::collections::HashMap;

use anyhow::{Result, anyhow};

use crate::{
    analyze::fingerprint::display_recorded_subtree_fingerprint,
    display::{
        list::{
            BitmapDisplayItem, BitmapPaintStyle, DisplayClip, DisplayItem, DisplayRect,
            DisplayTransform, DrawScriptDisplayItem, LottieDisplayItem, RectDisplayItem,
            RectPaintStyle, SvgDisplayItem, SvgPathDisplayItem, SvgPathPaintStyle,
            TextDisplayItem,
            TimelineDisplayItem, TimelineTransitionDisplay,
        },
        tree::{DisplayNode, DisplayTree, HiddenChildDisplayNode},
    },
    frame_ctx::FrameCtx,
    geom::Mat4,
    layout::tree::{LayoutNode, LayoutTree},
    parse::transition::TransitionKind,
    resolve::{
        style::text_element_allows_wrap,
        tree::{ElementId, ElementKind, ElementNode},
    },
    style::{ClipPath, LengthPercentage},
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
            None,
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

/// `background-clip: text` 容器（非文本叶）的背景下传给后代文字作字形填充：
/// 容器自身不画背景，后代 Text 以容器盒为映射盒。`rect` 是容器盒在当前
/// 节点坐标系里的位置（逐层按子节点布局偏移平移；中间节点的 CSS 变换不计）。
#[derive(Clone, Copy)]
struct ClipTextFill<'a> {
    layers: &'a [crate::style::BackgroundFill],
    rect: DisplayRect,
}

impl<'a> ClipTextFill<'a> {
    /// 节点自身是 clip:text 容器时开新上下文，否则沿用上层的。
    fn for_node(element: &'a ElementNode, layout: &LayoutNode, inherited: Option<Self>) -> Option<Self> {
        let visual = &element.style.visual;
        if visual.background_clip == crate::style::BackgroundClip::Text
            && !visual.background.is_empty()
            && !matches!(element.kind, ElementKind::Text(_))
        {
            return Some(Self {
                layers: &visual.background,
                rect: DisplayRect { x: 0.0, y: 0.0, width: layout.rect.width, height: layout.rect.height },
            });
        }
        inherited
    }

    fn for_child(self, child_layout: &LayoutNode) -> Self {
        Self {
            rect: DisplayRect {
                x: self.rect.x - child_layout.rect.x,
                y: self.rect.y - child_layout.rect.y,
                ..self.rect
            },
            ..self
        }
    }
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

fn patch_cached_subtree_apply<'a>(
    mut cached: CachedDisplayNode,
    element: &'a ElementNode,
    layout: &LayoutNode,
    frame_ctx: &FrameCtx,
    clip_text: Option<ClipTextFill<'a>>,
) -> CachedDisplayNode {
    let own_clip_text = ClipTextFill::for_node(element, layout, clip_text);
    let mut child_map: HashMap<ElementId, CachedDisplayNode> = cached
        .children
        .into_iter()
        .map(|c| (c.element_id, c))
        .collect();

    let mut child_pairs: Vec<_> = element
        .children
        .iter()
        .zip(layout.children.iter())
        .collect();
    // 与全建路径同一排序：补丁路径若按文档序直拼，z-index 兄弟在两帧间
    // 会画出不同的遮挡关系（第一帧 z 排序、第二帧文档序）。
    child_pairs.sort_by_key(|(child, _)| child.style.layout.z_index);
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
            let patched = patch_cached_subtree_apply(
                prev,
                child_elem,
                child_layout,
                frame_ctx,
                own_clip_text.map(|c| c.for_child(child_layout)),
            );
            built_children.push(patched.node.clone());
            cached_children.push(patched);
        }
    }

    let node = assemble_display_node(element, layout, frame_ctx, built_children, clip_text);

    cached.apply_input_subtree = element.fingerprints.apply_input_subtree;
    cached.node = node;
    cached.children = cached_children;
    cached
}

fn update_or_build<'a>(
    element: &'a ElementNode,
    layout: &LayoutNode,
    frame_ctx: &FrameCtx,
    cached: Option<CachedDisplayNode>,
    stats: &mut DisplayBuildStats,
    clip_text: Option<ClipTextFill<'a>>,
) -> Result<(DisplayNode, CachedDisplayNode)> {
    // clip:text 容器下的后代：字形填充取决于容器的背景与相对位置，不在自身
    // 指纹里，缓存命中会留旧填充——整棵重建（这类子树只有几个文字节点）。
    if let Some(entry) = cached.as_ref().filter(|_| clip_text.is_none()) {
        if !crate::ablate::ablated(crate::ablate::Ablation::DisplayMerkle)
            && cached_matches(entry, element, layout)
        {
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
            let patched = patch_cached_subtree_apply(entry, element, layout, frame_ctx, None);
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

    let own_clip_text = ClipTextFill::for_node(element, layout, clip_text);
    for (child_element, child_layout) in child_pairs {
        let prev = cached_children_by_id.remove(&child_element.id);
        let (child_node, child_cached) = update_or_build(
            child_element,
            child_layout,
            frame_ctx,
            prev,
            stats,
            own_clip_text.map(|c| c.for_child(child_layout)),
        )?;
        built_children.push(child_node);
        cached_children_next.push(child_cached);
    }

    let node = assemble_display_node(element, layout, frame_ctx, built_children, clip_text);
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

/// 把已经构建好（按 z_index 排序）的 `built_children` 装配为父节点。
///
/// `DisplayBuildSession::update_or_build` 每帧 merkle 失效后的局部重建复用。
/// The node's CSS `transform` chain as one 4×4, pivoted at `origin`.
///
/// `None` when every function is 2-D: those already render through the plain
/// `DrawOp` path, and keeping them out of the 4×4 path leaves every existing
/// composition's draw-op stream untouched.
pub(crate) fn local_m4(
    transforms: &[crate::style::Transform],
    origin: crate::style::TransformOrigin,
    bounds: DisplayRect,
) -> Option<Mat4> {
    use crate::style::Transform;

    let has_3d = transforms.iter().any(|t| {
        matches!(
            t,
            Transform::TranslateZ { .. }
                | Transform::RotateXDeg { .. }
                | Transform::RotateYDeg { .. }
                | Transform::Rotate3 { .. }
        )
    });
    if !has_3d {
        return None;
    }

    let mut m = Mat4::IDENTITY;
    let (ox, oy) = origin.point(bounds.width, bounds.height);
    let pivot = Mat4::translate(ox, oy, 0.0);
    let unpivot = Mat4::translate(-ox, -oy, 0.0);
    for t in transforms {
        m = m.mul(&match *t {
            Transform::TranslateX { value } => Mat4::translate(value, 0.0, 0.0),
            Transform::TranslateY { value } => Mat4::translate(0.0, value, 0.0),
            Transform::Translate { x, y } => Mat4::translate(x, y, 0.0),
            Transform::TranslateZ { value } => Mat4::translate(0.0, 0.0, value),
            Transform::Scale { value } => Mat4::scale(value, value),
            Transform::ScaleX { value } => Mat4::scale(value, 1.0),
            Transform::ScaleY { value } => Mat4::scale(1.0, value),
            Transform::RotateDeg { value } => Mat4::rotate_z_deg(value),
            Transform::RotateXDeg { value } => Mat4::rotate_x_deg(value),
            Transform::RotateYDeg { value } => Mat4::rotate_y_deg(value),
            Transform::SkewXDeg { value } => Mat4::skew(value, 0.0),
            Transform::SkewYDeg { value } => Mat4::skew(0.0, value),
            Transform::SkewDeg { x, y } => Mat4::skew(x, y),
            // 调用方（DisplayTransform 构造）已把百分比位移折算成 px；
            // 其它入口（布局探针读样式）到不了这里。
            Transform::TranslateXPercent { .. } | Transform::TranslateYPercent { .. } => {
                Mat4::IDENTITY
            }
            // GSAP's own projective form. The 4×4 projection reproduces the
            // matrix the 2-D `Rotate3` arm has always emitted.
            Transform::Rotate3 {
                rotation_x,
                rotation_y,
                perspective,
            } => Mat4::rotate3(rotation_x, rotation_y, perspective),
        });
    }
    Some(pivot.mul(&m).mul(&unpivot))
}

fn assemble_display_node(
    element: &ElementNode,
    layout: &LayoutNode,
    frame_ctx: &FrameCtx,
    built_children: Vec<DisplayNode>,
    clip_text: Option<ClipTextFill<'_>>,
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
        let mut inner_bounds = DisplayRect {
            x: bounds.x + border_left_w,
            y: bounds.y + border_top_w,
            width: (bounds.width - border_left_w - border_right_w).max(0.0),
            height: (bounds.height - border_top_w - border_bottom_w).max(0.0),
        };
        let outer_radius = visual.border_radius;
        let mut inner_radius = crate::style::BorderRadius {
            top_left: (outer_radius.top_left - border_top_w.max(border_left_w)).max(0.0),
            top_right: (outer_radius.top_right - border_top_w.max(border_right_w)).max(0.0),
            bottom_right: (outer_radius.bottom_right - border_bottom_w.max(border_right_w))
                .max(0.0),
            bottom_left: (outer_radius.bottom_left - border_bottom_w.max(border_left_w)).max(0.0),
        };
        // 只裁横向：纵向放开（远超画面的矩形），圆角不适用。
        if visual.clip_x_only {
            const UNBOUNDED: f32 = 1.0e6;
            inner_bounds.y = -UNBOUNDED;
            inner_bounds.height = 2.0 * UNBOUNDED;
            inner_radius = crate::style::BorderRadius::default();
        }
        Some(DisplayClip {
            bounds: inner_bounds,
            border_radius: inner_radius,
        })
    } else {
        None
    };
    let paint_clip = visual
        .clip_path
        .map(|clip_path| display_clip_for_clip_path(clip_path, bounds))
        // 文本叶节点的 overflow:hidden 裁它自己的字形（宽度揭示式打字机）。
        // `clip` 只包子节点，文本叶没有子节点。
        .or_else(|| match element.kind {
            ElementKind::Text(_) => clip.clone(),
            _ => None,
        });

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
    // `visibility: hidden`：盒子、变换、不透明度照常作用于子树（子元素可以
    // `visible` 重新显示），只有自身的绘制（背景/边框/阴影/文字/位图/
    // 画布命令/背景滤镜）不出。
    let item = if visual.hidden {
        DisplayItem::Rect(RectDisplayItem { bounds, paint: RectPaintStyle::default() })
    } else {
        display_item_for_node(
            element,
            bounds,
            frame_ctx,
            if hidden_is_empty {
                Vec::new()
            } else {
                hidden_subtree.clone()
            },
            clip_text,
            layout.inline_fragments.clone(),
            layout.wrap_width,
        )
    };

    let draw_slot = if element.draw_slot.commands.is_empty() || visual.hidden {
        None
    } else {
        Some(DrawScriptDisplayItem {
            bounds,
            commands: element.draw_slot.commands.clone(),
            bitmap_scale: None,
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
        transform: {
            // 百分比位移在解析期没有盒子尺寸，这里按边界盒折算成 px；
            // 渲染端与 local_m4 只消费 px 变体。
            let transforms =
                resolve_percent_transforms(&element.style.visual.transforms, bounds);
            DisplayTransform {
                translation_x: layout.rect.x,
                translation_y: layout.rect.y,
                bounds,
                transforms: transforms.clone(),
                origin: element.style.visual.transform_origin,
                local_m4: local_m4(&transforms, element.style.visual.transform_origin, bounds),
                perspective: element.style.visual.perspective,
                preserve_3d: element.style.visual.transform_style_preserve_3d,
            }
        },
        element_id: element.id,
        opacity: element.style.visual.opacity,
        css_filter_image: crate::render::css_filter_image_filter(
            &element.style.visual.css_filter,
        ),
        backdrop_blur_sigma: element
            .style
            .visual
            .backdrop_blur_sigma
            .filter(|_| !visual.hidden),
        backdrop_filter_image: if visual.hidden {
            None
        } else {
            crate::render::css_filter_image_filter(&element.style.visual.backdrop_filter)
        },
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

/// 百分比位移按边界盒折算成 px（CSS `translate(50%)` 参考元素自身盒）。
pub(crate) fn resolve_percent_transforms(
    transforms: &[crate::style::Transform],
    bounds: DisplayRect,
) -> Vec<crate::style::Transform> {
    use crate::style::Transform;
    let has_percent = transforms
        .iter()
        .any(|t| matches!(t, Transform::TranslateXPercent { .. } | Transform::TranslateYPercent { .. }));
    if !has_percent {
        return transforms.to_vec();
    }
    transforms
        .iter()
        .map(|t| match *t {
            Transform::TranslateXPercent { value } => Transform::TranslateX {
                value: value / 100.0 * bounds.width,
            },
            Transform::TranslateYPercent { value } => Transform::TranslateY {
                value: value / 100.0 * bounds.height,
            },
            t => t,
        })
        .collect()
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

/// HTML 画布：脚本在位图空间（width/height 属性）作画，浏览器把位图缩放
/// 铺满 CSS 盒（object-fit 默认 fill）。盒尺寸与位图不同时返回缩放比
/// （bounds 原点是盒左上角），渲染时在命令流之前施加；相同时为 None。
fn canvas_bitmap_scale(
    canvas: &crate::resolve::tree::ElementCanvas,
    bounds: DisplayRect,
) -> Option<(f32, f32)> {
    canvas.bitmap.and_then(|(bw, bh)| {
        let (sx, sy) = (bounds.width / bw, bounds.height / bh);
        (bw > 0.0 && bh > 0.0 && ((sx - 1.0).abs() > 1e-6 || (sy - 1.0).abs() > 1e-6))
            .then_some((sx, sy))
    })
}

fn display_item_for_node(
    element: &ElementNode,
    bounds: DisplayRect,
    frame_ctx: &FrameCtx,
    hidden_subtree: Vec<HiddenChildDisplayNode>,
    clip_text: Option<ClipTextFill<'_>>,
    fragments: Option<std::sync::Arc<[crate::layout::tree::TextFragment]>>,
    wrap_width: Option<f32>,
) -> DisplayItem {
    match &element.kind {
        ElementKind::Div(_) => DisplayItem::Rect(RectDisplayItem {
            bounds,
            paint: RectPaintStyle {
                // clip:text 容器的背景只透过后代文字显示（见 ClipTextFill）。
                background: match element.style.visual.background_clip {
                    crate::style::BackgroundClip::Text => Vec::new(),
                    _ => element.style.visual.background.clone(),
                },
                blend_mode: element.style.visual.blend_mode,
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
                blend_mode: element.style.visual.blend_mode,
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
            let (text_fill, text_fill_box) = match element.style.visual.background_clip {
                crate::style::BackgroundClip::Text
                    if !element.style.visual.background.is_empty() =>
                {
                    (Some(element.style.visual.background.clone()), None)
                }
                _ => match clip_text {
                    Some(fill) => (Some(fill.layers.to_vec()), Some(fill.rect)),
                    None => (None, None),
                },
            };
            // A normal (non clip:text) background paints as the inline box
            // behind the glyphs — pills/badges (`bg-[rgba(...)]` on `<p>`).
            let node_background = match element.style.visual.background_clip {
                crate::style::BackgroundClip::Text => Vec::new(),
                _ => element.style.visual.background.clone(),
            };
            DisplayItem::Text(TextDisplayItem {
                bounds,
                text: text.text.clone(),
                style: text.text_style.clone(),
                text_fill,
                text_fill_box,
                background: node_background,
                background_border_radius: element.style.visual.border_radius,
                background_blend_mode: element.style.visual.blend_mode,
                allow_wrap: text_element_allows_wrap(element),
                truncate: element.style.layout.truncate,
                wrap_width: wrap_width.unwrap_or(bounds.width),
                fragments,
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
            bitmap_scale: canvas_bitmap_scale(canvas, bounds),
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
        ElementKind::Svg(svg) => DisplayItem::Svg(SvgDisplayItem {
            bounds,
            src: svg.src.clone(),
            inline: svg.inline.clone(),
            preserve_aspect_ratio: svg.preserve_aspect_ratio.clone(),
        }),
    }
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


