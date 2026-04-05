use crate::{
    FrameCtx, Node,
    assets::AssetsMap,
    element::{
        style::{ComputedLayoutStyle, ComputedStyle, ComputedVisualStyle},
        tree::{
            ElementBitmap, ElementDiv, ElementId, ElementKind, ElementNode, ElementText,
            TransitionElement,
        },
    },
    media::MediaContext,
    nodes::{Div, Image, Text, Video},
    script::StyleMutations,
    style::{ComputedTextStyle, NodeStyle, resolve_text_style},
    transitions::TransitionNode,
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
    inherited_text: &'a ComputedTextStyle,
    assets: &'a mut AssetsMap,
    mutations: Option<&'a StyleMutations>,
}

pub fn resolve_ui_tree(
    node: &Node,
    frame_ctx: &FrameCtx,
    media: &mut MediaContext,
    assets: &mut AssetsMap,
    mutations: Option<&StyleMutations>,
) -> ElementNode {
    let mut ids = ElementIdAllocator::default();
    let inherited_text = ComputedTextStyle::default();
    let mut cx = ResolveContext {
        frame_ctx,
        ids: &mut ids,
        inherited_text: &inherited_text,
        assets,
        mutations,
    };
    resolve_node(node, &mut cx, media)
}

fn resolve_node(node: &Node, cx: &mut ResolveContext<'_>, media: &mut MediaContext) -> ElementNode {
    match node.kind() {
        NodeKind::Component(component) => resolve_component(component, cx, media),
        NodeKind::Video(video) => resolve_video(video, cx, media),
        NodeKind::Image(image) => resolve_image(image, cx, media),
        NodeKind::Div(div) => resolve_div(div, cx, media),
        NodeKind::Text(text) => resolve_text(text, cx),
        NodeKind::Transition(transition) => resolve_transition(transition, cx, media),
    }
}

fn resolve_component(
    component: &ComponentNode,
    cx: &mut ResolveContext<'_>,
    media: &mut MediaContext,
) -> ElementNode {
    let resolved = component.render(cx.frame_ctx);
    resolve_node(&resolved, cx, media)
}

fn resolve_div(div: &Div, cx: &mut ResolveContext<'_>, media: &mut MediaContext) -> ElementNode {
    let mut style = div.style_ref().clone();
    let data_id = style.data_id.clone();
    if let Some(mutations) = cx.mutations {
        mutations.apply_to_node(&mut style, &data_id);
    }
    let computed = compute_style(&style, cx.inherited_text);
    let mut children = Vec::new();
    for child in div.children_ref() {
        let mut child_cx = ResolveContext {
            frame_ctx: cx.frame_ctx,
            ids: cx.ids,
            inherited_text: &computed.text,
            assets: cx.assets,
            mutations: cx.mutations,
        };
        children.push(resolve_node(child, &mut child_cx, media));
    }

    ElementNode {
        id: cx.ids.alloc(),
        kind: ElementKind::Div(ElementDiv),
        style: computed,
        children,
    }
}

fn resolve_text(text: &Text, cx: &mut ResolveContext<'_>) -> ElementNode {
    let mut style = text.style_ref().clone();
    let data_id = style.data_id.clone();
    if let Some(mutations) = cx.mutations {
        mutations.apply_to_node(&mut style, &data_id);
    }
    let computed = compute_style(&style, cx.inherited_text);

    ElementNode {
        id: cx.ids.alloc(),
        kind: ElementKind::Text(ElementText {
            text: text.content().to_string(),
            text_style: computed.text,
        }),
        style: computed,
        children: Vec::new(),
    }
}

fn resolve_video(
    video: &Video,
    cx: &mut ResolveContext<'_>,
    media: &mut MediaContext,
) -> ElementNode {
    let mut style = video.style_ref().clone();
    let data_id = style.data_id.clone();
    if let Some(mutations) = cx.mutations {
        mutations.apply_to_node(&mut style, &data_id);
    }
    let computed = compute_style(&style, cx.inherited_text);

    let info = media
        .video_info(video.source())
        .unwrap_or_else(|_| crate::media::VideoInfo {
            width: 0,
            height: 0,
        });

    let asset_id = cx
        .assets
        .register_dimensions(video.source(), info.width, info.height);

    ElementNode {
        id: cx.ids.alloc(),
        kind: ElementKind::Bitmap(ElementBitmap {
            asset_id,
            width: info.width,
            height: info.height,
        }),
        style: computed,
        children: Vec::new(),
    }
}

fn resolve_image(
    image: &Image,
    cx: &mut ResolveContext<'_>,
    _media: &mut MediaContext,
) -> ElementNode {
    let mut style = image.style_ref().clone();
    let data_id = style.data_id.clone();
    if let Some(mutations) = cx.mutations {
        mutations.apply_to_node(&mut style, &data_id);
    }
    let computed = compute_style(&style, cx.inherited_text);

    let asset_id = cx.assets.register(image.source());
    let (width, height) = cx.assets.dimensions(&asset_id);

    ElementNode {
        id: cx.ids.alloc(),
        kind: ElementKind::Bitmap(ElementBitmap {
            asset_id,
            width,
            height,
        }),
        style: computed,
        children: Vec::new(),
    }
}

fn resolve_transition(
    transition: &TransitionNode,
    cx: &mut ResolveContext<'_>,
    media: &mut MediaContext,
) -> ElementNode {
    let mut style = transition.style_ref().clone();
    let data_id = style.data_id.clone();
    if let Some(mutations) = cx.mutations {
        mutations.apply_to_node(&mut style, &data_id);
    }
    let from = Box::new(resolve_node(transition.from_node(), cx, media));
    let to = Box::new(resolve_node(transition.to_node(), cx, media));
    let (progress, kind) = transition.params();
    let computed = compute_style(&style, cx.inherited_text);

    ElementNode {
        id: cx.ids.alloc(),
        kind: ElementKind::Transition(TransitionElement {
            from,
            to,
            progress,
            kind,
        }),
        style: computed,
        children: Vec::new(),
    }
}

fn compute_style(style: &NodeStyle, inherited_text: &ComputedTextStyle) -> ComputedStyle {
    let text = resolve_text_style(inherited_text, style);
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
        data_id: style.data_id.clone(),
    }
}
