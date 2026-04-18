use anyhow::Result;

use crate::runtime::{
    annotation::AnnotatedDisplayTree,
    compositor::{LayeredScene, layer::DynamicLayer},
    fingerprint::{CompositeSig, PaintVariance, scene_static_skeleton_fingerprint},
    profile::{BackendCountMetric, record_backend_count},
    render_engine::{SceneRenderContext, SharedRenderEngine},
};

pub(crate) fn record_layered_scene(
    runtime: &mut SceneRenderContext<'_>,
    render_engine: SharedRenderEngine,
    display_tree: &AnnotatedDisplayTree,
) -> Result<LayeredScene> {
    let skeleton_fp = scene_static_skeleton_fingerprint(display_tree);
    let static_cache = runtime.cache_registry.scene_static_picture_cache();

    let static_layer = if let Some(snapshot) = static_cache.borrow_mut().get_cloned(&skeleton_fp) {
        record_backend_count(BackendCountMetric::SceneStaticCacheHit, 1);
        Some(snapshot)
    } else {
        let snapshot = render_engine.record_display_tree_static_snapshot(runtime, display_tree)?;
        static_cache
            .borrow_mut()
            .insert(skeleton_fp, snapshot.clone());
        record_backend_count(BackendCountMetric::SceneStaticCacheMiss, 1);
        Some(snapshot)
    };

    let mut dynamic = Vec::new();
    let mut transform_chain = Vec::new();
    collect_dynamic_layers(
        display_tree,
        display_tree.root,
        &mut transform_chain,
        &mut dynamic,
    );

    Ok(LayeredScene {
        static_layer,
        dynamic,
        bounds: display_tree.root_node().transform.bounds,
    })
}

