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
    layout::tree::{LayoutNode, LayoutRect, LayoutTree},
    runtime::{
        render_registry,
        text_engine::{SharedTextEngine, TextEngine, TextMeasureRequest},
    },
    scene::primitives::{AlignItems, JustifyContent, Position},
    style::ComputedTextStyle,
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
    pub raster_dirty_nodes: usize,
    pub composite_dirty_nodes: usize,
}

impl LayoutPassStats {
    pub fn paint_dirty_nodes(&self) -> usize {
        self.raster_dirty_nodes + self.composite_dirty_nodes
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum CachedNodeKind {
    Div,
    Text,
    Bitmap,
    Canvas,
    Lucide,
}

struct CachedLayoutNode {
    identity: u64,
    kind: CachedNodeKind,
    taffy_node: taffy::NodeId,
    layout_hash: u64,
    raster_hash: u64,
    composite_hash: u64,
    children: Vec<CachedLayoutNode>,
}

pub struct LayoutSession {
    taffy: TaffyTree<TextMeasureContext>,
    root: Option<CachedLayoutNode>,
    cached_layout_tree: Option<LayoutTree>,
    last_layout_size: Option<(i32, i32)>,
}

impl LayoutSession {
    pub fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
            root: None,
            cached_layout_tree: None,
            last_layout_size: None,
        }
    }

    pub fn compute_layout(
        &mut self,
        root: &ElementNode,
        frame_ctx: &FrameCtx,
    ) -> Result<(LayoutTree, LayoutPassStats)> {
        let text_engine = default_text_engine();
        self.compute_layout_with_text_engine(root, frame_ctx, text_engine.as_ref())
    }

