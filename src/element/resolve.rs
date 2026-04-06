use anyhow::{Result, ensure};

use crate::{
    FrameCtx, Node,
    assets::AssetsMap,
    element::{
        style::{ComputedLayoutStyle, ComputedStyle, ComputedVisualStyle, InheritedStyle},
        tree::{
            ElementBitmap, ElementDiv, ElementId, ElementKind, ElementLucide, ElementNode,
            ElementText,
        },
    },
    media::MediaContext,
    nodes::{Div, Image, Lucide, Text, Video},
    script::{ScriptRuntimeCache, StyleMutations},
    style::{NodeStyle, resolve_text_style},
    view::{ComponentNode, NodeKind},
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
    ids: &'a mut ElementIdAllocator,
    inherited_style: &'a InheritedStyle,
    assets: &'a mut AssetsMap,
    script_runtime: &'a mut ScriptRuntimeCache,
    mutation_stack: &'a mut Vec<StyleMutations>,
}

pub fn resolve_ui_tree(
    node: &Node,
    frame_ctx: &FrameCtx,
    media: &mut MediaContext,
    assets: &mut AssetsMap,
    mutations: Option<&StyleMutations>,
) -> Result<ElementNode> {
    let mut script_runtime = ScriptRuntimeCache::default();
    resolve_ui_tree_with_script_cache(
        node,
        frame_ctx,
        media,
        assets,
        mutations,
        &mut script_runtime,
    )
}

pub(crate) fn resolve_ui_tree_with_script_cache(
    node: &Node,
    frame_ctx: &FrameCtx,
    media: &mut MediaContext,
    assets: &mut AssetsMap,
    mutations: Option<&StyleMutations>,
    script_runtime: &mut ScriptRuntimeCache,
) -> Result<ElementNode> {
    let mut ids = ElementIdAllocator::default();
    let inherited_style = InheritedStyle::default();
    let mut mutation_stack = Vec::new();
    if let Some(mutations) = mutations.filter(|mutations| !mutations.is_empty()) {
        mutation_stack.push(mutations.clone());
    }
    let mut cx = ResolveContext {
        frame_ctx,
        ids: &mut ids,
        inherited_style: &inherited_style,
        assets,
        script_runtime,
        mutation_stack: &mut mutation_stack,
    };
    resolve_node(node, &mut cx, media)
}

fn resolve_node(
    node: &Node,
    cx: &mut ResolveContext<'_>,
    media: &mut MediaContext,
) -> Result<ElementNode> {
    match node.kind() {
        NodeKind::Component(component) => resolve_component(component, cx, media),
        NodeKind::Video(video) => resolve_video(video, cx, media),
        NodeKind::Image(image) => resolve_image(image, cx, media),
        NodeKind::Div(div) => resolve_div(div, cx, media),
        NodeKind::Text(text) => resolve_text(text, cx),
        NodeKind::Lucide(lucide) => resolve_lucide(lucide, cx),
        NodeKind::Timeline(_) => {
            unreachable!("timeline nodes must be resolved before UI tree construction")
        }
    }
}

fn resolve_component(
    component: &ComponentNode,
    cx: &mut ResolveContext<'_>,
    media: &mut MediaContext,
) -> Result<ElementNode> {
    let pushed = push_script_scope(component.style_ref(), cx)?;
    let resolved = component.render(cx.frame_ctx);
    let result = resolve_node(&resolved, cx, media);
    if pushed {
        cx.mutation_stack.pop();
    }
    result
}

