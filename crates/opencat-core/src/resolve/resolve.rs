use anyhow::{Result, ensure};

use crate::{
    FrameCtx, Node,
    frame_ctx::{ScriptFrameCtx, frames_to_duration_secs},
    ir::asset_id::{AssetId, ResourceKind, asset_id_for_video_url},
    ir::draw_types::ImageRef,
    media::VideoFrameTiming,
    parse::{
        node::NodeKind,
        primitives::{
            Canvas, Div, Image, Lottie, Path, Svg, Text, Video, VideoSource,
        },
        time::TimelineNode,
        time::{FrameState, TimelineOrigin, frame_state_for_root_at},
    },
    probe::catalog::{PreparedResourceCatalog, VideoInfoMeta},
    resolve::{
        path_bounds,
        style::{ComputedLayoutStyle, ComputedStyle, ComputedVisualStyle, InheritedStyle},
        tree::{
            ElementBitmap, ElementCanvas, ElementDiv, ElementDrawSlot, ElementId, ElementKind,
            ElementLottie, ElementNode, ElementSvg, ElementSvgPath, ElementText,
            ElementTimeline,
            ElementTimelineTransition,
        },
    },
    script::{ScriptHost, ScriptTargetRegistry, StyleMutations, TextUnitOverrideBatch},
    semantic::fingerprint::compute_element_input_fingerprints,
    style::LengthPercentageAuto,
    style::{NodeStyle, resolve_text_style},
};

use crate::ir::draw_op::DrawOp;

use std::sync::Arc;

use crate::css::VarLink;

#[derive(Default)]
struct ElementIdAllocator {
    next: u64,
}

impl ElementIdAllocator {
    fn alloc(&mut self) -> ElementId {
        let id = ElementId(self.next);
        self.next += 1;
        id
    }
}

struct ResolveContext<'a> {
    frame_ctx: &'a FrameCtx,
    script_frame_ctx: &'a ScriptFrameCtx,
    ids: &'a mut ElementIdAllocator,
    inherited_style: &'a InheritedStyle,
    assets: &'a mut PreparedResourceCatalog,

    script_runtime: &'a mut dyn ScriptHost,
    mutation_stack: &'a mut Vec<StyleMutations>,
}

pub fn resolve_ui_tree(
    node: &Node,
    frame_ctx: &FrameCtx,
    assets: &mut PreparedResourceCatalog,
    mutations: Option<&StyleMutations>,
    script_runtime: &mut dyn ScriptHost,
) -> Result<ElementNode> {
    let script_frame_ctx = ScriptFrameCtx::global(frame_ctx);
    resolve_ui_tree_with_script_cache(
        node,
        frame_ctx,
        &script_frame_ctx,
        assets,
        mutations,
        script_runtime,
    )
}

pub fn resolve_ui_tree_with_script_cache(
    node: &Node,
    frame_ctx: &FrameCtx,
    script_frame_ctx: &ScriptFrameCtx,
    assets: &mut PreparedResourceCatalog,
    mutations: Option<&StyleMutations>,
    script_runtime: &mut dyn ScriptHost,
) -> Result<ElementNode> {
    let mut ids = ElementIdAllocator::default();
    let inherited_style = InheritedStyle::default();
    let mut mutation_stack = Vec::new();
    if let Some(mutations) = mutations.filter(|mutations| !mutations.is_empty()) {
        mutation_stack.push(mutations.clone());
    }
    let mut registry = ScriptTargetRegistry::default();
    #[cfg(feature = "profile")]
    let _collect_span = tracing::span!(
        target: "render.scene",
        tracing::Level::TRACE,
        "collect_script_targets"
    )
    .entered();
    collect_visual_script_targets(node, &mut registry);
    #[cfg(feature = "profile")]
    drop(_collect_span);
    script_runtime.set_target_registry(registry);
    script_runtime.clear_text_sources();
    let mut cx = ResolveContext {
        frame_ctx,
        script_frame_ctx,
        ids: &mut ids,
        inherited_style: &inherited_style,
        assets,

        script_runtime,
        mutation_stack: &mut mutation_stack,
    };
    // resolve_build includes script_run_frame (push_script_scope runs inside);
    // subtract script_run_frame for the pure tree-construction cost.
    #[cfg(feature = "profile")]
    let _build_span =
        tracing::span!(target: "render.scene", tracing::Level::TRACE, "resolve_build").entered();
    let mut root = resolve_node_optional(node, &mut cx)?.unwrap_or_else(|| empty_root_div(&mut cx));
    #[cfg(feature = "profile")]
    drop(_build_span);
    #[cfg(feature = "profile")]
    let _fp_span =
        tracing::span!(target: "render.scene", tracing::Level::TRACE, "element_fingerprints")
            .entered();
    compute_element_input_fingerprints(&mut root);
    #[cfg(feature = "profile")]
    drop(_fp_span);
    Ok(root)
}

fn resolve_node(node: &Node, cx: &mut ResolveContext<'_>) -> Result<ElementNode> {
    // HTML 可见窗（docs/html-align/DESIGN.md）：`[start, start+duration)`
    // 半开窗外 = 整子树退出布局与绘制（空占位盒）。窗口盖住合成末尾的
    // 拥有「终止瞬间」（hyperframes 语义），末帧不隐藏。
    if clip_window_hides(node.style_ref(), cx.frame_ctx) || script_display_none(node.style_ref(), cx) {
        return Ok(hidden_clip_placeholder(cx));
    }
    match node.kind() {
        NodeKind::Video(video) => resolve_video(video, cx),
        NodeKind::Image(image) => resolve_image(image, cx),
        NodeKind::Lottie(lottie) => resolve_lottie(lottie, cx),
        NodeKind::Div(div) => resolve_div(div, cx),
        NodeKind::Canvas(canvas) => resolve_canvas(canvas, cx),
        NodeKind::Text(text) => resolve_text(text, cx),
        NodeKind::Path(path) => resolve_path(path, cx),
        NodeKind::Svg(svg) => resolve_svg(svg, cx),
        NodeKind::Timeline(timeline) => resolve_timeline(timeline, cx),
    }
}

/// 脚本写的 `display:none`（gsap `set(el, {display:"none"})`）：栈顶优先。
fn script_display_none(style: &NodeStyle, cx: &ResolveContext<'_>) -> bool {
    cx.mutation_stack
        .iter()
        .rev()
        .find_map(|m| m.mutations.get(&style.id).and_then(|n| n.display_none))
        .unwrap_or(false)
}

/// 脚本显式解锁（gsap `set(el, {display:"block"})`）：浏览器里内联
/// `display:block` 覆盖样式表的 `display:none`；静态级联烤死的 taffy
/// `None` 须同步翻回，否则元素永远无盒（texture 的 domain-warp canvas
/// `set(display:block)` 后整段流体不可见）。store 只留布尔，恢复值按
/// 块处理（`display:flex`/`inline` 的显式恢复挂账）。
fn script_display_unlocked(style: &NodeStyle, cx: &ResolveContext<'_>) -> bool {
    cx.mutation_stack
        .iter()
        .rev()
        .find_map(|m| m.mutations.get(&style.id).and_then(|n| n.display_none))
        == Some(false)
}

/// HTML 可见窗判定：`t = frame/fps`（合成全局秒）落在半开窗外即隐藏。
/// 窗终点在合成末尾 ± 半帧内视为「终止瞬间持有」，永不隐藏。
///
/// 同步布局探针的测量遍历不剪窗：hyperframes 运行时用 `visibility: hidden`
/// 藏窗外 clip，元素照常参与布局、有盒。窗外退出布局是绘制期的事，漏进
/// 几何查询会让 gsap 在窗外读不到声明变换折成的矩阵——它据此识别的
/// `xPercent`（`translate(-50%, -50%)` 居中）就此丢失，持久文档里再不恢复。
fn clip_window_hides(style: &NodeStyle, frame_ctx: &FrameCtx) -> bool {
    let Some(start) = style.clip_start else {
        return false;
    };
    if crate::script::layout_probe::measuring() {
        return false;
    }
    let fps = frame_ctx.fps.max(1) as f64;
    let t = frame_ctx.frame as f64 / fps;
    if t < start {
        return true;
    }
    let Some(duration) = style.clip_duration else {
        return false;
    };
    let end = start + duration;
    let total = crate::time::frames_to_duration_secs(frame_ctx.frames, frame_ctx.fps) as f64;
    if end >= total - 0.5 / fps {
        return false;
    }
    t >= end - 1e-9
}

