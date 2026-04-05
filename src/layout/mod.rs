pub mod tree;

use anyhow::Result;
use taffy::{
    AvailableSpace, TaffyTree,
    prelude::{Dimension, JustifyContent as TaffyJustifyContent, Style},
};

use crate::{
    FrameCtx,
    element::{
        style::ComputedLayoutStyle,
        tree::{ElementKind, ElementNode},
    },
    layout::tree::{
        LayoutBitmapPaint, LayoutNode, LayoutPaint, LayoutPaintKind, LayoutRect, LayoutTextPaint,
        LayoutTransitionPaint, LayoutTree,
    },
    nodes::{AlignItems, JustifyContent, Position},
    style::ComputedTextStyle,
    typography,
};

#[derive(Clone)]
struct TextMeasureContext {
    text: String,
    style: ComputedTextStyle,
}

pub fn compute_layout(root: &ElementNode, frame_ctx: &FrameCtx) -> Result<LayoutTree> {
    let mut taffy: TaffyTree<TextMeasureContext> = TaffyTree::new();
    let root_id = build_taffy_subtree(&mut taffy, root)?;

    taffy.compute_layout_with_measure(
        root_id,
        taffy::geometry::Size {
            width: AvailableSpace::Definite(frame_ctx.width as f32),
            height: AvailableSpace::Definite(frame_ctx.height as f32),
        },
        |known_dimensions, available_space, _node_id, node_context, _style| {
            measure_node(known_dimensions, available_space, node_context)
        },
    )?;

    Ok(LayoutTree {
        root: build_layout_tree(root, &taffy, root_id, frame_ctx)?,
    })
}

fn measure_node(
    known_dimensions: taffy::geometry::Size<Option<f32>>,
    available_space: taffy::geometry::Size<AvailableSpace>,
    node_context: Option<&mut TextMeasureContext>,
) -> taffy::geometry::Size<f32> {
    let Some(text) = node_context else {
        return taffy::geometry::Size::ZERO;
    };

    let max_width = known_dimensions
        .width
        .or_else(|| match available_space.width {
            AvailableSpace::Definite(width) => Some(width),
            AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
        })
        .unwrap_or(f32::INFINITY);

    let measured = typography::measure_text_in_width(&text.text, &text.style, max_width);

    taffy::geometry::Size {
        width: known_dimensions.width.unwrap_or(measured.0),
        height: known_dimensions.height.unwrap_or(measured.1),
    }
}

fn build_taffy_subtree(
    taffy: &mut TaffyTree<TextMeasureContext>,
    element: &ElementNode,
) -> Result<taffy::NodeId> {
    let mut children = Vec::new();
    for child in &element.children {
        children.push(build_taffy_subtree(taffy, child)?);
    }

    let layout = &element.style.layout;
    let style = match &element.kind {
        ElementKind::Div(_) => Style {
            display: taffy::prelude::Display::Flex,
            size: match layout.position {
                Position::Absolute => taffy::geometry::Size {
                    width: resolve_dimension(layout.width, layout.width_full, Dimension::auto()),
                    height: resolve_dimension(layout.height, layout.height_full, Dimension::auto()),
                },
                Position::Relative => taffy::geometry::Size {
                    width: resolve_dimension(
                        layout.width,
                        layout.width_full,
                        Dimension::percent(1.0),
                    ),
                    height: resolve_dimension(
                        layout.height,
                        layout.height_full,
                        Dimension::percent(1.0),
                    ),
                },
            },
            padding: taffy::geometry::Rect {
                left: taffy::style::LengthPercentage::length(layout.padding_x),
                top: taffy::style::LengthPercentage::length(layout.padding_y),
                right: taffy::style::LengthPercentage::length(layout.padding_x),
                bottom: taffy::style::LengthPercentage::length(layout.padding_y),
            },
            flex_direction: map_flex_direction(Some(layout.flex_direction)),
            justify_content: Some(map_justify(layout.justify_content)),
            align_items: Some(map_align(layout.align_items)),
            gap: taffy::geometry::Size {
                width: taffy::style::LengthPercentage::length(layout.gap),
                height: taffy::style::LengthPercentage::length(layout.gap),
            },
            ..base_style(layout)
        },
        ElementKind::Text(_text) => Style {
            size: taffy::geometry::Size {
                width: resolve_dimension(layout.width, layout.width_full, Dimension::percent(1.0)),
                height: resolve_dimension(layout.height, layout.height_full, Dimension::auto()),
            },
            ..base_style(layout)
        },
        ElementKind::Bitmap(bitmap) => Style {
            size: taffy::geometry::Size {
                width: resolve_dimension(
                    layout.width,
                    layout.width_full,
                    Dimension::length(bitmap.width as f32),
                ),
                height: resolve_dimension(
                    layout.height,
                    layout.height_full,
                    Dimension::length(bitmap.height as f32),
                ),
            },
            ..base_style(layout)
        },
        ElementKind::Transition(_) => Style {
            size: taffy::geometry::Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            ..base_style(layout)
        },
    };

    let id = match &element.kind {
        ElementKind::Text(text) => taffy.new_leaf_with_context(
            style,
            TextMeasureContext {
                text: text.text.clone(),
                style: text.text_style,
            },
        )?,
        _ if children.is_empty() => taffy.new_leaf(style)?,
        _ => taffy.new_with_children(style, &children)?,
    };
    Ok(id)
}