fn resolve_div(
    div: &Div,
    cx: &mut ResolveContext<'_>,
    media: &mut MediaContext,
) -> Result<ElementNode> {
    let pushed = push_script_scope(div.style_ref(), cx)?;
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
                ids: &mut *cx.ids,
                inherited_style: &inherited_style,
                assets: &mut *cx.assets,
                script_runtime: &mut *cx.script_runtime,
                mutation_stack: &mut *cx.mutation_stack,
            };
            children.push(resolve_node(child, &mut child_cx, media)?);
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
    let pushed = push_script_scope(text.style_ref(), cx)?;
    let result = (|| {
        let mut style = text.style_ref().clone();
        ensure!(
            !style.id.is_empty(),
            "node id is required for text nodes before rendering"
        );
        apply_mutation_stack(&mut style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style);

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::Text(ElementText {
                text: text.content().to_string(),
                text_style: computed.text,
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

fn resolve_video(
    video: &Video,
    cx: &mut ResolveContext<'_>,
    media: &mut MediaContext,
) -> Result<ElementNode> {
    let pushed = push_script_scope(video.style_ref(), cx)?;
    let result = (|| {
        let mut style = video.style_ref().clone();
        ensure!(
            !style.id.is_empty(),
            "node id is required for video nodes before rendering"
        );
        apply_mutation_stack(&mut style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style);

        let info = media
            .video_info(video.source())
            .unwrap_or_else(|_| crate::media::VideoInfo {
                width: 0,
                height: 0,
            });

        let asset_id = cx
            .assets
            .register_dimensions(video.source(), info.width, info.height);

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::Bitmap(ElementBitmap {
                asset_id,
                width: info.width,
                height: info.height,
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
    _media: &mut MediaContext,
) -> Result<ElementNode> {
    let pushed = push_script_scope(image.style_ref(), cx)?;
    let result = (|| {
        let mut style = image.style_ref().clone();
        ensure!(
            !style.id.is_empty(),
            "node id is required for image nodes before rendering"
        );
        apply_mutation_stack(&mut style, cx.mutation_stack);
        let computed = compute_style(&style, cx.inherited_style);

        let asset_id = cx.assets.register_image_source(image.source())?;
        let (width, height) = cx.assets.dimensions(&asset_id);

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::Bitmap(ElementBitmap {
                asset_id,
                width,
                height,
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

fn resolve_lucide(lucide: &Lucide, cx: &mut ResolveContext<'_>) -> Result<ElementNode> {
    let pushed = push_script_scope(lucide.style_ref(), cx)?;
    let result = (|| {
        let mut style = lucide.style_ref().clone();
        ensure!(
            !style.id.is_empty(),
            "node id is required for lucide nodes before rendering"
        );
        apply_mutation_stack(&mut style, cx.mutation_stack);
        ensure_valid_lucide_icon(lucide.icon())?;
        let computed = compute_style(&style, cx.inherited_style);

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::Lucide(ElementLucide {
                icon: lucide.icon().to_string(),
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

fn ensure_valid_lucide_icon(name: &str) -> Result<()> {
    if crate::lucide_icons::lucide_icon_paths(name).is_some() {
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
    let mut scored: Vec<(usize, &'static str)> = crate::lucide_icons::lucide_icon_names()
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

    let mutations = cx
        .script_runtime
        .run(driver, cx.frame_ctx.frame, cx.frame_ctx.frames)?;
    if mutations.is_empty() {
        return Ok(false);
    }

    cx.mutation_stack.push(mutations);
    Ok(true)
}

fn apply_mutation_stack(style: &mut NodeStyle, stack: &[StyleMutations]) {
    let id = style.id.clone();
    for mutations in stack {
        mutations.apply_to_node(style, &id);
    }
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
            height: style.height,
            width_full: style.width_full,
            height_full: style.height_full,
            padding_x: style.padding_x.or(style.padding).unwrap_or(0.0),
            padding_y: style.padding_y.or(style.padding).unwrap_or(0.0),
            margin_x: style.margin_x.or(style.margin).unwrap_or(0.0),
            margin_y: style.margin_y.or(style.margin).unwrap_or(0.0),
            flex_direction: style.flex_direction.unwrap_or_default(),
            justify_content: style.justify_content.unwrap_or_default(),
            align_items: style.align_items.unwrap_or_default(),
            gap: style.gap.unwrap_or(0.0),
            flex_grow: style.flex_grow.unwrap_or(0.0),
        },
        visual: ComputedVisualStyle {
            opacity: style.opacity.unwrap_or(1.0),
            background: style.bg_color,
            border_radius: style.border_radius.unwrap_or(0.0),
            border_width: style.border_width,
            border_color: style.border_color,
            object_fit: style.object_fit.unwrap_or_default(),
            transforms: style.transforms.clone(),
            shadow: style.shadow,
        },
        text,
        id: style.id.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_ui_tree, resolve_ui_tree_with_script_cache};
    use crate::{
        FrameCtx,
        assets::AssetsMap,
        media::MediaContext,
        nodes::{div, lucide, text},
        script::ScriptRuntimeCache,
        timeline::{FrameState, frame_state_for_root},
        transitions::{linear, slide, transition_series},
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
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();

        let err = resolve_ui_tree(&div().into(), &frame_ctx, &mut media, &mut assets, None)
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
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();

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

        let resolved = resolve_ui_tree(&scene.into(), &frame_ctx, &mut media, &mut assets, None)
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
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();

        let root = div()
            .id("root")
            .text_blue()
            .font_bold()
            .line_height(1.8)
            .child(text("A").id("label"))
            .child(lucide("play").id("icon").size(24.0, 24.0));

        let resolved = resolve_ui_tree(&root.into(), &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");

        assert_eq!(
            resolved.children[0].style.text.color,
            crate::style::ColorToken::Blue
        );
        assert_eq!(
            resolved.children[0].style.text.font_weight,
            crate::style::FontWeight::Bold
        );
        assert_eq!(resolved.children[0].style.text.line_height, 1.8);
        assert_eq!(
            resolved.children[1].style.text.color,
            crate::style::ColorToken::Blue
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
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();

        let root = div()
            .id("root")
            .bg_red()
            .border_w(3.0)
            .border_blue()
            .child(text("A").id("label"))
            .child(lucide("play").id("icon").size(24.0, 24.0));

        let resolved = resolve_ui_tree(&root.into(), &frame_ctx, &mut media, &mut assets, None)
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
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();

        let root = div()
            .id("root")
            .opacity(0.4)
            .rotate_deg(12.0)
            .child(text("A").id("label"));

        let resolved = resolve_ui_tree(&root.into(), &frame_ctx, &mut media, &mut assets, None)
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
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();

        let root = div()
            .id("root")
            .child(lucide("pla").id("icon").size(24.0, 24.0));

        let err = resolve_ui_tree(&root.into(), &frame_ctx, &mut media, &mut assets, None)
            .expect_err("unknown icon should fail during resolution");

        let message = err.to_string();
        assert!(message.contains("unknown lucide icon `pla`"));
        assert!(message.contains("play"));
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
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();
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
        let root = transition_series()
            .sequence(10, from_scene.into())
            .transition(slide().timing(linear().duration(10)))
            .sequence(10, to_scene.into())
            .into();

        let FrameState::Transition { from, to, .. } = frame_state_for_root(&root, &frame_ctx)
        else {
            panic!("expected transition frame");
        };

        let from_resolved = resolve_ui_tree_with_script_cache(
            &from,
            &frame_ctx,
            &mut media,
            &mut assets,
            None,
            &mut script_runtime,
        )
        .expect("from scene should resolve");
        let to_resolved = resolve_ui_tree_with_script_cache(
            &to,
            &frame_ctx,
            &mut media,
            &mut assets,
            None,
            &mut script_runtime,
        )
        .expect("to scene should resolve");

        assert_eq!(from_resolved.children[0].style.visual.opacity, 0.2);
        assert_eq!(to_resolved.children[0].style.visual.opacity, 0.8);
    }
}
