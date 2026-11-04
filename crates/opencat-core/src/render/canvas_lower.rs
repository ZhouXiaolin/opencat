use crate::display::list::{
    DisplayItem, DrawScriptDisplayItem,
};
use crate::display::tree::{DisplayNode, HiddenChildDisplayNode};
use crate::ir::GeneratedImageTable;
use crate::ir::draw_op::{
    ColorU8, DrawOp, Rect4,
};
use crate::ir::draw_types::ImageRef;
use crate::ir::draw_types::{
    ChildRange, PathOp, RuntimeEffectChildRef,
    ScriptRuntimeEffectChild, SubtreeId,
};
use crate::script::canvas_op::CanvasOp;
use crate::render::builder::DrawOpBuilder;

use super::RenderError;
use super::ctx::RenderCtx;

use kurbo::BezPath;

use super::items::{CanvasShadow, LocalPaintState, SENTINEL_FILL, SENTINEL_STROKE, rect4_xywh};
use super::shadow_filter::{push_shadowed, shadow_layer_paint_id};

pub fn render_draw_script(
    ctx: &mut RenderCtx,
    item: &DrawScriptDisplayItem,
) -> Result<(), RenderError> {
    let mut state = LocalPaintState::default();

    let needs_alpha_layer = item
        .commands
        .iter()
        .any(|cmd| matches!(cmd, CanvasOp::Op(DrawOp::Clear { .. })));

    let clip_rect = rect4_xywh(
        item.bounds.x,
        item.bounds.y,
        item.bounds.width,
        item.bounds.height,
    );

    if needs_alpha_layer {
        ctx.builder.push(DrawOp::SaveLayer {
            bounds: Some(clip_rect),
            paint: None,
            alpha: 1.0,
            backdrop: false,
        });
    } else {
        ctx.builder.push(DrawOp::Save);
        ctx.builder.push(DrawOp::BeginPath);
        ctx.builder.push(DrawOp::Path(PathOp::AddRect {
            x: item.bounds.x,
            y: item.bounds.y,
            width: item.bounds.width,
            height: item.bounds.height,
        }));
        ctx.builder.push(DrawOp::ClipPath { anti_alias: true, fill_type: Default::default() });
    }

    for command in &item.commands {
        match command {
            CanvasOp::Op(op @ DrawOp::DrawSubtreePicture { .. }) => {
                execute_draw_subtree_picture(ctx, op, &item.hidden_subtree)?;
            }
            CanvasOp::RuntimeEffect { .. } => {
                execute_script_runtime_effect(ctx, command, &item.hidden_subtree, &mut state)?;
            }
            other => {
                lower_canvas_op(&mut ctx.builder, other, &mut state)?;
            }
        }
    }

    ctx.builder.push(DrawOp::Restore);
    Ok(())
}

fn execute_draw_subtree_picture(
    ctx: &mut RenderCtx,
    op: &DrawOp,
    hidden_subtree: &[HiddenChildDisplayNode],
) -> Result<(), RenderError> {
    let DrawOp::DrawSubtreePicture { owner_id, x, y } = op else {
        return Ok(());
    };
    if ctx.hidden_picture_stack.contains(owner_id) {
        return Err(RenderError::InvalidArgument(format!(
            "recursive hidden canvas picture `{owner_id}`"
        )));
    }
    let subtree = record_hidden_subtree(ctx, owner_id, hidden_subtree)?;
    ctx.builder.push(DrawOp::ReplaySubtreePicture {
        subtree,
        x: *x,
        y: *y,
    });
    Ok(())
}

