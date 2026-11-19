#[cfg(feature = "profile")]
use tracing::{Level, event, span};

use crate::analyze::compositor::OrderedSceneOp;
use crate::canvas::paint::{BlendMode, FillSpec, ImageFilterSpec, PaintSpec, PaintStyle};
use crate::display::list::{DisplayItem, DisplayRect, DisplayTransform, RectDisplayItem};
use crate::display::tree::DisplayNode;
use crate::geom::Mat4;
use crate::ir::draw_op::{DrawOp, Rect4};
use crate::ir::draw_types::{DrawOpRange, PaintId, PathOp};
use crate::media::VideoFrameRequest;
use crate::parse::transition::{SlideDirection, TransitionKind, WipeDirection};
use crate::render::builder::DrawOpBuilder;
use crate::style::{BorderRadius, Transform};

use super::{RenderCtx, RenderError};

// ── Display Item dispatch ──────────────────────────────────────────────

pub fn render_display_item(ctx: &mut RenderCtx, item: &DisplayItem) -> Result<(), RenderError> {
    render_display_item_direct(ctx, item)
}

fn render_display_item_direct(ctx: &mut RenderCtx, item: &DisplayItem) -> Result<(), RenderError> {
    match item {
        DisplayItem::Rect(rect) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_rect").entered();
            #[cfg(feature = "profile")]
            event!(target: "render.draw", Level::TRACE, kind = "draw", name = "rect", result = "count", amount = 1_u64);
            super::render_rect_with_shadows(ctx, rect)
        }
        DisplayItem::Timeline(timeline) => {
            #[cfg(feature = "profile")]
            let _span =
                span!(target: "render.backend", Level::TRACE, "draw_item_timeline").entered();
            super::render_timeline(ctx, timeline)
        }
        DisplayItem::Text(text) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_text").entered();
            #[cfg(feature = "profile")]
            event!(target: "render.draw", Level::TRACE, kind = "draw", name = "text", result = "count", amount = 1_u64);
            super::text::render_text_with_shadows(ctx, text)
        }
        DisplayItem::Bitmap(bitmap) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_bitmap").entered();
            super::render_bitmap_with_shadows(ctx, bitmap)
        }
        DisplayItem::Lottie(lottie) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_lottie").entered();
            super::render_lottie_with_shadows(ctx, lottie)
        }
        DisplayItem::DrawScript(script) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_script").entered();
            #[cfg(feature = "profile")]
            event!(target: "render.draw", Level::TRACE, kind = "draw", name = "script", result = "count", amount = 1_u64);
            super::render_draw_script(ctx, script)
        }
        DisplayItem::SvgPath(svg) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_svg").entered();
            super::render_svg_path(ctx, svg)
        }
        DisplayItem::Svg(item) => {
            #[cfg(feature = "profile")]
            let _span =
                span!(target: "render.backend", Level::TRACE, "draw_item_svg_file").entered();
            super::render_svg(ctx, item)
        }
    }
}

// ── Display Tree render ───────────────────────────────────────────────

fn display_rect_to_rect4(r: DisplayRect) -> Rect4 {
    Rect4 {
        x: r.x,
        y: r.y,
        width: r.width,
        height: r.height,
    }
}

pub(crate) struct BackdropLayerState {
    pub(crate) uses_layer: bool,
    pub(crate) has_backdrop_clip: bool,
    pub(crate) has_content_layer: bool,
    pub(crate) mask_paint: Option<PaintId>,
    pub(crate) mask_rect: Option<Rect4>,
}

