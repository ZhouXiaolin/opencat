use anyhow::{Result, anyhow};

use crate::{
    display::{
        list::{
            BitmapDisplayItem, BitmapPaintStyle, DisplayClip, DisplayItem, DisplayRect,
            DisplayTransform, DrawScriptDisplayItem, RectDisplayItem, RectPaintStyle,
            SvgPathDisplayItem, SvgPathPaintStyle, TextDisplayItem, TimelineDisplayItem,
            TimelineTransitionDisplay,
        },
        tree::{DisplayNode, DisplayTree},
    },
    element::tree::{ElementKind, ElementNode},
    layout::tree::{LayoutNode, LayoutTree},
    resource::assets::AssetsMap,
};

pub fn build_display_tree(
    element_root: &ElementNode,
    layout_tree: &LayoutTree,
    _assets: &AssetsMap,
) -> Result<DisplayTree> {
    Ok(DisplayTree {
        root: build_display_node(element_root, &layout_tree.root)?,
    })
}

fn build_display_node(element: &ElementNode, layout: &LayoutNode) -> Result<DisplayNode> {
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
        .map(|(child, child_layout)| build_display_node(child, child_layout))
        .collect::<Result<Vec<_>>>()?;

    let visual = &element.style.visual;
    let uniform_border = visual.border_width.unwrap_or(0.0);
    let border_top_w = visual.border_top_width.unwrap_or(uniform_border);
    let border_right_w = visual.border_right_width.unwrap_or(uniform_border);
    let border_bottom_w = visual.border_bottom_width.unwrap_or(uniform_border);
    let border_left_w = visual.border_left_width.unwrap_or(uniform_border);

    let clip = if visual.clip_contents {
        let inner_bounds = DisplayRect {
            x: bounds.x + border_left_w,
            y: bounds.y + border_top_w,
            width: (bounds.width - border_left_w - border_right_w).max(0.0),
            height: (bounds.height - border_top_w - border_bottom_w).max(0.0),
        };
        let outer_radius = visual.border_radius;
        let inner_radius = crate::style::BorderRadius {
            top_left: (outer_radius.top_left - border_top_w.max(border_left_w)).max(0.0),
            top_right: (outer_radius.top_right - border_top_w.max(border_right_w)).max(0.0),
            bottom_right: (outer_radius.bottom_right - border_bottom_w.max(border_right_w))
                .max(0.0),
            bottom_left: (outer_radius.bottom_left - border_bottom_w.max(border_left_w)).max(0.0),
        };
        Some(DisplayClip {
            bounds: inner_bounds,
            border_radius: inner_radius,
        })
    } else {
        None
    };

    Ok(DisplayNode {
        transform: DisplayTransform {
            translation_x: layout.rect.x,
            translation_y: layout.rect.y,
            bounds,
            transforms: element.style.visual.transforms.clone(),
        },
        element_id: element.id,
        opacity: element.style.visual.opacity,
        backdrop_blur_sigma: element.style.visual.backdrop_blur_sigma,
        clip,
        item,
        children,
    })
}