fn record_hidden_subtree(
    ctx: &mut RenderCtx,
    owner_id: &str,
    hidden_subtree: &[HiddenChildDisplayNode],
) -> Result<SubtreeId, RenderError> {
    if ctx.hidden_picture_stack.iter().any(|item| item == owner_id) {
        return Err(RenderError::InvalidArgument(format!(
            "recursive hidden canvas picture `{owner_id}`"
        )));
    }
    ctx.hidden_picture_stack.push(owner_id.to_string());

    let stack = ctx.hidden_picture_stack.clone();
    let catalog = ctx.catalog;
    let frame_ctx = ctx.frame_ctx;
    let display_tree = ctx.display_tree;
    let ordered_scene = ctx.ordered_scene;
    let font_db = ctx.font_db;
    let generated_images: &mut GeneratedImageTable = ctx.generated_images;
    let space3 = ctx.space3;
    let result = ctx.builder.record_subtree(|builder| {
        let mut subtree_ctx = RenderCtx {
            catalog,
            frame_ctx,
            display_tree,
            ordered_scene,
            builder,
            font_db,
            hidden_picture_stack: stack,
            generated_images,
            space3,
        };
        for child in hidden_subtree {
            if child.owner_id == owner_id {
                render_hidden_child_node(&mut subtree_ctx, &child.node)?;
            }
        }
        Ok(())
    });

    ctx.hidden_picture_stack.pop();
    result
}

/// Expand a `DrawOp::ScriptRuntimeEffect` into a canonical `DrawOp::RuntimeEffect`,
/// resolving `ScriptRuntimeEffectChild::PictureSubtree` children into isolated
/// subtree programs referenced by `RuntimeEffectChildRef::SubtreePicture`.
fn execute_script_runtime_effect(
    ctx: &mut RenderCtx,
    op: &CanvasOp,
    hidden_subtree: &[HiddenChildDisplayNode],
    _state: &mut LocalPaintState,
) -> Result<(), RenderError> {
    let CanvasOp::RuntimeEffect {
        sksl,
        uniforms_bytes,
        children,
        dst,
    } = op
    else {
        return Ok(());
    };

    // Resolve children. Picture children require recording the matching
    // subtree ops into the main op stream first so we can point a
    // `DrawOpRange` at them.
    let mut resolved: Vec<RuntimeEffectChildRef> = Vec::with_capacity(children.len());
    for c in children {
        match c {
            ScriptRuntimeEffectChild::Image(img) => {
                resolved.push(RuntimeEffectChildRef::Image(img.clone()));
            }
            ScriptRuntimeEffectChild::PictureSubtree { owner_id } => {
                let subtree = record_hidden_subtree(ctx, owner_id, hidden_subtree)?;
                resolved.push(RuntimeEffectChildRef::SubtreePicture(subtree));
            }
        }
    }

    use std::hash::{Hash, Hasher};
    let mut hasher = ahash::AHasher::default();
    sksl.as_bytes().hash(&mut hasher);
    let hash = hasher.finish();
    let effect_id = ctx.builder.intern_effect(hash, sksl);
    let uniforms_id = ctx.builder.intern_bytes(uniforms_bytes);
    let child_start = ctx.builder.children_len() as u32;
    let child_len = resolved.len() as u32;
    for c in resolved {
        ctx.builder.push_child(c);
    }
    ctx.builder.push(DrawOp::RuntimeEffect {
        effect: effect_id,
        uniforms: uniforms_id,
        children: ChildRange {
            start: child_start,
            len: child_len,
        },
        dst: *dst,
    });
    Ok(())
}