/// 窗外占位盒：display:none 语义的最小 Div（不进布局流、无子树）。
fn hidden_clip_placeholder(cx: &mut ResolveContext<'_>) -> ElementNode {
    let style = NodeStyle {
        id: "__clip_hidden".to_string(),
        // 走 CSS 直通腿：legacy 腿的 Div 默认 100%×100% 流内盒，会把后续
        // 同级流内盒整体挤出父盒（overflow 裁掉 = 视频消失）。
        css_layout: Some(crate::style::CssLayout(taffy::prelude::Style {
            display: taffy::prelude::Display::None,
            ..Default::default()
        })),
        ..Default::default()
    };
    ElementNode {
        id: cx.ids.alloc(),
        kind: ElementKind::Div(ElementDiv),
        style: compute_style(&style, cx.inherited_style, script_display_unlocked(&style, cx)),
        children: Vec::new(),
        draw_slot: ElementDrawSlot::default(),
        fingerprints: Default::default(),
    }
}

fn resolve_frame_state_as_children(
    frame_state: &FrameState,
    cx: &mut ResolveContext<'_>,
) -> Result<Vec<ElementNode>> {
    match frame_state {
        FrameState::Scene {
            scene,
            script_frame_ctx,
        } => {
            let child = resolve_with_script_frame_ctx(scene, script_frame_ctx, cx)?;
            Ok(vec![timeline_fill_wrapper(child, cx.ids.alloc())])
        }
        FrameState::Transition { from, to, .. } => {
            let (from_script_frame_ctx, to_script_frame_ctx) = match frame_state {
                FrameState::Transition {
                    from_script_frame_ctx,
                    to_script_frame_ctx,
                    ..
                } => (from_script_frame_ctx, to_script_frame_ctx),
                _ => unreachable!(),
            };
            let from_el = resolve_with_script_frame_ctx(from, from_script_frame_ctx, cx)?;
            let to_el = resolve_with_script_frame_ctx(to, to_script_frame_ctx, cx)?;
            Ok(vec![
                timeline_fill_wrapper(from_el, cx.ids.alloc()),
                timeline_fill_wrapper(to_el, cx.ids.alloc()),
            ])
        }
    }
}

fn resolve_timeline(timeline: &TimelineNode, cx: &mut ResolveContext<'_>) -> Result<ElementNode> {
    let pushed = push_script_scope_for_visible_subtree(timeline, timeline.style_ref(), cx)?;
    let result = (|| {
        let mut style = timeline.style_ref().clone();
        let vars = apply_style_layers(&mut style, cx.inherited_style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style, script_display_unlocked(&style, cx));
        let inherited_style = InheritedStyle::for_child(&computed).with_vars(vars);
        // A timeline's segments are placed on its OWN clock. A top-level
        // timeline starts at composition zero; one nested inside a scene starts
        // where that scene does, so the scenes it owns run their clocks from
        // their own segment starts — `ctx.current.time` stays the component's
        // own lifetime at any depth. The enclosing scene's clock is exactly
        // that origin, so read it off `script_frame_ctx` rather than threading
        // a separate value through the resolver.
        let origin = TimelineOrigin::of_scene(cx.script_frame_ctx);
        let frame_state =
            frame_state_for_root_at(&Node::from(timeline.clone()), cx.frame_ctx, origin);
        let transition = match &frame_state {
            FrameState::Transition { progress, kind, .. } => Some(ElementTimelineTransition {
                progress: *progress,
                kind: kind.clone(),
            }),
            FrameState::Scene { .. } => None,
        };
        let mut child_cx = ResolveContext {
            frame_ctx: cx.frame_ctx,
            script_frame_ctx: cx.script_frame_ctx,
            ids: &mut *cx.ids,
            inherited_style: &inherited_style,
            assets: &mut *cx.assets,

            script_runtime: &mut *cx.script_runtime,
            mutation_stack: &mut *cx.mutation_stack,
        };
        let children = resolve_frame_state_as_children(&frame_state, &mut child_cx)?;

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::Timeline(ElementTimeline { transition }),
            style: computed.clone(),
            children,
            draw_slot: draw_slot_for(&style.id, cx.mutation_stack),
            fingerprints: Default::default(),
        })
    })();
    if pushed {
        cx.mutation_stack.pop();
    }
    result
}

fn resolve_with_script_frame_ctx(
    node: &Node,
    script_frame_ctx: &ScriptFrameCtx,
    cx: &mut ResolveContext<'_>,
) -> Result<ElementNode> {
    let mut child_cx = ResolveContext {
        frame_ctx: cx.frame_ctx,
        script_frame_ctx,
        ids: &mut *cx.ids,
        inherited_style: cx.inherited_style,
        assets: &mut *cx.assets,

        script_runtime: &mut *cx.script_runtime,
        mutation_stack: &mut *cx.mutation_stack,
    };
    resolve_node_optional(node, &mut child_cx)
        .map(|opt| opt.unwrap_or_else(|| empty_root_div(&mut child_cx)))
}

fn timeline_fill_wrapper(child: ElementNode, id: ElementId) -> ElementNode {
    let mut style = child.style.clone();
    style.id = format!("{}::__timeline_fill", style.id);
    style.layout.position = crate::parse::primitives::Position::Absolute;
    style.layout.inset_left = Some(LengthPercentageAuto::Length(0.0));
    style.layout.inset_top = Some(LengthPercentageAuto::Length(0.0));
    style.layout.inset_right = Some(LengthPercentageAuto::Length(0.0));
    style.layout.inset_bottom = Some(LengthPercentageAuto::Length(0.0));
    style.layout.width = None;
    style.layout.height = None;
    style.layout.width_percent = Some(1.0);
    style.layout.height_percent = Some(1.0);
    style.layout.margin_top = LengthPercentageAuto::Length(0.0);
    style.layout.margin_right = LengthPercentageAuto::Length(0.0);
    style.layout.margin_bottom = LengthPercentageAuto::Length(0.0);
    style.layout.margin_left = LengthPercentageAuto::Length(0.0);
    style.visual.background.clear();
    style.visual.border_width = None;
    style.visual.border_top_width = None;
    style.visual.border_right_width = None;
    style.visual.border_bottom_width = None;
    style.visual.border_left_width = None;
    style.visual.border_color = None;
    style.visual.border_style = None;
    style.visual.box_shadow.clear();
    style.visual.inset_shadow.clear();
    style.visual.drop_shadow.clear();
    style.visual.css_filter = Default::default();
    style.visual.backdrop_blur_sigma = None;
    style.visual.backdrop_filter = Default::default();
    style.visual.border_radius = crate::style::BorderRadius::default();
    style.visual.clip_contents = false;
    style.visual.clip_x_only = false;
    style.visual.clip_path = None;
    style.visual.transforms.clear();
    style.visual.opacity = 1.0;
    // 包装层只承载子场景，不是行内流容器（否则子场景被当原子盒做基线对齐）。
    style.layout.inline_flow = false;

    ElementNode {
        id,
        kind: ElementKind::Div(ElementDiv),
        style,
        children: vec![child],
        draw_slot: ElementDrawSlot::default(),
        fingerprints: Default::default(),
    }
}

fn resolve_node_optional(node: &Node, cx: &mut ResolveContext<'_>) -> Result<Option<ElementNode>> {
    resolve_node(node, cx).map(Some)
}

