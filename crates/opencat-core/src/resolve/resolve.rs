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
use crate::script::recorder::MutationRecorder;

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
        apply_mutation_stack(&mut style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style);
        let inherited_style = InheritedStyle::for_child(&computed);
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
    style.layout.width_full = true;
    style.layout.height_full = true;
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
    style.visual.clip_path = None;
    style.visual.transforms.clear();
    style.visual.opacity = 1.0;

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
        style: compute_style(&style, cx.inherited_style),
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
        apply_mutation_stack(&mut style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style);

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

        let child_inherited_style = InheritedStyle::for_child(&computed);
        let children = resolve_hidden_children(canvas, cx, &child_inherited_style)?;

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::Canvas(ElementCanvas { commands }),
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
        apply_mutation_stack(&mut style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style);
        let inherited_style = InheritedStyle::for_child(&computed);

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
        apply_mutation_stack(&mut style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style);

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
        apply_mutation_stack(&mut style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style);

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
        apply_mutation_stack(&mut style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style);
        let inline = svg.inline().map(String::from);
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

    push_script_scope(style, cx)
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

fn apply_canvas_mutation_stack(commands: &mut Vec<DrawOp>, stack: &[StyleMutations], id: &str) {
    for mutations in stack {
        mutations.apply_to_canvas(commands, id);
    }
}

