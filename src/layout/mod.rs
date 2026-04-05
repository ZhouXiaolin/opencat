pub mod tree;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use anyhow::Result;
use taffy::{
    AvailableSpace, TaffyTree,
    prelude::{Dimension, JustifyContent as TaffyJustifyContent, Style},
};

use crate::{
    FrameCtx,
    element::{
        style::ComputedLayoutStyle,
        tree::{ElementKind, ElementNode},
    },
    layout::tree::{
        LayoutBitmapPaint, LayoutNode, LayoutPaint, LayoutPaintKind, LayoutRect, LayoutTextPaint,
        LayoutTree,
    },
    nodes::{AlignItems, JustifyContent, Position},
    style::ComputedTextStyle,
    typography,
};

#[derive(Clone)]
struct TextMeasureContext {
    text: String,
    style: ComputedTextStyle,
    allow_wrap: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LayoutPassStats {
    pub structure_rebuild: bool,
    pub reused_nodes: usize,
    pub layout_dirty_nodes: usize,
    pub paint_only_nodes: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum CachedNodeKind {
    Div,
    Text,
    Bitmap,
}

struct CachedLayoutNode {
    identity: u64,
    kind: CachedNodeKind,
    taffy_node: taffy::NodeId,
    layout_hash: u64,
    paint_hash: u64,
    children: Vec<CachedLayoutNode>,
}

pub struct LayoutSession {
    taffy: TaffyTree<TextMeasureContext>,
    root: Option<CachedLayoutNode>,
}

impl LayoutSession {
    pub fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
            root: None,
        }
    }

    pub fn compute_layout(
        &mut self,
        root: &ElementNode,
        frame_ctx: &FrameCtx,
    ) -> Result<(LayoutTree, LayoutPassStats)> {
        let mut stats = LayoutPassStats::default();

        let root_id = if self
            .root
            .as_ref()
            .is_some_and(|cached| same_structure(cached, root, 0))
        {
            let cached = self.root.as_mut().expect("root checked above");
            update_cached_subtree(root, cached, 0, &mut self.taffy, &mut stats)?;
            cached.taffy_node
        } else {
            self.rebuild(root, &mut stats)?
        };

        self.taffy.compute_layout_with_measure(
            root_id,
            taffy::geometry::Size {
                width: AvailableSpace::Definite(frame_ctx.width as f32),
                height: AvailableSpace::Definite(frame_ctx.height as f32),
            },
            |known_dimensions, available_space, _node_id, node_context, _style| {
                measure_node(known_dimensions, available_space, node_context)
            },
        )?;

        Ok((
            LayoutTree {
                root: build_layout_tree(root, &self.taffy, root_id)?,
            },
            stats,
        ))
    }

    fn rebuild(
        &mut self,
        root: &ElementNode,
        stats: &mut LayoutPassStats,
    ) -> Result<taffy::NodeId> {
        self.taffy = TaffyTree::new();
        let (root_id, cache_root) = build_taffy_subtree(&mut self.taffy, root, 0)?;
        stats.structure_rebuild = true;
        stats.layout_dirty_nodes = count_nodes(root);
        self.root = Some(cache_root);
        Ok(root_id)
    }
}

impl Default for LayoutSession {
    fn default() -> Self {
        Self::new()
    }
}

pub fn compute_layout(root: &ElementNode, frame_ctx: &FrameCtx) -> Result<LayoutTree> {
    let mut session = LayoutSession::new();
    let (layout_tree, _) = session.compute_layout(root, frame_ctx)?;
    Ok(layout_tree)
}

fn measure_node(
    known_dimensions: taffy::geometry::Size<Option<f32>>,
    available_space: taffy::geometry::Size<AvailableSpace>,
    node_context: Option<&mut TextMeasureContext>,
) -> taffy::geometry::Size<f32> {
    let Some(text) = node_context else {
        return taffy::geometry::Size::ZERO;
    };

    let max_width = if text.allow_wrap {
        known_dimensions
            .width
            .or_else(|| match available_space.width {
                AvailableSpace::Definite(width) => Some(width),
                AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
            })
            .unwrap_or(f32::INFINITY)
    } else {
        f32::INFINITY
    };

    let measured = typography::measure_text_in_width(&text.text, &text.style, max_width);

    taffy::geometry::Size {
        width: known_dimensions.width.unwrap_or(measured.0),
        height: known_dimensions.height.unwrap_or(measured.1),
    }
}

