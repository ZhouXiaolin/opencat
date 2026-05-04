pub mod tree;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use anyhow::Result;
use taffy::{
    AvailableSpace, TaffyTree,
    prelude::{
        AlignContent as TaffyAlignContent, Dimension, FromFr,
        JustifyContent as TaffyJustifyContent, Style, TaffyAuto, TaffyGridLine,
    },
    style::TrackSizingFunction,
    style_helpers::{flex, span},
};

use crate::{
    FrameCtx,
    element::tree::{ElementKind, ElementNode},
    layout::tree::{LayoutNode, LayoutRect, LayoutTree},
    scene::primitives::{AlignItems, JustifyContent, Position},
    style::{ComputedTextStyle, LengthPercentageAuto},
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
    Timeline,
    Text,
    Bitmap,
    Canvas,
    SvgPath,
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
        let db = default_font_db();
        self.compute_layout_with_font_db(root, frame_ctx, &db)
    }

    pub(crate) fn compute_layout_with_font_db(
        &mut self,
        root: &ElementNode,
        frame_ctx: &FrameCtx,
        font_db: &fontdb::Database,
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
                        font_db,
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
    let db = default_font_db();
    compute_layout_with_font_db_fn(root, frame_ctx, &db)
}

#[cfg(test)]
fn default_font_db() -> fontdb::Database {
    crate::text::default_font_db(&[])
}

#[cfg(test)]
pub(crate) fn compute_layout_with_font_db_fn(
    root: &ElementNode,
    frame_ctx: &FrameCtx,
    font_db: &fontdb::Database,
) -> Result<LayoutTree> {
    let mut session = LayoutSession::new();
    let (layout_tree, _) =
        session.compute_layout_with_font_db(root, frame_ctx, font_db)?;
    Ok(layout_tree)
}

fn measure_node(
    known_dimensions: taffy::geometry::Size<Option<f32>>,
    available_space: taffy::geometry::Size<AvailableSpace>,
    node_context: Option<&mut TextMeasureContext>,
    font_db: &fontdb::Database,
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

    let measured = crate::text::measure_text(
        &text.text,
        &text.style,
        max_width,
        text.allow_wrap,
        font_db,
    );

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

    for (index, child) in ordered_children(element) {
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

fn ordered_children(element: &ElementNode) -> Vec<(usize, &ElementNode)> {
    let mut children = element.children.iter().enumerate().collect::<Vec<_>>();
    if element.style.layout.is_flex || element.style.layout.is_grid {
        children.sort_by_key(|(index, child)| (child.style.layout.order, *index));
    }
    children
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

    for ((index, child), cached_child) in ordered_children(element)
        .into_iter()
        .zip(cached.children.iter_mut())
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
        .zip(
            ordered_children(element)
                .into_iter()
                .map(|(_, child)| child),
        )
        .enumerate()
        .all(|(index, (cached_child, child))| same_structure(cached_child, child, index))
}

fn count_nodes(element: &ElementNode) -> usize {
    1 + element.children.iter().map(count_nodes).sum::<usize>()
}

fn cached_node_kind(element: &ElementNode) -> CachedNodeKind {
    match &element.kind {
        ElementKind::Div(_) => CachedNodeKind::Div,
        ElementKind::Timeline(_) => CachedNodeKind::Timeline,
        ElementKind::Text(_) => CachedNodeKind::Text,
        ElementKind::Bitmap(_) => CachedNodeKind::Bitmap,
        ElementKind::Canvas(_) => CachedNodeKind::Canvas,
        ElementKind::SvgPath(_) => CachedNodeKind::SvgPath,
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
    calculate_hash(&LayoutFingerprint(element))
}

fn raster_affect_hash(element: &ElementNode) -> u64 {
    calculate_hash(&RasterFingerprint(element))
}

fn composite_affect_hash(element: &ElementNode) -> u64 {
    calculate_hash(&CompositeFingerprint(element))
}

fn calculate_hash(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[derive(Clone, Copy)]
struct F32Hash(f32);

impl Hash for F32Hash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

struct LayoutFingerprint<'a>(&'a ElementNode);

impl Hash for LayoutFingerprint<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        LayoutStyleFingerprint(&self.0.style.layout).hash(state);
        self.0.style.visual.border_width.map(F32Hash).hash(state);
        self.0
            .style
            .visual
            .border_top_width
            .map(F32Hash)
            .hash(state);
        self.0
            .style
            .visual
            .border_right_width
            .map(F32Hash)
            .hash(state);
        self.0
            .style
            .visual
            .border_bottom_width
            .map(F32Hash)
            .hash(state);
        self.0
            .style
            .visual
            .border_left_width
            .map(F32Hash)
            .hash(state);

        match &self.0.kind {
            ElementKind::Div(_) | ElementKind::Timeline(_) | ElementKind::Canvas(_) => {}
            ElementKind::Text(text) => {
                text.text.hash(state);
                TextLayoutStyleFingerprint(&text.text_style).hash(state);
            }
            ElementKind::Bitmap(bitmap) => {
                bitmap.width.hash(state);
                bitmap.height.hash(state);
            }
            ElementKind::SvgPath(svg) => {
                svg.intrinsic_size
                    .map(|(w, h)| (F32Hash(w), F32Hash(h)))
                    .hash(state);
            }
        }
    }
}

struct RasterFingerprint<'a>(&'a ElementNode);