fn render_hidden_child_node(ctx: &mut RenderCtx, node: &DisplayNode) -> Result<(), RenderError> {
    if node.opacity <= 0.0 {
        return Ok(());
    }

    ctx.builder.push(DrawOp::Save);
    super::dispatch::apply_transform(ctx.builder, &node.transform);

    let bounds = node.item.visual_bounds();
    let backdrop_radius = match &node.item {
        DisplayItem::Rect(rect) => rect.paint.border_radius,
        _ => Default::default(),
    };
    let layer_state = super::dispatch::save_composite_layer(
        ctx.builder,
        node.opacity,
        node.css_filter_image.as_ref(),
        node.backdrop_filter_image.as_ref(),
        node.backdrop_blur_sigma,
        Some(bounds),
        node.transform.bounds,
        backdrop_radius,
    );

    match &node.item {
        DisplayItem::Rect(rect) => super::render_rect_with_shadows(ctx, rect)?,
        DisplayItem::Text(text) => super::text::render_text_with_shadows(ctx, text)?,
        DisplayItem::DrawScript(script) => super::render_draw_script(ctx, script)?,
        DisplayItem::SvgPath(svg) => super::render_svg_path(ctx, svg)?,
        DisplayItem::Svg(item) => super::render_svg(ctx, item)?,
        DisplayItem::Bitmap(bitmap) => super::render_bitmap_with_shadows(ctx, bitmap)?,
        DisplayItem::Lottie(lottie) => super::render_lottie_with_shadows(ctx, lottie)?,
        DisplayItem::Timeline(timeline) => super::render_timeline(ctx, timeline)?,
    }

    if let Some(clip) = &node.clip {
        ctx.builder.push(DrawOp::Save);
        super::dispatch::clip_bounds_with_radius(
            ctx.builder,
            Rect4 {
                x: clip.bounds.x,
                y: clip.bounds.y,
                width: clip.bounds.width,
                height: clip.bounds.height,
            },
            &clip.border_radius,
        );
    }

    if let Some(slot) = &node.draw_slot
        && !slot.commands.is_empty()
    {
        super::render_draw_script(ctx, slot)?;
    }

    for child in &node.children {
        render_hidden_child_node(ctx, child)?;
    }

    if node.clip.is_some() {
        ctx.builder.push(DrawOp::Restore);
    }
    super::dispatch::restore_backdrop_blur_layer(ctx.builder, &layer_state);
    ctx.builder.push(DrawOp::Restore);
    Ok(())
}