fn empty_root_div(cx: &mut ResolveContext<'_>) -> ElementNode {
    let style = NodeStyle {
        id: "__empty_root".to_string(),
        ..Default::default()
    };
    ElementNode {
        id: cx.ids.alloc(),
        kind: ElementKind::Div(ElementDiv),
        style: compute_style(&style, cx.inherited_style, script_display_unlocked(&style, cx)),
        children: Vec::new(),
        draw_slot: ElementDrawSlot::default(),
        fingerprints: Default::default(),
    }
}

fn timing_for_script_frame_ctx(
    timing: VideoFrameTiming,
    script_frame_ctx: &ScriptFrameCtx,
) -> VideoFrameTiming {
    let scene_start_frame = script_frame_ctx
        .frame
        .saturating_sub(script_frame_ctx.current_frame);
    let scene_start_secs = frames_to_duration_secs(scene_start_frame, script_frame_ctx.fps);
    VideoFrameTiming {
        timeline_start_secs: timing.timeline_start_secs + scene_start_secs,
        ..timing
    }
}

fn resolve_div(div: &Div, cx: &mut ResolveContext<'_>) -> Result<ElementNode> {
    let pushed = push_script_scope_for_visible_subtree(div, div.style_ref(), cx)?;
    let result = (|| {
        let mut style = div.style_ref().clone();
        ensure!(
            !style.id.is_empty(),
            "node id is required for div nodes before rendering"
        );
        let vars = apply_style_layers(&mut style, cx.inherited_style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style, script_display_unlocked(&style, cx));
        let inherited_style = InheritedStyle::for_child(&computed).with_vars(vars);
        let mut children = Vec::new();
        for child in div.children_ref() {
            let mut child_cx = ResolveContext {
                frame_ctx: cx.frame_ctx,
                script_frame_ctx: cx.script_frame_ctx,
                ids: &mut *cx.ids,
                inherited_style: &inherited_style,
                assets: &mut *cx.assets,

                script_runtime: &mut *cx.script_runtime,
                mutation_stack: &mut *cx.mutation_stack,
            };
            if let Some(child) = resolve_node_optional(child, &mut child_cx)? {
                children.push(child);
            }
        }

        // 渲染期 textContent 写入落在 `<id>:t`（html_api.js 的 setter）。种子里
        // 没有这个文本子节点时（提取期从未写过文本），按浏览器语义现场新建，
        // 继承本元素的文本样式；写入替换全部子节点——种子里声明的文本 run
        // （解析期合成的 P）不能留着与新文本叠印。
        let slot_id = format!("{}:t", style.id);
        if !div.children_ref().iter().any(|child| child.style_ref().id == slot_id)
            && let Some(content) = text_content_from_stack(cx.mutation_stack, &slot_id)
        {
            children.clear();
            // 写入替换全部子节点：新文本是本元素唯一的行，首尾空白不占位。
            let content = content
                .trim_matches(|c: char| c.is_whitespace() && c != '\u{a0}')
                .to_string();
            if !content.is_empty() {
                let slot_style = NodeStyle { id: slot_id.clone(), ..NodeStyle::default() };
                let slot_computed = compute_style(&slot_style, &inherited_style, script_display_unlocked(&slot_style, cx));
                let text_unit_overrides = script_split_overrides(cx, &slot_id, &content);
                children.push(ElementNode {
                    id: cx.ids.alloc(),
                    kind: ElementKind::Text(ElementText {
                        text: content,
                        text_style: slot_computed.text.clone(),
                        text_unit_overrides,
                    }),
                    style: slot_computed,
                    children: Vec::new(),
                    draw_slot: draw_slot_for(&slot_id, cx.mutation_stack),
                    fingerprints: Default::default(),
                });
            }
        }

        // 块容器里的行内级子盒（inline-block 等）：taffy 没有行内格式化，块
        // 布局会把它们拉成父宽。按浏览器语义改成 shrink-to-fit，水平位置由
        // 本元素的 text-align 决定（auto 边距）。
        if !computed.layout.inline_flow
            && computed
                .css_layout
                .as_ref()
                .is_some_and(|css| css.0.display == taffy::prelude::Display::Block)
        {
            for child in &mut children {
                shrink_inline_level_child(child, computed.text.text_align);
            }
        }
        // 行内流容器的子盒：taffy 按单行 flex row 排它们，布局端行内 pass
        // 再折行重排。子盒须保持 shrink-to-fit 尺寸——flex 收缩会把原子盒与
        // 文本 run 压窄、各自内部折行；原子盒宽度上限为容器内容宽。
        if computed.layout.inline_flow {
            for child in &mut children {
                size_inline_flow_child(child);
            }
        }

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::Div(ElementDiv),
            style: computed.clone(),
            children,
            draw_slot: draw_slot_for(&style.id, cx.mutation_stack),
            fingerprints: Default::default(),
        })
    })();
    if pushed {
        cx.mutation_stack.pop();
    }
    result
}

fn size_inline_flow_child(child: &mut ElementNode) {
    let is_text = matches!(child.kind, ElementKind::Text(_));
    let Some(css) = child.style.css_layout.as_mut() else {
        return;
    };
    let t = &mut css.0;
    if t.position == taffy::prelude::Position::Absolute {
        return;
    }
    t.flex_shrink = 0.0;
    if !is_text && t.size.width.is_auto() && t.max_size.width.is_auto() {
        t.max_size.width = taffy::prelude::LengthPercentageAuto::percent(1.0);
    }
}

fn shrink_inline_level_child(child: &mut ElementNode, align: crate::style::TextAlign) {
    use taffy::prelude::{Dimension, LengthPercentageAuto as TaffyLPA};
    if !child.style.layout.inline_level {
        return;
    }
    let Some(css) = child.style.css_layout.as_mut() else {
        return;
    };
    let t = &mut css.0;
    if t.position == taffy::prelude::Position::Absolute {
        return;
    }
    if t.size.width.is_auto() {
        t.size.width = Dimension::fit_content();
    }
    let zero = TaffyLPA::length(0.0);
    if t.margin.left == zero && t.margin.right == zero {
        match align {
            crate::style::TextAlign::Center => {
                t.margin.left = TaffyLPA::auto();
                t.margin.right = TaffyLPA::auto();
            }
            crate::style::TextAlign::Right => t.margin.left = TaffyLPA::auto(),
            crate::style::TextAlign::Left => {}
        }
    }
}

fn resolve_text(text: &Text, cx: &mut ResolveContext<'_>) -> Result<ElementNode> {
    let pushed = push_script_scope_for_visible_subtree(text, text.style_ref(), cx)?;
    let result = (|| {
        let mut style = text.style_ref().clone();
        ensure!(
            !style.id.is_empty(),
            "node id is required for text nodes before rendering"
        );
        apply_style_layers(&mut style, cx.inherited_style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style, script_display_unlocked(&style, cx));

        // 脚本写入的文本与 DOM 构建期同一空白语义：行内 run 之外按行首行尾
        // 裁掉可折叠空白（`&nbsp;` 不可折叠）。
        let content = match text_content_from_stack(cx.mutation_stack, &style.id) {
            Some(written) if !style.inline_run => written
                .trim_matches(|c: char| c.is_whitespace() && c != '\u{a0}')
                .to_string(),
            Some(written) => written,
            None => text.content().to_string(),
        };

        let text_unit_overrides = script_split_overrides(cx, &style.id, &content);

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::Text(ElementText {
                text: content,
                text_style: computed.text.clone(),
                text_unit_overrides,
            }),
            style: computed,
            children: Vec::new(),
            draw_slot: draw_slot_for(&style.id, cx.mutation_stack),
            fingerprints: Default::default(),
        })
    })();
    if pushed {
        cx.mutation_stack.pop();
    }
    result
}