pub(crate) fn save_composite_layer(
    builder: &mut DrawOpBuilder,
    opacity: f32,
    css_filter_image: Option<&ImageFilterSpec>,
    backdrop_filter_image: Option<&ImageFilterSpec>,
    backdrop_blur: Option<f32>,
    mask_image: Option<&crate::style::BackgroundFill>,
    bounds: Option<DisplayRect>,
    backdrop_bounds: DisplayRect,
    backdrop_radius: BorderRadius,
) -> BackdropLayerState {
    // `backdrop-filter` 生效条件：blur sigma 或非 blur 算子任一存在。
    let backdrop_sigma = backdrop_blur.filter(|sigma| *sigma > 0.0);
    // 折算产物 None ⇔ 全 identity 算子（`css_filter_image_filter` 跳过
    // identity 且七种颜色算子恒可折算——判定等价，见 H1）。
    let uses_backdrop_filter = backdrop_filter_image.is_some();
    let uses_css_filter = css_filter_image.is_some();
    let uses_mask = mask_image.is_some();
    let uses_layer = opacity < 1.0
        || backdrop_sigma.is_some()
        || uses_backdrop_filter
        || uses_css_filter
        || uses_mask;
    let mut has_backdrop_clip = false;
    let mut has_content_layer = false;
    if uses_layer {
        #[cfg(feature = "profile")]
        event!(
            target: "render.layer",
            Level::TRACE,
            kind = "layer",
            name = "save_layer",
            result = "count",
            amount = 1_u64
        );
        // `None` means "let the layer cover whatever the subtree paints": a
        // 3-D subtree projects outside the element rect, so clipping to it
        // would cut off the parts nearer the camera.
        let bounds_rect4 = display_rect_to_rect4(bounds.unwrap_or(backdrop_bounds));
        let layer_bounds = bounds.map(display_rect_to_rect4);
        // The stored value carries the CSS `backdrop-filter: blur(R)` radius —
        // same Gaussian sigma = R semantics as `filter: blur()` (see
        // `css_filter_image_filter`); pass through to Skia unchanged.
        if backdrop_sigma.is_some() || uses_backdrop_filter {
            // CSS applies `backdrop-filter` functions in declaration order;
            // Skia's `Compose(outer, inner)` runs `inner` first, so blur is
            // the inner filter and the non-blur ops (folded in declaration
            // order by `css_filter_image_filter`) wrap around it. The
            // element's own `filter` must not touch the backdrop, so only
            // `backdrop_filter` feeds this chain.
            let blur_filter = backdrop_sigma.map(|sigma| ImageFilterSpec::Blur {
                sigma_x: sigma,
                sigma_y: sigma,
                crop_rect: None,
                decal: false,
            });
            let color_filter_chain = backdrop_filter_image.cloned();
            let image_filter = match (blur_filter, color_filter_chain) {
                (Some(blur), Some(chain)) => {
                    ImageFilterSpec::Compose(Box::new(chain), Box::new(blur))
                }
                (Some(blur), None) => blur,
                (None, Some(chain)) => chain,
                // 入口条件已排除两者皆空。
                (None, None) => unreachable!("backdrop layer without any backdrop filter"),
            };
            let paint = PaintSpec {
                fill: FillSpec::Solid([1.0, 1.0, 1.0, opacity]),
                style: PaintStyle::Fill,
                stroke: None,
                anti_alias: true,
                blend_mode: BlendMode::SrcOver,
                image_filter: Some(image_filter),
                color_filter: None,
                mask_filter: None,
                path_effect: None,
            };
            let paint_id = builder.intern_paint(paint);
            builder.push(DrawOp::Save);
            builder.push(DrawOp::BeginPath);
            builder.push(DrawOp::Path(PathOp::AddRect {
                x: bounds_rect4.x,
                y: bounds_rect4.y,
                width: bounds_rect4.width,
                height: bounds_rect4.height,
            }));
            builder.push(DrawOp::ClipPath {
                anti_alias: false,
                fill_type: Default::default(),
            });
            has_backdrop_clip = true;
            // Backdrop semantics: the blur applies to what's beneath this
            // node's bounds; the node's own subtree paints unfiltered inside.
            // `bounds` carries the *element* rect — CSS backdrop-filter is
            // clipped to the border box, not to the shadow-padded layer
            // bounds the box-shadow needs.
            let backdrop_rect4 = display_rect_to_rect4(backdrop_bounds);
            builder.push(DrawOp::SaveBackdropLayer {
                bounds: Some(backdrop_rect4),
                radius: [
                    backdrop_radius.top_left,
                    backdrop_radius.top_right,
                    backdrop_radius.bottom_right,
                    backdrop_radius.bottom_left,
                ],
                paint: Some(paint_id),
            });
            // The node's own `filter` never touches the backdrop, but it still
            // applies to the subtree. Nest a content layer so both run, in the
            // CSS order (backdrop first, then the element's own filter).
            if let Some(image_filter) = css_filter_image.cloned() {
                let paint = PaintSpec {
                    fill: FillSpec::Solid([1.0, 1.0, 1.0, 1.0]),
                    style: PaintStyle::Fill,
                    stroke: None,
                    anti_alias: true,
                    blend_mode: BlendMode::SrcOver,
                    image_filter: Some(image_filter),
                    color_filter: None,
                    mask_filter: None,
                    path_effect: None,
                };
                let paint_id = builder.intern_paint(paint);
                builder.push(DrawOp::SaveLayer {
                    bounds: layer_bounds,
                    paint: Some(paint_id),
                    alpha: opacity,
                    backdrop: false,
                });
                has_content_layer = true;
            }
        } else if let Some(image_filter) = css_filter_image.cloned() {
            let paint = PaintSpec {
                fill: FillSpec::Solid([1.0, 1.0, 1.0, 1.0]),
                style: PaintStyle::Fill,
                stroke: None,
                anti_alias: true,
                blend_mode: BlendMode::SrcOver,
                image_filter: Some(image_filter),
                color_filter: None,
                mask_filter: None,
                path_effect: None,
            };
            let paint_id = builder.intern_paint(paint);
            builder.push(DrawOp::SaveLayer {
                bounds: layer_bounds,
                paint: Some(paint_id),
                alpha: opacity,
                backdrop: false,
            });
        } else {
            builder.push(DrawOp::SaveLayer {
                bounds: layer_bounds,
                paint: None,
                alpha: opacity,
                backdrop: false,
            });
        }
    }
    let mask_paint = mask_image.map(|fill| {
        let bounds = DisplayRect {
            x: 0.0,
            y: 0.0,
            ..backdrop_bounds
        };
        let mut paint = super::items::background_fill_to_paint_spec(
            fill,
            super::items::kurbo_rect(bounds),
            crate::canvas::paint::BlendMode::DstIn,
        );
        match &mut paint.fill {
            FillSpec::Solid(color) => *color = [0.0, 0.0, 0.0, color[3]],
            FillSpec::Shader(shader) => clear_shader_colors(shader),
        }
        builder.intern_paint(paint)
    });
    let mask_rect = mask_image.map(|_| {
        display_rect_to_rect4(DisplayRect {
            x: 0.0,
            y: 0.0,
            ..backdrop_bounds
        })
    });
    BackdropLayerState {
        uses_layer,
        has_backdrop_clip,
        has_content_layer,
        mask_paint,
        mask_rect,
    }
}