fn build_taffy_subtree(
    taffy: &mut TaffyTree<TextMeasureContext>,
    element: &ElementNode,
    sibling_index: usize,
) -> Result<(taffy::NodeId, CachedLayoutNode)> {
    let mut children = Vec::new();
    let mut child_ids = Vec::new();

    for (index, child) in element.children.iter().enumerate() {
        let (child_id, child_cache) = build_taffy_subtree(taffy, child, index)?;
        child_ids.push(child_id);
        children.push(child_cache);
    }

    let style = taffy_style_for_element(element);
    let id = match text_measure_context_for_element(element) {
        Some(ctx) => taffy.new_leaf_with_context(style, ctx)?,
        None if child_ids.is_empty() => taffy.new_leaf(style)?,
        None => taffy.new_with_children(style, &child_ids)?,
    };

    Ok((
        id,
        CachedLayoutNode {
            identity: node_identity(element, sibling_index),
            kind: cached_node_kind(element),
            taffy_node: id,
            layout_hash: layout_affect_hash(element),
            paint_hash: paint_affect_hash(element),
            children,
        },
    ))
}

fn update_cached_subtree(
    element: &ElementNode,
    cached: &mut CachedLayoutNode,
    sibling_index: usize,
    taffy: &mut TaffyTree<TextMeasureContext>,
    stats: &mut LayoutPassStats,
) -> Result<()> {
    cached.identity = node_identity(element, sibling_index);

    let next_layout_hash = layout_affect_hash(element);
    let next_paint_hash = paint_affect_hash(element);

    if cached.layout_hash != next_layout_hash {
        taffy.set_style(cached.taffy_node, taffy_style_for_element(element))?;
        taffy.set_node_context(cached.taffy_node, text_measure_context_for_element(element))?;
        cached.layout_hash = next_layout_hash;
        cached.paint_hash = next_paint_hash;
        stats.layout_dirty_nodes += 1;
    } else if cached.paint_hash != next_paint_hash {
        cached.paint_hash = next_paint_hash;
        stats.paint_only_nodes += 1;
    } else {
        stats.reused_nodes += 1;
    }

    for (index, (child, cached_child)) in element
        .children
        .iter()
        .zip(cached.children.iter_mut())
        .enumerate()
    {
        update_cached_subtree(child, cached_child, index, taffy, stats)?;
    }

    Ok(())
}

fn same_structure(cached: &CachedLayoutNode, element: &ElementNode, sibling_index: usize) -> bool {
    if cached.identity != node_identity(element, sibling_index) {
        return false;
    }

    if cached.kind != cached_node_kind(element) {
        return false;
    }

    if cached.children.len() != element.children.len() {
        return false;
    }

    cached
        .children
        .iter()
        .zip(element.children.iter())
        .enumerate()
        .all(|(index, (cached_child, child))| same_structure(cached_child, child, index))
}

fn count_nodes(element: &ElementNode) -> usize {
    1 + element.children.iter().map(count_nodes).sum::<usize>()
}

fn cached_node_kind(element: &ElementNode) -> CachedNodeKind {
    match &element.kind {
        ElementKind::Div(_) => CachedNodeKind::Div,
        ElementKind::Text(_) => CachedNodeKind::Text,
        ElementKind::Bitmap(_) => CachedNodeKind::Bitmap,
    }
}

fn node_identity(element: &ElementNode, sibling_index: usize) -> u64 {
    let mut hasher = DefaultHasher::new();
    cached_node_kind(element).hash(&mut hasher);
    sibling_index.hash(&mut hasher);
    element.style.data_id.hash(&mut hasher);
    hasher.finish()
}

fn layout_affect_hash(element: &ElementNode) -> u64 {
    let mut hasher = DefaultHasher::new();
    hash_layout_style(&element.style.layout, &mut hasher);

    match &element.kind {
        ElementKind::Div(_) => {}
        ElementKind::Text(text) => {
            text.text.hash(&mut hasher);
            hash_text_layout_style(&text.text_style, &mut hasher);
        }
        ElementKind::Bitmap(bitmap) => {
            bitmap.width.hash(&mut hasher);
            bitmap.height.hash(&mut hasher);
        }
    }

    hasher.finish()
}

fn paint_affect_hash(element: &ElementNode) -> u64 {
    let mut hasher = DefaultHasher::new();
    hash_visual_style(&element.style.visual, &mut hasher);

    match &element.kind {
        ElementKind::Div(_) => {}
        ElementKind::Text(text) => {
            text.text.hash(&mut hasher);
            hash_text_style(&text.text_style, &mut hasher);
        }
        ElementKind::Bitmap(bitmap) => {
            bitmap.asset_id.hash(&mut hasher);
            bitmap.width.hash(&mut hasher);
            bitmap.height.hash(&mut hasher);
        }
    }

    hasher.finish()
}