pub(crate) fn execute_draw_op(
    b: &mut crate::render::builder::DrawOpBuilder,
    op: &DrawOp,
    state: &mut LocalPaintState,
) -> Result<(), RenderError> {
    match op {
        // ── Stack management ──────────────────────────────────────────
        DrawOp::Save => {
            b.push(DrawOp::Save);
        }
        DrawOp::SaveLayer {
            bounds,
            paint,
            alpha,
            ..
        } => {
            b.push(DrawOp::SaveLayer {
                bounds: *bounds,
                paint: *paint,
                alpha: *alpha,
                backdrop: false,
            });
        }
        // svg `<pattern>` fill — emitted by the svg backend, forwarded
        // verbatim (subtree id stays valid: same builder table).
        DrawOp::FillPattern {
            path,
            subtree,
            tile,
            pattern_transform,
            opacity,
        } => {
            b.push(DrawOp::FillPattern {
                path: *path,
                subtree: *subtree,
                tile: *tile,
                pattern_transform: *pattern_transform,
                opacity: *opacity,
            });
        }
        // svg `<pattern>` stroke — same forwarding contract.
        DrawOp::StrokePattern {
            path,
            subtree,
            tile,
            pattern_transform,
            opacity,
            stroke,
        } => {
            b.push(DrawOp::StrokePattern {
                path: *path,
                subtree: *subtree,
                tile: *tile,
                pattern_transform: *pattern_transform,
                opacity: *opacity,
                stroke: stroke.clone(),
            });
        }
        // svg `<mask>` — same forwarding contract.
        DrawOp::MaskClip {
            subtree,
            kind,
            region,
        } => {
            b.push(DrawOp::MaskClip {
                subtree: *subtree,
                kind: *kind,
                region: *region,
            });
        }
        // Not produced by the canvas script API (only by the renderer's own
        // backdrop-filter emission), but pass it through faithfully.
        DrawOp::SaveBackdropLayer { bounds, radius, paint } => {
            b.push(DrawOp::SaveBackdropLayer {
                bounds: *bounds,
                radius: *radius,
                paint: *paint,
            });
        }
        DrawOp::Restore => {
            b.push(DrawOp::Restore);
        }
        DrawOp::RestoreToCount { count } => {
            b.push(DrawOp::RestoreToCount { count: *count });
        }

        // ── Transforms ────────────────────────────────────────────────
        DrawOp::Translate { x, y } => {
            b.push(DrawOp::Translate { x: *x, y: *y });
        }
        DrawOp::Scale { x, y } => {
            b.push(DrawOp::Scale { x: *x, y: *y });
        }
        DrawOp::Rotate { degrees, cx, cy } => {
            b.push(DrawOp::Rotate {
                degrees: *degrees,
                cx: *cx,
                cy: *cy,
            });
        }
        DrawOp::Skew { sx, sy } => {
            b.push(DrawOp::Skew { sx: *sx, sy: *sy });
        }
        DrawOp::Concat { matrix } => {
            b.push(DrawOp::Concat { matrix: *matrix });
        }

        // ── Paint state setters ───────────────────────────────────────
        DrawOp::SetFillStyle { color } => {
            state.fill_color = *color;
            b.push(DrawOp::SetFillStyle { color: *color });
        }
        DrawOp::SetStrokeStyle { color } => {
            state.stroke_color = *color;
            b.push(DrawOp::SetStrokeStyle { color: *color });
        }
        DrawOp::SetLineWidth { width } => {
            state.line_width = *width;
            b.push(DrawOp::SetLineWidth { width: *width });
        }
        DrawOp::SetLineCap { cap } => {
            state.line_cap = *cap;
            b.push(DrawOp::SetLineCap { cap: *cap });
        }
        DrawOp::SetLineJoin { join } => {
            state.line_join = *join;
            b.push(DrawOp::SetLineJoin { join: *join });
        }
        DrawOp::SetLineDash { intervals, phase } => {
            state.line_dash = None;
            state.line_dash_phase = *phase;
            b.push(DrawOp::SetLineDash {
                intervals: *intervals,
                phase: *phase,
            });
        }
        // Pre-intern form from the script recorder (`canvas_set_line_dash`):
        // intern the intervals and emit the canonical dash op.
        DrawOp::ClearLineDash => {
            state.line_dash = None;
            state.line_dash_phase = 0.0;
            b.push(DrawOp::ClearLineDash);
        }
        DrawOp::SetStyleLinearGradient { stroke, start, end, stops } => {
            b.push(DrawOp::SetStyleLinearGradient {
                stroke: *stroke,
                start: *start,
                end: *end,
                stops: *stops,
            });
        }
        // Same forwarding for the radial form (createRadialGradient).
        DrawOp::SetStyleRadialGradient { stroke, start, end, stops } => {
            b.push(DrawOp::SetStyleRadialGradient {
                stroke: *stroke,
                start: *start,
                end: *end,
                stops: *stops,
            });
        }
        DrawOp::SetGlobalAlpha { alpha } => {
            state.global_alpha = alpha.clamp(0.0, 1.0);
            b.push(DrawOp::SetGlobalAlpha {
                alpha: state.global_alpha,
            });
        }
        DrawOp::SetAntiAlias { enabled } => {
            state.anti_alias = *enabled;
            b.push(DrawOp::SetAntiAlias { enabled: *enabled });
        }

        // ── Clear ─────────────────────────────────────────────────────
        DrawOp::Clear { color } => {
            b.push(DrawOp::Clear { color: *color });
        }

        // ── Path ops (pushed as-is; paint state managed by executor) ──
        DrawOp::BeginPath => {
            b.push(DrawOp::BeginPath);
        }
        DrawOp::Path(path_op) => {
            b.push(DrawOp::Path(path_op.clone()));
        }
        DrawOp::FillPath => push_shadowed(b, state.shadow, DrawOp::FillPath),
        DrawOp::StrokePath => push_shadowed(b, state.shadow, DrawOp::StrokePath),
        DrawOp::ClipPath { anti_alias, fill_type } => {
            b.push(DrawOp::ClipPath {
                anti_alias: *anti_alias,
                fill_type: *fill_type,
            });
        }

        // ── Paint-bearing ops with sentinel resolution ────────────────
        DrawOp::Arc {
            rect,
            start,
            sweep,
            use_center,
            paint,
        } if paint.0 == SENTINEL_FILL => {
            let paint_id = b.intern_paint(state.fill_paint_spec());
            b.push(DrawOp::Arc {
                rect: *rect,
                start: *start,
                sweep: *sweep,
                use_center: *use_center,
                paint: paint_id,
            });
        }
        DrawOp::Points {
            mode,
            points,
            paint,
        } if paint.0 == SENTINEL_STROKE => {
            let paint_id = b.intern_paint(state.stroke_paint_spec());
            b.push(DrawOp::Points {
                mode: *mode,
                points: *points,
                paint: paint_id,
            });
        }
        DrawOp::DRRect {
            outer,
            inner,
            paint,
        } if paint.0 == SENTINEL_FILL => {
            let paint_id = b.intern_paint(state.fill_paint_spec());
            b.push(DrawOp::DRRect {
                outer: *outer,
                inner: *inner,
                paint: paint_id,
            });
        }
        DrawOp::DRRect {
            outer,
            inner,
            paint,
        } if paint.0 == SENTINEL_STROKE => {
            let paint_id = b.intern_paint(state.stroke_paint_spec());
            b.push(DrawOp::DRRect {
                outer: *outer,
                inner: *inner,
                paint: paint_id,
            });
        }
        DrawOp::Paint { paint } if paint.0 == SENTINEL_FILL => {
            let paint_id = b.intern_paint(state.fill_paint_spec());
            b.push(DrawOp::Paint { paint: paint_id });
        }

        // ── Image ops (push as-is) ────────────────────────────────────
        DrawOp::Image { image, x, y, paint } => {
            b.push(DrawOp::Image {
                image: image.clone(),
                x: *x,
                y: *y,
                paint: *paint,
            });
        }
        DrawOp::ImageRect {
            image,
            src,
            dst,
            paint,
        } => {
            b.push(DrawOp::ImageRect {
                image: image.clone(),
                src: *src,
                dst: *dst,
                paint: *paint,
            });
        }
        DrawOp::LottieRect {
            bundle_id,
            frame,
            dst,
        } => {
            b.push(DrawOp::LottieRect {
                bundle_id: bundle_id.clone(),
                frame: *frame,
                dst: *dst,
            });
        }

        // ── Fallback: push remaining variants as-is ───────────────────
        DrawOp::Rect { rect, paint } => {
            b.push(DrawOp::Rect {
                rect: *rect,
                paint: *paint,
            });
        }
        DrawOp::RRect { rect, radii, paint } => {
            b.push(DrawOp::RRect {
                rect: *rect,
                radii: *radii,
                paint: *paint,
            });
        }
        DrawOp::DRRect {
            outer,
            inner,
            paint,
        } => {
            b.push(DrawOp::DRRect {
                outer: *outer,
                inner: *inner,
                paint: *paint,
            });
        }
        DrawOp::Oval { rect, paint } => {
            b.push(DrawOp::Oval {
                rect: *rect,
                paint: *paint,
            });
        }
        DrawOp::Circle {
            cx,
            cy,
            radius,
            paint,
        } => {
            b.push(DrawOp::Circle {
                cx: *cx,
                cy: *cy,
                radius: *radius,
                paint: *paint,
            });
        }
        DrawOp::Arc {
            rect,
            start,
            sweep,
            use_center,
            paint,
        } => {
            b.push(DrawOp::Arc {
                rect: *rect,
                start: *start,
                sweep: *sweep,
                use_center: *use_center,
                paint: *paint,
            });
        }
        DrawOp::Line {
            x0,
            y0,
            x1,
            y1,
            paint,
        } => {
            b.push(DrawOp::Line {
                x0: *x0,
                y0: *y0,
                x1: *x1,
                y1: *y1,
                paint: *paint,
            });
        }
        DrawOp::Points {
            mode,
            points,
            paint,
        } => {
            b.push(DrawOp::Points {
                mode: *mode,
                points: *points,
                paint: *paint,
            });
        }
        DrawOp::Paint { paint } => {
            b.push(DrawOp::Paint { paint: *paint });
        }
        DrawOp::DrawPath { path, paint } => {
            b.push(DrawOp::DrawPath {
                path: *path,
                paint: *paint,
            });
        }
        DrawOp::RuntimeEffect {
            effect,
            uniforms,
            children,
            dst,
        } => {
            b.push(DrawOp::RuntimeEffect {
                effect: *effect,
                uniforms: *uniforms,
                children: *children,
                dst: *dst,
            });
        }
        DrawOp::ReplayRange { range } => {
            b.push(DrawOp::ReplayRange { range: *range });
        }
        DrawOp::DrawSubtreePicture { owner_id, x, y } => {
            b.push(DrawOp::DrawSubtreePicture {
                owner_id: owner_id.clone(),
                x: *x,
                y: *y,
            });
        }
        DrawOp::ReplaySubtreePicture { subtree, x, y } => {
            b.push(DrawOp::ReplaySubtreePicture {
                subtree: *subtree,
                x: *x,
                y: *y,
            });
        }
    }
    Ok(())
}