pub(crate) fn restore_backdrop_blur_layer(builder: &mut DrawOpBuilder, state: &BackdropLayerState) {
    if state.uses_layer {
        if state.has_content_layer {
            builder.push(DrawOp::Restore);
        }
        builder.push(DrawOp::Restore);
        if state.has_backdrop_clip {
            builder.push(DrawOp::Restore);
        }
    }
}

/// CSS 遮罩取图像的 alpha 通道：色标 RGB 清零，只保留 alpha。
fn clear_shader_colors(shader: &mut crate::canvas::paint::ShaderSpec) {
    use crate::canvas::paint::ShaderSpec;
    let colors = match shader {
        ShaderSpec::LinearGradient { colors, .. }
        | ShaderSpec::RadialGradient { colors, .. }
        | ShaderSpec::SweepGradient { colors, .. } => colors,
        _ => return,
    };
    for color in colors.iter_mut() {
        *color = [0.0, 0.0, 0.0, color[3]];
    }
}

/// 每条渲染路径取 composite 层 bounds 的策略。这是主树与隐藏子树投影
/// 之间唯一被允许的差异。
#[derive(Clone, Copy, Debug)]
pub(crate) enum LayerBoundsPolicy {
    /// 主显示树：节点可能进入 3-D 上下文，层 bounds 需为 3-D 外溢让位。
    MainTree,
    /// 隐藏子树：没有 3-D 上下文，层 bounds 就是 item 的可视 bounds。
    HiddenSubtree,
}

/// The composite inputs one node contributes to its layer prefix.
///
/// Both the main display tree and the hidden-subtree path build one of these
/// via [`NodeCompositeParams::from_node`] — the single projection from
/// `DisplayNode` fields — and hand it to [`begin_node_composite`], so the two
/// paths emit the same Save / transform / layer / mask prefix and the same
/// reverse suffix. The [`LayerBoundsPolicy`] argument is their only intended
/// difference.
#[derive(Clone, Debug)]
pub(crate) struct NodeCompositeParams<'a> {
    /// The node's transform — applied between the Save and the layer.
    transform: &'a DisplayTransform,
    opacity: f32,
    css_filter_image: Option<&'a ImageFilterSpec>,
    backdrop_filter_image: Option<&'a ImageFilterSpec>,
    backdrop_blur_sigma: Option<f32>,
    mask_image: Option<&'a crate::style::BackgroundFill>,
    /// Border radius of the backdrop clip.
    backdrop_radius: BorderRadius,
    /// Bounds of the composite layer (`None` = unbounded layer).
    layer_bounds: Option<DisplayRect>,
    /// Bounds the backdrop filter is clipped to (the element rect).
    backdrop_bounds: DisplayRect,
}

impl<'a> NodeCompositeParams<'a> {
    /// `DisplayNode` → composite params 的唯一投影（M4-⑥：主路径与隐藏
    /// 子树曾各自手填同一组字段，隐藏侧漏过 mask；两路现都走这里）。
    /// `mask_image` / `backdrop_blur_sigma` 是活语义，两路同样生效；
    /// `bounds` 是显式的唯一路径差异。
    pub(crate) fn from_node(node: &'a DisplayNode, bounds: LayerBoundsPolicy) -> Self {
        let backdrop_radius = match &node.item {
            DisplayItem::Rect(rect) => rect.paint.border_radius,
            _ => BorderRadius::default(),
        };
        let layer_bounds = match bounds {
            LayerBoundsPolicy::MainTree => {
                // A node inside a 3-D context paints outside its own element
                // rect (a `translateZ` toward the camera magnifies it), so
                // clipping its layer to `layer_bounds` would cut that overflow
                // off. Drop the bounds there.
                if is_3d(&node.transform) {
                    None
                } else {
                    Some(node.layer_bounds)
                }
            }
            LayerBoundsPolicy::HiddenSubtree => Some(node.item.visual_bounds()),
        };
        Self {
            transform: &node.transform,
            opacity: node.opacity,
            css_filter_image: node.css_filter_image.as_ref(),
            backdrop_filter_image: node.backdrop_filter_image.as_ref(),
            backdrop_blur_sigma: node.backdrop_blur_sigma,
            mask_image: node.mask_image.as_ref(),
            backdrop_radius,
            layer_bounds,
            backdrop_bounds: node.transform.bounds,
        }
    }
}

/// The composite frame a node opens. Opaque: callers only hand it back to
/// [`finish_node_composite`], so the layer/mask bookkeeping stays in one
/// place.
pub(crate) struct NodeCompositeFrame {
    layer_state: BackdropLayerState,
    mask_paint: Option<PaintId>,
    mask_rect: Option<Rect4>,
}

/// Open a node's composite frame: `Save` → transform → composite layer →
/// mask layer.
pub(crate) fn begin_node_composite(
    builder: &mut DrawOpBuilder,
    params: &NodeCompositeParams<'_>,
) -> NodeCompositeFrame {
    builder.push(DrawOp::Save);
    apply_transform(builder, params.transform);
    let mut layer_state = save_composite_layer(
        builder,
        params.opacity,
        params.css_filter_image,
        params.backdrop_filter_image,
        params.backdrop_blur_sigma,
        params.mask_image,
        params.layer_bounds,
        params.backdrop_bounds,
        params.backdrop_radius,
    );
    let mask_paint = layer_state.mask_paint.take();
    let mask_rect = layer_state.mask_rect.take();
    if mask_paint.is_some() {
        builder.push(DrawOp::SaveLayer {
            bounds: mask_rect,
            paint: None,
            alpha: 1.0,
            backdrop: false,
        });
    }
    NodeCompositeFrame {
        layer_state,
        mask_paint,
        mask_rect,
    }
}

