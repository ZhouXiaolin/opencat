use anyhow::{Result, anyhow};

use crate::{
    display::{
        list::{
            BitmapDisplayItem, BitmapPaintStyle, DisplayClip, DisplayCommand, DisplayItem,
            DisplayLayer, DisplayList, DisplayRect, DisplayTransform, DrawScriptDisplayItem,
            LucideDisplayItem, LucidePaintStyle, RectDisplayItem, RectPaintStyle, TextDisplayItem,
        },
        tree::{DisplayNode, DisplayTree},
    },
    element::tree::{ElementKind, ElementNode},
    layout::tree::{LayoutNode, LayoutTree},
    resource::assets::AssetsMap,
    runtime::fingerprint::{self, PaintVariance},
};

pub fn build_display_tree(
    element_root: &ElementNode,
    layout_tree: &LayoutTree,
    assets: &AssetsMap,
) -> Result<DisplayTree> {
    Ok(DisplayTree {
        root: build_display_node(element_root, &layout_tree.root, assets)?,
    })
}

#[cfg(test)]
pub fn build_display_list(
    element_root: &ElementNode,
    layout_tree: &LayoutTree,
    assets: &AssetsMap,
) -> Result<DisplayList> {
    let tree = build_display_tree(element_root, layout_tree, assets)?;
    Ok(build_display_list_from_tree(&tree))
}

pub fn build_display_list_from_tree(tree: &DisplayTree) -> DisplayList {
    let mut list = DisplayList::default();
    push_display_node_commands(&tree.root, &mut list);
    list
}

fn build_display_node(
    element: &ElementNode,
    layout: &LayoutNode,
    assets: &AssetsMap,
) -> Result<DisplayNode> {
    if element.children.len() != layout.children.len() {
        return Err(anyhow!(
            "element/layout child count mismatch while building display tree"
        ));
    }

    let bounds = DisplayRect {
        x: 0.0,
        y: 0.0,
        width: layout.rect.width,
        height: layout.rect.height,
    };

    let mut child_pairs = element
        .children
        .iter()
        .zip(layout.children.iter())
        .collect::<Vec<_>>();
    child_pairs.sort_by_key(|(child, _)| child.style.layout.z_index);

    let item = display_item_for_node(element, bounds);
    let children = child_pairs
        .into_iter()
        .map(|(child, child_layout)| build_display_node(child, child_layout, assets))
        .collect::<Result<Vec<_>>>()?;

    let paint_variance = fingerprint::classify_paint(&item, assets);
    let subtree_contains_time_variant = matches!(paint_variance, PaintVariance::TimeVariant)
        || children
            .iter()
            .any(|child| child.subtree_contains_time_variant);

    let mut node = DisplayNode {
        transform: DisplayTransform {
            translation_x: layout.rect.x,
            translation_y: layout.rect.y,
            bounds,
            transforms: element.style.visual.transforms.clone(),
        },
        opacity: element.style.visual.opacity,
        backdrop_blur_sigma: element.style.visual.backdrop_blur_sigma,
        clip: element.style.visual.clip_contents.then_some(DisplayClip {
            bounds,
            border_radius: element.style.visual.border_radius,
        }),
        item,
        children,
        paint_variance,
        composite_dirty: false,
        subtree_contains_time_variant,
        subtree_contains_dynamic: subtree_contains_time_variant,
        snapshot_fingerprint: None,
    };

    // 只在整棵子树都是 Stable 时才计算 subtree snapshot fingerprint。
    if !subtree_contains_time_variant {
        node.snapshot_fingerprint = fingerprint::subtree_snapshot_fingerprint(&node, assets);
    }

    Ok(node)
}

