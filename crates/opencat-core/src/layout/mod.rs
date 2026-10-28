pub mod tree;

#[cfg(feature = "profile")]
use tracing::Level;

use std::hash::{Hash, Hasher};

use anyhow::Result;
use taffy::{
    AvailableSpace, TaffyTree,
    prelude::{
        AlignContent as TaffyAlignContent, Dimension, FromFr,
        JustifyContent as TaffyJustifyContent, LengthPercentageAuto as TaffyLPA, Style,
        TaffyAuto, TaffyGridLine,
    },
    style::TrackSizingFunction,
    style_helpers::{flex, span},
};

use crate::{
    FrameCtx,
    layout::tree::{LayoutNode, LayoutOutputFingerprint, LayoutRect, LayoutTree},
    parse::primitives::{AlignItems, JustifyContent, Position},
    resolve::{
        style::text_element_allows_wrap,
        tree::{ElementKind, ElementNode},
    },
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
    pub input_merkle_full_hit_subtrees: usize,
    pub input_merkle_full_hit_nodes: usize,
    pub layout_merkle_skipped_subtrees: usize,
    pub layout_merkle_skipped_nodes: usize,
    pub layout_dirty_nodes: usize,
    pub raster_dirty_nodes: usize,
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
    structure_subtree_hash: u64,
    layout_input_subtree_hash: u64,
    paint_input_subtree_hash: u64,
    node_count: usize,
    layout_input_local_hash: u64,
    paint_input_local_hash: u64,
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

    

    pub fn compute_layout_with_font_db(
        &mut self,
        root: &ElementNode,
        frame_ctx: &FrameCtx,
        font_db: &fontdb::Database,
    ) -> Result<(LayoutTree, LayoutPassStats)> {
        let mut stats = LayoutPassStats::default();
        let viewport_size = (frame_ctx.width, frame_ctx.height);

        #[cfg(feature = "profile")]
        let _update_span = tracing::span!(
            target: "render.scene",
            Level::TRACE,
            "layout_structure_update"
        )
        .entered();
        let root_id = if !crate::ablate::ablated(crate::ablate::Ablation::LayoutMerkle)
            && self
                .root
                .as_ref()
                .is_some_and(|cached| same_structure(cached, root, 0))
        {
            let cached = self.root.as_mut().expect("root checked above");
            update_cached_subtree(root, cached, 0, &mut self.taffy, &mut stats, false)?;
            cached.taffy_node
        } else {
            self.rebuild(root, &mut stats)?
        };
        #[cfg(feature = "profile")]
        drop(_update_span);

        let layout_must_recompute = crate::ablate::ablated(crate::ablate::Ablation::LayoutMerkle)
            || stats.structure_rebuild
            || stats.layout_dirty_nodes > 0
            || self.cached_layout_tree.is_none()
            || self.last_layout_size != Some(viewport_size);

        if layout_must_recompute {
            #[cfg(feature = "profile")]
            let _resolve_span = tracing::span!(
                target: "render.scene",
                Level::TRACE,
                "layout_resolve"
            )
            .entered();
            self.taffy.compute_layout_with_measure(
                root_id,
                taffy::geometry::Size {
                    width: AvailableSpace::Definite(frame_ctx.width as f32),
                    height: AvailableSpace::Definite(frame_ctx.height as f32),
                },
                |inputs: taffy::LayoutInput, _node_id, node_context, _style| {
                    taffy::LayoutOutput::from_outer_size(measure_node(
                        inputs.known_dimensions,
                        inputs.available_space,
                        node_context,
                        font_db,
                    ))
                },
            )?;

            let layout_tree = LayoutTree {
                root: build_layout_tree(root, &self.taffy, root_id, font_db)?,
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

    pub fn compute_layout_with_provider(
        &mut self,
        element_root: &ElementNode,
        frame_ctx: &FrameCtx,
        fonts: &dyn crate::text::FontProvider,
    ) -> Result<(LayoutTree, LayoutPassStats)> {
        self.compute_layout_with_font_db(element_root, frame_ctx, fonts.font_db())
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







fn measure_node(
    known_dimensions: taffy::geometry::Size<Option<f32>>,
    available_space: taffy::geometry::Size<AvailableSpace>,
    node_context: Option<&mut TextMeasureContext>,
    font_db: &fontdb::Database,
) -> taffy::geometry::Size<f32> {
    let measured = match node_context {
        Some(text) => {
            let max_width = if text.allow_wrap {
                known_dimensions
                    .width
                    .or(match available_space.width {
                        AvailableSpace::Definite(width) => Some(width),
                        // CSS min-content sizing: the longest unbreakable run (word).
                        // taffy consumes this as the flex item's automatic minimum size
                        // (css-flexbox #min-size-auto); returning INFINITY here makes
                        // min-content == max-content and defeats flex-shrink entirely
                        // (shrunk text leaves also keep their single-line height).
                        AvailableSpace::MinContent => {
                            Some(text_min_content_width(&text.text, &text.style, font_db))
                        }
                        AvailableSpace::MaxContent => None,
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
        // Context-less leaves (SvgPath/Lottie/Canvas) have no measurable content:
        // their box size lives in the taffy style. taffy 0.14 measures leaves in
        // ContentSize mode, where style sizes are ignored, so echo the known
        // (style-derived) dimensions back instead of returning zero.
        None => taffy::geometry::Size {
            width: known_dimensions.width.unwrap_or(0.0),
            height: known_dimensions.height.unwrap_or(0.0),
        },
    };

    measured
}

/// CSS min-content width of a text run: the budget handed to the shaper so it
/// wraps at every soft wrap opportunity. The word widths here are the initial
/// split; the shaper's Unicode line breaking (hyphen included) decides the
/// final unbreakable fragments, matching Chrome's min-content contribution.
fn text_min_content_width(
    text: &str,
    style: &crate::style::ComputedTextStyle,
    font_db: &fontdb::Database,
) -> f32 {
    text.split_whitespace()
        .map(|word| crate::text::measure_text(word, style, f32::INFINITY, false, font_db).width)
        .fold(0.0_f32, f32::max)
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
            structure_subtree_hash: element.fingerprints.structure_subtree,
            layout_input_subtree_hash: element.fingerprints.layout_input_subtree,
            paint_input_subtree_hash: element.fingerprints.paint_input_subtree,
            node_count: element.fingerprints.node_count,
            layout_input_local_hash: element.fingerprints.layout_input_local,
            paint_input_local_hash: element.fingerprints.paint_input_local,
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
    layout_skip_already_counted: bool,
) -> Result<()> {
    if cached.structure_subtree_hash == element.fingerprints.structure_subtree
        && cached.layout_input_subtree_hash == element.fingerprints.layout_input_subtree
        && cached.paint_input_subtree_hash == element.fingerprints.paint_input_subtree
    {
        stats.reused_nodes += cached.node_count;
        stats.input_merkle_full_hit_subtrees += 1;
        stats.input_merkle_full_hit_nodes += cached.node_count;
        if !layout_skip_already_counted {
            stats.layout_merkle_skipped_subtrees += 1;
            stats.layout_merkle_skipped_nodes += cached.node_count;
        }
        return Ok(());
    }

    let layout_subtree_skipped = cached.layout_input_subtree_hash
        == element.fingerprints.layout_input_subtree
        && !layout_skip_already_counted;
    if layout_subtree_skipped {
        stats.layout_merkle_skipped_subtrees += 1;
        stats.layout_merkle_skipped_nodes += cached.node_count;
    }

    cached.identity = node_identity(element, sibling_index);

    let next_layout_hash = element.fingerprints.layout_input_local;
    let next_paint_hash = element.fingerprints.paint_input_local;

    if cached.layout_input_local_hash != next_layout_hash {
        taffy.set_style(cached.taffy_node, taffy_style_for_element(element))?;
        taffy.set_node_context(cached.taffy_node, text_measure_context_for_element(element))?;
        cached.layout_input_local_hash = next_layout_hash;
        cached.paint_input_local_hash = next_paint_hash;
        stats.layout_dirty_nodes += 1;
    } else {
        let paint_changed = cached.paint_input_local_hash != next_paint_hash;
        cached.paint_input_local_hash = next_paint_hash;

        if paint_changed {
            stats.raster_dirty_nodes += 1;
        } else {
            stats.reused_nodes += 1;
        }
    }

    for ((index, child), cached_child) in ordered_children(element)
        .into_iter()
        .zip(cached.children.iter_mut())
    {
        update_cached_subtree(
            child,
            cached_child,
            index,
            taffy,
            stats,
            layout_skip_already_counted || layout_subtree_skipped,
        )?;
    }

    cached.structure_subtree_hash = element.fingerprints.structure_subtree;
    cached.layout_input_subtree_hash = element.fingerprints.layout_input_subtree;
    cached.paint_input_subtree_hash = element.fingerprints.paint_input_subtree;
    cached.node_count = element.fingerprints.node_count;

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
        ElementKind::Lottie(_) => CachedNodeKind::Bitmap,
        ElementKind::Canvas(_) => CachedNodeKind::Canvas,
        ElementKind::SvgPath(_) => CachedNodeKind::SvgPath,
        ElementKind::Svg(_) => CachedNodeKind::Bitmap,
    }
}

fn node_identity(element: &ElementNode, sibling_index: usize) -> u64 {
    let mut hasher = ahash::AHasher::default();
    cached_node_kind(element).hash(&mut hasher);
    sibling_index.hash(&mut hasher);
    element.style.id.hash(&mut hasher);
    hasher.finish()
}

fn text_measure_context_for_element(element: &ElementNode) -> Option<TextMeasureContext> {
    match &element.kind {
        ElementKind::Text(text) => Some(TextMeasureContext {
            text: text.text.clone(),
            style: text.text_style.clone(),
            allow_wrap: text_element_allows_wrap(element),
        }),
        _ => None,
    }
}

fn taffy_style_for_element(element: &ElementNode) -> Style {
    let layout = &element.style.layout;
    // 标准直通腿：CSS 路径产出的 taffy 样式整体克隆。border 宽度也已在
    // `css::to_taffy` 落好（含 thin/medium/thick），不再用 paint 腿的
    // f32 镜像覆盖——那会把关键字宽度压成 0。固有尺寸（Bitmap/Lottie/
    // SvgPath、文本测量）由 measure 回调在 size 为 auto 时提供，与浏览器
    // `width: auto` 语义一致，不在此覆盖。
    if let Some(css) = &element.style.css_layout {
        // <svg> 的 auto 尺寸回填元素 width/height 属性或 svg intrinsic
        // （浏览器 width:auto 语义）；CSS 显式尺寸优先，不覆盖。
        let mut style = css.0.clone();
        if let ElementKind::Svg(svg) = &element.kind {
            let fallback_w = svg
                .width
                .map(Dimension::length)
                .or_else(|| svg.intrinsic_size.map(|(w, _)| Dimension::length(w)));
            let fallback_h = svg
                .height
                .map(Dimension::length)
                .or_else(|| svg.intrinsic_size.map(|(_, h)| Dimension::length(h)));
            if style.size.width.is_auto() {
                if let Some(w) = fallback_w {
                    style.size.width = w;
                }
            }
            if style.size.height.is_auto() {
                if let Some(h) = fallback_h {
                    style.size.height = h;
                }
            }
        }
        return style;
    }
    match &element.kind {
        ElementKind::Div(_) | ElementKind::Timeline(_) | ElementKind::Canvas(_) => Style {
            display: if layout.is_grid {
                taffy::prelude::Display::Grid
            } else if layout.is_flex {
                taffy::prelude::Display::Flex
            } else {
                taffy::prelude::Display::Block
            },
            grid_template_columns: layout.grid_template_columns.map_or_else(Vec::new, |cols| {
                (0..cols)
                    .map(|_| taffy::style::GridTemplateComponent::Single(flex(1.0_f32)))
                    .collect()
            }),
            grid_template_rows: layout.grid_template_rows.map_or_else(Vec::new, |rows| {
                (0..rows)
                    .map(|_| taffy::style::GridTemplateComponent::Single(flex(1.0_f32)))
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
                    crate::style::GridAutoRows::Fr => TrackSizingFunction::from_fr(1.0_f32),
                }]
            }),
            justify_items: layout.justify_items.map(map_align_items_to_justify_items),
            grid_column: resolve_grid_axis_placement(layout.col_start, layout.col_end),
            grid_row: resolve_grid_axis_placement(layout.row_start, layout.row_end),
            max_size: taffy::geometry::Size {
                width: layout
                    .max_width
                    .map(TaffyLPA::length)
                    .unwrap_or_else(TaffyLPA::auto),
                height: TaffyLPA::auto(),
            },
            min_size: taffy::geometry::Size {
                width: TaffyLPA::auto(),
                height: layout.min_height.map_or_else(TaffyLPA::auto, |v| match v {
                    crate::style::LengthPercentageAuto::Auto => TaffyLPA::auto(),
                    crate::style::LengthPercentageAuto::Length(px) => TaffyLPA::length(px),
                    crate::style::LengthPercentageAuto::Percent(pct) => {
                        let resolved = if pct < 0.0 { 1.0 } else { pct };
                        TaffyLPA::percent(resolved)
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
                        layout.height_percent,
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
                        layout.height_percent,
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
        ElementKind::Lottie(lottie) => Style {
            size: taffy::geometry::Size {
                width: resolve_dimension(
                    layout.width,
                    layout.width_percent,
                    layout.width_full,
                    Dimension::length(lottie.width as f32),
                ),
                height: resolve_dimension(
                    layout.height,
                    None,
                    layout.height_full,
                    Dimension::length(lottie.height as f32),
                ),
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
        ElementKind::Svg(svg) => {
            // 元素 width/height 属性覆盖 intrinsic；CSS 尺寸在 layout.*
            // 里已优先（resolve_dimension 第一参），与 image 同序。
            let default_size = svg
                .intrinsic_size
                .map(|(w, h)| (Dimension::length(w), Dimension::length(h)))
                .or_else(|| {
                    svg.width
                        .zip(svg.height)
                        .map(|(w, h)| (Dimension::length(w), Dimension::length(h)))
                })
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
    font_db: &fontdb::Database,
) -> Result<LayoutNode> {
    let layout = taffy.layout(node_id)?;
    let mut children = Vec::new();
    let taffy_children = taffy.children(node_id)?;

    // Note: in-flow (relative/static) inset offsets are already applied by
    // taffy itself into `layout.location` (flexbox `main_relative_inset` /
    // `cross_relative_inset`), and absolute insets by the absolute algorithm.
    // Do NOT re-apply them here — that would double the offset. Locked by
    // `relative_inset_offsets_subtree_from_normal_flow_position`.

    for ((_, element_child), taffy_child) in
        ordered_children(element).into_iter().zip(taffy_children)
    {
        children.push(build_layout_tree(
            element_child,
            taffy,
            taffy_child,
            font_db,
        )?);
    }

    let rect = LayoutRect {
        x: layout.location.x,
        y: layout.location.y,
        width: layout.size.width,
        height: layout.size.height,
    };
    let output_fingerprint = layout_output_fingerprint(rect, &children);

    Ok(LayoutNode {
        id: element.style.id.clone(),
        rect,
        output_fingerprint,
        children,
    })
}

fn layout_output_fingerprint(rect: LayoutRect, children: &[LayoutNode]) -> LayoutOutputFingerprint {
    let mut record_size_hasher = ahash::AHasher::default();
    F32Bits(rect.width).hash(&mut record_size_hasher);
    F32Bits(rect.height).hash(&mut record_size_hasher);
    let record_size = record_size_hasher.finish();

    let mut local_hasher = ahash::AHasher::default();
    F32Bits(rect.x).hash(&mut local_hasher);
    F32Bits(rect.y).hash(&mut local_hasher);
    F32Bits(rect.width).hash(&mut local_hasher);
    F32Bits(rect.height).hash(&mut local_hasher);
    let local = local_hasher.finish();

    let mut subtree_hasher = ahash::AHasher::default();
    local.hash(&mut subtree_hasher);
    children.len().hash(&mut subtree_hasher);
    for child in children {
        child.output_fingerprint.subtree.hash(&mut subtree_hasher);
    }

    LayoutOutputFingerprint {
        local,
        subtree: subtree_hasher.finish(),
        record_size,
    }
}

#[derive(Clone, Copy)]
struct F32Bits(f32);

impl Hash for F32Bits {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

fn base_border(element: &ElementNode) -> taffy::geometry::Rect<taffy::style::LengthPercentage> {
    let visual = &element.style.visual;
    let uniform_border = visual.border_width.unwrap_or(0.0);
    let border_top = visual.border_top_width.unwrap_or(uniform_border);
    let border_right = visual.border_right_width.unwrap_or(uniform_border);
    let border_bottom = visual.border_bottom_width.unwrap_or(uniform_border);
    let border_left = visual.border_left_width.unwrap_or(uniform_border);
    taffy::geometry::Rect {
        left: taffy::style::LengthPercentage::length(border_left),
        top: taffy::style::LengthPercentage::length(border_top),
        right: taffy::style::LengthPercentage::length(border_right),
        bottom: taffy::style::LengthPercentage::length(border_bottom),
    }
}

fn base_style(element: &ElementNode) -> Style {
    let layout = &element.style.layout;
    let border = base_border(element);
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
        min_size: taffy::geometry::Size {
            width: layout.min_width.map_or_else(TaffyLPA::auto, |v| match v {
                crate::style::LengthPercentageAuto::Auto => TaffyLPA::auto(),
                crate::style::LengthPercentageAuto::Length(px) => TaffyLPA::length(px),
                crate::style::LengthPercentageAuto::Percent(pct) => {
                    let resolved = if pct < 0.0 { 1.0 } else { pct };
                    TaffyLPA::percent(resolved)
                }
            }),
            height: layout.min_height.map_or_else(TaffyLPA::auto, |v| match v {
                crate::style::LengthPercentageAuto::Auto => TaffyLPA::auto(),
                crate::style::LengthPercentageAuto::Length(px) => TaffyLPA::length(px),
                crate::style::LengthPercentageAuto::Percent(pct) => {
                    let resolved = if pct < 0.0 { 1.0 } else { pct };
                    TaffyLPA::percent(resolved)
                }
            }),
        },
        flex_grow: layout.flex_grow,
        align_self: layout.align_self.map(map_align),
        justify_self: layout.justify_self.map(map_align_items_to_justify_self),
        aspect_ratio: layout.aspect_ratio,
        border,
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
        JustifyContent::Start => TaffyJustifyContent::FLEX_START,
        JustifyContent::Center => TaffyJustifyContent::CENTER,
        JustifyContent::End => TaffyJustifyContent::FLEX_END,
        JustifyContent::Between => TaffyJustifyContent::SPACE_BETWEEN,
        JustifyContent::Around => TaffyJustifyContent::SPACE_AROUND,
        JustifyContent::Evenly => TaffyJustifyContent::SPACE_EVENLY,
        JustifyContent::Stretch => TaffyJustifyContent::STRETCH,
    }
}

fn map_align(value: AlignItems) -> taffy::prelude::AlignItems {
    match value {
        AlignItems::Start => taffy::prelude::AlignItems::FLEX_START,
        AlignItems::Center => taffy::prelude::AlignItems::CENTER,
        AlignItems::End => taffy::prelude::AlignItems::FLEX_END,
        AlignItems::Baseline => taffy::prelude::AlignItems::BASELINE,
        AlignItems::Stretch => taffy::prelude::AlignItems::STRETCH,
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
        JustifyContent::Start => TaffyAlignContent::FLEX_START,
        JustifyContent::Center => TaffyAlignContent::CENTER,
        JustifyContent::End => TaffyAlignContent::FLEX_END,
        JustifyContent::Between => TaffyAlignContent::SPACE_BETWEEN,
        JustifyContent::Around => TaffyAlignContent::SPACE_AROUND,
        JustifyContent::Evenly => TaffyAlignContent::SPACE_EVENLY,
        JustifyContent::Stretch => TaffyAlignContent::STRETCH,
    }
}