/// Close a node's composite frame: mask `Restore` + DstIn rect → backdrop
/// restore → outer `Restore`. The exact reverse of [`begin_node_composite`].
pub(crate) fn finish_node_composite(builder: &mut DrawOpBuilder, frame: &NodeCompositeFrame) {
    if let (Some(paint), Some(rect)) = (frame.mask_paint, frame.mask_rect) {
        builder.push(DrawOp::Restore);
        builder.push(DrawOp::Rect { rect, paint });
    }
    restore_backdrop_blur_layer(builder, &frame.layer_state);
    builder.push(DrawOp::Restore);
}

/// Whether this node opens a 3-D context of its own, or is transformed into
/// one its parent opened.
fn is_3d(transform: &DisplayTransform) -> bool {
    transform.local_m4.is_some() || transform.perspective.is_some() || transform.preserve_3d
}

/// The 4×4 every node in the current 3-D context inherits.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Space3 {
    /// Maps this context's local space into the parent's, `z = 0` being the
    /// plane the enclosing draw-op transform already established.
    abs: Mat4,
    /// Homography `abs` projects to. `None` when it is exactly the identity,
    /// which is the common case and lets us skip the `Concat`.
    abs_h: Option<[f32; 9]>,
}

impl Space3 {
    pub(crate) fn root() -> Self {
        Self {
            abs: Mat4::IDENTITY,
            abs_h: None,
        }
    }

    fn compose(&self, local: Mat4) -> Self {
        let abs = self.abs.mul(&local);
        let h = abs.to_homography3();
        let is_identity = h == Mat4::IDENTITY.to_homography3();
        Self {
            abs,
            abs_h: if is_identity { None } else { Some(h) },
        }
    }
}

/// A node's accumulated 4×4 in the space its parent's children are drawn in.
/// The node's own `Translate` is part of it, because `apply_transform` has
/// already emitted it by the time this runs.
fn node_abs(space: Space3, transform: &DisplayTransform) -> Option<Space3> {
    let local = transform.local_m4?;
    Some(
        space
            .compose(Mat4::translate(
                transform.translation_x,
                transform.translation_y,
                0.0,
            ))
            .compose(local),
    )
}

/// CSS `backface-visibility: hidden`: a node whose own 3-D transform turns
/// its front face away is skipped before it emits any draw ops.
fn is_backfacing(space: Space3, transform: &DisplayTransform) -> bool {
    if !transform.backface_hidden {
        return false;
    }
    let Some(rotation) = transform.rotation_m4 else {
        return false;
    };
    let normal = space.abs.mul(&rotation);
    normal.0[10] <= 0.0
}

/// Enter a node's own 3-D space: the `Concat` that takes the enclosing draw-op
/// transform (which already includes this node's `Translate`) to its own.
fn enter_node_space(ctx: &mut RenderCtx, transform: &DisplayTransform) -> Space3 {
    let parent = ctx.space3;
    let Some(node) = node_abs(parent, transform) else {
        return parent;
    };
    let Some(node_h) = node.abs_h else {
        return parent;
    };
    // Incremental, so the enclosing `Save`/`Restore` still unwinds it and the
    // composite-layer bounds keep their meaning.
    let after_translate = parent.compose(Mat4::translate(
        transform.translation_x,
        transform.translation_y,
        0.0,
    ));
    // `Concat` pre-concatenates (CTM' = CTM · M), so M has to be
    // inv(CTM) · target, not target · inv(CTM).
    let matrix = match after_translate.abs_h {
        None => node_h,
        Some(parent_h) => match crate::geom::invert3(parent_h) {
            Some(inv) => crate::geom::mul3(inv, node_h),
            None => return parent,
        },
    };
    ctx.builder.push(DrawOp::Concat { matrix });
    ctx.space3 = node;
    node
}

/// The space a node's children are drawn in: its own, projected through its
/// CSS `perspective`, with the depth dropped when it is `transform-style: flat`.
///
/// The invariant `abs_h` must keep is *what the draw-op CTM currently holds*,
/// so every space change that is not already in the CTM has to be pushed here.
fn child_space(ctx: &mut RenderCtx, node: Space3, transform: &DisplayTransform) -> Space3 {
    let mut space = match transform.perspective {
        Some(distance) if distance > 0.0 => {
            let projected = node.compose(Mat4::perspective(distance));
            if let Some(projected_h) = projected.abs_h {
                let matrix = match node.abs_h {
                    None => projected_h,
                    Some(node_h) => match crate::geom::invert3(node_h) {
                        Some(inv) => crate::geom::mul3(inv, projected_h),
                        None => node_h,
                    },
                };
                ctx.builder.push(DrawOp::Concat { matrix });
            }
            projected
        }
        _ => node,
    };
    if !transform.preserve_3d {
        // Flattening only zeroes the z output row, which `to_homography3` never
        // reads, so the CTM still holds `space.abs_h` unchanged.
        space.abs = space.abs.flatten_depth();
    }
    space
}

pub fn render_display_tree(ctx: &mut RenderCtx) -> Result<(), RenderError> {
    ctx.space3 = Space3::root();
    render_scene_op(ctx, &ctx.ordered_scene.root)
}