fn display_item_for_node(element: &ElementNode, bounds: DisplayRect) -> DisplayItem {
    match &element.kind {
        ElementKind::Div(_) => DisplayItem::Rect(RectDisplayItem {
            bounds,
            paint: RectPaintStyle {
                background: element.style.visual.background,
                border_radius: element.style.visual.border_radius,
                border_width: element.style.visual.border_width,
                border_top_width: element.style.visual.border_top_width,
                border_right_width: element.style.visual.border_right_width,
                border_bottom_width: element.style.visual.border_bottom_width,
                border_left_width: element.style.visual.border_left_width,
                border_color: element.style.visual.border_color,
                border_style: element.style.visual.border_style,
                blur_sigma: element.style.visual.blur_sigma,
                box_shadow: element.style.visual.box_shadow,
                inset_shadow: element.style.visual.inset_shadow,
                drop_shadow: element.style.visual.drop_shadow,
            },
        }),
        ElementKind::Timeline(timeline) => DisplayItem::Timeline(TimelineDisplayItem {
            bounds,
            paint: RectPaintStyle {
                background: element.style.visual.background,
                border_radius: element.style.visual.border_radius,
                border_width: element.style.visual.border_width,
                border_top_width: element.style.visual.border_top_width,
                border_right_width: element.style.visual.border_right_width,
                border_bottom_width: element.style.visual.border_bottom_width,
                border_left_width: element.style.visual.border_left_width,
                border_color: element.style.visual.border_color,
                border_style: element.style.visual.border_style,
                blur_sigma: element.style.visual.blur_sigma,
                box_shadow: element.style.visual.box_shadow,
                inset_shadow: element.style.visual.inset_shadow,
                drop_shadow: element.style.visual.drop_shadow,
            },
            transition: timeline
                .transition
                .as_ref()
                .map(|transition| TimelineTransitionDisplay {
                    progress: transition.progress,
                    kind: transition.kind,
                }),
        }),
        ElementKind::Text(text) => {
            let (visual_expand_x, visual_expand_y) = conservative_text_visual_expansion(
                text.text_unit_overrides.as_ref(),
                text.text_style.text_px,
            );
            DisplayItem::Text(TextDisplayItem {
                bounds,
                text: text.text.clone(),
                style: text.text_style,
                allow_wrap: element.style.text.wrap_text
                    || element.style.layout.width.is_some()
                    || element.style.layout.width_full,
                drop_shadow: element.style.visual.drop_shadow,
                text_unit_overrides: text.text_unit_overrides.clone(),
                visual_expand_x,
                visual_expand_y,
            })
        }
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
                border_top_width: element.style.visual.border_top_width,
                border_right_width: element.style.visual.border_right_width,
                border_bottom_width: element.style.visual.border_bottom_width,
                border_left_width: element.style.visual.border_left_width,
                border_color: element.style.visual.border_color,
                border_style: element.style.visual.border_style,
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
        ElementKind::SvgPath(svg) => DisplayItem::SvgPath(SvgPathDisplayItem {
            bounds,
            path_data: svg.path_data.clone(),
            paint: SvgPathPaintStyle {
                fill: element.style.visual.fill,
                stroke_width: element.style.visual.stroke_width,
                stroke_color: element.style.visual.stroke_color,
                drop_shadow: element.style.visual.drop_shadow,
            },
            view_box: svg.view_box,
        }),
    }
}

fn conservative_text_visual_expansion(
    batch: Option<&crate::scene::script::TextUnitOverrideBatch>,
    text_px: f32,
) -> (f32, f32) {
    let Some(batch) = batch else {
        return (0.0, 0.0);
    };
    let mut max_x = 0.0_f32;
    let mut max_y = 0.0_f32;
    let base = text_px.max(1.0);
    for unit in &batch.overrides {
        max_x = max_x.max(unit.translate_x.unwrap_or(0.0).abs());
        max_y = max_y.max(unit.translate_y.unwrap_or(0.0).abs());
        let scale = unit.scale.unwrap_or(1.0);
        if scale > 1.0 {
            max_x = max_x.max((scale - 1.0) * base);
            max_y = max_y.max((scale - 1.0) * base);
        }
        if unit.rotation_deg.unwrap_or(0.0) != 0.0 {
            max_x = max_x.max(base * 0.5);
            max_y = max_y.max(base * 0.5);
        }
    }
    (max_x.ceil(), max_y.ceil())
}

#[cfg(test)]
mod tests {
    use super::build_display_tree;
    use crate::{
        FrameCtx,
        element::resolve::resolve_ui_tree,
        parse,
        resource::assets::AssetsMap,
        resource::media::MediaContext,
        runtime::annotation::{annotate_display_tree, compute_display_tree_fingerprints},
        scene::primitives::{div, lucide},
        style::{ColorToken, ObjectFit},
    };
    use crate::{
        display::list::DisplayItem,
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

        let tree = build_display_tree(&resolved, &layout_tree, &assets)
            .expect("display tree should build");
        let DisplayItem::Bitmap(bitmap) = &tree.root.children[0].item else {
            panic!("expected bitmap draw item");
        };

        assert_eq!(bitmap.object_fit, ObjectFit::Cover);
        assert_eq!(
            bitmap.paint.border_radius,
            crate::style::BorderRadius::default()
        );
    }

