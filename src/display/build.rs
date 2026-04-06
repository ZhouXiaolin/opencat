use anyhow::Result;

use crate::{
    display::list::{
        BitmapDisplayItem, BitmapPaintStyle, DisplayCommand, DisplayItem, DisplayLayer,
        DisplayList, DisplayTransform, LucideDisplayItem, LucidePaintStyle, RectDisplayItem,
        RectPaintStyle, TextDisplayItem,
    },
    layout::tree::{LayoutNode, LayoutPaintKind, LayoutRect, LayoutTree},
};

pub fn build_display_list(layout_tree: &LayoutTree) -> Result<DisplayList> {
    let mut list = DisplayList::default();
    build_layout_node_display_list(&layout_tree.root, &mut list)?;
    Ok(list)
}

fn sorted_children_by_z_index(children: &[LayoutNode]) -> Vec<&LayoutNode> {
    let mut sorted = children.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|child| child.paint.z_index);
    sorted
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

    push_paint_commands(layout, rect, list)?;

    for child in sorted_children_by_z_index(&layout.children) {
        build_layout_node_display_list(child, list)?;
    }

    if uses_layer {
        list.push(DisplayCommand::Restore);
    }

    list.push(DisplayCommand::Restore);
    Ok(())
}

fn push_paint_commands(
    layout: &LayoutNode,
    rect: LayoutRect,
    list: &mut DisplayList,
) -> Result<()> {
    match &layout.paint.kind {
        LayoutPaintKind::Div => list.push(DisplayCommand::Draw {
            item: DisplayItem::Rect(RectDisplayItem {
                bounds: rect,
                paint: RectPaintStyle {
                    background: layout.paint.visual.background,
                    border_radius: layout.paint.visual.border_radius,
                    border_width: layout.paint.visual.border_width,
                    border_color: layout.paint.visual.border_color,
                    shadow: layout.paint.visual.shadow,
                },
            }),
        }),
        LayoutPaintKind::Text(text) => list.push(DisplayCommand::Draw {
            item: DisplayItem::Text(TextDisplayItem {
                bounds: rect,
                text: text.text.clone(),
                style: text.style,
                allow_wrap: text.allow_wrap,
            }),
        }),
        LayoutPaintKind::Bitmap(bitmap) => list.push(DisplayCommand::Draw {
            item: DisplayItem::Bitmap(BitmapDisplayItem {
                bounds: rect,
                asset_id: bitmap.asset_id.clone(),
                width: bitmap.width,
                height: bitmap.height,
                object_fit: bitmap.object_fit,
                paint: BitmapPaintStyle {
                    background: layout.paint.visual.background,
                    border_radius: layout.paint.visual.border_radius,
                    border_width: layout.paint.visual.border_width,
                    border_color: layout.paint.visual.border_color,
                    shadow: layout.paint.visual.shadow,
                },
            }),
        }),
        LayoutPaintKind::Lucide(lucide) => list.push(DisplayCommand::Draw {
            item: DisplayItem::Lucide(LucideDisplayItem {
                bounds: rect,
                icon: lucide.icon.clone(),
                paint: LucidePaintStyle {
                    foreground: lucide.foreground,
                    background: layout.paint.visual.background,
                    border_width: layout.paint.visual.border_width,
                    border_color: layout.paint.visual.border_color,
                },
            }),
        }),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::build_display_list;
    use crate::{
        assets::AssetId,
        display::list::DisplayItem,
        layout::tree::{
            LayoutBitmapPaint, LayoutNode, LayoutPaint, LayoutPaintKind, LayoutRect, LayoutTree,
        },
        style::{ObjectFit, Transform},
    };

    #[test]
    fn bitmap_display_item_preserves_object_fit() {
        let layout_tree = LayoutTree {
            root: LayoutNode {
                rect: LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 320.0,
                    height: 180.0,
                },
                paint: LayoutPaint {
                    visual: crate::element::style::ComputedVisualStyle {
                        opacity: 1.0,
                        background: None,
                        border_radius: 0.0,
                        border_width: None,
                        border_color: None,
                        object_fit: ObjectFit::Contain,
                        clip_contents: false,
                        transforms: Vec::<Transform>::new(),
                        shadow: None,
                    },
                    kind: LayoutPaintKind::Bitmap(LayoutBitmapPaint {
                        asset_id: AssetId("test://fake".to_string()),
                        width: 2,
                        height: 2,
                        object_fit: ObjectFit::Cover,
                    }),
                    id: "test://fake".to_string(),
                    z_index: 0,
                },
                children: Vec::new(),
            },
        };

        let list = build_display_list(&layout_tree).expect("display list should build");
        let bitmap = list
            .commands
            .iter()
            .find_map(|command| match command {
                crate::display::list::DisplayCommand::Draw {
                    item: DisplayItem::Bitmap(bitmap),
                } => Some(bitmap),
                _ => None,
            })
            .expect("bitmap draw item should exist");

        assert_eq!(bitmap.object_fit, ObjectFit::Cover);
        assert_eq!(bitmap.paint.border_radius, 0.0);
    }

    #[test]
    fn display_list_sorts_children_by_z_index_for_painting() {
        let text_style = crate::style::ComputedTextStyle::default();
        let layout_tree = LayoutTree {
            root: LayoutNode {
                rect: LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 320.0,
                    height: 180.0,
                },
                paint: LayoutPaint {
                    visual: crate::element::style::ComputedVisualStyle {
                        opacity: 1.0,
                        background: None,
                        border_radius: 0.0,
                        border_width: None,
                        border_color: None,
                        object_fit: ObjectFit::Contain,
                        clip_contents: false,
                        transforms: Vec::<Transform>::new(),
                        shadow: None,
                    },
                    kind: LayoutPaintKind::Div,
                    id: "root".to_string(),
                    z_index: 0,
                },
                children: vec![
                    LayoutNode {
                        rect: LayoutRect {
                            x: 0.0,
                            y: 0.0,
                            width: 50.0,
                            height: 20.0,
                        },
                        paint: LayoutPaint {
                            visual: crate::element::style::ComputedVisualStyle {
                                opacity: 1.0,
                                background: None,
                                border_radius: 0.0,
                                border_width: None,
                                border_color: None,
                                object_fit: ObjectFit::Contain,
                                clip_contents: false,
                                transforms: Vec::<Transform>::new(),
                                shadow: None,
                            },
                            kind: LayoutPaintKind::Text(crate::layout::tree::LayoutTextPaint {
                                text: "front".to_string(),
                                style: text_style,
                                allow_wrap: false,
                            }),
                            id: "front".to_string(),
                            z_index: 10,
                        },
                        children: Vec::new(),
                    },
                    LayoutNode {
                        rect: LayoutRect {
                            x: 0.0,
                            y: 0.0,
                            width: 50.0,
                            height: 20.0,
                        },
                        paint: LayoutPaint {
                            visual: crate::element::style::ComputedVisualStyle {
                                opacity: 1.0,
                                background: None,
                                border_radius: 0.0,
                                border_width: None,
                                border_color: None,
                                object_fit: ObjectFit::Contain,
                                clip_contents: false,
                                transforms: Vec::<Transform>::new(),
                                shadow: None,
                            },
                            kind: LayoutPaintKind::Text(crate::layout::tree::LayoutTextPaint {
                                text: "back".to_string(),
                                style: text_style,
                                allow_wrap: false,
                            }),
                            id: "back".to_string(),
                            z_index: 0,
                        },
                        children: Vec::new(),
                    },
                ],
            },
        };

        let list = build_display_list(&layout_tree).expect("display list should build");
        let texts = list
            .commands
            .iter()
            .filter_map(|command| match command {
                crate::display::list::DisplayCommand::Draw {
                    item: DisplayItem::Text(text),
                } => Some(text.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(texts, vec!["back", "front"]);
    }
}