fn hash_layout_style(style: &crate::element::style::ComputedLayoutStyle, state: &mut impl Hasher) {
    style.position.hash(state);
    hash_option_f32(style.inset_left, state);
    hash_option_f32(style.inset_top, state);
    hash_option_f32(style.inset_right, state);
    hash_option_f32(style.inset_bottom, state);
    hash_option_f32(style.width, state);
    hash_option_f32(style.height, state);
    style.width_full.hash(state);
    style.height_full.hash(state);
    hash_f32(style.padding_x, state);
    hash_f32(style.padding_y, state);
    hash_f32(style.margin_x, state);
    hash_f32(style.margin_y, state);
    style.flex_direction.hash(state);
    style.justify_content.hash(state);
    style.align_items.hash(state);
    hash_f32(style.gap, state);
    hash_f32(style.flex_grow, state);
}

fn hash_visual_style(style: &crate::element::style::ComputedVisualStyle, state: &mut impl Hasher) {
    hash_f32(style.opacity, state);
    style.background.hash(state);
    hash_f32(style.border_radius, state);
    hash_option_f32(style.border_width, state);
    style.border_color.hash(state);
    style.object_fit.hash(state);
    for transform in &style.transforms {
        hash_transform(transform, state);
    }
    style.shadow.hash(state);
}

fn hash_text_style(style: &ComputedTextStyle, state: &mut impl Hasher) {
    style.color.hash(state);
    hash_f32(style.text_px, state);
    style.font_weight.hash(state);
    hash_f32(style.letter_spacing, state);
    style.text_align.hash(state);
    hash_f32(style.line_height, state);
}

fn hash_text_layout_style(style: &ComputedTextStyle, state: &mut impl Hasher) {
    hash_f32(style.text_px, state);
    style.font_weight.hash(state);
    hash_f32(style.letter_spacing, state);
    hash_f32(style.line_height, state);
}

fn hash_transform(transform: &crate::style::Transform, state: &mut impl Hasher) {
    match *transform {
        crate::style::Transform::TranslateX(x) => {
            0_u8.hash(state);
            hash_f32(x, state);
        }
        crate::style::Transform::TranslateY(y) => {
            1_u8.hash(state);
            hash_f32(y, state);
        }
        crate::style::Transform::Translate(x, y) => {
            2_u8.hash(state);
            hash_f32(x, state);
            hash_f32(y, state);
        }
        crate::style::Transform::Scale(value) => {
            3_u8.hash(state);
            hash_f32(value, state);
        }
        crate::style::Transform::ScaleX(value) => {
            4_u8.hash(state);
            hash_f32(value, state);
        }
        crate::style::Transform::ScaleY(value) => {
            5_u8.hash(state);
            hash_f32(value, state);
        }
        crate::style::Transform::RotateDeg(value) => {
            6_u8.hash(state);
            hash_f32(value, state);
        }
        crate::style::Transform::SkewXDeg(value) => {
            7_u8.hash(state);
            hash_f32(value, state);
        }
        crate::style::Transform::SkewYDeg(value) => {
            8_u8.hash(state);
            hash_f32(value, state);
        }
        crate::style::Transform::SkewDeg(x, y) => {
            9_u8.hash(state);
            hash_f32(x, state);
            hash_f32(y, state);
        }
    }
}

fn hash_option_f32(value: Option<f32>, state: &mut impl Hasher) {
    value.map(f32::to_bits).hash(state);
}

fn hash_f32(value: f32, state: &mut impl Hasher) {
    value.to_bits().hash(state);
}

fn text_measure_context_for_element(element: &ElementNode) -> Option<TextMeasureContext> {
    match &element.kind {
        ElementKind::Text(text) => Some(TextMeasureContext {
            text: text.text.clone(),
            style: text.text_style,
            allow_wrap: element.style.layout.width.is_some() || element.style.layout.width_full,
        }),
        _ => None,
    }
}