fn build_layout_tree(
    element: &ElementNode,
    taffy: &TaffyTree<TextMeasureContext>,
    node_id: taffy::NodeId,
    frame_ctx: &FrameCtx,
) -> Result<LayoutNode> {
    let layout = taffy.layout(node_id)?;
    let mut children = Vec::new();
    let taffy_children = taffy.children(node_id)?;

    for (element_child, taffy_child) in element.children.iter().zip(taffy_children.into_iter()) {
        children.push(build_layout_tree(
            element_child,
            taffy,
            taffy_child,
            frame_ctx,
        )?);
    }

    Ok(LayoutNode {
        rect: LayoutRect {
            x: layout.location.x,
            y: layout.location.y,
            width: layout.size.width,
            height: layout.size.height,
        },
        paint: LayoutPaint {
            visual: element.style.visual.clone(),
            kind: match &element.kind {
                ElementKind::Div(_) => LayoutPaintKind::Div,
                ElementKind::Text(text) => LayoutPaintKind::Text(LayoutTextPaint {
                    text: text.text.clone(),
                    style: text.text_style,
                }),
                ElementKind::Bitmap(bitmap) => LayoutPaintKind::Bitmap(LayoutBitmapPaint {
                    asset_id: bitmap.asset_id.clone(),
                    width: bitmap.width,
                    height: bitmap.height,
                    object_fit: element.style.visual.object_fit,
                }),
                ElementKind::Transition(t) => {
                    let from_tree = compute_layout(&t.from, frame_ctx)?;
                    let to_tree = compute_layout(&t.to, frame_ctx)?;
                    LayoutPaintKind::Transition(LayoutTransitionPaint {
                        from: Box::new(from_tree.root),
                        to: Box::new(to_tree.root),
                        progress: t.progress,
                        kind: t.kind,
                    })
                }
            },
            data_id: element.style.data_id.clone(),
        },
        children,
    })
}

fn base_style(layout: &ComputedLayoutStyle) -> Style {
    Style {
        position: map_position(layout.position),
        inset: taffy::geometry::Rect {
            left: layout
                .inset_left
                .map(taffy::style::LengthPercentageAuto::length)
                .unwrap_or(taffy::style::LengthPercentageAuto::auto()),
            top: layout
                .inset_top
                .map(taffy::style::LengthPercentageAuto::length)
                .unwrap_or(taffy::style::LengthPercentageAuto::auto()),
            right: layout
                .inset_right
                .map(taffy::style::LengthPercentageAuto::length)
                .unwrap_or(taffy::style::LengthPercentageAuto::auto()),
            bottom: layout
                .inset_bottom
                .map(taffy::style::LengthPercentageAuto::length)
                .unwrap_or(taffy::style::LengthPercentageAuto::auto()),
        },
        margin: taffy::geometry::Rect {
            left: taffy::style::LengthPercentageAuto::length(layout.margin_x),
            top: taffy::style::LengthPercentageAuto::length(layout.margin_y),
            right: taffy::style::LengthPercentageAuto::length(layout.margin_x),
            bottom: taffy::style::LengthPercentageAuto::length(layout.margin_y),
        },
        flex_grow: layout.flex_grow,
        ..Default::default()
    }
}

fn resolve_dimension(value: Option<f32>, full: bool, fallback: Dimension) -> Dimension {
    if full {
        Dimension::percent(1.0)
    } else {
        value.map(Dimension::length).unwrap_or(fallback)
    }
}

fn map_flex_direction(value: Option<crate::style::FlexDirection>) -> taffy::prelude::FlexDirection {
    match value {
        None | Some(crate::style::FlexDirection::Row) => taffy::prelude::FlexDirection::Row,
        Some(crate::style::FlexDirection::Col) => taffy::prelude::FlexDirection::Column,
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
        JustifyContent::Start => TaffyJustifyContent::FlexStart,
        JustifyContent::Center => TaffyJustifyContent::Center,
        JustifyContent::End => TaffyJustifyContent::FlexEnd,
        JustifyContent::Between => TaffyJustifyContent::SpaceBetween,
        JustifyContent::Around => TaffyJustifyContent::SpaceAround,
        JustifyContent::Evenly => TaffyJustifyContent::SpaceEvenly,
    }
}

fn map_align(value: AlignItems) -> taffy::prelude::AlignItems {
    match value {
        AlignItems::Start => taffy::prelude::AlignItems::FlexStart,
        AlignItems::Center => taffy::prelude::AlignItems::Center,
        AlignItems::End => taffy::prelude::AlignItems::FlexEnd,
        AlignItems::Stretch => taffy::prelude::AlignItems::Stretch,
    }
}