fn collect_dynamic_layers(
    display_tree: &AnnotatedDisplayTree,
    handle: crate::runtime::annotation::AnnotatedNodeHandle,
    transform_chain: &mut Vec<crate::display::list::DisplayTransform>,
    dynamic: &mut Vec<DynamicLayer>,
) {
    let node = display_tree.node(handle);
    transform_chain.push(node.transform.clone());

    let invalidation = display_tree.invalidation(handle);
    if !invalidation.subtree_contains_dynamic {
        transform_chain.pop();
        return;
    }

    let analysis = display_tree.analysis(handle);
    if analysis.paint_variance == PaintVariance::TimeVariant || invalidation.composite_dirty {
        dynamic.push(DynamicLayer {
            root: handle,
            composite: CompositeSig::from_annotated_node(node),
            transform_chain: transform_chain.clone(),
            opacity: node.opacity,
            backdrop_blur_sigma: node.backdrop_blur_sigma,
            clip: node.clip.clone(),
            bounds: display_tree.layer_bounds(handle),
        });
        transform_chain.pop();
        return;
    }

    for &child_handle in display_tree.children(handle) {
        collect_dynamic_layers(display_tree, child_handle, transform_chain, dynamic);
    }

    transform_chain.pop();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        display::list::{DisplayItem, DisplayRect, DisplayTransform, RectDisplayItem, RectPaintStyle},
        runtime::{
            analysis::{
                DisplayAnalysisTable, DisplayInvalidationTable, DisplayNodeAnalysis,
                DisplayNodeInvalidation,
            },
            annotation::{AnnotatedDisplayNode, AnnotatedDisplayTree, AnnotatedNodeHandle, RenderNodeKey},
            fingerprint::PaintVariance,
        },
        style::BorderRadius,
    };

    fn rect_bounds() -> DisplayRect {
        DisplayRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        }
    }

    fn rect_node(children: Vec<AnnotatedNodeHandle>) -> AnnotatedDisplayNode {
        AnnotatedDisplayNode {
            transform: DisplayTransform {
                translation_x: 0.0,
                translation_y: 0.0,
                bounds: rect_bounds(),
                transforms: Vec::new(),
            },
            opacity: 1.0,
            backdrop_blur_sigma: None,
            clip: None,
            item: DisplayItem::Rect(RectDisplayItem {
                bounds: rect_bounds(),
                paint: RectPaintStyle {
                    background: None,
                    border_radius: BorderRadius::default(),
                    border_width: None,
                    border_color: None,
                    blur_sigma: None,
                    box_shadow: None,
                    inset_shadow: None,
                    drop_shadow: None,
                },
            }),
            children,
        }
    }

    #[test]
    fn collect_dynamic_layers_keeps_only_topmost_dynamic_roots() {
        let root = AnnotatedNodeHandle(0);
        let animated_parent = AnnotatedNodeHandle(1);
        let animated_grandchild = AnnotatedNodeHandle(2);
        let time_variant_sibling = AnnotatedNodeHandle(3);

        let tree = AnnotatedDisplayTree {
            root,
            nodes: vec![
                rect_node(vec![animated_parent, time_variant_sibling]),
                rect_node(vec![animated_grandchild]),
                rect_node(Vec::new()),
                rect_node(Vec::new()),
            ],
            keys: vec![
                RenderNodeKey(1),
                RenderNodeKey(2),
                RenderNodeKey(3),
                RenderNodeKey(4),
            ],
            layer_bounds: vec![rect_bounds(), rect_bounds(), rect_bounds(), rect_bounds()],
            analysis: {
                let mut table = DisplayAnalysisTable::default();
                table.insert(
                    root,
                    DisplayNodeAnalysis {
                        paint_variance: PaintVariance::Stable,
                        subtree_contains_time_variant: true,
                        paint_fingerprint: None,
                        snapshot_fingerprint: None,
                    },
                );
                table.insert(
                    animated_parent,
                    DisplayNodeAnalysis {
                        paint_variance: PaintVariance::Stable,
                        subtree_contains_time_variant: true,
                        paint_fingerprint: None,
                        snapshot_fingerprint: Some(42),
                    },
                );
                table.insert(
                    animated_grandchild,
                    DisplayNodeAnalysis {
                        paint_variance: PaintVariance::TimeVariant,
                        subtree_contains_time_variant: true,
                        paint_fingerprint: None,
                        snapshot_fingerprint: None,
                    },
                );
                table.insert(
                    time_variant_sibling,
                    DisplayNodeAnalysis {
                        paint_variance: PaintVariance::TimeVariant,
                        subtree_contains_time_variant: true,
                        paint_fingerprint: None,
                        snapshot_fingerprint: None,
                    },
                );
                table
            },
            invalidation: {
                let mut table = DisplayInvalidationTable::with_len(4);
                table.insert(
                    root,
                    DisplayNodeInvalidation {
                        composite_dirty: false,
                        subtree_contains_dynamic: true,
                    },
                );
                table.insert(
                    animated_parent,
                    DisplayNodeInvalidation {
                        composite_dirty: true,
                        subtree_contains_dynamic: true,
                    },
                );
                table.insert(
                    animated_grandchild,
                    DisplayNodeInvalidation {
                        composite_dirty: false,
                        subtree_contains_dynamic: true,
                    },
                );
                table.insert(
                    time_variant_sibling,
                    DisplayNodeInvalidation {
                        composite_dirty: false,
                        subtree_contains_dynamic: true,
                    },
                );
                table
            },
        };

        let mut dynamic = Vec::new();
        let mut transform_chain = Vec::new();
        collect_dynamic_layers(&tree, tree.root, &mut transform_chain, &mut dynamic);

        let roots = dynamic.into_iter().map(|layer| layer.root.0).collect::<Vec<_>>();
        assert_eq!(
            roots,
            vec![animated_parent.0, time_variant_sibling.0],
            "dynamic layer collection should keep DFS order and avoid duplicating descendants under a dynamic parent"
        );
    }

    #[test]
    fn collect_dynamic_layers_captures_transform_chain_and_bounds() {
        let root = AnnotatedNodeHandle(0);
        let nested_dynamic = AnnotatedNodeHandle(1);
        let tree = AnnotatedDisplayTree {
            root,
            nodes: vec![
                AnnotatedDisplayNode {
                    transform: DisplayTransform {
                        translation_x: 10.0,
                        translation_y: 20.0,
                        bounds: rect_bounds(),
                        transforms: Vec::new(),
                    },
                    opacity: 1.0,
                    backdrop_blur_sigma: None,
                    clip: None,
                    item: DisplayItem::Rect(RectDisplayItem {
                        bounds: rect_bounds(),
                        paint: RectPaintStyle {
                            background: None,
                            border_radius: BorderRadius::default(),
                            border_width: None,
                            border_color: None,
                            blur_sigma: None,
                            box_shadow: None,
                            inset_shadow: None,
                            drop_shadow: None,
                        },
                    }),
                    children: vec![nested_dynamic],
                },
                AnnotatedDisplayNode {
                    transform: DisplayTransform {
                        translation_x: 30.0,
                        translation_y: 40.0,
                        bounds: rect_bounds(),
                        transforms: Vec::new(),
                    },
                    opacity: 0.5,
                    backdrop_blur_sigma: Some(6.0),
                    clip: None,
                    item: DisplayItem::Rect(RectDisplayItem {
                        bounds: rect_bounds(),
                        paint: RectPaintStyle {
                            background: None,
                            border_radius: BorderRadius::default(),
                            border_width: None,
                            border_color: None,
                            blur_sigma: None,
                            box_shadow: None,
                            inset_shadow: None,
                            drop_shadow: None,
                        },
                    }),
                    children: Vec::new(),
                },
            ],
            keys: vec![RenderNodeKey(1), RenderNodeKey(2)],
            layer_bounds: vec![
                DisplayRect {
                    x: 0.0,
                    y: 0.0,
                    width: 130.0,
                    height: 140.0,
                },
                rect_bounds(),
            ],
            analysis: {
                let mut table = DisplayAnalysisTable::default();
                table.insert(
                    root,
                    DisplayNodeAnalysis {
                        paint_variance: PaintVariance::Stable,
                        subtree_contains_time_variant: true,
                        paint_fingerprint: None,
                        snapshot_fingerprint: None,
                    },
                );
                table.insert(
                    nested_dynamic,
                    DisplayNodeAnalysis {
                        paint_variance: PaintVariance::TimeVariant,
                        subtree_contains_time_variant: true,
                        paint_fingerprint: None,
                        snapshot_fingerprint: None,
                    },
                );
                table
            },
            invalidation: {
                let mut table = DisplayInvalidationTable::with_len(2);
                table.insert(
                    root,
                    DisplayNodeInvalidation {
                        composite_dirty: false,
                        subtree_contains_dynamic: true,
                    },
                );
                table.insert(
                    nested_dynamic,
                    DisplayNodeInvalidation {
                        composite_dirty: false,
                        subtree_contains_dynamic: true,
                    },
                );
                table
            },
        };

        let mut dynamic = Vec::new();
        let mut transform_chain = Vec::new();
        collect_dynamic_layers(&tree, tree.root, &mut transform_chain, &mut dynamic);

        assert_eq!(dynamic.len(), 1);
        let layer = &dynamic[0];
        assert_eq!(layer.root, nested_dynamic);
        assert_eq!(layer.transform_chain.len(), 2);
        assert_eq!(layer.transform_chain[0].translation_x, 10.0);
        assert_eq!(layer.transform_chain[0].translation_y, 20.0);
        assert_eq!(layer.transform_chain[1].translation_x, 30.0);
        assert_eq!(layer.transform_chain[1].translation_y, 40.0);
        assert_eq!(layer.opacity, 0.5);
        assert_eq!(layer.backdrop_blur_sigma, Some(6.0));
        assert_eq!(layer.bounds.width, 100.0);
        assert_eq!(layer.bounds.height, 100.0);
    }
}
