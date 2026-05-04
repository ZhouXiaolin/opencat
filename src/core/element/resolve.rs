use anyhow::{Result, ensure};

#[cfg(feature = "host-default")]
use crate::host::script::ScriptRuntimeCache;

use crate::{
    FrameCtx, Node,
    core::element::{
        style::{ComputedLayoutStyle, ComputedStyle, ComputedVisualStyle, InheritedStyle},
        tree::{
            ElementBitmap, ElementCanvas, ElementDiv, ElementId, ElementKind, ElementNode,
            ElementSvgPath, ElementText, ElementTimeline, ElementTimelineTransition,
        },
    },
    core::frame_ctx::ScriptFrameCtx,
    core::resource::asset_catalog::AssetId,
    core::resource::catalog::{ResourceCatalog, VideoInfoMeta},
    core::scene::script::{ScriptHost, StyleMutations, TextUnitOverrideBatch},
    core::scene::{
        node::{ComponentNode, NodeKind},
        primitives::{Canvas, CaptionNode, Div, Image, Lucide, Path, Text, Video},
        time::TimelineNode,
        time::{FrameState, frame_state_for_root},
    },
    core::style::LengthPercentageAuto,
    core::style::{NodeStyle, resolve_text_style},
};

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
    assets: &'a mut dyn ResourceCatalog,
    script_runtime: &'a mut ScriptRuntimeCache,
    mutation_stack: &'a mut Vec<StyleMutations>,
}

pub fn resolve_ui_tree(
    node: &Node,
    frame_ctx: &FrameCtx,
    assets: &mut dyn ResourceCatalog,
    mutations: Option<&StyleMutations>,
) -> Result<ElementNode> {
    let mut script_runtime = ScriptRuntimeCache::default();
    let script_frame_ctx = ScriptFrameCtx::global(frame_ctx);
    resolve_ui_tree_with_script_cache(
        node,
        frame_ctx,
        &script_frame_ctx,
        assets,
        mutations,
        &mut script_runtime,
    )
}

pub(crate) fn resolve_ui_tree_with_script_cache(
    node: &Node,
    frame_ctx: &FrameCtx,
    script_frame_ctx: &ScriptFrameCtx,
    assets: &mut dyn ResourceCatalog,
    mutations: Option<&StyleMutations>,
    script_runtime: &mut ScriptRuntimeCache,
) -> Result<ElementNode> {
    let mut ids = ElementIdAllocator::default();
    let inherited_style = InheritedStyle::default();
    let mut mutation_stack = Vec::new();
    if let Some(mutations) = mutations.filter(|mutations| !mutations.is_empty()) {
        mutation_stack.push(mutations.clone());
    }
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
    Ok(resolve_node_optional(node, &mut cx)?.unwrap_or_else(|| empty_root_div(&mut cx)))
}