impl Hash for RasterFingerprint<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        RasterVisualStyleFingerprint(&self.0.style.visual).hash(state);

        match &self.0.kind {
            ElementKind::Div(_) | ElementKind::Timeline(_) => {}
            ElementKind::Text(text) => {
                text.text.hash(state);
                TextRasterStyleFingerprint(&text.text_style).hash(state);
            }
            ElementKind::Bitmap(bitmap) => {
                bitmap.asset_id.hash(state);
                bitmap.width.hash(state);
                bitmap.height.hash(state);
                bitmap.video_timing.hash(state);
            }
            ElementKind::Canvas(canvas) => {
                canvas.commands.hash(state);
            }
            ElementKind::SvgPath(svg) => {
                for data in &svg.path_data {
                    data.hash(state);
                }
                svg.view_box.map(F32Hash).hash(state);
            }
        }
    }
}

struct CompositeFingerprint<'a>(&'a ElementNode);

impl Hash for CompositeFingerprint<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        CompositeVisualStyleFingerprint(&self.0.style.visual).hash(state);
    }
}

struct LayoutStyleFingerprint<'a>(&'a crate::element::style::ComputedLayoutStyle);

impl Hash for LayoutStyleFingerprint<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let style = self.0;
        style.position.hash(state);
        style.inset_left.hash(state);
        style.inset_top.hash(state);
        style.inset_right.hash(state);
        style.inset_bottom.hash(state);
        style.width.map(F32Hash).hash(state);
        style.width_percent.map(F32Hash).hash(state);
        style.height.map(F32Hash).hash(state);
        style.max_width.map(F32Hash).hash(state);
        style.width_full.hash(state);
        style.height_full.hash(state);
        F32Hash(style.padding_top).hash(state);
        F32Hash(style.padding_right).hash(state);
        F32Hash(style.padding_bottom).hash(state);
        F32Hash(style.padding_left).hash(state);
        style.margin_top.hash(state);
        style.margin_right.hash(state);
        style.margin_bottom.hash(state);
        style.margin_left.hash(state);
        style.min_height.hash(state);
        style.is_flex.hash(state);
        style.is_grid.hash(state);
        style.grid_template_columns.hash(state);
        style.grid_template_rows.hash(state);
        style.grid_auto_flow.hash(state);
        style.grid_auto_rows.hash(state);
        style.col_start.hash(state);
        style.col_end.hash(state);
        style.row_start.hash(state);
        style.row_end.hash(state);
        style.auto_size.hash(state);
        style.flex_direction.hash(state);
        style.justify_content.hash(state);
        style.align_items.hash(state);
        style.flex_wrap.hash(state);
        style.align_content.hash(state);
        style.align_self.hash(state);
        style.justify_items.hash(state);
        style.justify_self.hash(state);
        F32Hash(style.gap).hash(state);
        style.gap_x.map(F32Hash).hash(state);
        style.gap_y.map(F32Hash).hash(state);
        style.order.hash(state);
        style.aspect_ratio.map(F32Hash).hash(state);
        style.flex_basis.hash(state);
        F32Hash(style.flex_grow).hash(state);
        style.flex_shrink.map(F32Hash).hash(state);
        style.z_index.hash(state);
        style.truncate.hash(state);
    }
}

struct RasterVisualStyleFingerprint<'a>(&'a crate::element::style::ComputedVisualStyle);