    #[test]
    fn display_tree_sorts_children_by_z_index_for_painting() {
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

        let tree = build_display_tree(&resolved, &layout_tree, &assets)
            .expect("display tree should build");
        let texts = tree
            .root
            .children
            .iter()
            .filter_map(|node| match &node.item {
                DisplayItem::Text(text) => Some(text.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(texts, vec!["back", "front"]);
    }

    #[test]
    fn display_tree_keeps_clip_for_overflow_hidden_nodes() {
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

        let tree = build_display_tree(&resolved, &layout_tree, &assets)
            .expect("display tree should build");
        let clip = tree.root.clip.as_ref();
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
    fn build_display_tree_annotates_paint_and_snapshot_fingerprints_separately() {
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
{"id":"child","parentId":"root","type":"div","className":"w-[10px] h-[10px] bg-red-500"}"#,
        )
        .expect("jsonl should parse");
        let resolved = resolve_ui_tree(&parsed.root, &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");

        let layout_a = LayoutTree {
            root: simple_layout(
                "root",
                LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                vec![simple_layout(
                    "child",
                    LayoutRect {
                        x: 0.0,
                        y: 0.0,
                        width: 10.0,
                        height: 10.0,
                    },
                    Vec::new(),
                )],
            ),
        };
        let layout_b = LayoutTree {
            root: simple_layout(
                "root",
                LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                vec![simple_layout(
                    "child",
                    LayoutRect {
                        x: 24.0,
                        y: 12.0,
                        width: 10.0,
                        height: 10.0,
                    },
                    Vec::new(),
                )],
            ),
        };

        let tree_a =
            build_display_tree(&resolved, &layout_a, &assets).expect("display tree should build");
        let tree_b =
            build_display_tree(&resolved, &layout_b, &assets).expect("display tree should build");
        let mut annotated_a = annotate_display_tree(&tree_a, &assets);
        compute_display_tree_fingerprints(&mut annotated_a);
        let mut annotated_b = annotate_display_tree(&tree_b, &assets);
        compute_display_tree_fingerprints(&mut annotated_b);

        let child_a = annotated_a.children(annotated_a.root)[0];
        let child_b = annotated_b.children(annotated_b.root)[0];
        assert_eq!(
            annotated_a.analysis(child_a).paint_fingerprint,
            annotated_b.analysis(child_b).paint_fingerprint
        );
        assert_eq!(
            annotated_a.analysis(child_a).snapshot_fingerprint,
            annotated_b.analysis(child_b).snapshot_fingerprint
        );
        assert_ne!(
            annotated_a.analysis(annotated_a.root).snapshot_fingerprint,
            annotated_b.analysis(annotated_b.root).snapshot_fingerprint
        );
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
                .stroke_color(ColorToken::Blue)
                .stroke_width(3.5)
                .fill_color(ColorToken::Sky200),
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
        let DisplayItem::SvgPath(svg) = &tree.root.children[0].item else {
            panic!("expected svg path item");
        };
        assert_eq!(svg.paint.stroke_color, Some(ColorToken::Blue));
        assert_eq!(svg.paint.stroke_width, Some(3.5));
        assert_eq!(
            svg.paint.fill,
            Some(crate::style::BackgroundFill::Solid(ColorToken::Sky200))
        );
        assert_eq!(svg.view_box, [0.0, 0.0, 24.0, 24.0]);
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

    #[test]
    fn display_tree_builds_path_visuals() {
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
            crate::scene::primitives::path("M0 0 L 100 0 L 50 100 Z")
                .id("triangle")
                .size(100.0, 100.0)
                .fill_color(ColorToken::Red500)
                .stroke_color(ColorToken::Blue)
                .stroke_width(2.0),
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
                    "triangle",
                    LayoutRect {
                        x: 0.0,
                        y: 0.0,
                        width: 100.0,
                        height: 100.0,
                    },
                    Vec::new(),
                )],
            ),
        };

        let tree = build_display_tree(&resolved, &layout_tree, &assets)
            .expect("display tree should build");
        let DisplayItem::SvgPath(svg) = &tree.root.children[0].item else {
            panic!(
                "expected svg path item, got {:?}",
                tree.root.children[0].item
            );
        };
        assert_eq!(svg.path_data, vec!["M0 0 L 100 0 L 50 100 Z"]);
        assert_eq!(svg.paint.stroke_width, Some(2.0));
        assert_eq!(svg.paint.stroke_color, Some(ColorToken::Blue));
        assert_eq!(
            svg.paint.fill,
            Some(crate::style::BackgroundFill::Solid(ColorToken::Red500))
        );
    }

    #[test]
    fn lucide_node_resolves_to_svg_path_with_default_stroke() {
        let frame_ctx = FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        };
        let mut media = MediaContext::new();
        let mut assets = AssetsMap::new();
        let root = div().id("root").child(lucide("play").id("icon"));
        let resolved = resolve_ui_tree(&root.into(), &frame_ctx, &mut media, &mut assets, None)
            .expect("tree should resolve");

        let child = &resolved.children[0];
        let crate::element::tree::ElementKind::SvgPath(svg) = &child.kind else {
            panic!("expected SvgPath element, got {:?}", child.kind);
        };
        assert!(!svg.path_data.is_empty());
        assert_eq!(svg.view_box, [0.0, 0.0, 24.0, 24.0]);
        assert_eq!(svg.intrinsic_size, Some((24.0, 24.0)));
    }
}