fn resolve_canvas(canvas: &Canvas, cx: &mut ResolveContext<'_>) -> Result<ElementNode> {
    let pushed = push_script_scope_for_visible_subtree(canvas, canvas.style_ref(), cx)?;
    let result = (|| {
        let mut style = canvas.style_ref().clone();
        ensure!(
            !style.id.is_empty(),
            "node id is required for canvas nodes before rendering"
        );
        let vars = apply_style_layers(&mut style, cx.inherited_style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style, script_display_unlocked(&style, cx));

        for asset in canvas.assets_ref() {
            let target = cx.assets.resolve_image(&asset.source)?;
            cx.assets.alias(
                AssetId::new(ResourceKind::Image, asset.asset_id.clone()),
                &target,
            )?;
        }

        let mut commands = Vec::new();
        apply_canvas_mutation_stack(&mut commands, cx.mutation_stack, &style.id);

        // Canvas draw-image ops carry the script-facing alias as their
        // `ImageRef::Static` asset_id. Resolve every alias to its canonical
        // `AssetId` here so `DrawOpFrame` / `FrameMediaPlan` never expose an
        // alias. Unknown references (neither a registered alias nor a known
        // canonical asset) are render errors.
        canonicalize_canvas_image_refs(&mut commands, cx.assets)?;

        let child_inherited_style = InheritedStyle::for_child(&computed).with_vars(vars);
        let children = resolve_hidden_children(canvas, cx, &child_inherited_style)?;

        // g-svg 渲染器的图表不产 DrawOp，产一棵 `<svg>` 子树：整块当作
        // 内联 SVG 渲染（usvg 管线，与 `<svg inline>` 同一条路），尺寸取
        // canvas 自己的盒子。
        let svg_markup = cx
            .mutation_stack
            .iter()
            .rev()
            .find_map(|m| m.svg_markup_for(&style.id))
            .map(str::to_string);
        let kind = match svg_markup {
            Some(markup) => ElementKind::Svg(ElementSvg {
                src: std::path::PathBuf::new(),
                inline: Some(markup),
                width: None,
                height: None,
                preserve_aspect_ratio: None,
                intrinsic_size: None,
            }),
            None => ElementKind::Canvas(ElementCanvas { commands, bitmap: canvas.bitmap }),
        };

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind,
            style: computed.clone(),
            children,
            // Canvas commands are carried by `ElementCanvas.commands` (the
            // node's DrawScript display item). `draw_slot_for` resolves the
            // exact same mutation-stack commands a second time, and the
            // renderer paints both the item (dispatch::render_display_item)
            // and the slot (dispatch: draw_slot branch), so filling the slot
            // here drew every canvas op twice per frame (alpha composed
            // twice: a_eff = 1-(1-a)^2). The slot is only a carrier for
            // non-canvas elements, which have no DrawScript item of their
            // own.
            draw_slot: ElementDrawSlot::default(),
            fingerprints: Default::default(),
        })
    })();
    if pushed {
        cx.mutation_stack.pop();
    }
    result
}

fn resolve_hidden_children(
    canvas: &Canvas,
    cx: &mut ResolveContext<'_>,
    inherited_style: &InheritedStyle,
) -> Result<Vec<ElementNode>> {
    let hidden = canvas.hidden_children_ref();
    if hidden.is_empty() {
        return Ok(Vec::new());
    }
    let mut children = Vec::with_capacity(hidden.len());
    for child in hidden {
        let mut child_cx = ResolveContext {
            frame_ctx: cx.frame_ctx,
            script_frame_ctx: cx.script_frame_ctx,
            ids: &mut *cx.ids,
            inherited_style,
            assets: &mut *cx.assets,

            script_runtime: &mut *cx.script_runtime,
            mutation_stack: &mut *cx.mutation_stack,
        };
        if let Some(child_node) = resolve_node_optional(child, &mut child_cx)? {
            children.push(child_node);
        }
    }
    Ok(children)
}

fn resolve_video(video: &Video, cx: &mut ResolveContext<'_>) -> Result<ElementNode> {
    let pushed = push_script_scope_for_visible_subtree(video, video.style_ref(), cx)?;
    let result = (|| {
        let mut style = video.style_ref().clone();
        ensure!(
            !style.id.is_empty(),
            "node id is required for video nodes before rendering"
        );
        let vars = apply_style_layers(&mut style, cx.inherited_style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style, script_display_unlocked(&style, cx));
        let inherited_style = InheritedStyle::for_child(&computed).with_vars(vars);

        let locator = match video.source() {
            VideoSource::Path(p) => format!("video:path:{p}"),
            VideoSource::Url(u) => asset_id_for_video_url(u).key,
        };

        let asset_id = cx.assets.register_video_dimensions(&locator, 0, 0, None);
        // Author-asserted YUV→RGB matrix (data-color-matrix): record it on the
        // asset so the frame media plan can route the host decoder's
        // conversion to the asset's real encoding.
        if let Some(matrix) = video.timing().color_matrix {
            cx.assets.set_video_color_matrix(&asset_id, matrix);
        }
        let info = cx.assets.video_info(&asset_id).unwrap_or(VideoInfoMeta {
            width: 0,
            height: 0,
            duration_micros: None,
            color_matrix: None,
        });
        let asset_id = cx.assets.register_video_dimensions(
            &locator,
            info.width,
            info.height,
            info.duration_secs(),
        );

        let mut children = Vec::new();
        for child in video.children_ref() {
            let mut child_cx = ResolveContext {
                frame_ctx: cx.frame_ctx,
                script_frame_ctx: cx.script_frame_ctx,
                ids: &mut *cx.ids,
                inherited_style: &inherited_style,
                assets: &mut *cx.assets,

                script_runtime: &mut *cx.script_runtime,
                mutation_stack: &mut *cx.mutation_stack,
            };
            if let Some(child) = resolve_node_optional(child, &mut child_cx)? {
                children.push(child);
            }
        }

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::Bitmap(ElementBitmap {
                asset_id,
                width: info.width,
                height: info.height,
                video_timing: Some(timing_for_script_frame_ctx(
                    video.timing(),
                    cx.script_frame_ctx,
                )),
            }),
            style: computed.clone(),
            children,
            draw_slot: draw_slot_for(&style.id, cx.mutation_stack),
            fingerprints: Default::default(),
        })
    })();
    if pushed {
        cx.mutation_stack.pop();
    }
    result
}

fn resolve_image(image: &Image, cx: &mut ResolveContext<'_>) -> Result<ElementNode> {
    let pushed = push_script_scope_for_visible_subtree(image, image.style_ref(), cx)?;
    let result = (|| {
        let mut style = image.style_ref().clone();
        ensure!(
            !style.id.is_empty(),
            "node id is required for image nodes before rendering"
        );
        apply_style_layers(&mut style, cx.inherited_style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style, script_display_unlocked(&style, cx));

        let asset_id = cx.assets.resolve_image(image.source())?;
        let (width, height) = cx.assets.dimensions(&asset_id);

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::Bitmap(ElementBitmap {
                asset_id,
                width,
                height,
                video_timing: None,
            }),
            style: computed.clone(),
            children: Vec::new(),
            draw_slot: draw_slot_for(&style.id, cx.mutation_stack),
            fingerprints: Default::default(),
        })
    })();
    if pushed {
        cx.mutation_stack.pop();
    }
    result
}

fn resolve_lottie(lottie: &Lottie, cx: &mut ResolveContext<'_>) -> Result<ElementNode> {
    let pushed = push_script_scope_for_visible_subtree(lottie, lottie.style_ref(), cx)?;
    let result = (|| {
        let mut style = lottie.style_ref().clone();
        ensure!(
            !style.id.is_empty(),
            "node id is required for lottie nodes before rendering"
        );
        apply_style_layers(&mut style, cx.inherited_style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style, script_display_unlocked(&style, cx));

        // Bundle identity is source-based: same locator → same bundle.
        let bundle_id = cx.assets.resolve_lottie(lottie.source())?;
        let meta = cx
            .assets
            .lottie_meta(&bundle_id)
            .unwrap_or(crate::lottie::LottieMeta {
                width: 100,
                height: 100,
                fps: 30.0,
                in_frame: 0.0,
                out_frame: 30.0,
                dependencies: vec![],
            });

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::Lottie(ElementLottie {
                bundle_id,
                width: meta.width,
                height: meta.height,
                fps: meta.fps,
                in_frame: meta.in_frame,
                out_frame: meta.out_frame,
                timing: timing_for_script_frame_ctx(lottie.timing(), cx.script_frame_ctx),
            }),
            style: computed.clone(),
            children: Vec::new(),
            draw_slot: draw_slot_for(&style.id, cx.mutation_stack),
            fingerprints: Default::default(),
        })
    })();
    if pushed {
        cx.mutation_stack.pop();
    }
    result
}