// ── Canvas 2D 翻译层 ─────────────────────────────────────────────────
//
// canvas 2D 录制面命令（`CanvasOp`）→ canonical `DrawOp` 流。浏览器语义
// （shadow 状态机、putImageData 直写、组级阴影层）与录制面无法 intern 的
// 裸 payload（图像字节、f32 stops、dash 区间）在这里消化；`execute_draw_op`
// 及下游（engine replay / 编码器）只见纯 Skia 语义。
pub(crate) fn lower_canvas_op(
    b: &mut DrawOpBuilder,
    op: &CanvasOp,
    state: &mut LocalPaintState,
) -> Result<(), RenderError> {
    match op {
        CanvasOp::Op(canonical) => execute_draw_op(b, canonical, state)?,
        // `ctx.drawImage`：编码字节 intern 进帧字节表，发标准 `ImageRect`。
        // drawImage 参与浏览器 alpha 合成（heatmap 逐点 globalAlpha 图章）：
        // globalAlpha<1 时 intern 带 alpha 的 SrcOver 漆——Skia 的
        // drawImageRect 会把漆 alpha 乘到图像上；globalAlpha=1 不发漆，
        // 保持无漆快路径。
        CanvasOp::Image { bytes, src, dst, alpha } => {
            let data = b.intern_bytes(bytes);
            let paint = (*alpha < 1.0).then(|| {
                b.intern_paint(crate::canvas::paint::PaintSpec {
                    fill: crate::canvas::paint::FillSpec::Solid([1.0, 1.0, 1.0, *alpha]),
                    style: crate::canvas::paint::PaintStyle::Fill,
                    stroke: None,
                    anti_alias: true,
                    blend_mode: crate::canvas::paint::BlendMode::SrcOver,
                    image_filter: None,
                    color_filter: None,
                    mask_filter: None,
                    path_effect: None,
                })
            });
            b.push(DrawOp::ImageRect {
                image: ImageRef::Encoded { data },
                src: *src,
                dst: *dst,
                paint,
            });
        }
        // `putImageData`：像素直写——intern Src 混合漆（不做 alpha 合成）
        // 后发标准 `ImageRect`。fill 必须不透明：Skia 的 drawImageRect 会把
        // 漆的 alpha 乘到图像上，透明 fill 等于整图消失（heatmap 白图根因）。
        CanvasOp::PutImage { image, dst } => {
            let paint = b.intern_paint(crate::canvas::paint::PaintSpec {
                fill: crate::canvas::paint::FillSpec::Solid([1.0, 1.0, 1.0, 1.0]),
                style: crate::canvas::paint::PaintStyle::Fill,
                stroke: None,
                anti_alias: false,
                blend_mode: crate::canvas::paint::BlendMode::Src,
                image_filter: None,
                color_filter: None,
                mask_filter: None,
                path_effect: None,
            });
            b.push(DrawOp::ImageRect {
                image: image.clone(),
                src: None,
                dst: *dst,
                paint: Some(paint),
            });
        }
        // `setLineDash`：intern 区间后发标准 `SetLineDash`。
        CanvasOp::SetLineDash { intervals, phase } => {
            let range = b.intern_f32_range(intervals);
            state.line_dash = None;
            state.line_dash_phase = *phase;
            b.push(DrawOp::SetLineDash {
                intervals: range,
                phase: *phase,
            });
        }
        // 渐变裸 stops：intern f32 池后发标准渐变 op。
        CanvasOp::SetStyleLinearGradient {
            stroke,
            start,
            end,
            stops,
        } => {
            let range = b.intern_f32_range(stops);
            b.push(DrawOp::SetStyleLinearGradient {
                stroke: *stroke,
                start: *start,
                end: *end,
                stops: range,
            });
        }
        CanvasOp::SetStyleRadialGradient {
            stroke,
            start,
            end,
            stops,
        } => {
            let range = b.intern_f32_range(stops);
            b.push(DrawOp::SetStyleRadialGradient {
                stroke: *stroke,
                start: *start,
                end: *end,
                stops: range,
            });
        }
        // canvas 阴影状态机：sticky 状态，无 Skia op——后续 fill/stroke 由
        // `push_shadowed` 包进 SaveLayer(DropShadow)。
        CanvasOp::SetShadow {
            color,
            blur,
            dx,
            dy,
        } => {
            state.shadow = if color.a == 0 {
                None
            } else {
                Some(CanvasShadow {
                    color: *color,
                    blur: *blur,
                    dx: *dx,
                    dy: *dy,
                })
            };
        }
        // 组级阴影（canvas pattern 填充）：开 Save + SaveLayer(DropShadow)，
        // 组内 op 的 alpha 被滤镜折成阴影；调用方用等量 Restore 收尾。
        CanvasOp::BeginShadowLayer => {
            let sh = state.shadow.unwrap_or(CanvasShadow {
                color: ColorU8 {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0,
                },
                blur: 0.0,
                dx: 0.0,
                dy: 0.0,
            });
            let paint_id = shadow_layer_paint_id(b, sh);
            b.push(DrawOp::Save);
            if sh.dx != 0.0 || sh.dy != 0.0 {
                b.push(DrawOp::Translate {
                    x: sh.dx,
                    y: sh.dy,
                });
            }
            b.push(DrawOp::SaveLayer {
                bounds: None,
                paint: Some(paint_id),
                alpha: 1.0,
                backdrop: false,
            });
        }
        // RuntimeEffect 永远走 `execute_script_runtime_effect`（render 循环
        // 就地分发，PictureSubtree 子项需要 hidden_subtree）。
        CanvasOp::RuntimeEffect { .. } => {
            return Err(RenderError::InvalidArgument(
                "RuntimeEffect must be expanded by execute_script_runtime_effect".into(),
            ));
        }
    }
    Ok(())
}

