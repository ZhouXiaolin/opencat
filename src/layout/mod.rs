pub mod tree;

use anyhow::Result;
use taffy::{
    prelude::{Dimension, JustifyContent as TaffyJustifyContent, Style},
    AvailableSpace, TaffyTree,
};

use crate::{
    element::tree::{ElementKind, ElementNode},
    layout::tree::{
        LayoutBitmapPaint, LayoutNode, LayoutPaint, LayoutPaintKind, LayoutRect, LayoutTextPaint,
        LayoutTransitionPaint, LayoutTree,
    },
    nodes::{AlignItems, JustifyContent, Position},
    typography, FrameCtx,
};

pub fn compute_layout(root: &ElementNode, frame_ctx: &FrameCtx) -> Result<LayoutTree> {
    let mut taffy = TaffyTree::new();
    let root_id = build_taffy_subtree(&mut taffy, root)?;

    taffy.compute_layout(
        root_id,
        taffy::geometry::Size {
            width: AvailableSpace::Definite(frame_ctx.width as f32),
            height: AvailableSpace::Definite(frame_ctx.height as f32),
        },
    )?;

    Ok(LayoutTree {
        root: build_layout_tree(root, &taffy, root_id, frame_ctx)?,
    })
}

fn build_taffy_subtree(taffy: &mut TaffyTree<()>, element: &ElementNode) -> Result<taffy::NodeId> {
    let mut children = Vec::new();
    for child in &element.children {
        children.push(build_taffy_subtree(taffy, child)?);
    }

    let layout = &element.style.layout;
    let position = layout.position;
    let size = if position == Position::Absolute {
        taffy::geometry::Size {
            width: layout
                .width
                .map(Dimension::length)
                .unwrap_or(Dimension::auto()),
            height: layout
                .height
                .map(Dimension::length)
                .unwrap_or(Dimension::auto()),
        }
    } else {
        taffy::geometry::Size {
            width: layout
                .width
                .map(Dimension::length)
                .unwrap_or(Dimension::percent(1.0)),
            height: layout
                .height
                .map(Dimension::length)
                .unwrap_or(Dimension::percent(1.0)),
        }
    };

    let style = match &element.kind {
        ElementKind::Div(_) => Style {
            display: taffy::prelude::Display::Flex,
            position: map_position(position),
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
            size,
            padding: taffy::geometry::Rect {
                left: taffy::style::LengthPercentage::length(layout.padding_x),
                top: taffy::style::LengthPercentage::length(layout.padding_y),
                right: taffy::style::LengthPercentage::length(layout.padding_x),
                bottom: taffy::style::LengthPercentage::length(layout.padding_y),
            },
            margin: taffy::geometry::Rect {
                left: taffy::style::LengthPercentageAuto::length(layout.margin_x),
                top: taffy::style::LengthPercentageAuto::length(layout.margin_y),
                right: taffy::style::LengthPercentageAuto::length(layout.margin_x),
                bottom: taffy::style::LengthPercentageAuto::length(layout.margin_y),
            },
            flex_direction: map_flex_direction(Some(layout.flex_direction)),
            justify_content: Some(map_justify(layout.justify_content)),
            align_items: Some(map_align(layout.align_items)),
            gap: taffy::geometry::Size {
                width: taffy::style::LengthPercentage::length(layout.gap),
                height: taffy::style::LengthPercentage::length(layout.gap),
            },
            flex_grow: layout.flex_grow,
            ..Default::default()
        },
        ElementKind::Text(text) => {
            let measured = typography::measure_text(&text.text, &text.text_style);
            Style {
                flex_grow: layout.flex_grow,
                size: taffy::geometry::Size {
                    width: Dimension::length(measured.0),
                    height: Dimension::length(measured.1),
                },
                ..Default::default()
            }
        }
        ElementKind::Bitmap(bitmap) => Style {
            size: taffy::geometry::Size {
                width: layout
                    .width
                    .map(Dimension::length)
                    .unwrap_or(Dimension::length(bitmap.width as f32)),
                height: layout
                    .height
                    .map(Dimension::length)
                    .unwrap_or(Dimension::length(bitmap.height as f32)),
            },
            ..Default::default()
        },
        ElementKind::Transition(_) => Style {
            size: taffy::geometry::Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            ..Default::default()
        },
    };

    let id = if children.is_empty() {
        taffy.new_leaf(style)?
    } else {
        taffy.new_with_children(style, &children)?
    };
    Ok(id)
}

fn build_layout_tree(
    element: &ElementNode,
    taffy: &TaffyTree<()>,
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