fn item_visible_at_frame(ctx: &RenderCtx, item: &DisplayItem) -> bool {
    let DisplayItem::Bitmap(bitmap) = item else {
        return true;
    };
    let Some(timing) = bitmap.video_timing else {
        return true;
    };

    VideoFrameRequest {
        composition_time_secs: ctx.frame_ctx.frame as f64 / ctx.frame_ctx.fps.max(1) as f64,
        timing,
    }
    .is_visible()
}

fn render_scene_op(ctx: &mut RenderCtx, op: &OrderedSceneOp) -> Result<(), RenderError> {
    match op {
        OrderedSceneOp::ReusedSubtree { node } => render_reused_subtree(ctx, node),
        OrderedSceneOp::LiveSubtree { node, children } => render_live_subtree(ctx, node, children),
    }
}

/// Render a node's children in painter's order.
///
/// Inside a 3-D context the DOM order is not the visible order: a segment with
/// a larger projected z is nearer the viewer and must be drawn last. The depth
/// changes every frame, so the ordering lives here rather than in the build-time
/// `z_index` sort.
fn render_children(ctx: &mut RenderCtx, children: &[OrderedSceneOp]) -> Result<(), RenderError> {
    if ctx.space3.abs_h.is_none() {
        for child in children {
            render_scene_op(ctx, child)?;
        }
        return Ok(());
    }
    let mut ordered: Vec<(f32, &OrderedSceneOp)> = children
        .iter()
        .map(|child| {
            let transform = &child.node().transform;
            let depth = match transform.local_m4 {
                Some(_) => {
                    let m = node_abs(ctx.space3, transform)
                        .map(|space| space.abs)
                        .unwrap_or(ctx.space3.abs);
                    let (ox, oy) = transform
                        .origin
                        .point(transform.bounds.width, transform.bounds.height);
                    m.z_at(ox, oy)
                }
                None => ctx
                    .space3
                    .abs
                    .z_at(transform.translation_x, transform.translation_y),
            };
            (depth, child)
        })
        .collect();
    // Nearer last; ties keep DOM order so the common all-coplanar case is
    // untouched.
    ordered.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, child) in ordered {
        render_scene_op(ctx, child)?;
    }
    Ok(())
}

fn render_reused_subtree(ctx: &mut RenderCtx, node: &DisplayNode) -> Result<(), RenderError> {
    if node.opacity <= 0.0 {
        return Ok(());
    }
    if !item_visible_at_frame(ctx, &node.item) {
        return Ok(());
    }
    if is_backfacing(ctx.space3, &node.transform) {
        return Ok(());
    }

    let apply_plan = NodeCompositeParams::from_node(node, LayerBoundsPolicy::MainTree);
    let parent_space = ctx.space3;
    let apply_frame = begin_node_composite(ctx.builder, &apply_plan);
    let node_space = enter_node_space(ctx, apply_plan.transform);
    ctx.space3 = child_space(ctx, node_space, apply_plan.transform);

    let has_paint_clip = node.paint_clip.is_some();
    let has_clip = node.clip.is_some();

    if let Some(clip) = &node.paint_clip {
        ctx.builder.push(DrawOp::Save);
        let clip_rect4 = display_rect_to_rect4(clip.bounds);
        clip_bounds_with_radius(ctx.builder, clip_rect4, &clip.border_radius);
    }
    render_display_item(ctx, &node.item)?;
    if let Some(clip) = &node.clip {
        ctx.builder.push(DrawOp::Save);
        let clip_rect4 = display_rect_to_rect4(clip.bounds);
        clip_bounds_with_radius(ctx.builder, clip_rect4, &clip.border_radius);
    }

    // Render children dynamically（决策树重建：原 build_subtree 语义 =
    // 对每个 child 跑一遍 build_scene_op 决策）。
    let children: Vec<OrderedSceneOp> =
        node.children.iter().map(crate::analyze::compositor::build_scene_op).collect();
    render_children(ctx, &children)?;
    if has_clip {
        ctx.builder.push(DrawOp::Restore);
    }
    if has_paint_clip {
        ctx.builder.push(DrawOp::Restore);
    }

    ctx.space3 = parent_space;
    finish_node_composite(ctx.builder, &apply_frame);
    Ok(())
}