fn resolve_node(
    node: &Node,
    cx: &mut ResolveContext<'_>,
) -> Result<ElementNode> {
    match node.kind() {
        NodeKind::Component(component) => resolve_component(component, cx),
        NodeKind::Video(video) => resolve_video(video, cx),
        NodeKind::Image(image) => resolve_image(image, cx),
        NodeKind::Div(div) => resolve_div(div, cx),
        NodeKind::Canvas(canvas) => resolve_canvas(canvas, cx),
        NodeKind::Text(text) => resolve_text(text, cx),
        NodeKind::Lucide(lucide) => resolve_lucide_svg_path(lucide, cx),
        NodeKind::Path(path) => resolve_path(path, cx),
        NodeKind::Timeline(timeline) => resolve_timeline(timeline, cx),
        NodeKind::Caption(caption) => resolve_caption(caption, cx)?
            .ok_or_else(|| anyhow::anyhow!("caption node has no active text for frame")),
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

fn resolve_timeline(
    timeline: &TimelineNode,
    cx: &mut ResolveContext<'_>,
) -> Result<ElementNode> {
    let pushed = push_script_scope_for_visible_subtree(timeline, timeline.style_ref(), cx)?;
    let result = (|| {
        let mut style = timeline.style_ref().clone();
        if style.id.is_empty() {
            style.id = format!("__timeline_{}", cx.ids.next);
        }
        apply_mutation_stack(&mut style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style);
        let inherited_style = InheritedStyle::for_child(&computed);
        let frame_state = frame_state_for_root(&Node::from(timeline.clone()), cx.frame_ctx);
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
            style: computed,
            children,
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
    resolve_node(node, &mut child_cx)
}

fn timeline_fill_wrapper(child: ElementNode, id: ElementId) -> ElementNode {
    let mut style = child.style.clone();
    style.id = format!("{}::__timeline_fill", style.id);
    style.layout.position = crate::core::scene::primitives::Position::Absolute;
    style.layout.inset_left = Some(LengthPercentageAuto::Length(0.0));
    style.layout.inset_top = Some(LengthPercentageAuto::Length(0.0));
    style.layout.inset_right = Some(LengthPercentageAuto::Length(0.0));
    style.layout.inset_bottom = Some(LengthPercentageAuto::Length(0.0));
    style.layout.width = None;
    style.layout.height = None;
    style.layout.width_full = true;
    style.layout.height_full = true;
    style.layout.margin_top = LengthPercentageAuto::Length(0.0);
    style.layout.margin_right = LengthPercentageAuto::Length(0.0);
    style.layout.margin_bottom = LengthPercentageAuto::Length(0.0);
    style.layout.margin_left = LengthPercentageAuto::Length(0.0);
    style.visual.background = None;
    style.visual.border_width = None;
    style.visual.border_top_width = None;
    style.visual.border_right_width = None;
    style.visual.border_bottom_width = None;
    style.visual.border_left_width = None;
    style.visual.border_color = None;
    style.visual.border_style = None;
    style.visual.box_shadow = None;
    style.visual.inset_shadow = None;
    style.visual.drop_shadow = None;
    style.visual.blur_sigma = None;
    style.visual.backdrop_blur_sigma = None;
    style.visual.border_radius = crate::core::style::BorderRadius::default();
    style.visual.clip_contents = false;
    style.visual.transforms.clear();
    style.visual.opacity = 1.0;

    ElementNode {
        id,
        kind: ElementKind::Div(ElementDiv),
        style,
        children: vec![child],
    }
}

fn resolve_node_optional(
    node: &Node,
    cx: &mut ResolveContext<'_>,
) -> Result<Option<ElementNode>> {
    match node.kind() {
        NodeKind::Component(component) => resolve_component_optional(component, cx),
        NodeKind::Caption(caption) => resolve_caption(caption, cx),
        _ => resolve_node(node, cx).map(Some),
    }
}

fn resolve_component_optional(
    component: &ComponentNode,
    cx: &mut ResolveContext<'_>,
) -> Result<Option<ElementNode>> {
    let resolved = component.render(cx.frame_ctx);
    let pushed = if component.style_ref().script_driver.is_some() {
        seed_text_sources_for_visible_subtree(
            &resolved,
            cx.frame_ctx,
            cx.script_frame_ctx,
            cx.mutation_stack,
            cx.script_runtime,
        );
        push_script_scope(component.style_ref(), cx)?
    } else {
        false
    };
    let result = resolve_node_optional(&resolved, cx);
    if pushed {
        cx.mutation_stack.pop();
    }
    result
}

fn empty_root_div(cx: &mut ResolveContext<'_>) -> ElementNode {
    let mut style = NodeStyle::default();
    style.id = "__empty_root".to_string();
    ElementNode {
        id: cx.ids.alloc(),
        kind: ElementKind::Div(ElementDiv),
        style: compute_style(&style, cx.inherited_style),
        children: Vec::new(),
    }
}

fn resolve_component(
    component: &ComponentNode,
    cx: &mut ResolveContext<'_>,
) -> Result<ElementNode> {
    let resolved = component.render(cx.frame_ctx);
    let pushed = if component.style_ref().script_driver.is_some() {
        seed_text_sources_for_visible_subtree(
            &resolved,
            cx.frame_ctx,
            cx.script_frame_ctx,
            cx.mutation_stack,
            cx.script_runtime,
        );
        push_script_scope(component.style_ref(), cx)?
    } else {
        false
    };
    let result = resolve_node(&resolved, cx);
    if pushed {
        cx.mutation_stack.pop();
    }
    result
}

fn resolve_div(
    div: &Div,
    cx: &mut ResolveContext<'_>,
) -> Result<ElementNode> {
    let pushed = push_script_scope_for_visible_subtree(div, div.style_ref(), cx)?;
    let result = (|| {
        let mut style = div.style_ref().clone();
        ensure!(
            !style.id.is_empty(),
            "node id is required for div nodes before rendering"
        );
        apply_mutation_stack(&mut style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style);
        let inherited_style = InheritedStyle::for_child(&computed);
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

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::Div(ElementDiv),
            style: computed,
            children,
        })
    })();
    if pushed {
        cx.mutation_stack.pop();
    }
    result
}

fn resolve_text(text: &Text, cx: &mut ResolveContext<'_>) -> Result<ElementNode> {
    let pushed = push_script_scope_for_visible_subtree(text, text.style_ref(), cx)?;
    let result = (|| {
        let mut style = text.style_ref().clone();
        ensure!(
            !style.id.is_empty(),
            "node id is required for text nodes before rendering"
        );
        apply_mutation_stack(&mut style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style);

        let content = text_content_from_stack(cx.mutation_stack, &style.id)
            .unwrap_or_else(|| text.content().to_string());

        let text_unit_overrides = merge_text_unit_overrides(cx.mutation_stack, &style.id);

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::Text(ElementText {
                text: content,
                text_style: computed.text,
                text_unit_overrides,
            }),
            style: computed,
            children: Vec::new(),
        })
    })();
    if pushed {
        cx.mutation_stack.pop();
    }
    result
}