    pub(crate) fn compute_layout_with_text_engine(
        &mut self,
        root: &ElementNode,
        frame_ctx: &FrameCtx,
        text_engine: &dyn TextEngine,
    ) -> Result<(LayoutTree, LayoutPassStats)> {
        let mut stats = LayoutPassStats::default();
        let viewport_size = (frame_ctx.width, frame_ctx.height);

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

        let layout_must_recompute = stats.structure_rebuild
            || stats.layout_dirty_nodes > 0
            || self.cached_layout_tree.is_none()
            || self.last_layout_size != Some(viewport_size);

        if layout_must_recompute {
            self.taffy.compute_layout_with_measure(
                root_id,
                taffy::geometry::Size {
                    width: AvailableSpace::Definite(frame_ctx.width as f32),
                    height: AvailableSpace::Definite(frame_ctx.height as f32),
                },
                |known_dimensions, available_space, _node_id, node_context, _style| {
                    measure_node(known_dimensions, available_space, node_context, text_engine)
                },
            )?;

            let layout_tree = LayoutTree {
                root: build_layout_tree(root, &self.taffy, root_id)?,
            };
            self.cached_layout_tree = Some(layout_tree.clone());
            self.last_layout_size = Some(viewport_size);
            return Ok((layout_tree, stats));
        }

        Ok((
            self.cached_layout_tree
                .as_ref()
                .expect("cached layout tree must exist when layout is clean")
                .clone(),
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
        self.cached_layout_tree = None;
        Ok(root_id)
    }
}

impl Default for LayoutSession {
    fn default() -> Self {
        Self::new()
    }
}

pub fn compute_layout(root: &ElementNode, frame_ctx: &FrameCtx) -> Result<LayoutTree> {
    let text_engine = default_text_engine();
    compute_layout_with_text_engine(root, frame_ctx, text_engine.as_ref())
}

pub(crate) fn compute_layout_with_text_engine(
    root: &ElementNode,
    frame_ctx: &FrameCtx,
    text_engine: &dyn TextEngine,
) -> Result<LayoutTree> {
    let mut session = LayoutSession::new();
    let (layout_tree, _) = session.compute_layout_with_text_engine(root, frame_ctx, text_engine)?;
    Ok(layout_tree)
}

fn measure_node(
    known_dimensions: taffy::geometry::Size<Option<f32>>,
    available_space: taffy::geometry::Size<AvailableSpace>,
    node_context: Option<&mut TextMeasureContext>,
    text_engine: &dyn TextEngine,
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

    let measured = text_engine.measure(&TextMeasureRequest {
        text: &text.text,
        style: &text.style,
        max_width,
        allow_wrap: text.allow_wrap,
    });

    taffy::geometry::Size {
        width: known_dimensions.width.unwrap_or(measured.width),
        height: known_dimensions.height.unwrap_or(measured.height),
    }
}

fn default_text_engine() -> SharedTextEngine {
    render_registry::default_text_engine()
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
            raster_hash: raster_affect_hash(element),
            composite_hash: composite_affect_hash(element),
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
    let next_raster_hash = raster_affect_hash(element);
    let next_composite_hash = composite_affect_hash(element);

    if cached.layout_hash != next_layout_hash {
        taffy.set_style(cached.taffy_node, taffy_style_for_element(element))?;
        taffy.set_node_context(cached.taffy_node, text_measure_context_for_element(element))?;
        cached.layout_hash = next_layout_hash;
        cached.raster_hash = next_raster_hash;
        cached.composite_hash = next_composite_hash;
        stats.layout_dirty_nodes += 1;
    } else {
        let raster_changed = cached.raster_hash != next_raster_hash;
        let composite_changed = cached.composite_hash != next_composite_hash;
        cached.raster_hash = next_raster_hash;
        cached.composite_hash = next_composite_hash;

        if raster_changed {
            stats.raster_dirty_nodes += 1;
        }
        if composite_changed {
            stats.composite_dirty_nodes += 1;
        }
        if !raster_changed && !composite_changed {
            stats.reused_nodes += 1;
        }
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
        ElementKind::Canvas(_) => CachedNodeKind::Canvas,
        ElementKind::Lucide(_) => CachedNodeKind::Lucide,
    }
}

fn node_identity(element: &ElementNode, sibling_index: usize) -> u64 {
    let mut hasher = DefaultHasher::new();
    cached_node_kind(element).hash(&mut hasher);
    sibling_index.hash(&mut hasher);
    element.style.id.hash(&mut hasher);
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
        ElementKind::Canvas(_) => {}
        ElementKind::Lucide(lucide) => {
            lucide.icon.hash(&mut hasher);
        }
    }

    hasher.finish()
}

fn raster_affect_hash(element: &ElementNode) -> u64 {
    let mut hasher = DefaultHasher::new();
    hash_raster_style(&element.style.visual, &mut hasher);

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
        ElementKind::Canvas(canvas) => {
            canvas.commands.len().hash(&mut hasher);
            for command in &canvas.commands {
                hash_draw_script_command(command, &mut hasher);
            }
        }
        ElementKind::Lucide(lucide) => {
            lucide.icon.hash(&mut hasher);
            element.style.text.color.hash(&mut hasher);
        }
    }

    hasher.finish()
}

fn composite_affect_hash(element: &ElementNode) -> u64 {
    let mut hasher = DefaultHasher::new();
    hash_f32(element.style.visual.opacity, &mut hasher);
    for transform in &element.style.visual.transforms {
        hash_transform(transform, &mut hasher);
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
    hash_f32(style.padding_top, state);
    hash_f32(style.padding_right, state);
    hash_f32(style.padding_bottom, state);
    hash_f32(style.padding_left, state);
    hash_f32(style.margin_top, state);
    hash_f32(style.margin_right, state);
    hash_f32(style.margin_bottom, state);
    hash_f32(style.margin_left, state);
    style.flex_direction.hash(state);
    style.justify_content.hash(state);
    style.align_items.hash(state);
    hash_f32(style.gap, state);
    hash_f32(style.flex_grow, state);
    hash_option_f32(style.flex_shrink, state);
    style.z_index.hash(state);
}

fn hash_raster_style(style: &crate::element::style::ComputedVisualStyle, state: &mut impl Hasher) {
    style.background.hash(state);
    hash_f32(style.border_radius, state);
    hash_option_f32(style.border_width, state);
    style.border_color.hash(state);
    hash_option_f32(style.blur_sigma, state);
    style.object_fit.hash(state);
    style.clip_contents.hash(state);
    style.shadow.hash(state);
}

fn hash_text_style(style: &ComputedTextStyle, state: &mut impl Hasher) {
    style.color.hash(state);
    hash_f32(style.text_px, state);
    style.font_weight.hash(state);
    hash_f32(style.letter_spacing, state);
    style.text_align.hash(state);
    hash_f32(style.line_height, state);
    style.wrap_text.hash(state);
}

fn hash_text_layout_style(style: &ComputedTextStyle, state: &mut impl Hasher) {
    hash_f32(style.text_px, state);
    style.font_weight.hash(state);
    hash_f32(style.letter_spacing, state);
    hash_f32(style.line_height, state);
    style.wrap_text.hash(state);
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
            allow_wrap: element.style.text.wrap_text
                || element.style.layout.width.is_some()
                || element.style.layout.width_full,
        }),
        _ => None,
    }
}