fn push_display_node_commands(node: &DisplayNode, list: &mut DisplayList) {
    if node.opacity <= 0.0 {
        return;
    }

    list.push(DisplayCommand::Save);
    list.push(DisplayCommand::ApplyTransform {
        transform: node.transform.clone(),
    });

    if node.opacity < 1.0 || node.backdrop_blur_sigma.is_some() {
        list.push(DisplayCommand::SaveLayer {
            layer: DisplayLayer {
                bounds: node.layer_bounds(),
                opacity: node.opacity,
                backdrop_blur_sigma: node.backdrop_blur_sigma,
            },
        });
    }

    list.push(DisplayCommand::Draw {
        item: node.item.clone(),
    });

    if let Some(clip) = &node.clip {
        list.push(DisplayCommand::Save);
        list.push(DisplayCommand::Clip { clip: clip.clone() });
    }

    for child in &node.children {
        push_display_node_commands(child, list);
    }

    if node.clip.is_some() {
        list.push(DisplayCommand::Restore);
    }

    if node.opacity < 1.0 || node.backdrop_blur_sigma.is_some() {
        list.push(DisplayCommand::Restore);
    }

    list.push(DisplayCommand::Restore);
}

fn display_item_for_node(element: &ElementNode, bounds: DisplayRect) -> DisplayItem {
    match &element.kind {
        ElementKind::Div(_) => DisplayItem::Rect(RectDisplayItem {
            bounds,
            paint: RectPaintStyle {
                background: element.style.visual.background,
                border_radius: element.style.visual.border_radius,
                border_width: element.style.visual.border_width,
                border_color: element.style.visual.border_color,
                blur_sigma: element.style.visual.blur_sigma,
                box_shadow: element.style.visual.box_shadow,
                inset_shadow: element.style.visual.inset_shadow,
                drop_shadow: element.style.visual.drop_shadow,
            },
        }),
        ElementKind::Text(text) => DisplayItem::Text(TextDisplayItem {
            bounds,
            text: text.text.clone(),
            style: text.text_style,
            allow_wrap: element.style.text.wrap_text
                || element.style.layout.width.is_some()
                || element.style.layout.width_full,
            drop_shadow: element.style.visual.drop_shadow,
        }),
        ElementKind::Bitmap(bitmap) => DisplayItem::Bitmap(BitmapDisplayItem {
            bounds,
            asset_id: bitmap.asset_id.clone(),
            width: bitmap.width,
            height: bitmap.height,
            video_timing: bitmap.video_timing,
            object_fit: element.style.visual.object_fit,
            paint: BitmapPaintStyle {
                background: element.style.visual.background,
                border_radius: element.style.visual.border_radius,
                border_width: element.style.visual.border_width,
                border_color: element.style.visual.border_color,
                blur_sigma: element.style.visual.blur_sigma,
                box_shadow: element.style.visual.box_shadow,
                inset_shadow: element.style.visual.inset_shadow,
                drop_shadow: element.style.visual.drop_shadow,
            },
        }),
        ElementKind::Canvas(canvas) => DisplayItem::DrawScript(DrawScriptDisplayItem {
            bounds,
            commands: canvas.commands.clone(),
            drop_shadow: element.style.visual.drop_shadow,
        }),
        ElementKind::Lucide(lucide) => DisplayItem::Lucide(LucideDisplayItem {
            bounds,
            icon: lucide.icon.clone(),
            paint: LucidePaintStyle {
                foreground: element.style.text.color,
                background: element.style.visual.background,
                border_width: element.style.visual.border_width,
                border_color: element.style.visual.border_color,
                drop_shadow: element.style.visual.drop_shadow,
            },
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{build_display_list, build_display_tree};
    use crate::{
        FrameCtx,
        element::resolve::resolve_ui_tree,
        parse,
        resource::assets::AssetsMap,
        resource::media::MediaContext,
        scene::primitives::{div, lucide},
        style::{ColorToken, ObjectFit},
    };
    use crate::{
        display::list::{DisplayCommand, DisplayItem},
        layout::tree::{LayoutNode, LayoutRect, LayoutTree},
    };

    fn simple_layout(id: &str, rect: LayoutRect, children: Vec<LayoutNode>) -> LayoutNode {
        LayoutNode {
            id: id.to_string(),
            rect,
            children,
        }
    }

    #[test]
    fn bitmap_display_item_preserves_object_fit() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();
        let element = div()
            .id("root")
            .child(
                crate::scene::primitives::image()
                    .id("bitmap")
                    .path("/tmp/test-display-bitmap.png")
                    .size(2.0, 2.0)
                    .cover(),
            )
            .into();
        let resolved = resolve_ui_tree(&element, &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");
        let layout_tree = LayoutTree {
            root: simple_layout(
                "root",
                LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 320.0,
                    height: 180.0,
                },
                vec![simple_layout(
                    "bitmap",
                    LayoutRect {
                        x: 0.0,
                        y: 0.0,
                        width: 2.0,
                        height: 2.0,
                    },
                    Vec::new(),
                )],
            ),
        };

        let list = build_display_list(&resolved, &layout_tree, &assets)
            .expect("display list should build");
        let bitmap = list
            .commands
            .iter()
            .find_map(|command| match command {
                DisplayCommand::Draw {
                    item: DisplayItem::Bitmap(bitmap),
                } => Some(bitmap),
                _ => None,
            })
            .expect("bitmap draw item should exist");

        assert_eq!(bitmap.object_fit, ObjectFit::Cover);
        assert_eq!(
            bitmap.paint.border_radius,
            crate::style::BorderRadius::default()
        );
    }

    #[test]
    fn display_list_sorts_children_by_z_index_for_painting() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();
        let parsed = crate::parse(
            r#"{"type":"composition","width":320,"height":180,"fps":30,"frames":1}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"front","parentId":"root","type":"text","className":"text-[12px] z-10","text":"front"}
{"id":"back","parentId":"root","type":"text","className":"text-[12px]","text":"back"}"#,
        )
        .expect("jsonl should parse");
        let resolved = resolve_ui_tree(&parsed.root, &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");
        let layout_tree = LayoutTree {
            root: simple_layout(
                "root",
                LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 320.0,
                    height: 180.0,
                },
                vec![
                    simple_layout(
                        "front",
                        LayoutRect {
                            x: 0.0,
                            y: 0.0,
                            width: 50.0,
                            height: 20.0,
                        },
                        Vec::new(),
                    ),
                    simple_layout(
                        "back",
                        LayoutRect {
                            x: 0.0,
                            y: 0.0,
                            width: 50.0,
                            height: 20.0,
                        },
                        Vec::new(),
                    ),
                ],
            ),
        };

        let list = build_display_list(&resolved, &layout_tree, &assets)
            .expect("display list should build");
        let texts = list
            .commands
            .iter()
            .filter_map(|command| match command {
                DisplayCommand::Draw {
                    item: DisplayItem::Text(text),
                } => Some(text.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(texts, vec!["back", "front"]);
    }

    #[test]
    fn display_list_emits_clip_commands_for_overflow_hidden_nodes() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 40,
            height: 40,
            frames: 1,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();
        let element = div()
            .id("root")
            .rounded(12.0)
            .overflow_hidden()
            .child(div().id("child"))
            .into();
        let resolved = resolve_ui_tree(&element, &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");
        let layout_tree = LayoutTree {
            root: simple_layout(
                "root",
                LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 40.0,
                    height: 40.0,
                },
                vec![simple_layout(
                    "child",
                    LayoutRect {
                        x: 0.0,
                        y: 0.0,
                        width: 40.0,
                        height: 40.0,
                    },
                    Vec::new(),
                )],
            ),
        };

        let list = build_display_list(&resolved, &layout_tree, &assets)
            .expect("display list should build");
        let clip = list.commands.iter().find_map(|command| match command {
            DisplayCommand::Clip { clip } => Some(clip),
            _ => None,
        });

        assert!(clip.is_some());
        assert_eq!(
            clip.expect("clip command should exist").border_radius,
            crate::style::BorderRadius {
                top_left: 12.0,
                top_right: 12.0,
                bottom_right: 12.0,
                bottom_left: 12.0,
            }
        );
    }

    #[test]
    fn build_display_tree_preserves_sorted_children() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 100,
            height: 100,
            frames: 1,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();
        let parsed = crate::parse(
            r#"{"type":"composition","width":100,"height":100,"fps":30,"frames":1}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"late","parentId":"root","type":"text","className":"text-[12px] z-10","text":"late"}
{"id":"early","parentId":"root","type":"text","className":"text-[12px]","text":"early"}"#,
        )
        .expect("jsonl should parse");
        let resolved = resolve_ui_tree(&parsed.root, &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");
        let layout_tree = LayoutTree {
            root: simple_layout(
                "root",
                LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                vec![
                    simple_layout(
                        "late",
                        LayoutRect {
                            x: 1.0,
                            y: 0.0,
                            width: 10.0,
                            height: 10.0,
                        },
                        Vec::new(),
                    ),
                    simple_layout(
                        "early",
                        LayoutRect {
                            x: 2.0,
                            y: 0.0,
                            width: 10.0,
                            height: 10.0,
                        },
                        Vec::new(),
                    ),
                ],
            ),
        };

        let tree = build_display_tree(&resolved, &layout_tree, &assets)
            .expect("display tree should build");
        let texts = tree
            .root
            .children
            .iter()
            .map(|node| match &node.item {
                DisplayItem::Text(text) => text.text.as_str(),
                _ => panic!("expected text item"),
            })
            .collect::<Vec<_>>();

        assert_eq!(texts, vec!["early", "late"]);
    }

    #[test]
    fn display_tree_builds_lucide_visuals_from_element_style() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();
        let root = div().id("root").child(
            lucide("play")
                .id("icon")
                .size(24.0, 24.0)
                .text_blue()
                .border_color(ColorToken::Blue)
                .border_w(3.5)
                .bg(ColorToken::Sky200),
        );
        let resolved = resolve_ui_tree(&root.into(), &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");
        let layout_tree = LayoutTree {
            root: simple_layout(
                "root",
                LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 320.0,
                    height: 180.0,
                },
                vec![simple_layout(
                    "icon",
                    LayoutRect {
                        x: 0.0,
                        y: 0.0,
                        width: 24.0,
                        height: 24.0,
                    },
                    Vec::new(),
                )],
            ),
        };

        let tree = build_display_tree(&resolved, &layout_tree, &assets)
            .expect("display tree should build");
        let DisplayItem::Lucide(lucide) = &tree.root.children[0].item else {
            panic!("expected lucide item");
        };
        assert_eq!(lucide.paint.foreground, ColorToken::Blue);
        assert_eq!(lucide.paint.border_color, Some(ColorToken::Blue));
        assert_eq!(lucide.paint.border_width, Some(3.5));
        assert_eq!(
            lucide.paint.background,
            Some(crate::style::BackgroundFill::Solid(ColorToken::Sky200))
        );
    }

    #[test]
    fn build_display_tree_reports_structure_mismatch() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 100,
            height: 100,
            frames: 1,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();
        let parsed = parse(
            r#"{"type":"composition","width":100,"height":100,"fps":30,"frames":1}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"child","parentId":"root","type":"text","className":"text-[12px]","text":"A"}"#,
        )
        .expect("jsonl should parse");
        let resolved = resolve_ui_tree(&parsed.root, &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");
        let layout_tree = LayoutTree {
            root: simple_layout(
                "root",
                LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                Vec::new(),
            ),
        };

        let err =
            build_display_tree(&resolved, &layout_tree, &assets).expect_err("expected mismatch");
        assert!(err.to_string().contains("child count mismatch"));
    }
}