fn resolve_caption(
    caption: &CaptionNode,
    cx: &mut ResolveContext<'_>,
) -> Result<Option<ElementNode>> {
    let pushed = push_script_scope_for_visible_subtree(caption, caption.style_ref(), cx)?;
    let result = (|| {
        let mut style = caption.style_ref().clone();
        ensure!(
            !style.id.is_empty(),
            "node id is required for caption nodes before rendering"
        );
        apply_mutation_stack(&mut style, cx.mutation_stack);

        let content = text_content_from_stack(cx.mutation_stack, &style.id).or_else(|| {
            caption
                .active_text(cx.script_frame_ctx.current_frame)
                .map(|s| s.to_string())
        });

        let content = match content {
            Some(content) => content,
            None => return Ok(None),
        };

        let computed = compute_style(&style, cx.inherited_style);

        let text_unit_overrides = merge_text_unit_overrides(cx.mutation_stack, &style.id);

        Ok(Some(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::Text(ElementText {
                text: content,
                text_style: computed.text.clone(),
                text_unit_overrides,
            }),
            style: computed,
            children: Vec::new(),
        }))
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
        apply_mutation_stack(&mut style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style);

        for asset in canvas.assets_ref() {
            let target = cx.assets.resolve_image(&asset.source)?;
            cx.assets.alias(AssetId(asset.asset_id.clone()), &target)?;
        }

        let mut commands = Vec::new();
        apply_canvas_mutation_stack(&mut commands, cx.mutation_stack, &style.id);

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::Canvas(ElementCanvas { commands }),
            style: computed,
            children: Vec::new(),
        })
    })();
    if pushed {
        cx.mutation_stack.pop();
    }
    result
}

fn resolve_video(
    video: &Video,
    cx: &mut ResolveContext<'_>,
) -> Result<ElementNode> {
    let pushed = push_script_scope_for_visible_subtree(video, video.style_ref(), cx)?;
    let result = (|| {
        let mut style = video.style_ref().clone();
        ensure!(
            !style.id.is_empty(),
            "node id is required for video nodes before rendering"
        );
        apply_mutation_stack(&mut style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style);

        let asset_id = cx
            .assets
            .register_dimensions(&video.source().to_string_lossy(), 0, 0);
        let info = cx.assets.video_info(&asset_id).unwrap_or(VideoInfoMeta {
            width: 0,
            height: 0,
            duration_secs: None,
        });
        let asset_id = cx
            .assets
            .register_dimensions(&video.source().to_string_lossy(), info.width, info.height);

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::Bitmap(ElementBitmap {
                asset_id,
                width: info.width,
                height: info.height,
                video_timing: Some(video.timing()),
            }),
            style: computed,
            children: Vec::new(),
        })
    })();
    if pushed {
        cx.mutation_stack.pop();
    }
    result
}

fn resolve_image(
    image: &Image,
    cx: &mut ResolveContext<'_>,
) -> Result<ElementNode> {
    let pushed = push_script_scope_for_visible_subtree(image, image.style_ref(), cx)?;
    let result = (|| {
        let mut style = image.style_ref().clone();
        ensure!(
            !style.id.is_empty(),
            "node id is required for image nodes before rendering"
        );
        apply_mutation_stack(&mut style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style);

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
            style: computed,
            children: Vec::new(),
        })
    })();
    if pushed {
        cx.mutation_stack.pop();
    }
    result
}

fn resolve_lucide_svg_path(lucide: &Lucide, cx: &mut ResolveContext<'_>) -> Result<ElementNode> {
    let pushed = push_script_scope_for_visible_subtree(lucide, lucide.style_ref(), cx)?;
    let result = (|| {
        let mut style = lucide.style_ref().clone();
        ensure!(
            !style.id.is_empty(),
            "node id is required for lucide nodes before rendering"
        );
        apply_mutation_stack(&mut style, cx.mutation_stack);
        let icon = normalize_lucide_icon_name(lucide.icon());
        ensure_valid_lucide_icon(icon)?;

        // Lucide default stroke width 2.0, default stroke color Black
        if style.stroke_width.is_none() {
            style.stroke_width = Some(2.0);
        }
        if style.stroke_color.is_none() {
            style.stroke_color = Some(crate::core::style::ColorToken::Black);
        }

        let computed = compute_style(&style, cx.inherited_style);

        let (path_data, view_box) = if let Some(svg_path) = &computed.visual.svg_path {
            let pd = vec![svg_path.clone()];
            let vb = compute_path_view_box(&pd).unwrap_or([0.0, 0.0, 24.0, 24.0]);
            (pd, vb)
        } else {
            let paths = crate::core::lucide_icons::lucide_icon_paths(icon)
                .expect("already validated by ensure_valid_lucide_icon");
            let pd: Vec<String> = paths.iter().map(|s| s.to_string()).collect();
            let vb = [0.0, 0.0, 24.0, 24.0];
            (pd, vb)
        };

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::SvgPath(ElementSvgPath {
                path_data,
                view_box,
                intrinsic_size: Some((24.0, 24.0)),
            }),
            style: computed,
            children: Vec::new(),
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
        apply_mutation_stack(&mut style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style);

        let (path_data, view_box) = if let Some(svg_path) = &computed.visual.svg_path {
            let pd = vec![svg_path.clone()];
            let vb = compute_path_view_box(&pd).unwrap_or_else(|_| {
                compute_path_view_box(&[path.data().to_string()])
                    .unwrap_or([0.0, 0.0, 100.0, 100.0])
            });
            (pd, vb)
        } else {
            let pd = vec![path.data().to_string()];
            let vb = compute_path_view_box(&pd)?;
            (pd, vb)
        };

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::SvgPath(ElementSvgPath {
                path_data,
                view_box,
                intrinsic_size: None,
            }),
            style: computed,
            children: Vec::new(),
        })
    })();
    if pushed {
        cx.mutation_stack.pop();
    }
    result
}