fn resolve_path(path: &Path, cx: &mut ResolveContext<'_>) -> Result<ElementNode> {
    let pushed = push_script_scope_for_visible_subtree(path, path.style_ref(), cx)?;
    let result = (|| {
        let mut style = path.style_ref().clone();
        ensure!(
            !style.id.is_empty(),
            "node id is required for path nodes before rendering"
        );
        ensure!(!path.data().is_empty(), "path data must not be empty");
        apply_style_layers(&mut style, cx.inherited_style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style, script_display_unlocked(&style, cx));

        let (path_data, view_box) = if let Some(svg_path) = &computed.visual.svg_path {
            let pd = vec![svg_path.clone()];
            let vb = match path.view_box() {
                Some(explicit) => explicit,
                None => path_bounds::compute_view_box(&pd).unwrap_or_else(|_| {
                    path_bounds::compute_view_box(&[path.data().to_string()])
                        .unwrap_or(path_bounds::EMPTY_PATH_VIEW_BOX)
                }),
            };
            (pd, vb)
        } else {
            let pd = vec![path.data().to_string()];
            let vb = match path.view_box() {
                Some(explicit) => explicit,
                None => path_bounds::compute_view_box(&pd)?,
            };
            (pd, vb)
        };

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::SvgPath(ElementSvgPath {
                path_data,
                view_box,
                intrinsic_size: None,
            }),
            style: computed.clone(),
            children: Vec::new(),
            draw_slot: draw_slot_for(&style.id, cx.mutation_stack),
            fingerprints: Default::default(),
        })
    })();
    if pushed {
        cx.mutation_stack.pop();
    }
    result
}

fn resolve_svg(svg: &Svg, cx: &mut ResolveContext<'_>) -> Result<ElementNode> {
    let pushed = push_script_scope_for_visible_subtree(svg, svg.style_ref(), cx)?;
    let result = (|| {
        let mut style = svg.style_ref().clone();
        ensure!(
            !style.id.is_empty(),
            "node id is required for svg nodes before rendering"
        );
        apply_style_layers(&mut style, cx.inherited_style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style, script_display_unlocked(&style, cx));
        // 脚本改过内部元素属性的 html inline svg：本帧重序列化的 markup
        // 覆盖提取期快照（html_api.js svgWriteAttr → `svg_set_markup`）。
        let markup_override = cx
            .mutation_stack
            .iter()
            .rev()
            .find_map(|m| m.svg_markup_for(&style.id));
        let inline = markup_override
            .or(svg.inline())
            .map(|markup| {
                // 内部元素绘制注入在覆盖落地之后：脚本改写的 markup 由 JS
                // 侧重序列化、不带页面层叠，注入补上 CSS 瞄准的绘制声明；
                // 内联/脚本已写的属性注入时跳过（层叠序正确）。
                let markup = match svg.inner_paint.as_ref() {
                    Some(ip) => crate::parse::html::inject_markup_paint(
                        markup,
                        &ip.sheet,
                        &ip.host_ancestors,
                    ),
                    None => markup.to_string(),
                };
                inherit_svg_paint(&markup, style.svg_paint_css.as_deref(), &computed)
            });
        let intrinsic_size = match &inline {
            Some(markup) => crate::render::svg::load_tree_markup(markup)
                .map(|t| crate::render::svg::tree_size(&t)),
            None => {
                crate::render::svg::load_tree(svg.src()).map(|t| crate::render::svg::tree_size(&t))
            }
        };

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::Svg(ElementSvg {
                src: svg.src().clone(),
                inline,
                width: svg.width(),
                height: svg.height(),
                preserve_aspect_ratio: svg.preserve_aspect_ratio().map(String::from),
                intrinsic_size,
            }),
            style: computed,
            children: Vec::new(),
            draw_slot: draw_slot_for(&style.id, cx.mutation_stack),
            fingerprints: Default::default(),
        })
    })();
    if pushed {
        cx.mutation_stack.pop();
    }
    result
}

/// 页面 CSS 落在内联 `<svg>` 元素上的绘制属性（fill/stroke 系）与 `color`
/// 注入根元素 style：markup 由 DOM 原样序列化、不带页面层叠，usvg 只按
/// svg 内部继承下发——`.row svg{fill:none;stroke:currentColor}` 这类图标规则
/// 不注入就画成黑色实心。已有 style 属性时追加在后（同属性后者胜出）。
fn inherit_svg_paint(markup: &str, paint_css: Option<&str>, computed: &ComputedStyle) -> String {
    let mut decls = String::new();
    let root_tag_end = markup.find('>').unwrap_or(markup.len());
    // 根上自带 `color` 属性（JSX 图标垫片）时尊重它，不用层叠色覆盖。
    let has_color_attr = markup[..root_tag_end].contains(" color=");
    let uses_current = !has_color_attr
        && (markup.contains("currentColor")
            || markup.contains("currentcolor")
            || paint_css.is_some_and(|css| css.to_ascii_lowercase().contains("currentcolor")));
    if uses_current {
        let (r, g, b, a) = computed.text.color.rgba();
        decls.push_str(&format!("color:rgba({r},{g},{b},{});", a as f32 / 255.0));
    }
    if let Some(css) = paint_css {
        decls.push_str(css);
    }
    if decls.is_empty() {
        return markup.to_string();
    }
    let decls = decls.replace('"', "'");
    let Some(start) = markup.find("<svg") else {
        return markup.to_string();
    };
    // 根起始标签的结束位置：引号外的第一个 `>`。
    let mut quote: Option<u8> = None;
    let mut end = None;
    for (i, &b) in markup.as_bytes().iter().enumerate().skip(start + 4) {
        match quote {
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None if b == b'"' || b == b'\'' => quote = Some(b),
            None if b == b'>' => {
                end = Some(i);
                break;
            }
            None => {}
        }
    }
    let Some(end) = end else {
        return markup.to_string();
    };
    let tag = &markup[start..end];
    // 已有 style 属性：值末尾追加。
    if let Some(attr) = [" style=\"", " style='", "\tstyle=\"", "\nstyle=\""]
        .iter()
        .find_map(|needle| tag.find(needle).map(|at| at + needle.len()))
    {
        let value_start = start + attr;
        let close = markup.as_bytes()[value_start - 1];
        if let Some(len) = markup[value_start..end].find(close as char) {
            let value_end = value_start + len;
            return format!(
                "{};{}{}",
                &markup[..value_end],
                decls,
                &markup[value_end..]
            );
        }
    }
    let insert_at = if markup[..end].ends_with('/') { end - 1 } else { end };
    format!("{} style=\"{}\"{}", &markup[..insert_at], decls, &markup[insert_at..])
}

fn push_script_scope(style: &NodeStyle, cx: &mut ResolveContext<'_>) -> Result<bool> {
    let Some(driver) = style.script_driver.as_deref() else {
        return Ok(false);
    };

    let id = cx.script_runtime.install(&driver.source)?;
    let mut store = crate::script::recorder::MutationStore::default();
    #[cfg(feature = "profile")]
    let _run_frame_span =
        tracing::span!(target: "render.scene", tracing::Level::TRACE, "script_run_frame").entered();
    cx.script_runtime.run_frame(
        id,
        cx.script_frame_ctx,
        (!style.id.is_empty()).then_some(style.id.as_str()),
        &mut store,
    )?;
    #[cfg(feature = "profile")]
    drop(_run_frame_span);
    // Consuming take instead of snapshot: this store is created per scope
    // above and dropped right after, so mem::take is semantically identical
    // and skips the full deep clone of every canvas DrawOp per frame.
    let mutations = store.take_mutations();
    if mutations.is_empty() {
        return Ok(false);
    }

    cx.mutation_stack.push(mutations);
    Ok(true)
}