// ── SVG path ─────────────────────────────────────────────────────────

pub(crate) fn svg_path_to_ops(svg: &str) -> Option<Vec<PathOp>> {
    let bez = BezPath::from_svg(svg).ok()?;
    let mut ops = Vec::new();
    for el in bez.elements() {
        match el {
            kurbo::PathEl::MoveTo(p) => {
                ops.push(PathOp::MoveTo {
                    x: p.x as f32,
                    y: p.y as f32,
                });
            }
            kurbo::PathEl::LineTo(p) => {
                ops.push(PathOp::LineTo {
                    x: p.x as f32,
                    y: p.y as f32,
                });
            }
            kurbo::PathEl::QuadTo(p1, p2) => {
                ops.push(PathOp::QuadTo {
                    cx: p1.x as f32,
                    cy: p1.y as f32,
                    x: p2.x as f32,
                    y: p2.y as f32,
                });
            }
            kurbo::PathEl::CurveTo(p1, p2, p3) => {
                ops.push(PathOp::CubicTo {
                    c1x: p1.x as f32,
                    c1y: p1.y as f32,
                    c2x: p2.x as f32,
                    c2y: p2.y as f32,
                    x: p3.x as f32,
                    y: p3.y as f32,
                });
            }
            kurbo::PathEl::ClosePath => {
                ops.push(PathOp::Close);
            }
        }
    }
    Some(ops)
}