fn taffy_style_for_element(element: &ElementNode) -> Style {
    let layout = &element.style.layout;
    match &element.kind {
        ElementKind::Div(_) => Style {
            display: taffy::prelude::Display::Flex,
            size: match layout.position {
                Position::Absolute => taffy::geometry::Size {
                    width: resolve_dimension(layout.width, layout.width_full, Dimension::auto()),
                    height: resolve_dimension(layout.height, layout.height_full, Dimension::auto()),
                },
                Position::Relative => taffy::geometry::Size {
                    width: resolve_dimension(
                        layout.width,
                        layout.width_full,
                        Dimension::percent(1.0),
                    ),
                    height: resolve_dimension(
                        layout.height,
                        layout.height_full,
                        Dimension::percent(1.0),
                    ),
                },
            },
            padding: taffy::geometry::Rect {
                left: taffy::style::LengthPercentage::length(layout.padding_x),
                top: taffy::style::LengthPercentage::length(layout.padding_y),
                right: taffy::style::LengthPercentage::length(layout.padding_x),
                bottom: taffy::style::LengthPercentage::length(layout.padding_y),
            },
            flex_direction: map_flex_direction(Some(layout.flex_direction)),
            justify_content: Some(map_justify(layout.justify_content)),
            align_items: Some(map_align(layout.align_items)),
            gap: taffy::geometry::Size {
                width: taffy::style::LengthPercentage::length(layout.gap),
                height: taffy::style::LengthPercentage::length(layout.gap),
            },
            ..base_style(layout)
        },
        ElementKind::Text(_) => Style {
            size: taffy::geometry::Size {
                width: resolve_dimension(layout.width, layout.width_full, Dimension::auto()),
                height: resolve_dimension(layout.height, layout.height_full, Dimension::auto()),
            },
            ..base_style(layout)
        },
        ElementKind::Bitmap(bitmap) => Style {
            size: taffy::geometry::Size {
                width: resolve_dimension(
                    layout.width,
                    layout.width_full,
                    Dimension::length(bitmap.width as f32),
                ),
                height: resolve_dimension(
                    layout.height,
                    layout.height_full,
                    Dimension::length(bitmap.height as f32),
                ),
            },
            ..base_style(layout)
        },
    }
}

fn build_layout_tree(
    element: &ElementNode,
    taffy: &TaffyTree<TextMeasureContext>,
    node_id: taffy::NodeId,
) -> Result<LayoutNode> {
    let layout = taffy.layout(node_id)?;
    let mut children = Vec::new();
    let taffy_children = taffy.children(node_id)?;

    for (element_child, taffy_child) in element.children.iter().zip(taffy_children.into_iter()) {
        children.push(build_layout_tree(element_child, taffy, taffy_child)?);
    }

    Ok(LayoutNode {
        rect: LayoutRect {
            x: layout.location.x,
            y: layout.location.y,
            width: layout.size.width,
            height: layout.size.height,
        },
        paint: LayoutPaint {
            visual: element.style.visual.clone(),
            kind: match &element.kind {
                ElementKind::Div(_) => LayoutPaintKind::Div,
                ElementKind::Text(text) => LayoutPaintKind::Text(LayoutTextPaint {
                    text: text.text.clone(),
                    style: text.text_style,
                    allow_wrap: element.style.layout.width.is_some()
                        || element.style.layout.width_full,
                }),
                ElementKind::Bitmap(bitmap) => LayoutPaintKind::Bitmap(LayoutBitmapPaint {
                    asset_id: bitmap.asset_id.clone(),
                    width: bitmap.width,
                    height: bitmap.height,
                    object_fit: element.style.visual.object_fit,
                }),
            },
            data_id: element.style.data_id.clone(),
        },
        children,
    })
}

fn base_style(layout: &ComputedLayoutStyle) -> Style {
    Style {
        position: map_position(layout.position),
        inset: taffy::geometry::Rect {
            left: layout
                .inset_left
                .map(taffy::style::LengthPercentageAuto::length)
                .unwrap_or(taffy::style::LengthPercentageAuto::auto()),
            top: layout
                .inset_top
                .map(taffy::style::LengthPercentageAuto::length)
                .unwrap_or(taffy::style::LengthPercentageAuto::auto()),
            right: layout
                .inset_right
                .map(taffy::style::LengthPercentageAuto::length)
                .unwrap_or(taffy::style::LengthPercentageAuto::auto()),
            bottom: layout
                .inset_bottom
                .map(taffy::style::LengthPercentageAuto::length)
                .unwrap_or(taffy::style::LengthPercentageAuto::auto()),
        },
        margin: taffy::geometry::Rect {
            left: taffy::style::LengthPercentageAuto::length(layout.margin_x),
            top: taffy::style::LengthPercentageAuto::length(layout.margin_y),
            right: taffy::style::LengthPercentageAuto::length(layout.margin_x),
            bottom: taffy::style::LengthPercentageAuto::length(layout.margin_y),
        },
        flex_grow: layout.flex_grow,
        ..Default::default()
    }
}

