use crate::{
    FrameCtx, Node,
    element::{
        style::{ComputedLayoutStyle, ComputedStyle, ComputedVisualStyle},
        tree::{ElementDiv, ElementId, ElementKind, ElementNode, ElementText},
    },
    nodes::{Div, Text},
    style::{ComputedTextStyle, NodeStyle, resolve_text_style},
    view::ComponentNode,
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
}

pub fn resolve_ui_tree(node: &Node, frame_ctx: &FrameCtx) -> ElementNode {
    let mut ids = ElementIdAllocator::default();
    let inherited_text = ComputedTextStyle::default();
    let mut cx = ResolveContext {
        frame_ctx,
        ids: &mut ids,
        inherited_text: &inherited_text,
    };
    resolve_node(node, &mut cx)
}

fn resolve_node(node: &Node, cx: &mut ResolveContext<'_>) -> ElementNode {
    if let Some(component) = node.as_any().downcast_ref::<ComponentNode>() {
        return resolve_component(component, cx);
    }

    if let Some(div) = node.as_any().downcast_ref::<Div>() {
        return resolve_div(div, cx);
    }

    if let Some(text) = node.as_any().downcast_ref::<Text>() {
        return resolve_text(text, cx);
    }

    panic!("unknown node type encountered while resolving UI tree");
}

fn resolve_component(component: &ComponentNode, cx: &mut ResolveContext<'_>) -> ElementNode {
    let resolved = component.render(cx.frame_ctx);
    resolve_node(&resolved, cx)
}

fn resolve_div(div: &Div, cx: &mut ResolveContext<'_>) -> ElementNode {
    let computed = compute_style(div.style_ref(), cx.inherited_text);
    let mut children = Vec::new();
    for child in div.children_ref() {
        let mut child_cx = ResolveContext {
            frame_ctx: cx.frame_ctx,
            ids: cx.ids,
            inherited_text: &computed.text,
        };
        children.push(resolve_node(child, &mut child_cx));
    }

    ElementNode {
        id: cx.ids.alloc(),
        kind: ElementKind::Div(ElementDiv),
        style: computed,
        children,
    }
}

fn resolve_text(text: &Text, cx: &mut ResolveContext<'_>) -> ElementNode {
    let computed = compute_style(text.style_ref(), cx.inherited_text);

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
            transforms: style.transforms.clone(),
        },
        text,
    }
}