fn push_script_scope_for_visible_subtree<T>(
    node: &T,
    style: &NodeStyle,
    cx: &mut ResolveContext<'_>,
) -> Result<bool>
where
    T: Clone + Into<NodeKind>,
{
    if style.script_driver.is_none() {
        return Ok(false);
    }

    let seeded = Node::new(node.clone());
    seed_text_sources_for_visible_subtree(
        &seeded,
        cx.frame_ctx,
        cx.script_frame_ctx,
        cx.mutation_stack,
        cx.script_runtime,
    );

    // Collect base style defaults from the subtree before running scripts,
    // so the animation engine can read current values for ctx.to / ctx.from.
    seed_initial_style_defaults(&seeded, cx.script_runtime);

    // `getBoundingClientRect` is viewport-relative and scripts run before this
    // frame's layout pass: give them a synchronous-layout probe over the scope,
    // which measures the tree with the mutations written so far — the same
    // DOM state the browser lays out when a script reads geometry.
    if driver_wants_geometry(style) {
        crate::script::layout_probe::with_scope(
            &seeded,
            *cx.frame_ctx,
            *cx.script_frame_ctx,
            (*cx.assets).clone(),
            || push_script_scope(style, cx),
        )
    } else {
        push_script_scope(style, cx)
    }
}

fn driver_wants_geometry(style: &NodeStyle) -> bool {
    style
        .script_driver
        .as_deref()
        .is_some_and(|driver| crate::script::layout_probe::wants_geometry(&driver.source))
}

/// Traverse the node tree and push each node's resolved base style into the
/// script host via set_initial_style_from_node. All property extraction is
/// centralized inside MutationStore — this function just walks the tree.
fn seed_initial_style_defaults(node: &Node, script_runtime: &mut dyn ScriptHost) {
    let style = node.style_ref();
    if !style.id.is_empty() {
        script_runtime.set_initial_style_from_node(&style.id, style);
    }
    match node.kind() {
        NodeKind::Div(div) => {
            for child in div.children_ref() {
                seed_initial_style_defaults(child, script_runtime);
            }
        }
        _ => {}
    }
}

fn seed_text_sources_for_visible_subtree(
    node: &Node,
    frame_ctx: &FrameCtx,
    script_frame_ctx: &ScriptFrameCtx,
    mutation_stack: &[StyleMutations],
    script_runtime: &mut dyn ScriptHost,
) {
    match node.kind() {
        NodeKind::Div(div) => {
            for child in div.children_ref() {
                seed_text_sources_for_visible_subtree(
                    child,
                    frame_ctx,
                    script_frame_ctx,
                    mutation_stack,
                    script_runtime,
                );
            }
        }
        NodeKind::Text(text) => {
            let id = &text.style_ref().id;
            if id.is_empty() {
                return;
            }
            let content = text_content_from_stack(mutation_stack, id)
                .unwrap_or_else(|| text.content().to_string());
            script_runtime.register_text_source(
                id,
                crate::script::ScriptTextSource {
                    text: content,
                    kind: crate::script::ScriptTextSourceKind::TextNode,
                },
            );
        }
        NodeKind::Timeline(timeline) => {
            // Same origin rule as `resolve_timeline`: a nested timeline is
            // placed on the enclosing scene's clock, not the composition's.
            let origin = TimelineOrigin::of_scene(script_frame_ctx);
            let frame_state =
                frame_state_for_root_at(&Node::from(timeline.clone()), frame_ctx, origin);
            match frame_state {
                FrameState::Scene {
                    scene,
                    script_frame_ctx,
                } => seed_text_sources_for_visible_subtree(
                    &scene,
                    frame_ctx,
                    &script_frame_ctx,
                    mutation_stack,
                    script_runtime,
                ),
                FrameState::Transition {
                    from,
                    to,
                    from_script_frame_ctx,
                    to_script_frame_ctx,
                    ..
                } => {
                    seed_text_sources_for_visible_subtree(
                        &from,
                        frame_ctx,
                        &from_script_frame_ctx,
                        mutation_stack,
                        script_runtime,
                    );
                    seed_text_sources_for_visible_subtree(
                        &to,
                        frame_ctx,
                        &to_script_frame_ctx,
                        mutation_stack,
                        script_runtime,
                    );
                }
            }
        }
        NodeKind::Canvas(canvas) => {
            for child in canvas.hidden_children_ref() {
                seed_text_sources_for_visible_subtree(
                    child,
                    frame_ctx,
                    script_frame_ctx,
                    mutation_stack,
                    script_runtime,
                );
            }
        }
        NodeKind::Video(video) => {
            for child in video.children_ref() {
                seed_text_sources_for_visible_subtree(
                    child,
                    frame_ctx,
                    script_frame_ctx,
                    mutation_stack,
                    script_runtime,
                );
            }
        }
        NodeKind::Image(_) | NodeKind::Lottie(_) | NodeKind::Path(_) | NodeKind::Svg(_) => {}
    }
}

fn apply_mutation_stack(style: &mut NodeStyle, stack: &[StyleMutations]) {
    let id = style.id.clone();
    for mutations in stack {
        mutations.apply_to_node(style, &id);
    }
}

/// 样式的两层运行期改写：链上有脚本写入的自定义属性时先重放 var() 依赖
/// 声明（浏览器里变量是活的），再叠脚本直接写入的属性。返回子元素继承的
/// 变量链。
fn apply_style_layers(
    style: &mut NodeStyle,
    inherited: &InheritedStyle,
    stack: &[StyleMutations],
) -> Option<Arc<VarLink>> {
    let mut link = inherited.vars.clone();
    let deps = style.var_deps.clone();
    if let Some(deps) = deps.as_ref().filter(|deps| !deps.own.is_empty()) {
        link = Some(VarLink::push(link, deps.own.clone(), false));
    }
    let scripted = scripted_vars(stack, &style.id);
    if !scripted.is_empty() {
        link = Some(VarLink::push(link, Arc::new(scripted), true));
    }
    if let (Some(deps), Some(chain)) = (deps.as_ref(), link.as_ref())
        && chain.scripted
    {
        crate::css::replay_var_deps(style, deps, chain.as_ref());
    }
    apply_mutation_stack(style, stack);
    link
}

/// 脚本本帧写到元素上的自定义属性（`style.setProperty('--x', v)`）。
fn scripted_vars(stack: &[StyleMutations], id: &str) -> crate::css::CssVars {
    let mut vars = crate::css::CssVars::new();
    if id.is_empty() {
        return vars;
    }
    for mutations in stack {
        let Some(mutation) = mutations.get(id) else { continue };
        for (name, value) in &mutation.custom_props {
            if let Some(tokens) = crate::css::parse_custom_value(name, value) {
                vars.insert(name.trim_start_matches("--").to_string(), tokens);
            }
        }
    }
    vars
}

fn apply_canvas_mutation_stack(
    commands: &mut Vec<crate::script::canvas_op::CanvasOp>,
    stack: &[StyleMutations],
    id: &str,
) {
    for mutations in stack {
        mutations.apply_to_canvas(commands, id);
    }
}