fn render_live_subtree(
    ctx: &mut RenderCtx,
    node: &DisplayNode,
    children: &[OrderedSceneOp],
) -> Result<(), RenderError> {
    if node.opacity <= 0.0 {
        return Ok(());
    }
    if !item_visible_at_frame(ctx, &node.item) {
        return Ok(());
    }
    if is_backfacing(ctx.space3, &node.transform) {
        return Ok(());
    }

    let apply_plan = NodeCompositeParams::from_node(node, LayerBoundsPolicy::MainTree);
    let parent_space = ctx.space3;
    let apply_frame = begin_node_composite(ctx.builder, &apply_plan);
    let node_space = enter_node_space(ctx, apply_plan.transform);
    ctx.space3 = child_space(ctx, node_space, apply_plan.transform);

    // Transition compositing: render from/to subtrees and blend them
    if let DisplayItem::Timeline(timeline) = &node.item
        && let Some(ref transition) = timeline.transition
        && children.len() == 2
    {
        let rect_item = RectDisplayItem {
            bounds: timeline.bounds,
            paint: timeline.paint.clone(),
        };
        super::render_rect_with_shadows(ctx, &rect_item)?;

        if let Some(clip) = &node.clip {
            ctx.builder.push(DrawOp::Save);
            let clip_bounds_rect4 = display_rect_to_rect4(clip.bounds);
            clip_bounds_with_radius(ctx.builder, clip_bounds_rect4, &clip.border_radius);
        }

        let from_marker = ctx.builder.begin_range();
        render_scene_op(ctx, &children[0])?;
        let from_range = ctx.builder.end_range(from_marker);

        let to_marker = ctx.builder.begin_range();
        render_scene_op(ctx, &children[1])?;
        let to_range = ctx.builder.end_range(to_marker);

        #[cfg(feature = "profile")]
        let _trans_span = span!(
            target: "render.transition",
            Level::TRACE,
            "draw_transition",
            transition_kind = transition_kind_str(&transition.kind),
        )
        .entered();

        let p = transition.progress.clamp(0.0, 1.0);
        render_transition_composite(
            ctx,
            from_range,
            to_range,
            p,
            &transition.kind,
            timeline.bounds,
        );

        if node.clip.is_some() {
            ctx.builder.push(DrawOp::Restore);
        }
        ctx.space3 = parent_space;
        finish_node_composite(ctx.builder, &apply_frame);
        return Ok(());
    }

    #[cfg(feature = "profile")]
    let _item_span = span!(target: "render.backend", Level::TRACE, "draw_item").entered();
    if let Some(clip) = &node.paint_clip {
        ctx.builder.push(DrawOp::Save);
        let clip_bounds_rect4 = display_rect_to_rect4(clip.bounds);
        clip_bounds_with_radius(ctx.builder, clip_bounds_rect4, &clip.border_radius);
    }
    render_display_item(ctx, &node.item)?;

    if let Some(clip) = &node.clip {
        ctx.builder.push(DrawOp::Save);
        let clip_bounds_rect4 = display_rect_to_rect4(clip.bounds);
        clip_bounds_with_radius(ctx.builder, clip_bounds_rect4, &clip.border_radius);
    }

    if let Some(ref slot) = node.draw_slot {
        if !slot.commands.is_empty() {
            super::render_draw_script(ctx, slot)?;
        }
    }

    render_children(ctx, children)?;

    if node.clip.is_some() {
        ctx.builder.push(DrawOp::Restore);
    }
    if node.paint_clip.is_some() {
        ctx.builder.push(DrawOp::Restore);
    }
    ctx.space3 = parent_space;
    finish_node_composite(ctx.builder, &apply_frame);
    Ok(())
}

fn render_transition_composite(
    ctx: &mut RenderCtx,
    from_range: DrawOpRange,
    to_range: DrawOpRange,
    progress: f32,
    kind: &TransitionKind,
    bounds: DisplayRect,
) {
    ctx.builder.push(DrawOp::ReplayRange { range: from_range });

    match kind {
        TransitionKind::Fade => {
            let bounds_rect4 = display_rect_to_rect4(bounds);
            ctx.builder.push(DrawOp::SaveLayer {
                bounds: Some(bounds_rect4),
                paint: None,
                alpha: progress,
                backdrop: false,
            });
            ctx.builder.push(DrawOp::ReplayRange { range: to_range });
            ctx.builder.push(DrawOp::Restore);
        }
        TransitionKind::Slide(dir) => {
            let (dx, dy) = match dir {
                SlideDirection::FromLeft => (-(1.0 - progress) * bounds.width, 0.0),
                SlideDirection::FromRight => ((1.0 - progress) * bounds.width, 0.0),
                SlideDirection::FromTop => (0.0, -(1.0 - progress) * bounds.height),
                SlideDirection::FromBottom => (0.0, (1.0 - progress) * bounds.height),
            };
            ctx.builder.push(DrawOp::Save);
            ctx.builder.push(DrawOp::Translate { x: dx, y: dy });
            ctx.builder.push(DrawOp::ReplayRange { range: to_range });
            ctx.builder.push(DrawOp::Restore);
        }
        TransitionKind::Wipe(dir) => {
            let clip = match dir {
                WipeDirection::FromLeft => Rect4 {
                    x: bounds.x,
                    y: bounds.y,
                    width: bounds.width * progress,
                    height: bounds.height,
                },
                WipeDirection::FromRight => Rect4 {
                    x: bounds.x + bounds.width * (1.0 - progress),
                    y: bounds.y,
                    width: bounds.width * progress,
                    height: bounds.height,
                },
                WipeDirection::FromTop => Rect4 {
                    x: bounds.x,
                    y: bounds.y,
                    width: bounds.width,
                    height: bounds.height * progress,
                },
                WipeDirection::FromBottom => Rect4 {
                    x: bounds.x,
                    y: bounds.y + bounds.height * (1.0 - progress),
                    width: bounds.width,
                    height: bounds.height * progress,
                },
                WipeDirection::FromTopLeft => Rect4 {
                    x: bounds.x,
                    y: bounds.y,
                    width: bounds.width * progress,
                    height: bounds.height * progress,
                },
                WipeDirection::FromTopRight => Rect4 {
                    x: bounds.x + bounds.width * (1.0 - progress),
                    y: bounds.y,
                    width: bounds.width * progress,
                    height: bounds.height * progress,
                },
                WipeDirection::FromBottomLeft => Rect4 {
                    x: bounds.x,
                    y: bounds.y + bounds.height * (1.0 - progress),
                    width: bounds.width * progress,
                    height: bounds.height * progress,
                },
                WipeDirection::FromBottomRight => Rect4 {
                    x: bounds.x + bounds.width * (1.0 - progress),
                    y: bounds.y + bounds.height * (1.0 - progress),
                    width: bounds.width * progress,
                    height: bounds.height * progress,
                },
            };
            ctx.builder.push(DrawOp::Save);
            ctx.builder.push(DrawOp::BeginPath);
            ctx.builder.push(DrawOp::Path(PathOp::AddRect {
                x: clip.x,
                y: clip.y,
                width: clip.width,
                height: clip.height,
            }));
            ctx.builder.push(DrawOp::ClipPath {
                anti_alias: false,
                fill_type: Default::default(),
            });
            ctx.builder.push(DrawOp::ReplayRange { range: to_range });
            ctx.builder.push(DrawOp::Restore);
        }
        TransitionKind::Iris => {
            let cx = bounds.x + bounds.width / 2.0;
            let cy = bounds.y + bounds.height / 2.0;
            let scale = progress.max(0.001);
            ctx.builder.push(DrawOp::Save);
            ctx.builder.push(DrawOp::Translate { x: cx, y: cy });
            ctx.builder.push(DrawOp::Scale { x: scale, y: scale });
            ctx.builder.push(DrawOp::Translate { x: -cx, y: -cy });
            ctx.builder.push(DrawOp::ReplayRange { range: to_range });
            ctx.builder.push(DrawOp::Restore);
        }
        TransitionKind::LightLeak(params) => {
            super::render_light_leak_transition(
                ctx, from_range, to_range, progress, params, bounds,
            );
        }
        TransitionKind::Gl(effect) => {
            super::render_gl_transition(ctx, from_range, to_range, progress, effect, bounds);
        }
        TransitionKind::ClockWipe => {
            let bounds_rect4 = display_rect_to_rect4(bounds);
            ctx.builder.push(DrawOp::SaveLayer {
                bounds: Some(bounds_rect4),
                paint: None,
                alpha: progress,
                backdrop: false,
            });
            ctx.builder.push(DrawOp::ReplayRange { range: to_range });
            ctx.builder.push(DrawOp::Restore);
        }
        // No effect: the incoming scene covers the outgoing one outright.
        TransitionKind::Cut => {
            ctx.builder.push(DrawOp::ReplayRange { range: to_range });
        }
    }
}

