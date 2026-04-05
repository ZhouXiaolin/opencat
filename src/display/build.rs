use anyhow::Result;

use crate::{
    display::list::{
        BitmapDisplayItem, DisplayCommand, DisplayItem, DisplayLayer, DisplayList,
        DisplayTransform, DisplayTransitionCommand, RectDisplayItem, RectPaintStyle,
        TextDisplayItem,
    },
    frame_ctx::FrameCtx,
    layout::tree::{LayoutNode, LayoutPaintKind, LayoutRect, LayoutTree},
};

pub fn build_display_list(layout_tree: &LayoutTree, frame_ctx: &FrameCtx) -> Result<DisplayList> {
    let mut list = DisplayList::default();
    build_layout_node_display_list(&layout_tree.root, frame_ctx, &mut list)?;
    Ok(list)
}

fn build_layout_node_display_list(
    layout: &LayoutNode,
    frame_ctx: &FrameCtx,
    list: &mut DisplayList,
) -> Result<()> {
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

    push_paint_commands(layout, rect, frame_ctx, list)?;

    for child in &layout.children {
        build_layout_node_display_list(child, frame_ctx, list)?;
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
    frame_ctx: &FrameCtx,
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
            }),
        }),
        LayoutPaintKind::Transition(t) => {
            let mut from_list = DisplayList::default();
            build_layout_node_display_list(&t.from, frame_ctx, &mut from_list)?;

            let mut to_list = DisplayList::default();
            build_layout_node_display_list(&t.to, frame_ctx, &mut to_list)?;

            list.push(DisplayCommand::Transition {
                transition: DisplayTransitionCommand {
                    from: from_list,
                    to: to_list,
                    progress: t.progress,
                    kind: t.kind,
                },
            });
        }
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
                        transforms: Vec::<Transform>::new(),
                        shadow: None,
                    },
                    kind: LayoutPaintKind::Bitmap(LayoutBitmapPaint {
                        asset_id: AssetId("test://fake".to_string()),
                        width: 2,
                        height: 2,
                        object_fit: ObjectFit::Cover,
                    }),
                    data_id: None,
                },
                children: Vec::new(),
            },
        };

        let frame_ctx = crate::FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let list = build_display_list(&layout_tree, &frame_ctx).expect("display list should build");
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
    }
}
