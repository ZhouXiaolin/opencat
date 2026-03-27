use anyhow::Result;

use crate::{
    display::list::{
        DisplayCommand, DisplayItem, DisplayLayer, DisplayList, DisplayTransform, RectDisplayItem,
        RectPaintStyle, TextDisplayItem,
    },
    layout::tree::{LayoutNode, LayoutPaintKind, LayoutRect, LayoutTree},
};

pub fn build_display_list(layout_tree: &LayoutTree) -> Result<DisplayList> {
    let mut list = DisplayList::default();
    build_layout_node_display_list(&layout_tree.root, &mut list)?;
    Ok(list)
}

fn build_layout_node_display_list(layout: &LayoutNode, list: &mut DisplayList) -> Result<()> {
    if layout.paint.visual.opacity <= 0.0 {
        return Ok(());
    }

    let rect = LayoutRect {
        x: 0.0,
        y: 0.0,
        width: layout.rect.width,
        height: layout.rect.height,
    };

    list.push(DisplayCommand::Save);
    list.push(DisplayCommand::ApplyTransform {
        transform: DisplayTransform {
            translation_x: layout.rect.x,
            translation_y: layout.rect.y,
            bounds: rect,
            transforms: layout.paint.visual.transforms.clone(),
        },
    });

    let uses_layer = layout.paint.visual.opacity < 1.0;
    if uses_layer {
        list.push(DisplayCommand::SaveLayer {
            layer: DisplayLayer {
                bounds: rect,
                opacity: layout.paint.visual.opacity,
            },
        });
    }

    push_paint_commands(layout, rect, list);

    for child in &layout.children {
        build_layout_node_display_list(child, list)?;
    }

    if uses_layer {
        list.push(DisplayCommand::Restore);
    }

    list.push(DisplayCommand::Restore);
    Ok(())
}

fn push_paint_commands(layout: &LayoutNode, rect: LayoutRect, list: &mut DisplayList) {
    match &layout.paint.kind {
        LayoutPaintKind::Div => list.push(DisplayCommand::Draw {
            item: DisplayItem::Rect(RectDisplayItem {
                bounds: rect,
                paint: RectPaintStyle {
                    background: layout.paint.visual.background,
                    border_radius: layout.paint.visual.border_radius,
                    border_width: layout.paint.visual.border_width,
                    border_color: layout.paint.visual.border_color,
                },
            }),
        }),
        LayoutPaintKind::Text(text) => list.push(DisplayCommand::Draw {
            item: DisplayItem::Text(TextDisplayItem {
                bounds: rect,
                text: text.text.clone(),
                style: text.style,
            }),
        }),
    }
}