/// Resolve `ImageRef::Static` alias references to canonical `AssetId`s in
/// place. Video-frame refs already carry canonical ids and are left untouched.
/// An alias that is neither registered nor a known canonical asset is an error.
fn canonicalize_canvas_image_refs(
    commands: &mut [crate::script::canvas_op::CanvasOp],
    assets: &PreparedResourceCatalog,
) -> Result<()> {
    for cmd in commands.iter_mut() {
        let image_ref = match cmd {
            crate::script::canvas_op::CanvasOp::Op(DrawOp::Image { image, .. })
            | crate::script::canvas_op::CanvasOp::Op(DrawOp::ImageRect { image, .. })
            | crate::script::canvas_op::CanvasOp::PutImage { image, .. } => image,
            _ => continue,
        };
        if let ImageRef::Static { asset_id } = image_ref {
            let key = AssetId::new(ResourceKind::Image, asset_id.clone());
            if let Some(canonical) = assets.resolve_alias(&key) {
                *asset_id = canonical.key;
            } else if !assets.is_known_asset(&key) {
                anyhow::bail!(
                    "canvas draw-image references unknown asset `{asset_id}`; \
                     only declared canvas assets may be referenced"
                );
            }
        }
    }
    Ok(())
}

fn draw_slot_for(style_id: &str, stack: &[StyleMutations]) -> ElementDrawSlot {
    let mut commands = Vec::new();
    apply_canvas_mutation_stack(&mut commands, stack, style_id);
    ElementDrawSlot { commands }
}

fn text_content_from_stack(stack: &[StyleMutations], id: &str) -> Option<String> {
    stack
        .iter()
        .rev()
        .find_map(|m| m.text_content_for(id).map(str::to_string))
}

/// 官方 SplitText 的拆分结果 → 渲染面的 per-unit 覆盖。
///
/// SplitText 建的是真节点树（char/word/line wrapper），gsap 把样式写在
/// wrapper 元素上。渲染面不认识节点树，只认识"整串文本 + 每个字素一条
/// 覆盖"，所以这里做最小等价换算：把每个动态叶子的字节区间映射回字素
/// 索引，沿祖先链合并 wrapper 上的样式变更。
///
/// 分组按字素切（`describe_grapheme_ranges`），与渲染面
/// `render_text_with_unit_overrides` 读 batch 的粒度一致。
fn script_split_overrides(
    cx: &mut ResolveContext<'_>,
    id: &str,
    content: &str,
) -> Option<TextUnitOverrideBatch> {
    use crate::script::{TextUnitGranularity, TextUnitOverride};
    let leaves = cx.script_runtime.text_leaf_ranges(id);
    if leaves.is_empty() {
        return None;
    }
    let graphemes = crate::text::describe_grapheme_ranges(content);
    let index_of = |byte: usize| -> Option<usize> {
        graphemes
            .iter()
            .position(|range| range.start <= byte && byte < range.end)
    };

    let mut overrides: Vec<Option<TextUnitOverride>> = vec![None; graphemes.len()];
    for (chain, start, end) in leaves {
        let Some(first) = index_of(start) else {
            continue;
        };
        let last = index_of(end.saturating_sub(1)).unwrap_or(first);
        // 叶子本身通常没有样式（它是文本节点或空壳），真正被 tween 的是
        // wrapper。按叶子 → 最外层顺序合并，后者覆盖前者，与 DOM 上
        // 嵌套元素各自 transform 的叠加顺序一致（内层先应用）。
        for wrapper in chain.iter() {
            let Some(m) = cx
                .mutation_stack
                .iter()
                .rev()
                .find_map(|layer| layer.get(wrapper))
            else {
                continue;
            };
            for slot in overrides.iter_mut().take(last + 1).skip(first) {
                let target = slot.get_or_insert_with(TextUnitOverride::default);
                if let Some(v) = m.opacity {
                    target.opacity = Some(v);
                }
                if let Some(v) = m.read_transform_value("translateX") {
                    target.translate_x = Some(v);
                }
                if let Some(v) = m.read_transform_value("translateY") {
                    target.translate_y = Some(v);
                }
                if let Some(v) = m.read_transform_value("scale") {
                    target.scale = Some(v);
                }
                if let Some(v) = m.read_transform_value("rotation") {
                    target.rotation_deg = Some(v);
                }
                if let Some(color) = m.text_color {
                    target.color = Some(color);
                }
            }
        }
    }
    if overrides.iter().all(Option::is_none) {
        return None;
    }
    Some(TextUnitOverrideBatch {
        granularity: TextUnitGranularity::Grapheme,
        overrides: overrides
            .into_iter()
            .map(Option::unwrap_or_default)
            .collect(),
    })
}

fn compute_style(
    style: &NodeStyle,
    inherited_style: &InheritedStyle,
    display_unlock: bool,
) -> ComputedStyle {
    let text = resolve_text_style(&inherited_style.text, style);
    ComputedStyle {
        layout: ComputedLayoutStyle {
            position: style.position.unwrap_or_default(),
            positioned: style.position.is_some(),
            inset_left: style.inset_left,
            inset_top: style.inset_top,
            inset_right: style.inset_right,
            inset_bottom: style.inset_bottom,
            width: style.width,
            width_percent: style.width_percent,
            height_percent: style.height_percent,
            height: style.height,
            max_width: style.max_width,
            min_height: style.min_height,
            min_width: style.min_width,
            padding_top: style.padding_top.unwrap_or(0.0),
            padding_right: style.padding_right.unwrap_or(0.0),
            padding_bottom: style.padding_bottom.unwrap_or(0.0),
            padding_left: style.padding_left.unwrap_or(0.0),
            margin_top: style
                .margin_top
                .unwrap_or(LengthPercentageAuto::Length(0.0)),
            margin_right: style
                .margin_right
                .unwrap_or(LengthPercentageAuto::Length(0.0)),
            margin_bottom: style
                .margin_bottom
                .unwrap_or(LengthPercentageAuto::Length(0.0)),
            margin_left: style
                .margin_left
                .unwrap_or(LengthPercentageAuto::Length(0.0)),
            is_flex: style.is_flex,
            is_grid: style.is_grid,
            inline_flow: style.inline_flow,
            inline_level: style.inline_level,
            inline_atomic: style.inline_atomic,
            vertical_align: style.vertical_align.unwrap_or_default(),
            grid_template_columns: style.grid_template_columns,
            grid_template_rows: style.grid_template_rows,
            grid_auto_flow: style.grid_auto_flow,
            grid_auto_rows: style.grid_auto_rows,
            col_start: style.col_start,
            col_end: style.col_end,
            row_start: style.row_start,
            row_end: style.row_end,
            auto_size: style.auto_size,
            flex_direction: style.flex_direction.unwrap_or_default(),
            justify_content: style.justify_content.unwrap_or_default(),
            align_items: style.align_items.unwrap_or_default(),
            flex_wrap: style.flex_wrap.unwrap_or_default(),
            align_content: style.align_content,
            align_self: style.align_self,
            justify_items: style.justify_items,
            justify_self: style.justify_self,
            gap: style.gap.unwrap_or(0.0),
            gap_x: style.gap_x,
            gap_y: style.gap_y,
            order: style.order.unwrap_or(0),
            aspect_ratio: style.aspect_ratio,
            flex_basis: style.flex_basis,
            flex_grow: style.flex_grow.unwrap_or(0.0),
            flex_shrink: style.flex_shrink,
            z_index: style.z_index.unwrap_or(0),
            truncate: style.truncate,
        },
        visual: ComputedVisualStyle {
            opacity: style.opacity.unwrap_or(1.0),
            hidden: match style.visibility {
                Some(crate::style::Visibility::Hidden) => true,
                Some(crate::style::Visibility::Visible) => false,
                Some(crate::style::Visibility::Inherit) | None => inherited_style.hidden,
            },
            blend_mode: style.blend_mode.unwrap_or_default(),
            background_blend_mode: style.background_blend_mode.unwrap_or_default(),
            background: resolve_background(style),
            background_clip: style.background_clip,
            fill: style
                .fill_color
                .map(|color| crate::style::BackgroundFill::Solid { color }),
            border_radius: style.border_radius.unwrap_or_default(),
            border_width: style.border_width,
            border_top_width: style.border_top_width,
            border_right_width: style.border_right_width,
            border_bottom_width: style.border_bottom_width,
            border_left_width: style.border_left_width,
            border_color: style.border_color,
            stroke_color: style.stroke_color,
            stroke_width: style.stroke_width,
            stroke_dasharray: style.stroke_dasharray,
            stroke_dashoffset: style.stroke_dashoffset,
            border_style: style.border_style,
            css_filter: style.css_filter.clone(),
            backdrop_blur_sigma: style.backdrop_blur_sigma,
            backdrop_filter: style.backdrop_filter.clone(),
            object_fit: style.object_fit.unwrap_or_default(),
            clip_contents: style.overflow_hidden,
            clip_x_only: style.overflow_hidden && style.overflow_y_visible,
            clip_path: style.clip_path,
            transforms: style.transforms.clone(),
            transform_origin: style.transform_origin.unwrap_or_default(),
            perspective: style.perspective,
            transform_style_preserve_3d: style.transform_style_preserve_3d.unwrap_or(false),
            box_shadow: style
                .box_shadow
                .iter()
                .map(|shadow| shadow.with_color(style.box_shadow_color.unwrap_or(shadow.color)))
                .collect(),
            inset_shadow: style
                .inset_shadow
                .iter()
                .map(|shadow| shadow.with_color(style.inset_shadow_color.unwrap_or(shadow.color)))
                .collect(),
            drop_shadow: style
                .drop_shadow
                .iter()
                .map(|shadow| shadow.with_color(style.drop_shadow_color.unwrap_or(shadow.color)))
                .collect(),
            text_shadows: style.text_shadows.clone(),
            svg_path: style.svg_path.clone(),
        },
        text,
        id: style.id.clone(),
        css_layout: {
            let mut css = style.css_layout.clone();
            if display_unlock
                && let Some(c) = css.as_mut()
                && c.0.display == taffy::prelude::Display::None
            {
                c.0.display = taffy::prelude::Display::Block;
            }
            css
        },
    }
}

