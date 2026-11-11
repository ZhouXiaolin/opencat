mod inline;
pub mod tree;

#[cfg(feature = "profile")]
use tracing::Level;

use std::collections::HashMap;
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
    layout::tree::{LayoutNode, LayoutOutputFingerprint, LayoutRect, LayoutTree, TextFragment},
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

/// AbsSizeFix 热启动缓存条目的存活帧数（半秒 @30fps）。过期后由 want 重新
/// 推导：仍需要的值会再登记（多一趟 solve），不再需要的解除预置。
const FIX_CACHE_TTL: u64 = 30;

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
    /// AbsSizeFix 跨帧热启动：上帧收敛的定长（结构未变时 NodeId 稳定）。
    /// 预置为本帧初始样式后，want 推导与现值一致、不再登记 fix——免去逐帧
    /// 2–4 趟整树重排（stripe 每帧 17 会话×4 轮=68 次求解的减速根因）。
    /// 结构重建即清空；want 随输入变化时照常登记并刷新缓存，自纠偏。`seen`
    /// 只在登记时刷新：超过 `FIX_CACHE_TTL` 帧未登记的条目过期（节点不再
    /// 产出 fix 时预置不能永久钉住尺寸）。
    fix_cache: HashMap<taffy::NodeId, (Option<f32>, Option<f32>, u64)>,
}

impl LayoutSession {
    pub fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
            root: None,
            cached_layout_tree: None,
            last_layout_size: None,
            fix_cache: HashMap::new(),
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
            let viewport = LayoutRect {
                x: 0.0,
                y: 0.0,
                width: frame_ctx.width as f32,
                height: frame_ctx.height as f32,
            };
            let mut fixes = Vec::new();
            let solve_started = std::time::Instant::now();
            // 热启动预置：上帧收敛的 AbsSizeFix 定长先行写入，首趟 want 即
            // 与现值一致、不再登记 fix——稳态帧单趟收敛（原本逐帧 2–4 趟）。
            for (node, (width, height, _seen)) in self.fix_cache.iter() {
                let Ok(style) = self.taffy.style(*node) else {
                    continue;
                };
                let mut style = style.clone();
                if let Some(width) = width {
                    style.size.width = Dimension::length(*width);
                }
                if let Some(height) = height {
                    style.size.height = Dimension::length(*height);
                }
                let _ = self.taffy.set_style(*node, style);
            }
            self.compute_taffy(root_id, frame_ctx, font_db)?;
            let mut tree_root =
                build_layout_tree_at(root, &self.taffy, root_id, font_db, (0.0, 0.0), viewport, &mut fixes)?;
            // 静态父盒里的绝对定位盒按真包含块定尺寸（AbsSizeFix）：写成定长
            // 再排一趟。嵌套时外层改尺寸会改变内层的包含块，逐趟收敛；改写只
            // 存活于本次求解，结束后恢复原样式，taffy 树始终是声明值。
            let mut originals: Vec<(taffy::NodeId, Style)> = Vec::new();
            let mut fix_rounds = 0usize;
            let mut round_fix_counts: Vec<usize> = Vec::new();
            for _ in 0..4 {
                if fixes.is_empty() {
                    break;
                }
                fix_rounds += 1;
                round_fix_counts.push(fixes.len());
                for fix in fixes.drain(..) {
                    let mut style = self.taffy.style(fix.node)?.clone();
                    if !originals.iter().any(|(node, _)| *node == fix.node) {
                        originals.push((fix.node, style.clone()));
                    }
                    if let Some(width) = fix.width {
                        style.size.width = Dimension::length(width);
                    }
                    if let Some(height) = fix.height {
                        style.size.height = Dimension::length(height);
                    }
                    if std::env::var("OC_LAYOUT_DEBUG").as_deref() == Ok("2") {
                        let l = self.taffy.layout(fix.node).map(|l| l.size).ok();
                        eprintln!(
                            "[apply] node={:?} set=({:?},{:?}) style_h={:?} min_h={:?} aspect={:?} laid={:?}",
                            fix.node,
                            fix.width,
                            fix.height,
                            self.taffy.style(fix.node).map(|s| s.size.height),
                            self.taffy.style(fix.node).map(|s| s.min_size.height),
                            self.taffy.style(fix.node).map(|s| s.aspect_ratio),
                            l
                        );
                    }
                    self.taffy.set_style(fix.node, style)?;
                    // 热启动缓存：登记即刷新（结构重建已清空，陈旧 NodeId 由
                    // 预置侧的 style() Err 跳过）。
                    self.fix_cache
                        .insert(fix.node, (fix.width, fix.height, frame_ctx.frame as u64));
                }
                self.compute_taffy(root_id, frame_ctx, font_db)?;
                tree_root = build_layout_tree_at(
                    root,
                    &self.taffy,
                    root_id,
                    font_db,
                    (0.0, 0.0),
                    viewport,
                    &mut fixes,
                )?;
            }
            for (node, style) in originals {
                self.taffy.set_style(node, style)?;
            }
            // 过期条目清除：seen 只在登记时刷新。收敛节点由预置兜住、每
            // TTL 帧重新登记一次（多一趟 solve，摊销可忽略）；不再产出 fix
            // 的节点（inset 改 auto 等）最迟 TTL 帧后解除钉住。
            self.fix_cache.retain(|_, (_, _, seen)| {
                (frame_ctx.frame as u64).saturating_sub(*seen) <= FIX_CACHE_TTL
            });