pub(crate) fn clip_bounds_with_radius(
    builder: &mut DrawOpBuilder,
    rect: Rect4,
    radius: &BorderRadius,
) {
    let w = rect.width;
    let h = rect.height;
    let clamp = |r: f32| {
        if r <= 0.0 {
            0.0
        } else {
            r.min(w / 2.0).min(h / 2.0)
        }
    };
    let tl = clamp(radius.top_left);
    let tr = clamp(radius.top_right);
    let br = clamp(radius.bottom_right);
    let bl = clamp(radius.bottom_left);

    let has_radius = tl > 0.0 || tr > 0.0 || br > 0.0 || bl > 0.0;

    builder.push(DrawOp::BeginPath);
    if has_radius {
        let x = rect.x;
        let y = rect.y;
        let x1 = x + rect.width;
        let y1 = y + rect.height;
        builder.push(DrawOp::Path(PathOp::MoveTo { x: x + tl, y }));
        builder.push(DrawOp::Path(PathOp::LineTo { x: x1 - tr, y }));
        if tr > 0.0 {
            builder.push(DrawOp::Path(PathOp::QuadTo {
                cx: x1,
                cy: y,
                x: x1,
                y: y + tr,
            }));
        }
        builder.push(DrawOp::Path(PathOp::LineTo { x: x1, y: y1 - br }));
        if br > 0.0 {
            builder.push(DrawOp::Path(PathOp::QuadTo {
                cx: x1,
                cy: y1,
                x: x1 - br,
                y: y1,
            }));
        }
        builder.push(DrawOp::Path(PathOp::LineTo { x: x + bl, y: y1 }));
        if bl > 0.0 {
            builder.push(DrawOp::Path(PathOp::QuadTo {
                cx: x,
                cy: y1,
                x,
                y: y1 - bl,
            }));
        }
        builder.push(DrawOp::Path(PathOp::LineTo { x, y: y + tl }));
        if tl > 0.0 {
            builder.push(DrawOp::Path(PathOp::QuadTo {
                cx: x,
                cy: y,
                x: x + tl,
                y,
            }));
        }
        builder.push(DrawOp::Path(PathOp::Close));
    } else {
        builder.push(DrawOp::Path(PathOp::AddRect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        }));
    }
    builder.push(DrawOp::ClipPath {
        anti_alias: true,
        fill_type: Default::default(),
    });
}

#[cfg(feature = "profile")]
fn transition_kind_str(kind: &TransitionKind) -> &'static str {
    match kind {
        TransitionKind::Slide(_) => "slide",
        TransitionKind::LightLeak(_) => "light_leak",
        TransitionKind::Gl(_) => "gltransition",
        _ => "other",
    }
}