impl Hash for RasterVisualStyleFingerprint<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let style = self.0;
        style.background.hash(state);
        style.fill.hash(state);
        style.border_radius.hash(state);
        style.border_width.map(F32Hash).hash(state);
        style.border_top_width.map(F32Hash).hash(state);
        style.border_right_width.map(F32Hash).hash(state);
        style.border_bottom_width.map(F32Hash).hash(state);
        style.border_left_width.map(F32Hash).hash(state);
        style.border_color.hash(state);
        style.stroke_color.hash(state);
        style.stroke_width.map(F32Hash).hash(state);
        style.border_style.hash(state);
        style.blur_sigma.map(F32Hash).hash(state);
        style.object_fit.hash(state);
        style.clip_contents.hash(state);
        style.box_shadow.hash(state);
        style.inset_shadow.hash(state);
        style.drop_shadow.hash(state);
    }
}

struct CompositeVisualStyleFingerprint<'a>(&'a crate::element::style::ComputedVisualStyle);

impl Hash for CompositeVisualStyleFingerprint<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let style = self.0;
        F32Hash(style.opacity).hash(state);
        style.backdrop_blur_sigma.map(F32Hash).hash(state);
        style.transforms.hash(state);
    }
}

struct TextRasterStyleFingerprint<'a>(&'a ComputedTextStyle);

impl Hash for TextRasterStyleFingerprint<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let style = self.0;
        style.color.hash(state);
        F32Hash(style.text_px).hash(state);
        style.font_weight.hash(state);
        F32Hash(style.letter_spacing).hash(state);
        style.text_align.hash(state);
        F32Hash(style.line_height).hash(state);
        style.line_height_px.map(F32Hash).hash(state);
        style.text_transform.hash(state);
        style.wrap_text.hash(state);
        style.line_through.hash(state);
    }
}

struct TextLayoutStyleFingerprint<'a>(&'a ComputedTextStyle);

impl Hash for TextLayoutStyleFingerprint<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let style = self.0;
        F32Hash(style.text_px).hash(state);
        style.font_weight.hash(state);
        F32Hash(style.letter_spacing).hash(state);
        F32Hash(style.line_height).hash(state);
        style.line_height_px.map(F32Hash).hash(state);
        style.text_transform.hash(state);
        style.wrap_text.hash(state);
    }
}

fn text_measure_context_for_element(element: &ElementNode) -> Option<TextMeasureContext> {
    match &element.kind {
        ElementKind::Text(text) => Some(TextMeasureContext {
            text: text.text.clone(),
            style: text.text_style,
            allow_wrap: !element.style.layout.truncate
                && (element.style.text.wrap_text
                    || element.style.layout.width.is_some()
                    || element.style.layout.width_percent.is_some()
                    || element.style.layout.width_full),
        }),
        _ => None,
    }
}