            // 诊断（`OC_LAYOUT_DEBUG=1`）：每帧求解轮数与墙钟。fix 轮数>0
            // 表示该帧多跑一整套 taffy 求解+建树（inspector/stripe 减速排查面）。
            if std::env::var("OC_LAYOUT_DEBUG").as_deref() == Ok("1") {
                eprintln!(
                    "[layout] frame {} rounds={} fixes={:?} solve_ms={} dirty={}",
                    frame_ctx.frame,
                    fix_rounds,
                    round_fix_counts,
                    solve_started.elapsed().as_millis(),
                    stats.layout_dirty_nodes
                );
            }

            let layout_tree = LayoutTree { root: tree_root };
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

    fn compute_taffy(
        &mut self,
        root_id: taffy::NodeId,
        frame_ctx: &FrameCtx,
        font_db: &fontdb::Database,
    ) -> Result<()> {
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
        Ok(())
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
        self.fix_cache.clear();
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

/// 元素在布局里的实际定位值。CSS 路径（`css_layout` 存在）以 taffy 样式为准，
/// 其余回落到 NodeStyle。
fn effective_position(element: &ElementNode) -> Position {
    if let Some(css) = element.style.css_layout.as_ref()
        && css.0.position == taffy::prelude::Position::Absolute
    {
        return Position::Absolute;
    }
    element.style.layout.position
}

/// CSS 直通腿给定的宽/高（非 auto）。NodeStyle 的 width/height 镜像不含
/// calc/em 这类尺寸，它们只落在 `css_layout` 里。
fn css_definite_size(element: &ElementNode) -> (bool, bool) {
    element.style.css_layout.as_ref().map_or((false, false), |css| {
        (!css.0.size.width.is_auto(), !css.0.size.height.is_auto())
    })
}

fn taffy_style_for_element(element: &ElementNode) -> Style {    let layout = &element.style.layout;
    // 标准直通腿：CSS 路径产出的 taffy 样式整体克隆。border 宽度也已在
    // `css::to_taffy` 落好（含 thin/medium/thick），不再用 paint 腿的
    // f32 镜像覆盖——那会把关键字宽度压成 0。固有尺寸（Bitmap/Lottie/
    // SvgPath、文本测量）由 measure 回调在 size 为 auto 时提供，与浏览器
    // `width: auto` 语义一致，不在此覆盖。
    if let Some(css) = &element.style.css_layout {
        // <svg> 的 auto 尺寸回填元素 width/height 属性或 svg intrinsic
        // （浏览器 width:auto 语义）；CSS 显式尺寸优先，不覆盖。
        let mut style = css.0.clone();
        // <img> 是 replaced element：两维都 auto 用固有尺寸；只有一维 auto
        // 时按固有宽高比推出来（浏览器 `width:650px; height:auto` 语义）。
        // taffy 没有 replaced 语义，不给 aspect_ratio 就会塌成 0。
        if let ElementKind::Bitmap(bitmap) = &element.kind {
            let (iw, ih) = (bitmap.width as f32, bitmap.height as f32);
            if iw > 0.0 && ih > 0.0 {
                let w_auto = style.size.width.is_auto();
                let h_auto = style.size.height.is_auto();
                if w_auto && h_auto {
                    style.size.width = Dimension::length(iw);
                    style.size.height = Dimension::length(ih);
                } else if w_auto != h_auto {
                    style.aspect_ratio = Some(iw / ih);
                }
            }
        }
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
        // 行内流容器：taffy 没有 inline 层级。映射成单行 flex row（顶对齐、
        // 子盒 shrink-to-fit），让 taffy 自己算出行内容的固有宽度（子盒宽之
        // 和）——父盒的 shrink-to-fit 与居中才拿得到正确尺寸。行内串接的
        // 最终坐标仍由 build_layout_tree 的行内 pass 定稿。
        if layout.inline_flow {
            style.display = taffy::prelude::Display::Flex;
            style.flex_direction = taffy::prelude::FlexDirection::Row;
            style.flex_wrap = taffy::prelude::FlexWrap::NoWrap;
            style.align_items = Some(taffy::prelude::AlignItems::START);
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
                        Dimension::auto(),
                    ),
                    height: resolve_dimension(
                        layout.height,
                        layout.height_percent,
                        Dimension::auto(),
                    ),
                },
                Position::Relative => taffy::geometry::Size {
                    width: resolve_dimension(
                        layout.width,
                        layout.width_percent,
                        if layout.auto_size {
                            Dimension::auto()
                        } else {
                            Dimension::percent(1.0)
                        },
                    ),
                    height: resolve_dimension(
                        layout.height,
                        layout.height_percent,
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
                    Dimension::auto(),
                ),
                height: resolve_dimension(
                    layout.height,
                    layout.height_percent,
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
                    Dimension::length(bitmap.width as f32),
                ),
                height: resolve_dimension(
                    layout.height,
                    layout.height_percent,
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
                    Dimension::length(lottie.width as f32),
                ),
                height: resolve_dimension(
                    layout.height,
                    layout.height_percent,
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
                        default_size.0,
                    ),
                    height: resolve_dimension(
                        layout.height,
                        layout.height_percent,
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
                        default_size.0,
                    ),
                    height: resolve_dimension(
                        layout.height,
                        layout.height_percent,
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

/// 单轴绝对定位求解：`start`/`end` 是 left/right 或 top/bottom，`size` 是元素
/// 该轴尺寸，`static_pos` 是两侧都为 auto 时的静态位置。None = 交给 taffy 原值。
fn resolve_abs_axis(
    start: taffy::style::LengthPercentageAuto,
    end: taffy::style::LengthPercentageAuto,
    cb_start: f32,
    cb_len: f32,
    size: f32,
    static_pos: f32,
    (margin_start, margin_end): (f32, f32),
) -> Option<f32> {
    use taffy::style::ExpandedLengthPercentageAuto as E;
    let resolve = |v: taffy::style::LengthPercentageAuto, base: f32| match v.expand() {
        E::Length(px) => Some(px),
        E::Percent(p) => Some(p * base),
        E::Auto => None,
    };
    // inset 定位的是 margin box：border box 再错开一个 margin（gsap 网格的
    // `left:50%` + 负 margin 居中写法）。
    if let Some(px) = resolve(start, cb_len) {
        return Some(cb_start + px + margin_start);
    }
    if let Some(px) = resolve(end, cb_len) {
        return Some(cb_start + cb_len - px - size - margin_end);
    }
    Some(static_pos + margin_start)
}

/// 绝对定位的「包含块」上下文（`docs/html-align/DESIGN.md`）：CSS 规定绝对定位
/// 元素的包含块是**最近的非 static 祖先**的 padding box，没有就是视口；taffy 一律
/// 拿直接父盒当包含块。静态父盒高度为 0 时（k3 的 `#dpins`）会把 `bottom` 元素顶到
/// 画外，所以在这里按浏览器规则重算一次坐标。
#[derive(Clone, Copy)]
struct AbsCtx {
    /// 本元素 border box 左上角的绝对坐标。
    origin: (f32, f32),
    /// 最近的非 static 祖先的 padding box（绝对坐标）；全 static 链时是视口。
    cb: LayoutRect,
}

/// 静态父盒里绝对定位盒的尺寸修正：taffy 按直接父盒拉伸/折算它的尺寸，
/// 父盒是 0 高的静态包装层（`.clip` 一类）时整棵子树塌成 0 高。值是按真
/// 包含块求出的、写回 taffy `size` 的定长（None = 该轴不改）。
struct AbsSizeFix {
    node: taffy::NodeId,
    width: Option<f32>,
    height: Option<f32>,
}

/// 绝对定位盒在真包含块下的该轴 border-box 尺寸：两端 inset 都给定且尺寸
/// auto 时由包含块拉伸，百分比尺寸按包含块折算；其余与包含块无关（None）。
/// `pb` 是该轴 padding+border，content-box 时百分比尺寸要加回它。
fn abs_border_box_len(
    start: taffy::style::LengthPercentageAuto,
    end: taffy::style::LengthPercentageAuto,
    size: Dimension,
    cb_len: f32,
    (margin_start, margin_end): (f32, f32),
    pb: f32,
    content_box: bool,
) -> Option<f32> {
    use taffy::style::{ExpandedDimension as D, ExpandedLengthPercentageAuto as E};
    let resolve = |v: taffy::style::LengthPercentageAuto| match v.expand() {
        E::Length(px) => Some(px),
        E::Percent(p) => Some(p * cb_len),
        _ => None,
    };
    match size.expand() {
        D::Auto => Some(
            (cb_len - resolve(start)? - resolve(end)? - margin_start - margin_end).max(0.0),
        ),
        D::Percent(p) => Some(p * cb_len + if content_box { pb } else { 0.0 }),
        _ => None,
    }
}

fn build_layout_tree_at(
    element: &ElementNode,
    taffy: &TaffyTree<TextMeasureContext>,
    node_id: taffy::NodeId,
    font_db: &fontdb::Database,
    origin: (f32, f32),
    containing_block: LayoutRect,
    fixes: &mut Vec<AbsSizeFix>,
) -> Result<LayoutNode> {
    let layout = taffy.layout(node_id)?;
    let mut children = Vec::new();
    let taffy_children = taffy.children(node_id)?;

    // Note: in-flow (relative/static) inset offsets are already applied by
    // taffy itself into `layout.location` (flexbox `main_relative_inset` /
    // `cross_relative_inset`), and absolute insets by the absolute algorithm.
    // Do NOT re-apply them here — that would double the offset. Locked by
    // `relative_inset_offsets_subtree_from_normal_flow_position`.

    let mut rect = LayoutRect {
        x: layout.location.x,
        y: layout.location.y,
        width: layout.size.width,
        height: layout.size.height,
    };

    // CSS 直通腿的定位以 `css_layout` 为准：NodeStyle 的 position 只在非 CSS
    // 路径被填，拿它判定会把 `position:absolute` 的 CSS 元素当 static，绝对
    // 定位包含块整条失效（k3 的 .pin 全落到 (0,0)）。`position: relative` 也是
    // 包含块（vfx 的 caret 绝对定位在 relative 的 .html-word 里）。
    let positioned =
        element.style.layout.positioned || effective_position(element) != Position::Relative;
    let padding_box = LayoutRect {
        x: origin.0 + rect.x + layout.border.left + layout.padding.left,
        y: origin.1 + rect.y + layout.border.top + layout.padding.top,
        width: (rect.width - layout.border.left - layout.border.right
            - layout.padding.left
            - layout.padding.right)
            .max(0.0),
        height: (rect.height - layout.border.top - layout.border.bottom
            - layout.padding.top
            - layout.padding.bottom)
            .max(0.0),
    };
    let child_ctx = AbsCtx {
        origin: (origin.0 + rect.x, origin.1 + rect.y),
        cb: if positioned { padding_box } else { containing_block },
    };

    // 子盒 margin（行内排行要计入；taffy 的块布局把它们算进了纵向流）。
    let mut inline_margins: Vec<taffy::geometry::Rect<f32>> = Vec::new();
    for ((_, element_child), taffy_child) in
        ordered_children(element).into_iter().zip(taffy_children)
    {
        let margin = taffy.layout(taffy_child)?.margin;
        inline_margins.push(margin);
        let mut child = build_layout_tree_at(
            element_child,
            taffy,
            taffy_child,
            font_db,
            child_ctx.origin,
            child_ctx.cb,
            fixes,
        )?;
        // 静态父盒不是包含块：按最近非 static 祖先（或视口）重算绝对定位坐标。
        if !positioned
            && effective_position(element_child) == Position::Absolute
            && let Some(css) = element_child.style.css_layout.as_ref()
        {
            // 尺寸同理按真包含块求：与 taffy 现值不同就登记修正，由
            // compute_layout_with_font_db 写回样式重排（本趟坐标仍按现尺寸）。
            let child_layout = taffy.layout(taffy_child)?;
            let content_box = css.0.box_sizing == taffy::style::BoxSizing::ContentBox;
            let pb_x = child_layout.padding.left
                + child_layout.padding.right
                + child_layout.border.left
                + child_layout.border.right;
            let pb_y = child_layout.padding.top
                + child_layout.padding.bottom
                + child_layout.border.top
                + child_layout.border.bottom;
            let want = |bb: Option<f32>, current: f32, pb: f32| {
                // 0.5px：taffy 布局取整噪声不触发尺寸修正（同 inline-height）。
                bb.filter(|bb| (bb - current).abs() > 0.5)
                    .map(|bb| if content_box { (bb - pb).max(0.0) } else { bb })
            };
            let width = want(
                abs_border_box_len(
                    css.0.inset.left,
                    css.0.inset.right,
                    css.0.size.width,
                    child_ctx.cb.width,
                    (margin.left, margin.right),
                    pb_x,
                    content_box,
                ),
                child.rect.width,
                pb_x,
            );
            let height = want(
                abs_border_box_len(
                    css.0.inset.top,
                    css.0.inset.bottom,
                    css.0.size.height,
                    child_ctx.cb.height,
                    (margin.top, margin.bottom),
                    pb_y,
                    content_box,
                ),
                child.rect.height,
                pb_y,
            );
            if width.is_some() || height.is_some() {
                if std::env::var("OC_LAYOUT_DEBUG").as_deref() == Ok("2") {
                    eprintln!(
                        "[fix] abs-static id={} width={:?} height={:?} cur={:?}",
                        element_child.style.id,
                        width,
                        height,
                        (child.rect.width, child.rect.height)
                    );
                }
                fixes.push(AbsSizeFix {
                    node: taffy_child,
                    width,
                    height,
                });
            }
            if let Some(x) = resolve_abs_axis(
                css.0.inset.left,
                css.0.inset.right,
                child_ctx.cb.x,
                child_ctx.cb.width,
                child.rect.width,
                child_ctx.origin.0 + layout.border.left + layout.padding.left,
                (margin.left, margin.right),
            ) {
                child.rect.x = x - child_ctx.origin.0;
            }
            if let Some(y) = resolve_abs_axis(
                css.0.inset.top,
                css.0.inset.bottom,
                child_ctx.cb.y,
                child_ctx.cb.height,
                child.rect.height,
                child_ctx.origin.1 + layout.border.top + layout.padding.top,
                (margin.top, margin.bottom),
            ) {
                child.rect.y = y - child_ctx.origin.1;
            }
        }
        children.push(child);
    }

    // HTML 行内格式化（layout::inline）：taffy 没有 IFC，这里按容器最终内容
    // 宽把在流子项折行、基线对齐；容器 auto 高度回写 taffy 重排（父盒与后续
    // 兄弟随之下移）。绝对定位子节点不参与行流，保持 taffy 结果。
    let mut node_baseline = None;
    if element.style.layout.inline_flow && !children.is_empty() {
        node_baseline = inline_formatting_pass(
            element,
            layout,
            unrounded_content_width(taffy.unrounded_layout(node_id)),
            node_id,
            &mut rect,
            &mut children,
            &inline_margins,
            font_db,
            fixes,
            containing_block,
        );
    }

    let wrap_width = matches!(element.kind, ElementKind::Text(_))
        .then(|| taffy.unrounded_layout(node_id).size.width);
    let output_fingerprint = layout_output_fingerprint(rect, &children, None, wrap_width);

    Ok(LayoutNode {
        id: element.style.id.clone(),
        rect,
        output_fingerprint,
        children,
        inline_fragments: None,
        baseline: node_baseline,
        wrap_width,
    })
}

/// 取整前的内容盒宽。折行按它判定：像素取整会把收缩适配的盒宽砍掉不足
/// 1px（1416.95 → 1416），拿取整宽折行，恰好放下的末段就被挤到下一行。
/// 浏览器的布局单位是 1/64px，折行不受像素网格影响。
fn unrounded_content_width(layout: &taffy::Layout) -> f32 {
    (layout.size.width
        - layout.border.left
        - layout.border.right
        - layout.padding.left
        - layout.padding.right)
        .max(0.0)
}

/// 行内流容器的折行排版（见 [`inline`]）：子盒落位、跨行文本片段、容器
/// auto 高度（与 taffy 现值不同就登记 [`AbsSizeFix`] 重排）。返回容器末行
/// 基线（相对 rect 顶）。`content_w` 是取整前的内容盒宽。
#[allow(clippy::too_many_arguments)]
fn inline_formatting_pass(
    element: &ElementNode,
    layout: &taffy::Layout,
    content_w: f32,
    node_id: taffy::NodeId,
    rect: &mut LayoutRect,
    children: &mut [LayoutNode],
    margins: &[taffy::geometry::Rect<f32>],
    font_db: &fontdb::Database,
    fixes: &mut Vec<AbsSizeFix>,
    containing_block: LayoutRect,
) -> Option<f32> {
    use inline::{InlineItem, Placement};

    let left = layout.border.left + layout.padding.left;
    let top = layout.border.top + layout.padding.top;
    let right = layout.padding.right + layout.border.right;
    let bottom = layout.padding.bottom + layout.border.bottom;
    let ordered: Vec<&ElementNode> = ordered_children(element)
        .into_iter()
        .map(|(_, child)| child)
        .collect();

    let items: Vec<InlineItem<'_>> = children
        .iter()
        .zip(&ordered)
        .zip(margins)
        .map(|((child, child_el), margin)| {
            if effective_position(child_el) == Position::Absolute {
                return InlineItem::Skip;
            }
            match &child_el.kind {
                ElementKind::Text(text)
                    if !child_el.style.layout.inline_atomic && !has_inline_box_edges(child_el) =>
                {
                    InlineItem::Text {
                        text: &text.text,
                        style: &text.text_style,
                        nowrap: text.text_style.no_wrap,
                    }
                }
                _ => {
                    let baseline =
                        atomic_baseline(child_el, child, font_db).unwrap_or(child.rect.height);
                    InlineItem::Atomic {
                        width: margin.left + child.rect.width + margin.right,
                        height: margin.top + child.rect.height + margin.bottom,
                        baseline: margin.top + baseline,
                        align: child_el.style.layout.vertical_align,
                    }
                }
            }
        })
        .collect();
    if items.iter().all(|item| matches!(item, InlineItem::Skip)) {
        return None;
    }

    let text = &element.style.text;
    let nowrap_container = text.no_wrap || element.style.layout.truncate;
    let mut wrap_width = if nowrap_container { f32::INFINITY } else { content_w };
    let mut align_width = content_w;

    // CSS §10.3.7 收缩适配：absolute 盒 auto 宽、左 inset 给定、右 inset auto
    // 时，可用宽 = 包含块 − left − 横向 margin。taffy 的 absolute 量宽传整个
    // 包含块宽（stripe 的 `left:50%` 标题因此不折行、撑满一整行）。行内流里
    // 自量 min/max-content 补上：盒宽 = min(max(min-content, 可用宽),
    // max-content)，折行与 text-align 都按盒宽，盒宽差值登记修正重排。
    let css = element.style.css_layout.as_ref();
    let content_box = css.is_some_and(|css| css.0.box_sizing == taffy::style::BoxSizing::ContentBox);
    let mut width_fix = None;
    if !nowrap_container
        && effective_position(element) == Position::Absolute
        && !css_definite_size(element).0
        && css.is_some_and(|css| {
            !css.0.inset.left.is_auto() && css.0.inset.right.is_auto() && css.0.size.width.is_auto()
        })
    {
        let left_px = match css.expect("checked above").0.inset.left.expand() {
            taffy::style::ExpandedLengthPercentageAuto::Length(px) => Some(px),
            taffy::style::ExpandedLengthPercentageAuto::Percent(p) => {
                Some(p * containing_block.width)
            }
            _ => None,
        };
        if let Some(left_px) = left_px {
            let avail = (containing_block.width - left_px - layout.margin.left - layout.margin.right)
                .max(0.0);
            let mut min_c = 0.0f32;
            let mut max_c = 0.0f32;
            for (item, child_el) in items.iter().zip(&ordered) {
                if effective_position(child_el) == Position::Absolute {
                    continue;
                }
                match item {
                    InlineItem::Atomic { width, .. } => {
                        min_c = min_c.max(*width);
                        max_c += *width;
                    }
                    InlineItem::Text { text, style, nowrap } => {
                        let w = crate::text::measure_line(text, style, font_db).width;
                        if *nowrap {
                            min_c = min_c.max(w);
                        } else {
                            min_c = min_c.max(text_min_content_width(text, style, font_db));
                        }
                        max_c += w;
                    }
                    InlineItem::Skip => {}
                }
            }
            let used = min_c.max(avail).min(max_c);
            if (used - content_w).abs() > 0.5 {
                wrap_width = used;
                align_width = used;
                width_fix = Some(if content_box { used.max(0.0) } else { (used + left + right).max(0.0) });
            }
        }
    }

    let ifc = inline::layout_inline(&items, wrap_width, align_width, text, text.text_align, font_db);

    for ((child, margin), placement) in children.iter_mut().zip(margins).zip(&ifc.placements) {
        match placement {
            Placement::Atomic { x, y } => {
                child.rect.x = left + x + margin.left;
                child.rect.y = top + y + margin.top;
            }
            Placement::Text(frags) if !frags.is_empty() => {
                let x0 = frags.iter().map(|f| f.x).fold(f32::INFINITY, f32::min);
                let y0 = frags.iter().map(|f| f.y).fold(f32::INFINITY, f32::min);
                let x1 = frags.iter().map(|f| f.x + f.width).fold(f32::NEG_INFINITY, f32::max);
                let y1 = frags.iter().map(|f| f.y + f.height).fold(f32::NEG_INFINITY, f32::max);
                child.rect = LayoutRect {
                    x: left + x0,
                    y: top + y0,
                    width: x1 - x0,
                    height: y1 - y0,
                };
                let last = frags.last().expect("non-empty fragments");
                child.baseline = Some(last.y - y0 + last.baseline);
                // 单片段也走片段绘制：不折行、按片段行盒落基线，与排行一致。
                child.inline_fragments = Some(
                    frags
                        .iter()
                        .map(|f| TextFragment {
                            start: f.range.start,
                            end: f.range.end,
                            rect: LayoutRect {
                                x: f.x - x0,
                                y: f.y - y0,
                                width: f.width,
                                height: f.height,
                            },
                        })
                        .collect(),
                );
            }
            _ => continue,
        }
        child.output_fingerprint = layout_output_fingerprint(
            child.rect,
            &child.children,
            child.inline_fragments.as_deref(),
            child.wrap_width,
        );
    }

    // 容器 auto 高度 = 行盒高之和。两端 inset 都给定的绝对定位盒由包含块
    // 拉伸（k3 的 `.tickbox .txt{inset:0}`），高度归 taffy。
    let layout_style = &element.style.layout;
    let (_, css_height) = css_definite_size(element);
    let inset_stretched = effective_position(element) == Position::Absolute
        && css.is_some_and(|css| !css.0.inset.top.is_auto() && !css.0.inset.bottom.is_auto());
    if !inset_stretched
        && layout_style.height.is_none()
        && layout_style.height_percent.is_none()
        && !css_height
    {
        let height = top + ifc.height + bottom;
        let height_diff = (height - rect.height).abs();
        // 阈值 0.5px：taffy 布局取整（layout.size 四舍五入到整像素），fix 值
        // 与取整值的亚像素差（如 59.904 vs 60.0）永远满足 0.01——循环打满
        // 上限、每帧全量重登记（stripe 每帧 17 会话×4 轮的减速根因）。
        // Chrome 同样做布局取整，≤0.5px 视为取整噪声不重排。
        if height_diff > 0.5 || width_fix.is_some() {
            if height_diff > 0.5 {
                rect.height = height;
            }
            let size = if content_box {
                (height - top - bottom).max(0.0)
            } else {
                height
            };
            if std::env::var("OC_LAYOUT_DEBUG").as_deref() == Ok("2") {
                eprintln!(
                    "[fix] inline-height id={} width={width_fix:?} h_bb={height} rect_h={} d={height_diff} size={size} padb=({top},{bottom})",
                    element.style.id,
                    rect.height,
                );
            }
            fixes.push(AbsSizeFix {
                node: node_id,
                width: width_fix,
                height: Some(size),
            });
        }
    } else if let Some(width) = width_fix {
        if std::env::var("OC_LAYOUT_DEBUG").as_deref() == Ok("2") {
            eprintln!("[fix] inline-width id={} width={width:?}", element.style.id);
        }
        fixes.push(AbsSizeFix {
            node: node_id,
            width: Some(width),
            height: None,
        });
    }
    Some(top + ifc.last_baseline)
}

/// 文本元素自带横向盒边（padding/border/margin）：作为整体（原子）排行，
/// 不跨行断开。
fn has_inline_box_edges(element: &ElementNode) -> bool {
    let zero = taffy::style::LengthPercentage::length(0.0);
    let zero_auto = taffy::style::LengthPercentageAuto::length(0.0);
    element.style.css_layout.as_ref().is_some_and(|css| {
        css.0.padding.left != zero
            || css.0.padding.right != zero
            || css.0.border.left != zero
            || css.0.border.right != zero
            || css.0.margin.left != zero_auto
            || css.0.margin.right != zero_auto
    })
}

/// 原子行内盒的基线（相对其 border box 顶）：末个行盒的基线（flex 容器取
/// 首项）；没有行盒或 overflow 非 visible 时 `None`，按 CSS 落到 margin
/// box 底边。
fn atomic_baseline(
    element: &ElementNode,
    node: &LayoutNode,
    font_db: &fontdb::Database,
) -> Option<f32> {
    if element.style.visual.clip_contents {
        return None;
    }
    if let Some(baseline) = node.baseline {
        return Some(baseline);
    }
    match &element.kind {
        ElementKind::Text(text) => {
            let m = crate::text::measure_line(&text.text, &text.text_style, font_db);
            // 自身折行的文本取末行：行数按盒高折算。
            let lines = (node.rect.height / m.line_height).floor().max(1.0);
            Some((lines - 1.0) * m.line_height + m.baseline)
        }
        ElementKind::Div(_) => {
            let flex = element
                .style
                .css_layout
                .as_ref()
                .is_some_and(|css| css.0.display == taffy::prelude::Display::Flex);
            let in_flow: Vec<(&ElementNode, &LayoutNode)> = ordered_children(element)
                .into_iter()
                .map(|(_, child)| child)
                .zip(&node.children)
                .filter(|(child, _)| effective_position(child) != Position::Absolute)
                .collect();
            let pick = |&(child, child_node): &(&ElementNode, &LayoutNode)| {
                atomic_baseline(child, child_node, font_db).map(|b| child_node.rect.y + b)
            };
            if flex {
                in_flow.iter().find_map(pick)
            } else {
                in_flow.iter().rev().find_map(pick)
            }
        }
        _ => None,
    }
}

fn layout_output_fingerprint(
    rect: LayoutRect,
    children: &[LayoutNode],
    fragments: Option<&[TextFragment]>,
    wrap_width: Option<f32>,
) -> LayoutOutputFingerprint {
    let mut record_size_hasher = ahash::AHasher::default();
    F32Bits(rect.width).hash(&mut record_size_hasher);
    F32Bits(rect.height).hash(&mut record_size_hasher);
    let record_size = record_size_hasher.finish();

    let mut local_hasher = ahash::AHasher::default();
    F32Bits(rect.x).hash(&mut local_hasher);
    F32Bits(rect.y).hash(&mut local_hasher);
    F32Bits(rect.width).hash(&mut local_hasher);
    F32Bits(rect.height).hash(&mut local_hasher);
    if let Some(fragments) = fragments {
        for f in fragments {
            f.start.hash(&mut local_hasher);
            f.end.hash(&mut local_hasher);
            F32Bits(f.rect.x).hash(&mut local_hasher);
            F32Bits(f.rect.y).hash(&mut local_hasher);
            F32Bits(f.rect.width).hash(&mut local_hasher);
            F32Bits(f.rect.height).hash(&mut local_hasher);
        }
    }
    // 同一取整盒宽下，取整前的宽不同 = 折行可能不同。
    wrap_width.map(F32Bits).hash(&mut local_hasher);
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

fn resolve_dimension(value: Option<f32>, percent: Option<f32>, fallback: Dimension) -> Dimension {
    if let Some(pct) = percent {
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


