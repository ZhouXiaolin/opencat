pub mod tree;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use anyhow::Result;
use taffy::{
    AvailableSpace, TaffyTree,
    prelude::{
        AlignContent as TaffyAlignContent, Dimension, JustifyContent as TaffyJustifyContent, Style,
    },
};

use crate::{
    FrameCtx,
    element::{
        style::ComputedLayoutStyle,
        tree::{ElementKind, ElementNode},
    },
    layout::tree::{LayoutNode, LayoutRect, LayoutTree},
    scene::primitives::{AlignItems, JustifyContent, Position},
    style::{ComputedTextStyle, LengthPercentageAuto},
    text::{TextMeasureRequest, TextMeasurer},
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

    #[cfg(test)]
    pub fn compute_layout(
        &mut self,
        root: &ElementNode,
        frame_ctx: &FrameCtx,
    ) -> Result<(LayoutTree, LayoutPassStats)> {
        let text_measurer = default_text_measurer();
        self.compute_layout_with_text_engine(root, frame_ctx, text_measurer.as_ref())
    }

    pub(crate) fn compute_layout_with_text_engine(
        &mut self,
        root: &ElementNode,
        frame_ctx: &FrameCtx,
        text_measurer: &dyn TextMeasurer,
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
                    measure_node(
                        known_dimensions,
                        available_space,
                        node_context,
                        text_measurer,
                    )
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

#[cfg(test)]
pub fn compute_layout(root: &ElementNode, frame_ctx: &FrameCtx) -> Result<LayoutTree> {
    let text_measurer = default_text_measurer();
    compute_layout_with_text_engine(root, frame_ctx, text_measurer.as_ref())
}

#[cfg(test)]
fn default_text_measurer() -> crate::text::SharedTextMeasurer {
    crate::backend::skia::text::shared_text_engine()
}

#[cfg(test)]
pub(crate) fn compute_layout_with_text_engine(
    root: &ElementNode,
    frame_ctx: &FrameCtx,
    text_measurer: &dyn TextMeasurer,
) -> Result<LayoutTree> {
    let mut session = LayoutSession::new();
    let (layout_tree, _) =
        session.compute_layout_with_text_engine(root, frame_ctx, text_measurer)?;
    Ok(layout_tree)
}

fn measure_node(
    known_dimensions: taffy::geometry::Size<Option<f32>>,
    available_space: taffy::geometry::Size<AvailableSpace>,
    node_context: Option<&mut TextMeasureContext>,
    text_measurer: &dyn TextMeasurer,
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

    let measured = text_measurer.measure(&TextMeasureRequest {
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
    hash_option_length_percentage_auto(style.inset_left, state);
    hash_option_length_percentage_auto(style.inset_top, state);
    hash_option_length_percentage_auto(style.inset_right, state);
    hash_option_length_percentage_auto(style.inset_bottom, state);
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
    style.flex_wrap.hash(state);
    style.justify_content.hash(state);
    style.align_items.hash(state);
    style.align_content.hash(state);
    style.align_self.hash(state);
    hash_f32(style.gap, state);
    hash_option_length_percentage_auto(style.flex_basis, state);
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
    hash_option_f32(style.line_height_px, state);
    style.wrap_text.hash(state);
}

fn hash_text_layout_style(style: &ComputedTextStyle, state: &mut impl Hasher) {
    hash_f32(style.text_px, state);
    style.font_weight.hash(state);
    hash_f32(style.letter_spacing, state);
    hash_f32(style.line_height, state);
    hash_option_f32(style.line_height_px, state);
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

fn hash_option_length_percentage_auto(
    value: Option<LengthPercentageAuto>,
    state: &mut impl Hasher,
) {
    value.map(hash_length_percentage_auto_bits).hash(state);
}

fn hash_length_percentage_auto_bits(value: LengthPercentageAuto) -> (u8, u32) {
    match value {
        LengthPercentageAuto::Auto => (0, 0),
        LengthPercentageAuto::Length(length) => (1, length.to_bits()),
        LengthPercentageAuto::Percent(percent) => (2, percent.to_bits()),
    }
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
            flex_wrap: map_flex_wrap(layout.flex_wrap),
            align_content: layout.align_content.map(map_align_content),
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
            left: resolve_length_percentage_auto(layout.inset_left),
            top: resolve_length_percentage_auto(layout.inset_top),
            right: resolve_length_percentage_auto(layout.inset_right),
            bottom: resolve_length_percentage_auto(layout.inset_bottom),
        },
        margin: taffy::geometry::Rect {
            left: taffy::style::LengthPercentageAuto::length(layout.margin_left),
            top: taffy::style::LengthPercentageAuto::length(layout.margin_top),
            right: taffy::style::LengthPercentageAuto::length(layout.margin_right),
            bottom: taffy::style::LengthPercentageAuto::length(layout.margin_bottom),
        },
        flex_basis: layout
            .flex_basis
            .map(resolve_dimension_value)
            .unwrap_or(Dimension::auto()),
        flex_grow: layout.flex_grow,
        align_self: layout.align_self.map(map_align),
        ..Default::default()
    };
    if let Some(flex_shrink) = layout.flex_shrink {
        style.flex_shrink = flex_shrink;
    }
    style
}

fn resolve_length_percentage_auto(
    value: Option<LengthPercentageAuto>,
) -> taffy::style::LengthPercentageAuto {
    match value.unwrap_or(LengthPercentageAuto::Auto) {
        LengthPercentageAuto::Auto => taffy::style::LengthPercentageAuto::auto(),
        LengthPercentageAuto::Length(length) => taffy::style::LengthPercentageAuto::length(length),
        LengthPercentageAuto::Percent(percent) => {
            taffy::style::LengthPercentageAuto::percent(percent)
        }
    }
}

fn resolve_dimension_value(value: LengthPercentageAuto) -> Dimension {
    match value {
        LengthPercentageAuto::Auto => Dimension::auto(),
        LengthPercentageAuto::Length(length) => Dimension::length(length),
        LengthPercentageAuto::Percent(percent) => Dimension::percent(percent),
    }
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
        Some(crate::style::FlexDirection::RowReverse) => taffy::prelude::FlexDirection::RowReverse,
        Some(crate::style::FlexDirection::ColReverse) => {
            taffy::prelude::FlexDirection::ColumnReverse
        }
    }
}

fn map_flex_wrap(value: crate::style::FlexWrap) -> taffy::prelude::FlexWrap {
    match value {
        crate::style::FlexWrap::NoWrap => taffy::prelude::FlexWrap::NoWrap,
        crate::style::FlexWrap::Wrap => taffy::prelude::FlexWrap::Wrap,
        crate::style::FlexWrap::WrapReverse => taffy::prelude::FlexWrap::WrapReverse,
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
        JustifyContent::Stretch => TaffyJustifyContent::Stretch,
    }
}

fn map_align(value: AlignItems) -> taffy::prelude::AlignItems {
    match value {
        AlignItems::Start => taffy::prelude::AlignItems::FlexStart,
        AlignItems::Center => taffy::prelude::AlignItems::Center,
        AlignItems::End => taffy::prelude::AlignItems::FlexEnd,
        AlignItems::Baseline => taffy::prelude::AlignItems::Baseline,
        AlignItems::Stretch => taffy::prelude::AlignItems::Stretch,
    }
}

fn map_align_content(value: JustifyContent) -> TaffyAlignContent {
    match value {
        JustifyContent::Start => TaffyAlignContent::FlexStart,
        JustifyContent::Center => TaffyAlignContent::Center,
        JustifyContent::End => TaffyAlignContent::FlexEnd,
        JustifyContent::Between => TaffyAlignContent::SpaceBetween,
        JustifyContent::Around => TaffyAlignContent::SpaceAround,
        JustifyContent::Evenly => TaffyAlignContent::SpaceEvenly,
        JustifyContent::Stretch => TaffyAlignContent::Stretch,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::{LayoutSession, TextMeasureContext, compute_layout_with_text_engine, measure_node};
    use crate::{
        FrameCtx,
        element::resolve::resolve_ui_tree,
        jsonl::tailwind::parse_class_name,
        resource::{assets::AssetsMap, media::MediaContext},
        scene::primitives::{div, lucide, text},
        style::ComputedTextStyle,
        text::{TextMeasureRequest, TextMeasurement, TextMeasurer},
    };
    use taffy::{AvailableSpace, geometry::Size};

    #[derive(Debug, Clone)]
    struct RecordedMeasure {
        text: String,
        max_width: f32,
        allow_wrap: bool,
    }

    #[derive(Default)]
    struct RecordingTextMeasurer {
        requests: Mutex<Vec<RecordedMeasure>>,
    }

    impl RecordingTextMeasurer {
        fn request_for(&self, text: &str) -> Option<RecordedMeasure> {
            self.requests
                .lock()
                .expect("recording lock should not be poisoned")
                .iter()
                .find(|request| request.text == text)
                .cloned()
        }

        fn requests_for(&self, text: &str) -> Vec<RecordedMeasure> {
            self.requests
                .lock()
                .expect("recording lock should not be poisoned")
                .iter()
                .filter(|request| request.text == text)
                .cloned()
                .collect()
        }
    }

    impl TextMeasurer for RecordingTextMeasurer {
        fn measure(&self, request: &TextMeasureRequest<'_>) -> TextMeasurement {
            self.requests
                .lock()
                .expect("recording lock should not be poisoned")
                .push(RecordedMeasure {
                    text: request.text.to_string(),
                    max_width: request.max_width,
                    allow_wrap: request.allow_wrap,
                });

            let line_height = request.style.resolved_line_height_px();
            TextMeasurement {
                width: if request.allow_wrap && request.max_width.is_finite() {
                    request.max_width.min(120.0).max(1.0)
                } else {
                    120.0
                },
                height: line_height.max(1.0),
            }
        }
    }

    fn classed_div(
        id: &'static str,
        class_name: &'static str,
        children: Vec<crate::Node>,
    ) -> crate::scene::primitives::Div {
        let mut node = div();
        node.style = parse_class_name(class_name);
        node.style.id = id.to_string();
        node.children = children;
        node
    }

    fn classed_text(
        id: &'static str,
        class_name: &'static str,
        content: &'static str,
    ) -> crate::scene::primitives::Text {
        let mut node = text(content);
        node.style = parse_class_name(class_name);
        node.style.id = id.to_string();
        node
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
            super::default_text_measurer().as_ref(),
        );

        assert!(
            measured.width > 80.0,
            "expected auto-width text to ignore narrow available width and remain single-line"
        );
    }

    #[test]
    fn block_text_wrapper_passes_container_width_to_text_measurement() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 340,
            height: 240,
            frames: 1,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();
        let root = classed_div(
            "root",
            "w-full h-full p-[20px]",
            vec![
                classed_div(
                    "tight-wrap",
                    "mb-[12px]",
                    vec![
                        classed_text("lead-tight", "text-[16px] leading-[18px]", "Tight leading")
                            .into(),
                    ],
                )
                .into(),
            ],
        )
        .into();
        let resolved = resolve_ui_tree(&root, &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");
        let measurer = RecordingTextMeasurer::default();

        let _layout = compute_layout_with_text_engine(&resolved, &frame_ctx, &measurer)
            .expect("layout should succeed");

        let requests = measurer.requests_for("Tight leading");
        assert!(
            !requests.is_empty(),
            "expected to record Tight leading measurement"
        );
        assert!(requests.iter().all(|request| request.allow_wrap));
        assert!(
            requests.iter().all(|request| request.max_width >= 280.0),
            "expected block wrapper text to always measure against container width, got {:?}",
            requests
        );
    }

    #[test]
    fn stretched_flex_item_wrapper_passes_container_width_to_text_measurement() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 240,
            frames: 1,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();
        let root = classed_div(
            "root",
            "w-full h-full p-[20px]",
            vec![
                classed_div(
                    "copy-card",
                    "flex flex-col gap-[8px] w-[180px]",
                    vec![
                        classed_div(
                            "copy-title-wrap",
                            "",
                            vec![
                                classed_text(
                                    "copy-title",
                                    "text-[20px] leading-[24px]",
                                    "Layout parity",
                                )
                                .into(),
                            ],
                        )
                        .into(),
                    ],
                )
                .into(),
            ],
        )
        .into();
        let resolved = resolve_ui_tree(&root, &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");
        let measurer = RecordingTextMeasurer::default();

        let _layout = compute_layout_with_text_engine(&resolved, &frame_ctx, &measurer)
            .expect("layout should succeed");

        let requests = measurer.requests_for("Layout parity");
        assert!(
            !requests.is_empty(),
            "expected to record Layout parity measurement"
        );
        assert!(requests.iter().all(|request| request.allow_wrap));
        assert!(
            requests.iter().all(|request| request.max_width >= 170.0),
            "expected stretched flex item wrapper text to always measure against card width, got {:?}",
            requests
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
}