fn compute_path_view_box(path_data: &[String]) -> Result<[f32; 4]> {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    let mut has_any = false;

    for data in path_data {
        let path = skia_safe::Path::from_svg(data)
            .ok_or_else(|| anyhow::anyhow!("invalid SVG path data"))?;
        let bounds = path.bounds();
        min_x = min_x.min(bounds.left());
        min_y = min_y.min(bounds.top());
        max_x = max_x.max(bounds.right());
        max_y = max_y.max(bounds.bottom());
        has_any = true;
    }

    if !has_any {
        return Ok([0.0, 0.0, 24.0, 24.0]);
    }

    let w = (max_x - min_x).max(1.0);
    let h = (max_y - min_y).max(1.0);
    Ok([min_x, min_y, w, h])
}

fn normalize_lucide_icon_name(name: &str) -> &str {
    match name {
        // Lucide keeps `home` as deprecated metadata alias for the current `house` icon.
        "home" => "house",
        // Travel mock data often uses the more literal suitcase label.
        "suitcase" => "briefcase",
        _ => name,
    }
}

fn ensure_valid_lucide_icon(name: &str) -> Result<()> {
    if crate::core::lucide_icons::lucide_icon_paths(name).is_some() {
        return Ok(());
    }

    let suggestions = suggested_lucide_icons(name);
    let detail = if suggestions.is_empty() {
        "no similar icons found".to_string()
    } else {
        format!("did you mean {}?", suggestions.join(", "))
    };

    anyhow::bail!("unknown lucide icon `{name}`: {detail}")
}

fn suggested_lucide_icons(name: &str) -> Vec<&'static str> {
    let mut scored: Vec<(usize, &'static str)> = crate::core::lucide_icons::lucide_icon_names()
        .iter()
        .map(|candidate| (levenshtein_distance(name, candidate), *candidate))
        .collect();
    scored.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(right.1)));
    scored
        .into_iter()
        .take(5)
        .map(|(_, candidate)| candidate)
        .collect()
}