/// Resolve `ImageRef::Static` alias references to canonical `AssetId`s in
/// place. Video-frame refs already carry canonical ids and are left untouched.
/// An alias that is neither registered nor a known canonical asset is an error.
fn canonicalize_canvas_image_refs(
    commands: &mut [DrawOp],
    assets: &PreparedResourceCatalog,
) -> Result<()> {
    for op in commands.iter_mut() {
        let image_ref = match op {
            DrawOp::Image { image, .. } | DrawOp::ImageRect { image, .. } => image,
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
            height_percent: style.height_percent,
            height: style.height,
            max_width: style.max_width,
            width_full: style.width_full,
            height_full: style.height_full,
            min_height: style.min_height,
            min_width: style.min_width,
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
            blend_mode: style.blend_mode.unwrap_or_default(),
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
        css_layout: style.css_layout.clone(),
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
/// 此时 `bg_color` 作为 Solid 层沉到所有 image 层之下。
fn resolve_background(style: &NodeStyle) -> Vec<crate::style::BackgroundFill> {
    use crate::style::BackgroundFill;

    if !style.background_layers.is_empty() {
        let mut layers = style.background_layers.clone();
        if let Some(color) = style.bg_color {
            layers.insert(0, BackgroundFill::Solid { color });
        }
        return layers;
    }

    if let (Some(center), Some(from), Some(to)) = (
        style.bg_gradient_radial_center,
        style.bg_gradient_from,
        style.bg_gradient_to,
    ) {
        return vec![BackgroundFill::radial_from_via_to(
            center,
            from,
            style.bg_gradient_via,
            to,
        )];
    }

    if let (Some(direction), Some(from), Some(to)) = (
        style.bg_gradient_direction,
        style.bg_gradient_from,
        style.bg_gradient_to,
    ) {
        return vec![BackgroundFill::linear_from_via_to(
            direction,
            from,
            style.bg_gradient_via,
            to,
        )];
    }

    if let Some(color) = style.bg_color {
        return vec![BackgroundFill::Solid { color }];
    }

    Vec::new()
}

fn collect_visual_script_targets(node: &Node, registry: &mut ScriptTargetRegistry) {
    collect_visual_script_targets_at(node, registry, None);
}

/// `parent` is threaded through the walk so each node's facts record where it
/// sits in the tree — the DOM host turns that into `parentNode` / `children`.
fn collect_visual_script_targets_at(
    node: &Node,
    registry: &mut ScriptTargetRegistry,
    parent: Option<&str>,
) {
    let id = node.style_ref().id.as_str();
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
        // 文档序快照：脚本侧覆盖层据此组装默认子级视图（`<text>` 的文本
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
        // 盒子查询要整形，整形要样式。这里只取节点自己声明的文本属性
        // （字号/字体/字距/行高），不追继承 —— SplitText 用它判行与算
        // transform-origin，声明值已足够；继承来的值会随祖先变化，
        // 而这一层没有祖先上下文。
        registry.text_styles.insert(
            id.to_string(),
            crate::style::resolve_text_style(&Default::default(), node.style_ref()),
        );
    }
    let child_parent = (!id.is_empty()).then_some(id);
    match node.kind() {
        NodeKind::Div(div) => {
            for child in div.children_ref() {
                collect_visual_script_targets_at(child, registry, child_parent);
            }
        }
        NodeKind::Canvas(canvas) => {
            for child in canvas.hidden_children_ref() {
                collect_visual_script_targets_at(child, registry, child_parent);
            }
        }
        NodeKind::Video(video) => {
            for child in video.children_ref() {
                collect_visual_script_targets_at(child, registry, child_parent);
            }
        }
        NodeKind::Timeline(timeline) => {
            for segment in timeline.segments() {
                match segment {
                    crate::parse::time::TimelineSegment::Scene { scene, .. } => {
                        collect_visual_script_targets_at(scene, registry, child_parent);
                    }
                    crate::parse::time::TimelineSegment::Transition { from, to, .. } => {
                        collect_visual_script_targets_at(from, registry, child_parent);
                        collect_visual_script_targets_at(to, registry, child_parent);
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
        NodeKind::Text(_) => "text",
        NodeKind::Image(_) => "image",
        NodeKind::Lottie(_) => "lottie",
        NodeKind::Path(_) => "path",
        NodeKind::Svg(_) => "svg",
        NodeKind::Video(_) => "video",
        NodeKind::Timeline(_) => "tl",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::resolve_ui_tree;
    use crate::{
        FrameCtx,
        ir::{draw_op::DrawOp, draw_types::ImageRef},
        parse::primitives::{canvas, div, path, text, video},
        probe::catalog::PreparedResourceCatalog,
        resolve::tree::ElementKind,
        script::{
            NodeStyleMutations, StyleMutations, TextUnitGranularity, TextUnitOverride,
            TextUnitOverrideBatch,
        },
        test_support::MockScriptHost,
    };

    fn resolve(
        node: &crate::Node,
        frame_ctx: &FrameCtx,
        assets: &mut PreparedResourceCatalog,
    ) -> anyhow::Result<super::ElementNode> {
        let mut mock = MockScriptHost::default();
        resolve_ui_tree(node, frame_ctx, assets, None, &mut mock)
    }

    #[test]
    fn resolve_ui_tree_requires_explicit_node_id() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = PreparedResourceCatalog::default();

        let err = resolve(&div().into(), &frame_ctx, &mut assets)
            .expect_err("nodes without ids should fail during resolution");

        assert!(err.to_string().contains("node id is required"));
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
        let mut assets = PreparedResourceCatalog::default();

        let root = div()
            .id("root")
            .text_color(crate::style::ColorToken::Custom(43, 127, 255, 255))
            .font_bold()
            .line_height(1.8)
            .child(text("A").id("label"))
            .child(div().id("icon"));

        let resolved = resolve(&root.into(), &frame_ctx, &mut assets).expect("tree should resolve");

        assert_eq!(
            resolved.children[0].style.text.color,
            crate::style::ColorToken::Custom(43, 127, 255, 255)
        );
        assert_eq!(
            resolved.children[0].style.text.font_weight,
            crate::style::FontWeight::BOLD
        );
        assert_eq!(resolved.children[0].style.text.line_height, 1.8);
        assert_eq!(
            resolved.children[1].style.text.color,
            crate::style::ColorToken::Custom(43, 127, 255, 255)
        );
    }

    #[test]
    fn canvas_element_does_not_duplicate_mutation_commands_in_draw_slot() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = PreparedResourceCatalog::default();

        let root = div().id("root").child(canvas().id("c"));

        let mutations = StyleMutations {
            mutations: Default::default(),
            scene3_mutations: Default::default(),
            canvas_mutations: [(
                "c".to_string(),
                crate::script::CanvasMutations {
                    commands: vec![DrawOp::SetFillStyle {
                        color: crate::ir::draw_op::ColorU8 {
                            r: 223,
                            g: 223,
                            b: 221,
                            a: 187,
                        },
                    }],
                },
            )]
            .into_iter()
            .collect(),
        };

        let mut mock = MockScriptHost::default();
        let resolved = resolve_ui_tree(&root.into(), &frame_ctx, &mut assets, Some(&mutations), &mut mock)
            .expect("tree should resolve");

        let canvas_node = &resolved.children[0];
        let ElementKind::Canvas(canvas) = &canvas_node.kind else {
            panic!("expected canvas element");
        };
        // Commands land on the canvas item (the node's DrawScript display
        // item)…
        assert_eq!(canvas.commands.len(), 1);
        // …and must NOT be duplicated into the draw slot: the renderer paints
        // the item and the slot separately, so a filled slot here would draw
        // every canvas op twice per frame.
        assert!(
            canvas_node.draw_slot.commands.is_empty(),
            "canvas draw_slot must stay empty; commands are already carried by the canvas item"
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
        let mut assets = PreparedResourceCatalog::default();

        let root = div()
            .id("root")
            .bg(crate::style::ColorToken::Custom(251, 44, 54, 255))
            .border_w(3.0)
            .border_color(crate::style::ColorToken::Custom(43, 127, 255, 255))
            .child(text("A").id("label"))
            .child(text("B").id("icon"));

        let resolved = resolve(&root.into(), &frame_ctx, &mut assets).expect("tree should resolve");

        assert!(resolved.children[0].style.visual.background.is_empty());
        assert_eq!(resolved.children[0].style.visual.border_width, None);
        assert!(resolved.children[1].style.visual.background.is_empty());
        assert_eq!(resolved.children[1].style.visual.border_color, None);
    }

    #[test]
    fn resolved_merkle_separates_layout_paint_and_composite_input_semantics() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = PreparedResourceCatalog::default();

        let first = div()
            .id("root")
            .w(200.0)
            .bg(crate::style::ColorToken::Custom(251, 44, 54, 255))
            .child(text("A").id("label"));
        let second = div()
            .id("root")
            .w(200.0)
            .bg(crate::style::ColorToken::Custom(43, 127, 255, 255))
            .child(text("A").id("label"));
        let third = div()
            .id("root")
            .w(240.0)
            .bg(crate::style::ColorToken::Custom(251, 44, 54, 255))
            .child(text("A").id("label"));

        let first = resolve(&first.into(), &frame_ctx, &mut assets).expect("tree should resolve");
        let second = resolve(&second.into(), &frame_ctx, &mut assets).expect("tree should resolve");
        let third = resolve(&third.into(), &frame_ctx, &mut assets).expect("tree should resolve");

        assert_eq!(
            first.fingerprints.layout_input_subtree, second.fingerprints.layout_input_subtree,
            "paint-only changes must not dirty layout input semantics"
        );
        assert_ne!(
            first.fingerprints.paint_input_subtree, second.fingerprints.paint_input_subtree,
            "paint-only changes must dirty paint input semantics"
        );
        assert_ne!(
            first.fingerprints.layout_input_subtree, third.fingerprints.layout_input_subtree,
            "layout changes must dirty layout input semantics"
        );
        assert_eq!(first.fingerprints.node_count, 2);
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
        let mut assets = PreparedResourceCatalog::default();

        let root = div()
            .id("root")
            .opacity(0.4)
            .rotate_deg(12.0)
            .child(text("A").id("label"));

        let resolved = resolve(&root.into(), &frame_ctx, &mut assets).expect("tree should resolve");

        assert_eq!(resolved.style.visual.opacity, 0.4);
        assert_eq!(resolved.children[0].style.visual.opacity, 1.0);
        assert!(resolved.children[0].style.visual.transforms.is_empty());
    }

    #[test]
    fn resolve_path_uses_core_computed_view_box() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = PreparedResourceCatalog::default();

        let root = div().id("root").child(
            path("M0 0 L10 0 L10 20 L0 20 Z")
                .id("rect")
                .size(40.0, 80.0),
        );

        let resolved = resolve(&root.into(), &frame_ctx, &mut assets).expect("path should resolve");
        let ElementKind::SvgPath(svg) = &resolved.children[0].kind else {
            panic!("child should resolve to svg path element");
        };
        assert_eq!(svg.path_data, vec!["M0 0 L10 0 L10 20 L0 20 Z".to_string()]);
        assert_eq!(svg.view_box, [0.0, 0.0, 10.0, 20.0]);
        assert_eq!(svg.intrinsic_size, None);
    }

    #[test]
    fn resolve_path_honors_explicit_view_box_over_bounds() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = PreparedResourceCatalog::default();

        // Bounds of this path are [100, 200, 50, 50]; the explicit view box
        // must win so several sibling paths can share one coordinate space.
        let mut child = path("M100 200 L150 200 L150 250 L100 250 Z")
            .id("rect")
            .size(40.0, 80.0);
        child.set_view_box([0.0, 0.0, 512.0, 512.0]);
        let root = div().id("root").child(child);

        let resolved = resolve(&root.into(), &frame_ctx, &mut assets).expect("path should resolve");
        let ElementKind::SvgPath(svg) = &resolved.children[0].kind else {
            panic!("child should resolve to svg path element");
        };
        assert_eq!(svg.view_box, [0.0, 0.0, 512.0, 512.0]);
    }

    #[test]
    fn resolve_path_rejects_invalid_svg_data() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut assets = PreparedResourceCatalog::default();

        let root = div().id("root").child(path("definitely-not-svg").id("bad"));

        let err = resolve(&root.into(), &frame_ctx, &mut assets)
            .expect_err("invalid path data should fail during resolution");
        assert!(
            err.to_string().contains("invalid SVG path"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn resolve_video_falls_back_to_zero_dimensions_when_catalog_lacks_video_info() {
        let mut catalog = PreparedResourceCatalog::default();
        let v = video("/no/such/video.mp4").id("v");
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 30,
        };
        let element = resolve(&v.into(), &frame_ctx, &mut catalog).expect("resolve");
        if let ElementKind::Bitmap(b) = &element.kind {
            assert_eq!(b.width, 0);
            assert_eq!(b.height, 0);
            assert!(b.video_timing.is_some());
        } else {
            panic!("expected Bitmap kind");
        }
    }

    #[test]
    fn canonicalize_canvas_image_refs_resolves_alias_to_canonical() {
        // AC4: a canvas draw-image op carrying a script-facing alias must be
        // rewritten to its canonical AssetId before leaving resolve.
        let mut catalog = PreparedResourceCatalog::default();
        let canonical = catalog.register_dimensions("/tmp/a.png", 10, 10);
        let alias_id =
            crate::ir::asset_id::AssetId::new(crate::ir::asset_id::ResourceKind::Image, "hero");
        catalog.alias(alias_id.clone(), &canonical).unwrap();

        let mut commands = vec![DrawOp::Image {
            image: ImageRef::Static {
                asset_id: "hero".to_string(),
            },
            x: 0.0,
            y: 0.0,
            paint: None,
        }];
        super::canonicalize_canvas_image_refs(&mut commands, &mut catalog).unwrap();

        match &commands[0] {
            DrawOp::Image {
                image: ImageRef::Static { asset_id },
                ..
            } => assert_eq!(*asset_id, canonical.key),
            other => panic!("unexpected op: {other:?}"),
        }
    }

    #[test]
    fn canonicalize_canvas_image_refs_keeps_canonical_id() {
        // A canonical id that is not an alias is left untouched.
        let mut catalog = PreparedResourceCatalog::default();
        let canonical = catalog.register_dimensions("/tmp/a.png", 10, 10);

        let mut commands = vec![DrawOp::Image {
            image: ImageRef::Static {
                asset_id: canonical.key.clone(),
            },
            x: 0.0,
            y: 0.0,
            paint: None,
        }];
        super::canonicalize_canvas_image_refs(&mut commands, &mut catalog).unwrap();

        match &commands[0] {
            DrawOp::Image {
                image: ImageRef::Static { asset_id },
                ..
            } => assert_eq!(*asset_id, canonical.key),
            other => panic!("unexpected op: {other:?}"),
        }
    }

    #[test]
    fn canonicalize_canvas_image_refs_rejects_unknown_alias() {
        // AC3: an alias that is neither registered nor a known canonical asset
        // is a render error.
        let mut catalog = PreparedResourceCatalog::default();

        let mut commands = vec![DrawOp::Image {
            image: ImageRef::Static {
                asset_id: "ghost".to_string(),
            },
            x: 0.0,
            y: 0.0,
            paint: None,
        }];
        let err = super::canonicalize_canvas_image_refs(&mut commands, &mut catalog)
            .expect_err("unknown alias should error");
        assert!(
            err.to_string().contains("unknown asset"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn background_layers_coexist_with_bg_color_as_solid_base() {
        // CSS 语义：`bg-[#010101] bg-[linear-gradient(...)]` 时 bg_color 作为
        // Solid 沉底、渐变层叠上（此前渐变层会整体忽略/清除 bg_color）。
        use crate::style::{BackgroundFill, ColorToken, NodeStyle};

        let mut style = NodeStyle::default();
        style.bg_color = Some(ColorToken::Custom(1, 1, 1, 255));
        style.background_layers = vec![BackgroundFill::Solid {
            color: ColorToken::Custom(200, 200, 200, 8),
        }];

        let layers = super::resolve_background(&style);
        assert_eq!(layers.len(), 2);
        assert_eq!(
            layers[0],
            BackgroundFill::Solid {
                color: ColorToken::Custom(1, 1, 1, 255)
            },
            "bg_color should sink to the base layer"
        );

        // 渐变层存在但无 bg_color：行为不变。
        let mut style = NodeStyle::default();
        style.background_layers = vec![BackgroundFill::Solid {
            color: ColorToken::Custom(200, 200, 200, 8),
        }];
        assert_eq!(super::resolve_background(&style).len(), 1);

        // 只有 bg_color：仍为单个 Solid。
        let mut style = NodeStyle::default();
        style.bg_color = Some(ColorToken::Custom(1, 1, 1, 255));
        assert_eq!(super::resolve_background(&style).len(), 1);
    }
}