/// Resolve the background layer list for a node.
///
/// Priority: explicit arbitrary layers (`bg-[gradient]`) win; otherwise the
/// legacy `bg-gradient-*` / `bg-radial` shorthand builds a single layer; finally
/// `bg_color` is a flat solid fallback. Returns an empty `Vec` for no background.
///
/// CSS background 共存语义：`bg_color` 与任意渐变层可同时声明（如
/// `.tickbox` 的 `background-color:#010101` + 低 alpha `background-image`），
/// 此时 `bg_color` 作为 Solid 层沉到所有 image 层之下。层序同 CSS：第一层
/// 最靠近观察者，绘制端从末层画起，故底色排在末尾。
fn resolve_background(style: &NodeStyle) -> Vec<crate::style::BackgroundFill> {
    use crate::style::BackgroundFill;

    let mut layers = if !style.background_layers.is_empty() {
        let mut layers = style.background_layers.clone();
        if let Some(color) = style.bg_color {
            layers.push(BackgroundFill::Solid { color });
        }
        layers
    } else if let (Some(center), Some(from), Some(to)) = (
        style.bg_gradient_radial_center,
        style.bg_gradient_from,
        style.bg_gradient_to,
    ) {
        vec![BackgroundFill::radial_from_via_to(
            center,
            from,
            style.bg_gradient_via,
            to,
        )]
    } else if let (Some(direction), Some(from), Some(to)) = (
        style.bg_gradient_direction,
        style.bg_gradient_from,
        style.bg_gradient_to,
    ) {
        vec![BackgroundFill::linear_from_via_to(
            direction,
            from,
            style.bg_gradient_via,
            to,
        )]
    } else if let Some(color) = style.bg_color {
        vec![BackgroundFill::Solid { color }]
    } else {
        Vec::new()
    };

    // 文档画布传播色（HTML 根节点专用）：排在所有层之后（最远，最先画），
    // 作者背景——包括半透明纯色——正确地叠在画布底色之上。
    if let Some(color) = style.bg_canvas_color {
        layers.push(BackgroundFill::Solid { color });
    }

    layers
}

fn collect_visual_script_targets(node: &Node, registry: &mut ScriptTargetRegistry) {
    collect_visual_script_targets_at(node, registry, None, &Default::default());
}

/// `parent` is threaded through the walk so each node's facts record where it
/// sits in the tree — the DOM host turns that into `parentNode` / `children`.
/// `inherited` is the parent's computed text style: text properties inherit,
/// so an element that declares no `font-size` measures at its ancestor's.
fn collect_visual_script_targets_at(
    node: &Node,
    registry: &mut ScriptTargetRegistry,
    parent: Option<&str>,
    inherited: &crate::style::ComputedTextStyle,
) {
    let id = node.style_ref().id.as_str();
    let text_style = crate::style::resolve_text_style(inherited, node.style_ref());
    if !id.is_empty() {
        registry.visual_ids.insert(id.to_string());
        if matches!(node.kind(), NodeKind::Canvas(_)) {
            registry.canvas_ids.insert(id.to_string());
        }
        let facts = crate::script::host::ScriptNodeFacts {
            tag: node_tag_name(node),
            parent: parent.unwrap_or("").to_string(),
            path_data: match node.kind() {
                NodeKind::Path(path) => path.data().to_string(),
                _ => String::new(),
            },
        };
        registry.facts.insert(id.to_string(), facts);
        // 文档序快照：脚本侧覆盖层据此组装默认子级视图（`<p>` 的文本
        // 物化为一个文本节点）。DFS 前序即文档序。
        let order = registry.declared_nodes.len();
        let text = match node.kind() {
            NodeKind::Text(text) => text.content().to_string(),
            _ => String::new(),
        };
        registry.declared_nodes.push(crate::script::tree::DeclaredNode {
            id: id.to_string(),
            parent: parent.unwrap_or("").to_string(),
            order,
            tag: node_tag_name(node),
            text,
        });
        // 盒子查询要整形，整形要样式：声明树上的计算文本样式（声明值 +
        // 沿祖先继承）。脚本逐帧改写的祖先字号不在其中——那是 mutation 层。
        registry.text_styles.insert(id.to_string(), text_style.clone());
    }
    let child_parent = (!id.is_empty()).then_some(id);
    let inherited = &text_style;
    match node.kind() {
        NodeKind::Div(div) => {
            for child in div.children_ref() {
                collect_visual_script_targets_at(child, registry, child_parent, inherited);
            }
        }
        NodeKind::Canvas(canvas) => {
            for child in canvas.hidden_children_ref() {
                collect_visual_script_targets_at(child, registry, child_parent, inherited);
            }
        }
        NodeKind::Video(video) => {
            for child in video.children_ref() {
                collect_visual_script_targets_at(child, registry, child_parent, inherited);
            }
        }
        NodeKind::Timeline(timeline) => {
            for segment in timeline.segments() {
                match segment {
                    crate::parse::time::TimelineSegment::Scene { scene, .. } => {
                        collect_visual_script_targets_at(scene, registry, child_parent, inherited);
                    }
                    crate::parse::time::TimelineSegment::Transition { from, to, .. } => {
                        collect_visual_script_targets_at(from, registry, child_parent, inherited);
                        collect_visual_script_targets_at(to, registry, child_parent, inherited);
                    }
                }
            }
        }
        NodeKind::Text(_)
        | NodeKind::Image(_)
        | NodeKind::Lottie(_)
        | NodeKind::Path(_)
        | NodeKind::Svg(_) => {}
    }
}

/// The DOM tag the host exposes for a node. SVG plugins branch on it
/// (`tagName.toLowerCase()`), so it has to be the HTML/SVG spelling the
/// author wrote, not the internal kind name.
fn node_tag_name(node: &Node) -> String {
    match node.kind() {
        NodeKind::Div(_) => "div",
        NodeKind::Canvas(_) => "canvas",
        NodeKind::Text(text) => text.tag().as_str(),
        NodeKind::Image(_) => "image",
        NodeKind::Lottie(_) => "lottie",
        NodeKind::Path(_) => "path",
        NodeKind::Svg(_) => "svg",
        NodeKind::Video(_) => "video",
        NodeKind::Timeline(_) => "tl",
    }
    .to_string()
}