fn resolve_dimension(value: Option<f32>, full: bool, fallback: Dimension) -> Dimension {
    if full {
        Dimension::percent(1.0)
    } else {
        value.map(Dimension::length).unwrap_or(fallback)
    }
}

fn map_flex_direction(value: Option<crate::style::FlexDirection>) -> taffy::prelude::FlexDirection {
    match value {
        None | Some(crate::style::FlexDirection::Row) => taffy::prelude::FlexDirection::Row,
        Some(crate::style::FlexDirection::Col) => taffy::prelude::FlexDirection::Column,
    }
}

fn map_position(value: Position) -> taffy::style::Position {
    match value {
        Position::Relative => taffy::style::Position::Relative,
        Position::Absolute => taffy::style::Position::Absolute,
    }
}

fn map_justify(value: JustifyContent) -> TaffyJustifyContent {
    match value {
        JustifyContent::Start => TaffyJustifyContent::FlexStart,
        JustifyContent::Center => TaffyJustifyContent::Center,
        JustifyContent::End => TaffyJustifyContent::FlexEnd,
        JustifyContent::Between => TaffyJustifyContent::SpaceBetween,
        JustifyContent::Around => TaffyJustifyContent::SpaceAround,
        JustifyContent::Evenly => TaffyJustifyContent::SpaceEvenly,
    }
}

fn map_align(value: AlignItems) -> taffy::prelude::AlignItems {
    match value {
        AlignItems::Start => taffy::prelude::AlignItems::FlexStart,
        AlignItems::Center => taffy::prelude::AlignItems::Center,
        AlignItems::End => taffy::prelude::AlignItems::FlexEnd,
        AlignItems::Stretch => taffy::prelude::AlignItems::Stretch,
    }
}

#[cfg(test)]
mod tests {
    use super::{LayoutSession, TextMeasureContext, measure_node};
    use crate::{
        FrameCtx,
        assets::AssetsMap,
        element::resolve::resolve_ui_tree,
        media::MediaContext,
        nodes::{div, text},
        style::ComputedTextStyle,
    };
    use taffy::{AvailableSpace, geometry::Size};

    #[test]
    fn auto_width_text_stays_single_line() {
        let mut ctx = TextMeasureContext {
            text: "Ordered transforms".to_string(),
            style: ComputedTextStyle::default(),
            allow_wrap: false,
        };

        let measured = measure_node(
            Size {
                width: None,
                height: None,
            },
            Size {
                width: AvailableSpace::Definite(80.0),
                height: AvailableSpace::Definite(40.0),
            },
            Some(&mut ctx),
        );

        assert!(
            measured.width > 80.0,
            "expected auto-width text to ignore narrow available width and remain single-line"
        );
    }

    #[test]
    fn layout_session_reuses_layout_for_paint_only_change() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 2,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();
        let mut session = LayoutSession::new();

        let first = div()
            .data_id("root")
            .child(text("A").data_id("label"))
            .into();
        let second = div()
            .data_id("root")
            .opacity(0.4)
            .child(text("A").data_id("label").text_red())
            .into();

        let first_resolved = resolve_ui_tree(&first, &frame_ctx, &mut media, &mut assets, None);
        let second_resolved = resolve_ui_tree(&second, &frame_ctx, &mut media, &mut assets, None);

        let (_, first_stats) = session
            .compute_layout(&first_resolved, &frame_ctx)
            .expect("first layout should succeed");
        let (second_layout, second_stats) = session
            .compute_layout(&second_resolved, &frame_ctx)
            .expect("second layout should succeed");

        assert!(first_stats.structure_rebuild);
        assert_eq!(second_stats.layout_dirty_nodes, 0);
        assert!(second_stats.paint_only_nodes >= 1);
        assert_eq!(second_layout.root.rect.width, 320.0);
    }

    #[test]
    fn layout_session_marks_text_size_change_as_layout_dirty() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 2,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();
        let mut session = LayoutSession::new();

        let first = div()
            .data_id("root")
            .child(text("A").data_id("label"))
            .into();
        let second = div()
            .data_id("root")
            .child(text("A").data_id("label").text_px(48.0))
            .into();

        let first_resolved = resolve_ui_tree(&first, &frame_ctx, &mut media, &mut assets, None);
        let second_resolved = resolve_ui_tree(&second, &frame_ctx, &mut media, &mut assets, None);

        session
            .compute_layout(&first_resolved, &frame_ctx)
            .expect("first layout should succeed");
        let (_, second_stats) = session
            .compute_layout(&second_resolved, &frame_ctx)
            .expect("second layout should succeed");

        assert!(second_stats.layout_dirty_nodes >= 1);
    }
}