fn taffy_style_for_element(element: &ElementNode) -> Style {
    let layout = &element.style.layout;
    match &element.kind {
        ElementKind::Div(_) => Style {
            display: if layout.is_flex {
                taffy::prelude::Display::Flex
            } else {
                taffy::prelude::Display::Block
            },
            size: match layout.position {
                Position::Absolute => taffy::geometry::Size {
                    width: resolve_dimension(layout.width, layout.width_full, Dimension::auto()),
                    height: resolve_dimension(layout.height, layout.height_full, Dimension::auto()),
                },
                Position::Relative => taffy::geometry::Size {
                    width: resolve_dimension(
                        layout.width,
                        layout.width_full,
                        if layout.auto_size {
                            Dimension::auto()
                        } else {
                            Dimension::percent(1.0)
                        },
                    ),
                    height: resolve_dimension(
                        layout.height,
                        layout.height_full,
                        if layout.auto_size {
                            Dimension::auto()
                        } else {
                            Dimension::percent(1.0)
                        },
                    ),
                },
            },
            padding: taffy::geometry::Rect {
                left: taffy::style::LengthPercentage::length(layout.padding_left),
                top: taffy::style::LengthPercentage::length(layout.padding_top),
                right: taffy::style::LengthPercentage::length(layout.padding_right),
                bottom: taffy::style::LengthPercentage::length(layout.padding_bottom),
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
        ElementKind::Canvas(_) => Style {
            display: taffy::prelude::Display::Block,
            size: match layout.position {
                Position::Absolute => taffy::geometry::Size {
                    width: resolve_dimension(layout.width, layout.width_full, Dimension::auto()),
                    height: resolve_dimension(layout.height, layout.height_full, Dimension::auto()),
                },
                Position::Relative => taffy::geometry::Size {
                    width: resolve_dimension(
                        layout.width,
                        layout.width_full,
                        if layout.auto_size {
                            Dimension::auto()
                        } else {
                            Dimension::percent(1.0)
                        },
                    ),
                    height: resolve_dimension(
                        layout.height,
                        layout.height_full,
                        if layout.auto_size {
                            Dimension::auto()
                        } else {
                            Dimension::percent(1.0)
                        },
                    ),
                },
            },
            ..base_style(layout)
        },
        ElementKind::Lucide(_) => Style {
            size: taffy::geometry::Size {
                width: resolve_dimension(layout.width, layout.width_full, Dimension::length(24.0)),
                height: resolve_dimension(
                    layout.height,
                    layout.height_full,
                    Dimension::length(24.0),
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
        id: element.style.id.clone(),
        rect: LayoutRect {
            x: layout.location.x,
            y: layout.location.y,
            width: layout.size.width,
            height: layout.size.height,
        },
        children,
    })
}

fn base_style(layout: &ComputedLayoutStyle) -> Style {
    let mut style = Style {
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
            left: taffy::style::LengthPercentageAuto::length(layout.margin_left),
            top: taffy::style::LengthPercentageAuto::length(layout.margin_top),
            right: taffy::style::LengthPercentageAuto::length(layout.margin_right),
            bottom: taffy::style::LengthPercentageAuto::length(layout.margin_bottom),
        },
        flex_grow: layout.flex_grow,
        ..Default::default()
    };
    if let Some(flex_shrink) = layout.flex_shrink {
        style.flex_shrink = flex_shrink;
    }
    style
}

fn hash_draw_script_command(
    command: &crate::scene::script::CanvasCommand,
    state: &mut impl Hasher,
) {
    match command {
        crate::scene::script::CanvasCommand::Save => {
            0_u8.hash(state);
        }
        crate::scene::script::CanvasCommand::Restore => {
            1_u8.hash(state);
        }
        crate::scene::script::CanvasCommand::SetFillStyle { color } => {
            2_u8.hash(state);
            color.hash(state);
        }
        crate::scene::script::CanvasCommand::SetStrokeStyle { color } => {
            3_u8.hash(state);
            color.hash(state);
        }
        crate::scene::script::CanvasCommand::SetLineWidth { width } => {
            4_u8.hash(state);
            hash_f32(*width, state);
        }
        crate::scene::script::CanvasCommand::SetLineCap { cap } => {
            5_u8.hash(state);
            cap.hash(state);
        }
        crate::scene::script::CanvasCommand::SetLineJoin { join } => {
            6_u8.hash(state);
            join.hash(state);
        }
        crate::scene::script::CanvasCommand::SetLineDash { intervals, phase } => {
            7_u8.hash(state);
            intervals.len().hash(state);
            for interval in intervals {
                hash_f32(*interval, state);
            }
            hash_f32(*phase, state);
        }
        crate::scene::script::CanvasCommand::ClearLineDash => {
            8_u8.hash(state);
        }
        crate::scene::script::CanvasCommand::SetGlobalAlpha { alpha } => {
            9_u8.hash(state);
            hash_f32(*alpha, state);
        }
        crate::scene::script::CanvasCommand::Translate { x, y } => {
            10_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
        }
        crate::scene::script::CanvasCommand::Scale { x, y } => {
            11_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
        }
        crate::scene::script::CanvasCommand::Rotate { degrees } => {
            12_u8.hash(state);
            hash_f32(*degrees, state);
        }
        crate::scene::script::CanvasCommand::ClipRect {
            x,
            y,
            width,
            height,
        } => {
            13_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
        }
        crate::scene::script::CanvasCommand::Clear { color } => {
            14_u8.hash(state);
            color.hash(state);
        }
        crate::scene::script::CanvasCommand::FillRect {
            x,
            y,
            width,
            height,
            color,
        } => {
            15_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
            color.hash(state);
        }
        crate::scene::script::CanvasCommand::FillRRect {
            x,
            y,
            width,
            height,
            radius,
        } => {
            16_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
            hash_f32(*radius, state);
        }
        crate::scene::script::CanvasCommand::StrokeRect {
            x,
            y,
            width,
            height,
            color,
            stroke_width,
        } => {
            17_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
            color.hash(state);
            hash_f32(*stroke_width, state);
        }
        crate::scene::script::CanvasCommand::StrokeRRect {
            x,
            y,
            width,
            height,
            radius,
        } => {
            18_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
            hash_f32(*radius, state);
        }
        crate::scene::script::CanvasCommand::DrawLine { x0, y0, x1, y1 } => {
            19_u8.hash(state);
            hash_f32(*x0, state);
            hash_f32(*y0, state);
            hash_f32(*x1, state);
            hash_f32(*y1, state);
        }
        crate::scene::script::CanvasCommand::FillCircle { cx, cy, radius } => {
            20_u8.hash(state);
            hash_f32(*cx, state);
            hash_f32(*cy, state);
            hash_f32(*radius, state);
        }
        crate::scene::script::CanvasCommand::StrokeCircle { cx, cy, radius } => {
            21_u8.hash(state);
            hash_f32(*cx, state);
            hash_f32(*cy, state);
            hash_f32(*radius, state);
        }
        crate::scene::script::CanvasCommand::BeginPath => {
            22_u8.hash(state);
        }
        crate::scene::script::CanvasCommand::MoveTo { x, y } => {
            23_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
        }
        crate::scene::script::CanvasCommand::LineTo { x, y } => {
            24_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
        }
        crate::scene::script::CanvasCommand::QuadTo { cx, cy, x, y } => {
            25_u8.hash(state);
            hash_f32(*cx, state);
            hash_f32(*cy, state);
            hash_f32(*x, state);
            hash_f32(*y, state);
        }
        crate::scene::script::CanvasCommand::CubicTo {
            c1x,
            c1y,
            c2x,
            c2y,
            x,
            y,
        } => {
            26_u8.hash(state);
            hash_f32(*c1x, state);
            hash_f32(*c1y, state);
            hash_f32(*c2x, state);
            hash_f32(*c2y, state);
            hash_f32(*x, state);
            hash_f32(*y, state);
        }
        crate::scene::script::CanvasCommand::ClosePath => {
            27_u8.hash(state);
        }
        crate::scene::script::CanvasCommand::FillPath => {
            28_u8.hash(state);
        }
        crate::scene::script::CanvasCommand::StrokePath => {
            29_u8.hash(state);
        }
        crate::scene::script::CanvasCommand::DrawImage {
            asset_id,
            x,
            y,
            width,
            height,
            object_fit,
        } => {
            30_u8.hash(state);
            asset_id.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
            object_fit.hash(state);
        }
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
        element::resolve::resolve_ui_tree,
        element::tree::ElementNode,
        layout::tree::LayoutNode,
        parse,
        resource::{assets::AssetsMap, media::MediaContext},
        scene::primitives::{div, lucide, text},
        style::ComputedTextStyle,
    };
    use taffy::{AvailableSpace, geometry::Size};

    fn find_node_by_id<'a>(node: &'a LayoutNode, id: &str) -> Option<&'a LayoutNode> {
        if node.id == id {
            return Some(node);
        }

        node.children
            .iter()
            .find_map(|child| find_node_by_id(child, id))
    }

    fn find_element_by_id<'a>(node: &'a ElementNode, id: &str) -> Option<&'a ElementNode> {
        if node.style.id == id {
            return Some(node);
        }

        node.children
            .iter()
            .find_map(|child| find_element_by_id(child, id))
    }

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
            super::default_text_engine().as_ref(),
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

        let first = div().id("root").child(text("A").id("label")).into();
        let second = div()
            .id("root")
            .opacity(0.4)
            .child(text("A").id("label").text_red())
            .into();

        let first_resolved = resolve_ui_tree(&first, &frame_ctx, &mut media, &mut assets, None)
            .expect("first tree should resolve");
        let second_resolved = resolve_ui_tree(&second, &frame_ctx, &mut media, &mut assets, None)
            .expect("second tree should resolve");

        let (_, first_stats) = session
            .compute_layout(&first_resolved, &frame_ctx)
            .expect("first layout should succeed");
        let (second_layout, second_stats) = session
            .compute_layout(&second_resolved, &frame_ctx)
            .expect("second layout should succeed");

        assert!(first_stats.structure_rebuild);
        assert_eq!(second_stats.layout_dirty_nodes, 0);
        assert!(second_stats.raster_dirty_nodes >= 1);
        assert!(second_stats.composite_dirty_nodes >= 1);
        assert_eq!(second_layout.root.rect.width, 320.0);
        assert_eq!(second_layout.root.children[0].id, "label");
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

        let first = div().id("root").child(text("A").id("label")).into();
        let second = div()
            .id("root")
            .child(text("A").id("label").text_px(48.0))
            .into();

        let first_resolved = resolve_ui_tree(&first, &frame_ctx, &mut media, &mut assets, None)
            .expect("first tree should resolve");
        let second_resolved = resolve_ui_tree(&second, &frame_ctx, &mut media, &mut assets, None)
            .expect("second tree should resolve");

        session
            .compute_layout(&first_resolved, &frame_ctx)
            .expect("first layout should succeed");
        let (_, second_stats) = session
            .compute_layout(&second_resolved, &frame_ctx)
            .expect("second layout should succeed");

        assert!(second_stats.layout_dirty_nodes >= 1);
    }

    #[test]
    fn layout_session_marks_opacity_change_as_composite_dirty() {
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

        let first = div().id("root").child(text("A").id("label")).into();
        let second = div()
            .id("root")
            .opacity(0.5)
            .child(text("A").id("label"))
            .into();

        let first_resolved = resolve_ui_tree(&first, &frame_ctx, &mut media, &mut assets, None)
            .expect("first tree should resolve");
        let second_resolved = resolve_ui_tree(&second, &frame_ctx, &mut media, &mut assets, None)
            .expect("second tree should resolve");

        session
            .compute_layout(&first_resolved, &frame_ctx)
            .expect("first layout should succeed");
        let (_, second_stats) = session
            .compute_layout(&second_resolved, &frame_ctx)
            .expect("second layout should succeed");

        assert_eq!(second_stats.layout_dirty_nodes, 0);
        assert_eq!(second_stats.raster_dirty_nodes, 0);
        assert!(second_stats.composite_dirty_nodes >= 1);
    }

    #[test]
    fn layout_session_marks_transform_change_as_composite_dirty() {
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

        let first = div().id("root").child(text("A").id("label")).into();
        let second = div()
            .id("root")
            .rotate_deg(12.0)
            .child(text("A").id("label"))
            .into();

        let first_resolved = resolve_ui_tree(&first, &frame_ctx, &mut media, &mut assets, None)
            .expect("first tree should resolve");
        let second_resolved = resolve_ui_tree(&second, &frame_ctx, &mut media, &mut assets, None)
            .expect("second tree should resolve");

        session
            .compute_layout(&first_resolved, &frame_ctx)
            .expect("first layout should succeed");
        let (_, second_stats) = session
            .compute_layout(&second_resolved, &frame_ctx)
            .expect("second layout should succeed");

        assert_eq!(second_stats.layout_dirty_nodes, 0);
        assert_eq!(second_stats.raster_dirty_nodes, 0);
        assert!(second_stats.composite_dirty_nodes >= 1);
    }

    #[test]
    fn layout_session_marks_lucide_color_change_as_raster_dirty() {
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
            .id("root")
            .child(lucide("play").id("icon").size(24.0, 24.0).text_blue())
            .into();
        let second = div()
            .id("root")
            .child(lucide("play").id("icon").size(24.0, 24.0).text_pink())
            .into();

        let first_resolved = resolve_ui_tree(&first, &frame_ctx, &mut media, &mut assets, None)
            .expect("first tree should resolve");
        let second_resolved = resolve_ui_tree(&second, &frame_ctx, &mut media, &mut assets, None)
            .expect("second tree should resolve");

        session
            .compute_layout(&first_resolved, &frame_ctx)
            .expect("first layout should succeed");
        let (_, second_stats) = session
            .compute_layout(&second_resolved, &frame_ctx)
            .expect("second layout should succeed");

        assert_eq!(second_stats.layout_dirty_nodes, 0);
        assert!(second_stats.raster_dirty_nodes >= 1);
    }

    #[test]
    fn parsed_div_without_flex_class_uses_block_layout_flow() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();

        let parsed = parse(
            r#"{"type":"composition","width":320,"height":180,"fps":30,"frames":1}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"header","parentId":"root","type":"div","className":"pt-[20px] pb-[20px]"}
{"id":"header-text","parentId":"header","type":"text","className":"text-[24px]","text":"Header"}
{"id":"content","parentId":"root","type":"div","className":"pt-[10px] pb-[10px]"}
{"id":"content-text","parentId":"content","type":"text","className":"text-[18px]","text":"Content"}"#,
        )
        .expect("jsonl should parse");

        let resolved = resolve_ui_tree(&parsed.root, &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");
        let layout = super::compute_layout(&resolved, &frame_ctx).expect("layout should succeed");

        assert!(
            layout.root.children[1].rect.y
                >= layout.root.children[0].rect.y + layout.root.children[0].rect.height,
            "expected block flow to stack siblings vertically"
        );
        assert!(
            layout.root.children[0].rect.height < frame_ctx.height as f32,
            "expected auto-height block container instead of full-height expansion"
        );
    }

    #[test]
    fn parsed_text_wraps_within_parent_card_width() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 220,
            height: 180,
            frames: 1,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();

        let parsed = parse(
            r#"{"type":"composition","width":220,"height":180,"fps":30,"frames":1}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"card","parentId":"root","type":"div","className":"w-[160px] px-[8px] py-[8px]"}
{"id":"body","parentId":"card","type":"text","className":"text-[16px]","text":"从微小的原子到浩瀚的宇宙，科学无处不在。保持好奇心，勇敢提问。"}"#,
        )
        .expect("jsonl should parse");

        let resolved = resolve_ui_tree(&parsed.root, &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");
        let layout = super::compute_layout(&resolved, &frame_ctx).expect("layout should succeed");
        let body = find_element_by_id(&resolved, "body").expect("expected resolved body node");
        let text_style = body.style.text;
        let text_node = &layout.root.children[0].children[0];

        assert!(
            text_node.rect.height > text_style.text_px * text_style.line_height,
            "expected parsed text to wrap into multiple lines within card width"
        );
        assert!(
            text_node.rect.width <= 160.0,
            "expected wrapped text width to stay within the card"
        );
    }

    #[test]
    fn auto_sized_flex_column_labels_stay_single_line() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 390,
            height: 160,
            frames: 1,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();

        let parsed = parse(
            r#"{"type":"composition","width":390,"height":160,"fps":30,"frames":1}
{"id":"root","parentId":null,"type":"div","className":"flex flex-row justify-between w-full px-[20px] py-[16px]"}
{"id":"cat-pizza","parentId":"root","type":"div","className":"flex flex-col items-center gap-[8px]"}
{"id":"cat-pizza-icon","parentId":"cat-pizza","type":"div","className":"w-[56px] h-[56px]"}
{"id":"cat-pizza-text","parentId":"cat-pizza","type":"text","className":"text-[12px] font-medium","text":"Pizza"}
{"id":"cat-burger","parentId":"root","type":"div","className":"flex flex-col items-center gap-[8px]"}
{"id":"cat-burger-icon","parentId":"cat-burger","type":"div","className":"w-[56px] h-[56px]"}
{"id":"cat-burger-text","parentId":"cat-burger","type":"text","className":"text-[12px] font-medium","text":"Burger"}
{"id":"cat-sushi","parentId":"root","type":"div","className":"flex flex-col items-center gap-[8px]"}
{"id":"cat-sushi-icon","parentId":"cat-sushi","type":"div","className":"w-[56px] h-[56px]"}
{"id":"cat-sushi-text","parentId":"cat-sushi","type":"text","className":"text-[12px] font-medium","text":"Sushi"}
{"id":"cat-salad","parentId":"root","type":"div","className":"flex flex-col items-center gap-[8px]"}
{"id":"cat-salad-icon","parentId":"cat-salad","type":"div","className":"w-[56px] h-[56px]"}
{"id":"cat-salad-text","parentId":"cat-salad","type":"text","className":"text-[12px] font-medium","text":"Salad"}"#,
        )
        .expect("jsonl should parse");

        let resolved = resolve_ui_tree(&parsed.root, &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");
        let layout = super::compute_layout(&resolved, &frame_ctx).expect("layout should succeed");

        for id in ["cat-burger-text", "cat-sushi-text", "cat-salad-text"] {
            let node = find_node_by_id(&layout.root, id).expect("expected text node");
            let text_node = find_element_by_id(&resolved, id).expect("expected resolved text node");
            assert!(
                node.rect.height
                    <= text_node.style.text.text_px * text_node.style.text.line_height + 0.5,
                "expected {id} to stay on one line"
            );
        }
    }

    #[test]
    fn auto_sized_flex_column_text_prefers_single_line_before_wrapping() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 390,
            height: 160,
            frames: 1,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();

        let parsed = parse(
            r#"{"type":"composition","width":390,"height":160,"fps":30,"frames":1}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"promo-banner","parentId":"root","type":"div","className":"flex flex-row items-center w-[350px] px-[20px] py-[16px]"}
{"id":"promo-text","parentId":"promo-banner","type":"div","className":"flex flex-col gap-[4px]"}
{"id":"promo-title","parentId":"promo-text","type":"text","className":"text-[18px] font-bold","text":"50% OFF"}
{"id":"promo-desc","parentId":"promo-text","type":"text","className":"text-[13px]","text":"First order discount"}"#,
        )
        .expect("jsonl should parse");

        let resolved = resolve_ui_tree(&parsed.root, &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");
        let layout = super::compute_layout(&resolved, &frame_ctx).expect("layout should succeed");
        let node = find_node_by_id(&layout.root, "promo-desc").expect("expected promo text node");
        let text_node =
            find_element_by_id(&resolved, "promo-desc").expect("expected resolved promo text");

        assert!(
            node.rect.height
                <= text_node.style.text.text_px * text_node.style.text.line_height + 0.5,
            "expected promo copy to remain on one line while its column is auto-sized"
        );
    }

    #[test]
    fn fixed_width_flex_column_text_wraps() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 390,
            height: 160,
            frames: 1,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();

        let parsed = parse(
            r#"{"type":"composition","width":390,"height":160,"fps":30,"frames":1}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"promo-text","parentId":"root","type":"div","className":"flex flex-col w-[80px] gap-[4px]"}
{"id":"promo-title","parentId":"promo-text","type":"text","className":"text-[18px] font-bold","text":"50% OFF"}
{"id":"promo-desc","parentId":"promo-text","type":"text","className":"text-[13px]","text":"First order discount"}"#,
        )
        .expect("jsonl should parse");

        let resolved = resolve_ui_tree(&parsed.root, &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");
        let layout = super::compute_layout(&resolved, &frame_ctx).expect("layout should succeed");
        let node = find_node_by_id(&layout.root, "promo-desc").expect("expected promo text node");
        let text_node =
            find_element_by_id(&resolved, "promo-desc").expect("expected resolved promo text");

        assert!(
            node.rect.height > text_node.style.text.text_px * text_node.style.text.line_height,
            "expected promo copy to wrap when the flex column has a fixed width"
        );
    }

    #[test]
    fn stretched_flex_column_in_fixed_width_card_wraps_text() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 520,
            height: 220,
            frames: 1,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();

        let parsed = parse(
            r#"{"type":"composition","width":520,"height":220,"fps":30,"frames":1}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"card","parentId":"root","type":"div","className":"flex flex-col w-[440px] border-2 border-blue-200"}
{"id":"card-body","parentId":"card","type":"div","className":"flex flex-col gap-[16px] p-[20px]"}
{"id":"card-text","parentId":"card-body","type":"text","className":"text-[15px] text-slate-600 leading-relaxed","text":"从微小的原子到浩瀚的宇宙，科学无处不在。保持好奇心，勇敢提问，每一次实验都是新的发现！"}"#,
        )
        .expect("jsonl should parse");

        let resolved = resolve_ui_tree(&parsed.root, &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");
        let layout = super::compute_layout(&resolved, &frame_ctx).expect("layout should succeed");
        let node = find_node_by_id(&layout.root, "card-text").expect("expected card text node");
        let text_node =
            find_element_by_id(&resolved, "card-text").expect("expected resolved card text");

        assert!(
            node.rect.height > text_node.style.text.text_px * text_node.style.text.line_height,
            "expected card copy to wrap within a stretched flex-column body"
        );
        assert!(
            node.rect.width <= 440.0 - 40.0,
            "expected wrapped card copy to stay within the card body width"
        );
    }

    #[test]
    fn fixed_width_flex_row_text_stays_single_line() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 390,
            height: 120,
            frames: 1,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();

        let parsed = parse(
            r#"{"type":"composition","width":390,"height":120,"fps":30,"frames":1}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"status-bar","parentId":"root","type":"div","className":"flex flex-row justify-between items-center w-full h-[44px] px-[24px]"}
{"id":"status-time","parentId":"status-bar","type":"text","className":"text-[15px] font-semibold","text":"9:41"}"#,
        )
        .expect("jsonl should parse");

        let resolved = resolve_ui_tree(&parsed.root, &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");
        let layout = super::compute_layout(&resolved, &frame_ctx).expect("layout should succeed");
        let node = find_node_by_id(&layout.root, "status-time").expect("expected status text node");
        let text_node =
            find_element_by_id(&resolved, "status-time").expect("expected resolved status text");

        assert!(
            node.rect.height
                <= text_node.style.text.text_px * text_node.style.text.line_height + 0.5,
            "expected status-bar text to remain single-line inside a fixed-width flex row"
        );
    }

    #[test]
    fn flex_column_children_stretch_by_default() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();

        let root = div()
            .id("root")
            .size(320.0, 180.0)
            .flex_col()
            .child(div().id("header").h(40.0))
            .into();

        let resolved = resolve_ui_tree(&root, &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");
        let layout = super::compute_layout(&resolved, &frame_ctx).expect("layout should succeed");

        assert_eq!(
            layout.root.children[0].rect.width, 320.0,
            "expected flex column child to stretch across the container width by default"
        );
    }
}
