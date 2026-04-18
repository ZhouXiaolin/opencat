use crate::{
    display::{
        list::{DisplayClip, DisplayItem, DisplayRect, DisplayTransform},
        tree::{DisplayNode, DisplayTree},
    },
    resource::assets::AssetsMap,
    runtime::{
        analysis::{
            DisplayAnalysisTable, DisplayInvalidationTable, DisplayNodeAnalysis,
            DisplayNodeInvalidation,
        },
        fingerprint::{self, PaintVariance},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RenderNodeKey(pub u64);

#[derive(Clone, Debug)]
pub struct AnnotatedDisplayTree {
    pub root: AnnotatedDisplayNode,
    pub analysis: DisplayAnalysisTable,
    pub invalidation: DisplayInvalidationTable,
}

#[derive(Clone, Debug)]
pub struct AnnotatedDisplayNode {
    pub key: RenderNodeKey,
    pub transform: DisplayTransform,
    pub opacity: f32,
    pub backdrop_blur_sigma: Option<f32>,
    pub clip: Option<DisplayClip>,
    pub item: DisplayItem,
    pub children: Vec<AnnotatedDisplayNode>,
}

#[derive(Clone, Copy, Debug)]
pub struct RecordedNodeSemantics<'a> {
    pub bounds: DisplayRect,
    pub item: &'a DisplayItem,
    pub clip: Option<&'a DisplayClip>,
}

#[derive(Clone, Copy, Debug)]
pub struct DrawCompositeSemantics<'a> {
    pub transform: &'a DisplayTransform,
    pub opacity: f32,
    pub backdrop_blur_sigma: Option<f32>,
}

impl AnnotatedDisplayTree {
    pub fn analysis_for(&self, node: &AnnotatedDisplayNode) -> DisplayNodeAnalysis {
        self.analysis.require(node.key)
    }

    pub fn invalidation_for(&self, node: &AnnotatedDisplayNode) -> DisplayNodeInvalidation {
        self.invalidation.get(node.key)
    }

    pub fn contains_time_variant(&self) -> bool {
        self.analysis_for(&self.root).subtree_contains_time_variant
    }
}

impl AnnotatedDisplayNode {
    pub fn layer_bounds(&self) -> DisplayRect {
        let mut bounds = self.item.visual_bounds();
        for child in &self.children {
            let child_bounds = child
                .layer_bounds()
                .translate(child.transform.translation_x, child.transform.translation_y);
            bounds = bounds.union(child_bounds);
        }
        bounds
    }

    pub fn recorded_semantics(&self) -> RecordedNodeSemantics<'_> {
        RecordedNodeSemantics {
            bounds: self.transform.bounds,
            item: &self.item,
            clip: self.clip.as_ref(),
        }
    }

    pub fn draw_composite_semantics(&self) -> DrawCompositeSemantics<'_> {
        DrawCompositeSemantics {
            transform: &self.transform,
            opacity: self.opacity,
            backdrop_blur_sigma: self.backdrop_blur_sigma,
        }
    }
}

pub(crate) fn annotate_display_tree(
    display_tree: &DisplayTree,
    assets: &AssetsMap,
) -> AnnotatedDisplayTree {
    let mut analysis = DisplayAnalysisTable::default();
    let mut invalidation = DisplayInvalidationTable::default();
    let (root, root_analysis) =
        annotate_display_node(&display_tree.root, assets, &mut analysis, &mut invalidation);
    analysis.insert(root.key, root_analysis);
    invalidation.insert(
        root.key,
        DisplayNodeInvalidation {
            composite_dirty: false,
            subtree_contains_dynamic: root_analysis.subtree_contains_time_variant,
        },
    );

    AnnotatedDisplayTree {
        root,
        analysis,
        invalidation,
    }
}

fn annotate_display_node(
    node: &DisplayNode,
    assets: &AssetsMap,
    analysis: &mut DisplayAnalysisTable,
    invalidation: &mut DisplayInvalidationTable,
) -> (AnnotatedDisplayNode, DisplayNodeAnalysis) {
    let mut children = Vec::with_capacity(node.children.len());
    for child in &node.children {
        let (annotated_child, child_analysis) =
            annotate_display_node(child, assets, analysis, invalidation);
        invalidation.insert(
            annotated_child.key,
            DisplayNodeInvalidation {
                composite_dirty: false,
                subtree_contains_dynamic: child_analysis.subtree_contains_time_variant,
            },
        );
        analysis.insert(annotated_child.key, child_analysis);
        children.push(annotated_child);
    }

    let annotated = AnnotatedDisplayNode {
        key: RenderNodeKey(node.element_id.0),
        transform: node.transform.clone(),
        opacity: node.opacity,
        backdrop_blur_sigma: node.backdrop_blur_sigma,
        clip: node.clip.clone(),
        item: node.item.clone(),
        children,
    };

    let paint_variance = fingerprint::classify_paint(&node.item, assets);
    let subtree_contains_time_variant = matches!(paint_variance, PaintVariance::TimeVariant)
        || annotated
            .children
            .iter()
            .any(|child| analysis.require(child.key).subtree_contains_time_variant);

    let mut node_analysis = DisplayNodeAnalysis {
        paint_variance,
        subtree_contains_time_variant,
        paint_fingerprint: None,
        snapshot_fingerprint: None,
    };

    if !subtree_contains_time_variant {
        node_analysis.paint_fingerprint = fingerprint::annotated_subtree_paint_fingerprint(
            &annotated,
            analysis,
            subtree_contains_time_variant,
        );
        node_analysis.snapshot_fingerprint = fingerprint::annotated_subtree_snapshot_fingerprint(
            &annotated,
            analysis,
            subtree_contains_time_variant,
        );
    }

    (annotated, node_analysis)
}