pub(crate) fn apply_transform(builder: &mut DrawOpBuilder, transform: &DisplayTransform) {
    builder.push(DrawOp::Translate {
        x: transform.translation_x,
        y: transform.translation_y,
    });
    if transform.transforms.is_empty() {
        return;
    }
    // A chain carrying a 3-D operator is emitted by `enter_node_space` as one
    // 4x4 `Concat`; emitting the 2-D operators here as well would apply the
    // rotation twice.
    if transform.local_m4.is_some() {
        return;
    }
    // The pivot is the node's static `transform-origin` (default centre). Scale/
    // rotate/skew anchor here; translations are unaffected.
    let (center_x, center_y) = transform
        .origin
        .point(transform.bounds.width, transform.bounds.height);

    for t in transform.transforms.iter() {
        // 百分比位移在 DisplayTransform 构造期已按边界盒折算成 px 变体。
        match *t {
            Transform::TranslateX { value } => builder.push(DrawOp::Translate { x: value, y: 0.0 }),
            Transform::TranslateY { value } => builder.push(DrawOp::Translate { x: 0.0, y: value }),
            Transform::Translate { x, y } => builder.push(DrawOp::Translate { x, y }),
            Transform::Scale { value } => {
                builder.push(DrawOp::Translate {
                    x: center_x,
                    y: center_y,
                });
                builder.push(DrawOp::Scale { x: value, y: value });
                builder.push(DrawOp::Translate {
                    x: -center_x,
                    y: -center_y,
                });
            }
            Transform::ScaleX { value } => {
                builder.push(DrawOp::Translate {
                    x: center_x,
                    y: center_y,
                });
                builder.push(DrawOp::Scale { x: value, y: 1.0 });
                builder.push(DrawOp::Translate {
                    x: -center_x,
                    y: -center_y,
                });
            }
            Transform::ScaleY { value } => {
                builder.push(DrawOp::Translate {
                    x: center_x,
                    y: center_y,
                });
                builder.push(DrawOp::Scale { x: 1.0, y: value });
                builder.push(DrawOp::Translate {
                    x: -center_x,
                    y: -center_y,
                });
            }
            Transform::RotateDeg { value: deg } => builder.push(DrawOp::Rotate {
                degrees: deg,
                cx: center_x,
                cy: center_y,
            }),
            Transform::SkewXDeg { value: deg } => {
                builder.push(DrawOp::Translate {
                    x: center_x,
                    y: center_y,
                });
                builder.push(DrawOp::Skew {
                    sx: deg.to_radians().tan(),
                    sy: 0.0,
                });
                builder.push(DrawOp::Translate {
                    x: -center_x,
                    y: -center_y,
                });
            }
            Transform::SkewYDeg { value: deg } => {
                builder.push(DrawOp::Translate {
                    x: center_x,
                    y: center_y,
                });
                builder.push(DrawOp::Skew {
                    sx: 0.0,
                    sy: deg.to_radians().tan(),
                });
                builder.push(DrawOp::Translate {
                    x: -center_x,
                    y: -center_y,
                });
            }
            Transform::SkewDeg { x: x_deg, y: y_deg } => {
                builder.push(DrawOp::Translate {
                    x: center_x,
                    y: center_y,
                });
                builder.push(DrawOp::Skew {
                    sx: x_deg.to_radians().tan(),
                    sy: y_deg.to_radians().tan(),
                });
                builder.push(DrawOp::Translate {
                    x: -center_x,
                    y: -center_y,
                });
            }
            Transform::RotateXDeg { value: deg } => {
                builder.push(DrawOp::Translate {
                    x: center_x,
                    y: center_y,
                });
                builder.push(DrawOp::Concat {
                    matrix: crate::geom::Mat4::rotate_x_deg(deg).to_homography3(),
                });
                builder.push(DrawOp::Translate {
                    x: -center_x,
                    y: -center_y,
                });
            }
            Transform::RotateYDeg { value: deg } => {
                builder.push(DrawOp::Translate {
                    x: center_x,
                    y: center_y,
                });
                builder.push(DrawOp::Concat {
                    matrix: crate::geom::Mat4::rotate_y_deg(deg).to_homography3(),
                });
                builder.push(DrawOp::Translate {
                    x: -center_x,
                    y: -center_y,
                });
            }
            // A bare `translateZ` is a no-op on the node's own planar content;
            // it only moves the plane its children are drawn on, which the
            // 4×4 stack in `enter_node_space` already did.
            Transform::TranslateZ { .. } => {}
            // Percent translates are folded to px variants at DisplayTransform
            // construction (bounds known only there).
            Transform::TranslateXPercent { .. } | Transform::TranslateYPercent { .. } => {}
            Transform::Rotate3 {
                rotation_x,
                rotation_y,
                perspective,
            } => {
                // GSAP `rotationX`/`rotationY` + `transformPerspective`: GSAP does
                // NOT emit the CSS chain `perspective(P) rotateX(rx) rotateY(ry)`
                // — it composes its own matrix3d in which the point hits
                // rotateX FIRST (M = P·Ry·Rx, the transpose-order of the CSS
                // list). Verified against the reference render: with rx=6/ry=-7
                // the two orders diverge by ~5px at ±400px from the pivot, and
                // the reference raster matches Ry·Rx. `Mat4::rotate3` builds
                // exactly that matrix and folds the perspective row into `w`.
                // Emitted as a single column-major `DrawOp::Concat` about the
                // node's transform-origin (the pivot keeps z=0, so the planar
                // homography stays exact).
                builder.push(DrawOp::Translate {
                    x: center_x,
                    y: center_y,
                });
                builder.push(DrawOp::Concat {
                    matrix: crate::geom::Mat4::rotate3(rotation_x, rotation_y, perspective)
                        .to_homography3(),
                });
                builder.push(DrawOp::Translate {
                    x: -center_x,
                    y: -center_y,
                });
            }
        }
    }
}