fn taffy_style_for_element(element: &ElementNode) -> Style {
    let layout = &element.style.layout;
    match &element.kind {
        ElementKind::Div(_) | ElementKind::Timeline(_) => Style {
            display: if layout.is_grid {
                taffy::prelude::Display::Grid
            } else if layout.is_flex {
                taffy::prelude::Display::Flex
            } else {
                taffy::prelude::Display::Block
            },
            grid_template_columns: layout.grid_template_columns.map_or_else(Vec::new, |cols| {
                (0..cols)
                    .map(|_| taffy::style::GridTemplateComponent::Single(flex(1.0)))
                    .collect()
            }),
            grid_template_rows: layout.grid_template_rows.map_or_else(Vec::new, |rows| {
                (0..rows)
                    .map(|_| taffy::style::GridTemplateComponent::Single(flex(1.0)))
                    .collect()
            }),
            grid_auto_flow: layout.grid_auto_flow.map_or_else(
                taffy::style::GridAutoFlow::default,
                |flow| match flow {
                    crate::style::GridAutoFlow::Row => taffy::style::GridAutoFlow::Row,
                    crate::style::GridAutoFlow::Column => taffy::style::GridAutoFlow::Column,
                    crate::style::GridAutoFlow::RowDense => taffy::style::GridAutoFlow::RowDense,
                    crate::style::GridAutoFlow::ColumnDense => {
                        taffy::style::GridAutoFlow::ColumnDense
                    }
                },
            ),
            grid_auto_rows: layout.grid_auto_rows.map_or_else(Vec::new, |auto_rows| {
                use taffy::style::MaxTrackSizingFunction;
                vec![match auto_rows {
                    crate::style::GridAutoRows::Auto => {
                        // auto = minmax(auto, auto)
                        taffy::geometry::MinMax {
                            min: taffy::style::MinTrackSizingFunction::AUTO,
                            max: MaxTrackSizingFunction::AUTO,
                        }
                    }
                    crate::style::GridAutoRows::Min => taffy::geometry::MinMax {
                        min: taffy::style::MinTrackSizingFunction::AUTO,
                        max: MaxTrackSizingFunction::min_content(),
                    },
                    crate::style::GridAutoRows::Max => taffy::geometry::MinMax {
                        min: taffy::style::MinTrackSizingFunction::AUTO,
                        max: MaxTrackSizingFunction::max_content(),
                    },
                    crate::style::GridAutoRows::Fr => TrackSizingFunction::from_fr(1.0),
                }]
            }),
            justify_items: layout.justify_items.map(map_align_items_to_justify_items),
            grid_column: resolve_grid_axis_placement(layout.col_start, layout.col_end),
            grid_row: resolve_grid_axis_placement(layout.row_start, layout.row_end),
            max_size: taffy::geometry::Size {
                width: layout
                    .max_width
                    .map(Dimension::length)
                    .unwrap_or(Dimension::auto()),
                height: Dimension::auto(),
            },
            min_size: taffy::geometry::Size {
                width: Dimension::auto(),
                height: layout.min_height.map_or_else(Dimension::auto, |v| match v {
                    crate::style::LengthPercentageAuto::Auto => Dimension::auto(),
                    crate::style::LengthPercentageAuto::Length(px) => Dimension::length(px),
                    crate::style::LengthPercentageAuto::Percent(pct) => {
                        let resolved = if pct < 0.0 { 1.0 } else { pct };
                        Dimension::percent(resolved)
                    }
                }),
            },
            size: match layout.position {
                Position::Absolute => taffy::geometry::Size {
                    width: resolve_dimension(
                        layout.width,
                        layout.width_percent,
                        layout.width_full,
                        Dimension::auto(),
                    ),
                    height: resolve_dimension(
                        layout.height,
                        None,
                        layout.height_full,
                        Dimension::auto(),
                    ),
                },
                Position::Relative => taffy::geometry::Size {
                    width: resolve_dimension(
                        layout.width,
                        layout.width_percent,
                        layout.width_full,
                        if layout.auto_size {
                            Dimension::auto()
                        } else {
                            Dimension::percent(1.0)
                        },
                    ),
                    height: resolve_dimension(
                        layout.height,
                        None,
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
                width: taffy::style::LengthPercentage::length(layout.gap_x.unwrap_or(layout.gap)),
                height: taffy::style::LengthPercentage::length(layout.gap_y.unwrap_or(layout.gap)),
            },
            flex_wrap: map_flex_wrap(layout.flex_wrap),
            align_content: layout.align_content.map(map_align_content),
            ..base_style(element)
        },
        ElementKind::Text(_) => Style {
            size: taffy::geometry::Size {
                width: resolve_dimension(
                    layout.width,
                    layout.width_percent,
                    layout.width_full,
                    Dimension::auto(),
                ),
                height: resolve_dimension(
                    layout.height,
                    None,
                    layout.height_full,
                    Dimension::auto(),
                ),
            },
            ..base_style(element)
        },
        ElementKind::Bitmap(bitmap) => Style {
            size: taffy::geometry::Size {
                width: resolve_dimension(
                    layout.width,
                    layout.width_percent,
                    layout.width_full,
                    Dimension::length(bitmap.width as f32),
                ),
                height: resolve_dimension(
                    layout.height,
                    None,
                    layout.height_full,
                    Dimension::length(bitmap.height as f32),
                ),
            },
            ..base_style(element)
        },
        ElementKind::Canvas(_) => Style {
            display: taffy::prelude::Display::Block,
            size: match layout.position {
                Position::Absolute => taffy::geometry::Size {
                    width: resolve_dimension(
                        layout.width,
                        layout.width_percent,
                        layout.width_full,
                        Dimension::auto(),
                    ),
                    height: resolve_dimension(
                        layout.height,
                        None,
                        layout.height_full,
                        Dimension::auto(),
                    ),
                },
                Position::Relative => taffy::geometry::Size {
                    width: resolve_dimension(
                        layout.width,
                        layout.width_percent,
                        layout.width_full,
                        if layout.auto_size {
                            Dimension::auto()
                        } else {
                            Dimension::percent(1.0)
                        },
                    ),
                    height: resolve_dimension(
                        layout.height,
                        None,
                        layout.height_full,
                        if layout.auto_size {
                            Dimension::auto()
                        } else {
                            Dimension::percent(1.0)
                        },
                    ),
                },
            },
            ..base_style(element)
        },
        ElementKind::SvgPath(svg) => {
            let default_size = svg
                .intrinsic_size
                .map(|(w, h)| (Dimension::length(w), Dimension::length(h)))
                .unwrap_or((Dimension::auto(), Dimension::auto()));
            Style {
                size: taffy::geometry::Size {
                    width: resolve_dimension(
                        layout.width,
                        layout.width_percent,
                        layout.width_full,
                        default_size.0,
                    ),
                    height: resolve_dimension(
                        layout.height,
                        None,
                        layout.height_full,
                        default_size.1,
                    ),
                },
                ..base_style(element)
            }
        }
    }
}

fn resolve_grid_placement(placement: crate::style::GridPlacement) -> taffy::style::GridPlacement {
    match placement {
        crate::style::GridPlacement::Auto => taffy::style::GridPlacement::Auto,
        crate::style::GridPlacement::Line(index) => {
            taffy::style::GridPlacement::from_line_index(index)
        }
        crate::style::GridPlacement::Span(span_count) => span(span_count),
    }
}

fn resolve_grid_axis_placement(
    start: Option<crate::style::GridPlacement>,
    end: Option<crate::style::GridPlacement>,
) -> taffy::geometry::Line<taffy::style::GridPlacement> {
    match (start, end) {
        (None, None) => taffy::geometry::Line::default(),
        (Some(start), None) => taffy::geometry::Line {
            start: resolve_grid_placement(start),
            end: span(1),
        },
        (None, Some(end)) => taffy::geometry::Line {
            start: span(1),
            end: resolve_grid_placement(end),
        },
        (Some(start), Some(end)) => taffy::geometry::Line {
            start: resolve_grid_placement(start),
            end: resolve_grid_placement(end),
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

    for ((_, element_child), taffy_child) in ordered_children(element)
        .into_iter()
        .zip(taffy_children.into_iter())
    {
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

fn base_style(element: &ElementNode) -> Style {
    let layout = &element.style.layout;
    let uniform_border = element.style.visual.border_width.unwrap_or(0.0);
    let border_top = element
        .style
        .visual
        .border_top_width
        .unwrap_or(uniform_border);
    let border_right = element
        .style
        .visual
        .border_right_width
        .unwrap_or(uniform_border);
    let border_bottom = element
        .style
        .visual
        .border_bottom_width
        .unwrap_or(uniform_border);
    let border_left = element
        .style
        .visual
        .border_left_width
        .unwrap_or(uniform_border);
    let mut style = Style {
        position: map_position(layout.position),
        inset: taffy::geometry::Rect {
            left: resolve_length_percentage_auto(layout.inset_left),
            top: resolve_length_percentage_auto(layout.inset_top),
            right: resolve_length_percentage_auto(layout.inset_right),
            bottom: resolve_length_percentage_auto(layout.inset_bottom),
        },
        margin: taffy::geometry::Rect {
            left: resolve_length_percentage_auto(Some(layout.margin_left)),
            top: resolve_length_percentage_auto(Some(layout.margin_top)),
            right: resolve_length_percentage_auto(Some(layout.margin_right)),
            bottom: resolve_length_percentage_auto(Some(layout.margin_bottom)),
        },
        flex_basis: layout
            .flex_basis
            .map(resolve_dimension_value)
            .unwrap_or(Dimension::auto()),
        flex_grow: layout.flex_grow,
        align_self: layout.align_self.map(map_align),
        justify_self: layout.justify_self.map(map_align_items_to_justify_self),
        aspect_ratio: layout.aspect_ratio,
        border: taffy::geometry::Rect {
            left: taffy::style::LengthPercentage::length(border_left),
            top: taffy::style::LengthPercentage::length(border_top),
            right: taffy::style::LengthPercentage::length(border_right),
            bottom: taffy::style::LengthPercentage::length(border_bottom),
        },
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

fn resolve_dimension(
    value: Option<f32>,
    percent: Option<f32>,
    full: bool,
    fallback: Dimension,
) -> Dimension {
    if full {
        Dimension::percent(1.0)
    } else if let Some(pct) = percent {
        Dimension::percent(pct)
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

fn map_align_items_to_justify_items(value: AlignItems) -> taffy::prelude::AlignItems {
    map_align(value)
}

fn map_align_items_to_justify_self(value: AlignItems) -> taffy::prelude::AlignItems {
    map_align(value)
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
    use super::{LayoutSession, TextMeasureContext, compute_layout_with_font_db_fn, default_font_db, measure_node};
    use crate::{
        FrameCtx,
        element::resolve::resolve_ui_tree,
        jsonl::tailwind::parse_class_name,
        resource::{assets::AssetsMap, media::MediaContext},
        scene::primitives::{div, lucide, path, text},
        style::{ColorToken, ComputedTextStyle},
    };
    use taffy::{AvailableSpace, geometry::Size};

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
            &super::default_font_db(),
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

        let layout = compute_layout_with_font_db_fn(&resolved, &frame_ctx, &default_font_db())
            .expect("layout should succeed");

        // Verify the text node is laid out within the padded container
        let text_node = &layout.root.children[0].children[0];
        assert!(
            text_node.rect.width > 0.0,
            "text node should have non-zero width"
        );
        assert!(
            text_node.rect.width <= 300.0,
            "text node should fit within padded container (340 - 40 padding)"
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

        let layout = compute_layout_with_font_db_fn(&resolved, &frame_ctx, &default_font_db())
            .expect("layout should succeed");

        // Verify text node fits within the card
        let card = &layout.root.children[0];
        let title_wrap = &card.children[0];
        let title_text = &title_wrap.children[0];
        assert!(
            title_text.rect.width > 0.0,
            "title text should have non-zero width"
        );
        assert!(
            title_text.rect.width <= 180.0,
            "title text should fit within card width"
        );
    }

    #[test]
    fn absolute_auto_width_container_does_not_wrap_text_descendant() {
        // CSS `position: absolute` with `width: auto` resolves via shrink-to-fit:
        // its width comes from its own content, not from the containing block.
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 1280,
            height: 720,
            frames: 1,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();
        let root = classed_div(
            "root",
            "relative w-[1280px] h-[720px]",
            vec![
                classed_div(
                    "pill",
                    "absolute left-[32px] top-[28px] px-[18px] py-[10px] rounded-full",
                    vec![
                        classed_text(
                            "pill-text",
                            "text-[12px] font-semibold tracking-[2px]",
                            "TIMELINE NODE",
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

        let layout = compute_layout_with_font_db_fn(&resolved, &frame_ctx, &default_font_db())
            .expect("layout should succeed");

        // Verify pill text is positioned absolutely and sized by its content
        let pill = &layout.root.children[0];
        assert_eq!(pill.rect.x, 32.0, "pill should be at left 32px");
        assert_eq!(pill.rect.y, 28.0, "pill should be at top 28px");
        let pill_text = &pill.children[0];
        assert!(
            pill_text.rect.width > 0.0,
            "pill text should have non-zero width"
        );
    }

    #[test]
    fn percent_width_text_passes_constrained_width_to_text_measurement() {
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
            "w-[200px] h-full",
            vec![
                classed_text(
                    "headline",
                    "w-[50%] text-[16px]",
                    "Percent constrained copy",
                )
                .into(),
            ],
        )
        .into();
        let resolved = resolve_ui_tree(&root, &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");

        let layout = compute_layout_with_font_db_fn(&resolved, &frame_ctx, &default_font_db())
            .expect("layout should succeed");

        // Verify percent-width text is constrained to 50% of 200px = 100px
        let text_node = &layout.root.children[0];
        assert!(
            (text_node.rect.width - 100.0).abs() < 1.0,
            "text should be constrained to ~100px (50% of 200px parent), got {}",
            text_node.rect.width
        );
    }

    #[test]
    fn truncate_text_measures_as_single_line_even_with_definite_width() {
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
            "w-[200px] h-full",
            vec![
                classed_text(
                    "headline",
                    "w-[80px] truncate text-[16px]",
                    "Long copy that should remain one measured line",
                )
                .into(),
            ],
        )
        .into();
        let resolved = resolve_ui_tree(&root, &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");

        let layout = compute_layout_with_font_db_fn(&resolved, &frame_ctx, &default_font_db())
            .expect("layout should succeed");

        // Verify truncated text is constrained to 80px width
        let text_node = &layout.root.children[0];
        assert!(
            (text_node.rect.width - 80.0).abs() < 1.0,
            "truncated text should be 80px wide, got {}",
            text_node.rect.width
        );
    }

    #[test]
    fn percent_width_text_under_indefinite_parent_still_resolves_layout() {
        // 回归测试：父容器没有 definite width 时，子元素的 `w-[N%]` 不应让
        // layout 崩溃或让文本测量丢失。
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
            "h-full",
            vec![classed_text("headline", "w-[50%] text-[16px]", "Indefinite parent copy").into()],
        )
        .into();
        let resolved = resolve_ui_tree(&root, &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");

        let layout = compute_layout_with_font_db_fn(&resolved, &frame_ctx, &default_font_db())
            .expect("layout should succeed even when parent width is indefinite");

        // Verify layout completed without panicking and text has dimensions
        let text_node = &layout.root.children[0];
        assert!(
            text_node.rect.width > 0.0,
            "text node should have non-zero width even under indefinite parent"
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
    fn layout_session_marks_truncate_change_as_layout_dirty() {
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

        let first = classed_div(
            "root",
            "w-full h-full",
            vec![classed_text("label", "w-[80px]", "A long changing label").into()],
        )
        .into();
        let second = classed_div(
            "root",
            "w-full h-full",
            vec![classed_text("label", "w-[80px] truncate", "A long changing label").into()],
        )
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

        assert!(
            second_stats.layout_dirty_nodes >= 1,
            "truncate changes text measurement semantics and must dirty layout, got {:?}",
            second_stats
        );
    }

    #[test]
    fn layout_session_marks_line_through_change_as_raster_dirty() {
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

        let first = classed_div(
            "root",
            "w-full h-full",
            vec![classed_text("label", "text-[16px]", "$25.00").into()],
        )
        .into();
        let second = classed_div(
            "root",
            "w-full h-full",
            vec![classed_text("label", "text-[16px] line-through", "$25.00").into()],
        )
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
        assert!(
            second_stats.raster_dirty_nodes >= 1,
            "line-through only changes text raster output, got {:?}",
            second_stats
        );
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
    fn svg_path_layout_uses_icon_intrinsic_size_only() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();

        let icon_root = classed_div(
            "root",
            "w-full h-full flex items-start",
            vec![lucide("play").id("icon").into()],
        )
        .into();
        let path_root = classed_div(
            "root",
            "w-full h-full flex items-start",
            vec![path("M0 0 L120 0 L60 100 Z").id("shape").into()],
        )
        .into();

        let icon_resolved = resolve_ui_tree(&icon_root, &frame_ctx, &mut media, &mut assets, None)
            .expect("icon tree should resolve");
        let path_resolved = resolve_ui_tree(&path_root, &frame_ctx, &mut media, &mut assets, None)
            .expect("path tree should resolve");

        let mut session = LayoutSession::new();
        let (icon_layout, _) = session
            .compute_layout(&icon_resolved, &frame_ctx)
            .expect("icon layout should succeed");
        let mut session = LayoutSession::new();
        let (path_layout, _) = session
            .compute_layout(&path_resolved, &frame_ctx)
            .expect("path layout should succeed");

        assert_eq!(icon_layout.root.children[0].rect.width, 24.0);
        assert_eq!(icon_layout.root.children[0].rect.height, 24.0);
        assert_eq!(path_layout.root.children[0].rect.width, 0.0);
        assert_eq!(path_layout.root.children[0].rect.height, 0.0);
    }

    #[test]
    fn layout_session_marks_svg_paint_change_as_raster_dirty() {
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
            .child(
                path("M0 0 L100 0 L50 100 Z")
                    .id("shape")
                    .size(100.0, 100.0)
                    .fill_color(ColorToken::Red500)
                    .stroke_color(ColorToken::Blue)
                    .stroke_width(1.0),
            )
            .into();
        let second = div()
            .id("root")
            .child(
                path("M0 0 L100 0 L50 100 Z")
                    .id("shape")
                    .size(100.0, 100.0)
                    .fill_color(ColorToken::Emerald500)
                    .stroke_color(ColorToken::Rose500)
                    .stroke_width(3.0),
            )
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
    fn layout_session_marks_lucide_path_change_as_raster_dirty() {
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
            .child(lucide("play").id("icon").size(24.0, 24.0))
            .into();
        let second = div()
            .id("root")
            .child(lucide("pause").id("icon").size(24.0, 24.0))
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