fn levenshtein_distance(left: &str, right: &str) -> usize {
    let left_chars: Vec<char> = left.chars().collect();
    let right_chars: Vec<char> = right.chars().collect();

    if left_chars.is_empty() {
        return right_chars.len();
    }
    if right_chars.is_empty() {
        return left_chars.len();
    }

    let mut prev: Vec<usize> = (0..=right_chars.len()).collect();
    let mut curr = vec![0; right_chars.len() + 1];

    for (left_index, left_char) in left_chars.iter().enumerate() {
        curr[0] = left_index + 1;
        for (right_index, right_char) in right_chars.iter().enumerate() {
            let cost = usize::from(left_char != right_char);
            curr[right_index + 1] = (curr[right_index] + 1)
                .min(prev[right_index + 1] + 1)
                .min(prev[right_index] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[right_chars.len()]
}

fn push_script_scope(style: &NodeStyle, cx: &mut ResolveContext<'_>) -> Result<bool> {
    let Some(driver) = style.script_driver.as_deref() else {
        return Ok(false);
    };

    let id = cx.script_runtime.install(&driver.source)?;
    let mutations = cx.script_runtime.run_by_id(
        id,
        *cx.script_frame_ctx,
        (!style.id.is_empty()).then_some(style.id.as_str()),
    )?;
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
    push_script_scope(style, cx)
}

fn seed_text_sources_for_visible_subtree(
    node: &Node,
    frame_ctx: &FrameCtx,
    script_frame_ctx: &ScriptFrameCtx,
    mutation_stack: &[StyleMutations],
    script_runtime: &mut ScriptRuntimeCache,
) {
    match node.kind() {
        NodeKind::Component(component) => {
            let rendered = component.render(frame_ctx);
            seed_text_sources_for_visible_subtree(
                &rendered,
                frame_ctx,
                script_frame_ctx,
                mutation_stack,
                script_runtime,
            );
        }
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
                crate::core::scene::script::ScriptTextSource {
                    text: content,
                    kind: crate::core::scene::script::ScriptTextSourceKind::TextNode,
                },
            );
        }
        NodeKind::Caption(caption) => {
            let id = &caption.style_ref().id;
            if id.is_empty() {
                return;
            }
            let content = text_content_from_stack(mutation_stack, id).or_else(|| {
                caption
                    .active_text(script_frame_ctx.current_frame)
                    .map(str::to_string)
            });
            if let Some(content) = content {
                script_runtime.register_text_source(
                    id,
                    crate::core::scene::script::ScriptTextSource {
                        text: content,
                        kind: crate::core::scene::script::ScriptTextSourceKind::Caption,
                    },
                );
            }
        }
        NodeKind::Timeline(timeline) => {
            let frame_state = frame_state_for_root(&Node::from(timeline.clone()), frame_ctx);
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
        NodeKind::Canvas(_)
        | NodeKind::Image(_)
        | NodeKind::Lucide(_)
        | NodeKind::Path(_)
        | NodeKind::Video(_) => {}
    }
}

fn apply_mutation_stack(style: &mut NodeStyle, stack: &[StyleMutations]) {
    let id = style.id.clone();
    for mutations in stack {
        mutations.apply_to_node(style, &id);
    }
}

fn apply_canvas_mutation_stack(
    commands: &mut Vec<crate::core::scene::script::CanvasCommand>,
    stack: &[StyleMutations],
    id: &str,
) {
    for mutations in stack {
        mutations.apply_to_canvas(commands, id);
    }
}

fn text_content_from_stack(stack: &[StyleMutations], id: &str) -> Option<String> {
    stack
        .iter()
        .rev()
        .find_map(|m| m.text_content_for(id).map(str::to_string))
}

fn merge_text_unit_overrides(stack: &[StyleMutations], id: &str) -> Option<TextUnitOverrideBatch> {
    use crate::core::scene::script::TextUnitOverride;
    let mut merged: Option<TextUnitOverrideBatch> = None;
    for layer in stack {
        let Some(batch) = layer.get(id).and_then(|m| m.text_unit_overrides.as_ref()) else {
            continue;
        };

        match &mut merged {
            None => merged = Some(batch.clone()),
            Some(current) => {
                if current.granularity != batch.granularity {
                    return None;
                }
                if batch.overrides.len() > current.overrides.len() {
                    current
                        .overrides
                        .resize_with(batch.overrides.len(), TextUnitOverride::default);
                }
                for (index, incoming) in batch.overrides.iter().enumerate() {
                    let slot = &mut current.overrides[index];
                    if incoming.opacity.is_some() {
                        slot.opacity = incoming.opacity;
                    }
                    if incoming.translate_x.is_some() {
                        slot.translate_x = incoming.translate_x;
                    }
                    if incoming.translate_y.is_some() {
                        slot.translate_y = incoming.translate_y;
                    }
                    if incoming.scale.is_some() {
                        slot.scale = incoming.scale;
                    }
                    if incoming.rotation_deg.is_some() {
                        slot.rotation_deg = incoming.rotation_deg;
                    }
                    if incoming.color.is_some() {
                        slot.color = incoming.color;
                    }
                }
            }
        }
    }
    merged
}

fn compute_style(style: &NodeStyle, inherited_style: &InheritedStyle) -> ComputedStyle {
    let text = resolve_text_style(&inherited_style.text, style);
    ComputedStyle {
        layout: ComputedLayoutStyle {
            position: style.position.unwrap_or_default(),
            inset_left: style.inset_left,
            inset_top: style.inset_top,
            inset_right: style.inset_right,
            inset_bottom: style.inset_bottom,
            width: style.width,
            width_percent: style.width_percent,
            height: style.height,
            max_width: style.max_width,
            width_full: style.width_full,
            height_full: style.height_full,
            min_height: style.min_height,
            padding_top: style
                .padding_top
                .or(style.padding_y)
                .or(style.padding)
                .unwrap_or(0.0),
            padding_right: style
                .padding_right
                .or(style.padding_x)
                .or(style.padding)
                .unwrap_or(0.0),
            padding_bottom: style
                .padding_bottom
                .or(style.padding_y)
                .or(style.padding)
                .unwrap_or(0.0),
            padding_left: style
                .padding_left
                .or(style.padding_x)
                .or(style.padding)
                .unwrap_or(0.0),
            margin_top: style
                .margin_top
                .or(style.margin_y)
                .or(style.margin)
                .unwrap_or(LengthPercentageAuto::Length(0.0)),
            margin_right: style
                .margin_right
                .or(style.margin_x)
                .or(style.margin)
                .unwrap_or(LengthPercentageAuto::Length(0.0)),
            margin_bottom: style
                .margin_bottom
                .or(style.margin_y)
                .or(style.margin)
                .unwrap_or(LengthPercentageAuto::Length(0.0)),
            margin_left: style
                .margin_left
                .or(style.margin_x)
                .or(style.margin)
                .unwrap_or(LengthPercentageAuto::Length(0.0)),
            is_flex: style.is_flex,
            is_grid: style.is_grid,
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
            background: style
                .bg_gradient_direction
                .zip(style.bg_gradient_from)
                .zip(style.bg_gradient_to)
                .map(
                    |((direction, from), to)| crate::core::style::BackgroundFill::LinearGradient {
                        direction,
                        from,
                        via: style.bg_gradient_via,
                        to,
                    },
                )
                .or_else(|| style.bg_color.map(crate::core::style::BackgroundFill::Solid)),
            fill: style.fill_color.map(crate::core::style::BackgroundFill::Solid),
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
            blur_sigma: style.blur_sigma,
            backdrop_blur_sigma: style.backdrop_blur_sigma,
            object_fit: style.object_fit.unwrap_or_default(),
            clip_contents: style.overflow_hidden,
            transforms: style.transforms.clone(),
            box_shadow: style
                .box_shadow
                .map(|shadow| shadow.with_color(style.box_shadow_color.unwrap_or(shadow.color))),
            inset_shadow: style
                .inset_shadow
                .map(|shadow| shadow.with_color(style.inset_shadow_color.unwrap_or(shadow.color))),
            drop_shadow: style
                .drop_shadow
                .map(|shadow| shadow.with_color(style.drop_shadow_color.unwrap_or(shadow.color))),
            svg_path: style.svg_path.clone(),
        },
        text,
        id: style.id.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{merge_text_unit_overrides, resolve_ui_tree, resolve_ui_tree_with_script_cache};
    use crate::{
        FrameCtx,
        core::element::tree::ElementKind,
        core::frame_ctx::ScriptFrameCtx,
        core::resource::asset_catalog::AssetCatalog,
        core::resource::catalog::VideoInfoMeta,
        core::scene::easing::Easing,
        core::scene::primitives::{SrtEntry, caption, div, lucide, text, video},
        core::scene::script::{
            NodeStyleMutations, StyleMutations, TextUnitGranularity,
            TextUnitOverride, TextUnitOverrideBatch,
        },
        host::script::ScriptRuntimeCache,
        core::scene::time::{FrameState, frame_state_for_root},
        core::scene::transition::{slide, timeline},
    };

    #[test]
    fn resolve_ui_tree_requires_explicit_node_id() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = AssetCatalog::new();

        let err = resolve_ui_tree(&div().into(), &frame_ctx, &mut assets, None)
            .expect_err("nodes without ids should fail during resolution");

        assert!(err.to_string().contains("node id is required"));
    }

    #[test]
    fn node_script_only_affects_its_own_subtree() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = AssetCatalog::new();

        let scene = div()
            .id("root")
            .child(
                div()
                    .id("animated")
                    .script_source(r#"ctx.getNode("title").opacity(0.25);"#)
                    .expect("script should compile")
                    .child(text("A").id("title")),
            )
            .child(div().id("static").child(text("B").id("title")));

        let resolved = resolve_ui_tree(&scene.into(), &frame_ctx, &mut assets, None)
            .expect("tree should resolve");

        assert_eq!(resolved.children[0].children[0].style.visual.opacity, 0.25);
        assert_eq!(resolved.children[1].children[0].style.visual.opacity, 1.0);
    }

    #[test]
    fn only_text_defaults_inherit_to_children() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = AssetCatalog::new();

        let root = div()
            .id("root")
            .text_blue()
            .font_bold()
            .line_height(1.8)
            .child(text("A").id("label"))
            .child(lucide("play").id("icon").size(24.0, 24.0));

        let resolved = resolve_ui_tree(&root.into(), &frame_ctx, &mut assets, None)
            .expect("tree should resolve");

        assert_eq!(
            resolved.children[0].style.text.color,
            crate::core::style::ColorToken::Blue
        );
        assert_eq!(
            resolved.children[0].style.text.font_weight,
            crate::core::style::FontWeight::BOLD
        );
        assert_eq!(resolved.children[0].style.text.line_height, 1.8);
        assert_eq!(
            resolved.children[1].style.text.color,
            crate::core::style::ColorToken::Blue
        );
    }

    #[test]
    fn visual_box_styles_do_not_inherit_to_children() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = AssetCatalog::new();

        let root = div()
            .id("root")
            .bg_red()
            .border_w(3.0)
            .border_blue()
            .child(text("A").id("label"))
            .child(lucide("play").id("icon").size(24.0, 24.0));

        let resolved = resolve_ui_tree(&root.into(), &frame_ctx, &mut assets, None)
            .expect("tree should resolve");

        assert_eq!(resolved.children[0].style.visual.background, None);
        assert_eq!(resolved.children[0].style.visual.border_width, None);
        assert_eq!(resolved.children[1].style.visual.background, None);
        assert_eq!(resolved.children[1].style.visual.border_color, None);
    }

    #[test]
    fn subtree_effects_stay_local_to_the_parent_node() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = AssetCatalog::new();

        let root = div()
            .id("root")
            .opacity(0.4)
            .rotate_deg(12.0)
            .child(text("A").id("label"));

        let resolved = resolve_ui_tree(&root.into(), &frame_ctx, &mut assets, None)
            .expect("tree should resolve");

        assert_eq!(resolved.style.visual.opacity, 0.4);
        assert_eq!(resolved.children[0].style.visual.opacity, 1.0);
        assert!(resolved.children[0].style.visual.transforms.is_empty());
    }

    #[test]
    fn resolve_ui_tree_rejects_unknown_lucide_icons() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = AssetCatalog::new();

        let root = div()
            .id("root")
            .child(lucide("pla").id("icon").size(24.0, 24.0));

        let err = resolve_ui_tree(&root.into(), &frame_ctx, &mut assets, None)
            .expect_err("unknown icon should fail during resolution");

        let message = err.to_string();
        assert!(message.contains("unknown lucide icon `pla`"));
        assert!(message.contains("play"));
    }

    #[test]
    fn resolve_ui_tree_accepts_home_lucide_alias() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = AssetCatalog::new();

        let root = div()
            .id("root")
            .child(lucide("home").id("icon").size(24.0, 24.0));

        let resolved = resolve_ui_tree(&root.into(), &frame_ctx, &mut assets, None)
            .expect("deprecated alias should resolve");

        let ElementKind::SvgPath(svg) = &resolved.children[0].kind else {
            panic!("child should resolve to svg path element");
        };
        let expected_paths = crate::core::lucide_icons::lucide_icon_paths("house").expect("house icon");
        assert_eq!(svg.path_data, expected_paths);
        assert_eq!(svg.intrinsic_size, Some((24.0, 24.0)));
    }

    #[test]
    fn resolve_ui_tree_accepts_suitcase_lucide_alias() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = AssetCatalog::new();

        let root = div()
            .id("root")
            .child(lucide("suitcase").id("icon").size(24.0, 24.0));

        let resolved = resolve_ui_tree(&root.into(), &frame_ctx, &mut assets, None)
            .expect("deprecated alias should resolve");

        let ElementKind::SvgPath(svg) = &resolved.children[0].kind else {
            panic!("child should resolve to svg path element");
        };
        let expected_paths =
            crate::core::lucide_icons::lucide_icon_paths("briefcase").expect("briefcase icon");
        assert_eq!(svg.path_data, expected_paths);
        assert_eq!(svg.intrinsic_size, Some((24.0, 24.0)));
    }

    #[test]
    fn text_unit_overrides_merge_per_field_across_stack_layers() {
        let mut lower = NodeStyleMutations::default();
        lower.text_unit_overrides = Some(TextUnitOverrideBatch {
            granularity: TextUnitGranularity::Grapheme,
            overrides: vec![TextUnitOverride {
                translate_y: Some(-12.0),
                color: Some(crate::core::style::ColorToken::Cyan400),
                ..Default::default()
            }],
        });

        let mut upper = NodeStyleMutations::default();
        upper.text_unit_overrides = Some(TextUnitOverrideBatch {
            granularity: TextUnitGranularity::Grapheme,
            overrides: vec![TextUnitOverride {
                opacity: Some(0.5),
                ..Default::default()
            }],
        });

        let stack = vec![
            StyleMutations {
                mutations: [("t".to_string(), lower)].into_iter().collect(),
                canvas_mutations: Default::default(),
            },
            StyleMutations {
                mutations: [("t".to_string(), upper)].into_iter().collect(),
                canvas_mutations: Default::default(),
            },
        ];

        let merged = merge_text_unit_overrides(&stack, "t").expect("merged batch");
        assert_eq!(merged.overrides[0].translate_y, Some(-12.0));
        assert_eq!(merged.overrides[0].opacity, Some(0.5));
        assert_eq!(
            merged.overrides[0].color,
            Some(crate::core::style::ColorToken::Cyan400)
        );
    }

    #[test]
    fn transition_scenes_keep_node_scripts_isolated() {
        let frame_ctx = FrameCtx {
            frame: 10,
            fps: 30,
            width: 320,
            height: 180,
            frames: 30,
        };
        let mut assets = AssetCatalog::new();
        let mut script_runtime = ScriptRuntimeCache::default();

        let from_scene = div()
            .id("scene-a")
            .script_source(r#"ctx.getNode("title").opacity(0.2);"#)
            .expect("script should compile")
            .child(text("From").id("title"));
        let to_scene = div()
            .id("scene-b")
            .script_source(r#"ctx.getNode("title").opacity(0.8);"#)
            .expect("script should compile")
            .child(text("To").id("title"));
        let root = timeline()
            .sequence(10, from_scene.into())
            .transition(slide().timing(Easing::Linear, 10))
            .sequence(10, to_scene.into())
            .into();

        let FrameState::Transition {
            from,
            to,
            from_script_frame_ctx,
            to_script_frame_ctx,
            ..
        } = frame_state_for_root(&root, &frame_ctx)
        else {
            panic!("expected transition frame");
        };

        let from_resolved = resolve_ui_tree_with_script_cache(
            &from,
            &frame_ctx,
            &from_script_frame_ctx,
            &mut assets,
            None,
            &mut script_runtime,
        )
        .expect("from scene should resolve");
        let to_resolved = resolve_ui_tree_with_script_cache(
            &to,
            &frame_ctx,
            &to_script_frame_ctx,
            &mut assets,
            None,
            &mut script_runtime,
        )
        .expect("to scene should resolve");

        assert_eq!(from_resolved.children[0].style.visual.opacity, 0.2);
        assert_eq!(to_resolved.children[0].style.visual.opacity, 0.8);
    }

    #[test]
    fn timeline_scripts_receive_scene_local_frames() {
        let frame_ctx = FrameCtx {
            frame: 19,
            fps: 30,
            width: 320,
            height: 180,
            frames: 60,
        };
        let mut assets = AssetCatalog::new();
        let mut script_runtime = ScriptRuntimeCache::default();

        let scene = div()
            .id("scene-b")
            .script_source(
                r#"ctx.getNode("title").opacity(ctx.currentFrame === 4 && ctx.sceneFrames === 10 ? 0.6 : 0.1);"#,
            )
            .expect("script should compile")
            .child(text("B").id("title"));
        let root = timeline()
            .sequence(
                10,
                div().id("scene-a").child(text("A").id("a-title")).into(),
            )
            .transition(slide().timing(Easing::Linear, 5))
            .sequence(10, scene.into())
            .into();

        let FrameState::Scene {
            scene,
            script_frame_ctx,
        } = frame_state_for_root(&root, &frame_ctx)
        else {
            panic!("expected scene frame");
        };

        let resolved = resolve_ui_tree_with_script_cache(
            &scene,
            &frame_ctx,
            &script_frame_ctx,
            &mut assets,
            None,
            &mut script_runtime,
        )
        .expect("scene should resolve");

        assert_eq!(
            script_frame_ctx,
            ScriptFrameCtx::for_segment(&frame_ctx, 15, 10)
        );
        assert_eq!(resolved.children[0].style.visual.opacity, 0.6);
    }

    #[test]
    fn parent_script_can_split_descendant_text_before_child_resolution() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = AssetCatalog::new();

        let root = div()
            .id("root")
            .script_source(
                r#"
                var parts = ctx.splitText("title", { type: "chars" });
                parts[0].set({ opacity: 0.2 });
            "#,
            )
            .expect("script should compile")
            .child(text("Hello").id("title"));

        let resolved = resolve_ui_tree(&root.into(), &frame_ctx, &mut assets, None)
            .expect("parent script should see descendant text source");

        let ElementKind::Text(text) = &resolved.children[0].kind else {
            panic!("child should resolve to text");
        };
        let batch = text
            .text_unit_overrides
            .as_ref()
            .expect("text unit overrides should exist");
        assert_eq!(batch.overrides[0].opacity, Some(0.2));
    }

    #[test]
    fn resolve_caption_uses_scene_local_time_inside_timeline() {
        let caption_node = caption().id("subs").path("sub.srt").entries(vec![
            SrtEntry {
                index: 1,
                start_frame: 0,
                end_frame: 5,
                text: "Local A".into(),
            },
            SrtEntry {
                index: 2,
                start_frame: 5,
                end_frame: 10,
                text: "Local B".into(),
            },
        ]);
        let root = timeline()
            .sequence(10, div().id("scene-a").child(text("A").id("t")).into())
            .sequence(10, div().id("scene-b").child(caption_node).into())
            .into();
        let frame_ctx = FrameCtx {
            frame: 17,
            fps: 30,
            width: 320,
            height: 180,
            frames: 20,
        };
        let mut assets = AssetCatalog::new();
        let mut runtime = ScriptRuntimeCache::default();

        let FrameState::Scene {
            scene,
            script_frame_ctx,
        } = frame_state_for_root(&root, &frame_ctx)
        else {
            panic!("expected scene frame");
        };

        let tree = resolve_ui_tree_with_script_cache(
            &scene,
            &frame_ctx,
            &script_frame_ctx,
            &mut assets,
            None,
            &mut runtime,
        )
        .expect("caption tree should resolve");

        assert!(format!("{tree:?}").contains("Local B"));
    }

    #[test]
    fn resolve_caption_falls_back_to_global_time_when_no_nearer_time_context_exists() {
        let root = div()
            .id("root")
            .child(caption().id("subs").path("sub.srt").entries(vec![
                SrtEntry {
                    index: 1,
                    start_frame: 0,
                    end_frame: 5,
                    text: "Global A".into(),
                },
                SrtEntry {
                    index: 2,
                    start_frame: 5,
                    end_frame: 10,
                    text: "Global B".into(),
                },
            ]));
        let frame_ctx = FrameCtx {
            frame: 7,
            fps: 30,
            width: 320,
            height: 180,
            frames: 10,
        };
        let mut assets = AssetCatalog::new();

        let tree = resolve_ui_tree(&root.into(), &frame_ctx, &mut assets, None)
            .expect("root caption should resolve from global time");

        assert!(format!("{tree:?}").contains("Global B"));
    }

    #[test]
    fn resolve_caption_omits_inactive_entry() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 90,
        };
        let mut assets = AssetCatalog::new();

        let entries = vec![SrtEntry {
            index: 1,
            start_frame: 30,
            end_frame: 60,
            text: "Hello".to_string(),
        }];
        let caption_node = caption().id("subs").path("test.srt").entries(entries);
        let root = div().id("root").child(caption_node);

        let resolved = resolve_ui_tree(
            &root.clone().into(),
            &frame_ctx,
            &mut assets,
            None,
        )
        .expect("div with inactive caption should resolve");

        assert_eq!(
            resolved.children.len(),
            0,
            "inactive caption should be omitted"
        );

        let frame_ctx_active = FrameCtx {
            frame: 45,
            ..frame_ctx
        };
        let resolved_active = resolve_ui_tree(
            &root.into(),
            &frame_ctx_active,
            &mut assets,
            None,
        )
        .expect("layer with active caption should resolve");

        assert_eq!(resolved_active.children.len(), 1);
        let ElementKind::Text(elem) = &resolved_active.children[0].kind else {
            panic!("active caption should resolve to text");
        };
        assert_eq!(elem.text, "Hello");
    }

    #[test]
    fn resolve_video_falls_back_to_zero_dimensions_when_catalog_lacks_video_info() {
        let mut catalog = AssetCatalog::new();
        let v = video("/no/such/video.mp4").id("v");
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 30,
        };
        let element = resolve_ui_tree(&v.into(), &frame_ctx, &mut catalog, None).expect("resolve");
        if let ElementKind::Bitmap(b) = &element.kind {
            assert_eq!(b.width, 0);
            assert_eq!(b.height, 0);
            assert!(b.video_timing.is_some());
        } else {
            panic!("expected Bitmap kind");
        }
    }
}
