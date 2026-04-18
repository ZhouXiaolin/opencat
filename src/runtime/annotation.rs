use crate::{
    display::{
        list::{DisplayClip, DisplayItem, DisplayRect, DisplayTransform},
        tree::{DisplayNode, DisplayTree},
    },
    resource::assets::AssetsMap,
    runtime::fingerprint::{self, PaintVariance},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RenderNodeKey(pub u64);

#[derive(Clone, Debug)]
pub struct AnnotatedDisplayTree {
    pub root: AnnotatedDisplayNode,
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
    pub paint_variance: PaintVariance,
    pub composite_dirty: bool,
    pub subtree_contains_time_variant: bool,
    pub subtree_contains_dynamic: bool,
    pub paint_fingerprint: Option<u64>,
    pub snapshot_fingerprint: Option<u64>,
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
    AnnotatedDisplayTree {
        root: annotate_display_node(&display_tree.root, assets),
    }
}

fn annotate_display_node(node: &DisplayNode, assets: &AssetsMap) -> AnnotatedDisplayNode {
    let children = node
        .children
        .iter()
        .map(|child| annotate_display_node(child, assets))
        .collect::<Vec<_>>();

    let paint_variance = fingerprint::classify_paint(&node.item, assets);
    let subtree_contains_time_variant = matches!(paint_variance, PaintVariance::TimeVariant)
        || children
            .iter()
            .any(|child| child.subtree_contains_time_variant);

    let mut annotated = AnnotatedDisplayNode {
        key: RenderNodeKey(node.element_id.0),
        transform: node.transform.clone(),
        opacity: node.opacity,
        backdrop_blur_sigma: node.backdrop_blur_sigma,
        clip: node.clip.clone(),
        item: node.item.clone(),
        children,
        paint_variance,
        composite_dirty: false,
        subtree_contains_time_variant,
        subtree_contains_dynamic: subtree_contains_time_variant,
        paint_fingerprint: None,
        snapshot_fingerprint: None,
    };

    if !annotated.subtree_contains_time_variant {
        annotated.paint_fingerprint = fingerprint::annotated_subtree_paint_fingerprint(&annotated);
        annotated.snapshot_fingerprint =
            fingerprint::annotated_subtree_snapshot_fingerprint(&annotated);
    }

    annotated
}
