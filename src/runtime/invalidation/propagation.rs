use std::collections::HashMap;

use crate::runtime::{
    analysis::{DisplayAnalysisTable, DisplayInvalidationTable, DisplayNodeInvalidation},
    annotation::{AnnotatedDisplayNode, AnnotatedDisplayTree, RenderNodeKey},
    compositor::SceneSlot,
    fingerprint::{CompositeSig, PaintVariance},
};

#[derive(Default)]
pub(crate) struct CompositeHistory {
    scene: HashMap<RenderNodeKey, CompositeSig>,
    transition_from: HashMap<RenderNodeKey, CompositeSig>,
    transition_to: HashMap<RenderNodeKey, CompositeSig>,
}

impl CompositeHistory {
    fn history_for_slot(&self, slot: SceneSlot) -> &HashMap<RenderNodeKey, CompositeSig> {
        match slot {
            SceneSlot::Scene => &self.scene,
            SceneSlot::TransitionFrom => &self.transition_from,
            SceneSlot::TransitionTo => &self.transition_to,
        }
    }

    fn history_for_slot_mut(
        &mut self,
        slot: SceneSlot,
    ) -> &mut HashMap<RenderNodeKey, CompositeSig> {
        match slot {
            SceneSlot::Scene => &mut self.scene,
            SceneSlot::TransitionFrom => &mut self.transition_from,
            SceneSlot::TransitionTo => &mut self.transition_to,
        }
    }
}

pub(crate) fn mark_display_tree_composite_dirty(
    history: &mut CompositeHistory,
    slot: SceneSlot,
    display_tree: &mut AnnotatedDisplayTree,
    structure_rebuild: bool,
) {
    let empty = HashMap::new();
    let previous = if structure_rebuild {
        &empty
    } else {
        history.history_for_slot(slot)
    };
    let mut next = HashMap::new();
    let mut invalidation = DisplayInvalidationTable::default();
    mark_display_node_composite_dirty(
        &display_tree.root,
        &display_tree.analysis,
        &mut invalidation,
        previous,
        &mut next,
    );
    display_tree.invalidation = invalidation;
    *history.history_for_slot_mut(slot) = next;
}

fn mark_display_node_composite_dirty(
    node: &AnnotatedDisplayNode,
    analysis: &DisplayAnalysisTable,
    invalidation: &mut DisplayInvalidationTable,
    previous: &HashMap<RenderNodeKey, CompositeSig>,
    next: &mut HashMap<RenderNodeKey, CompositeSig>,
) -> bool {
    let current_sig = CompositeSig::from_annotated_node(node);
    let composite_dirty = previous
        .get(&node.key)
        .is_some_and(|previous_sig| *previous_sig != current_sig);
    next.insert(node.key, current_sig);

    let node_analysis = analysis.require(node.key);
    let mut subtree_contains_dynamic =
        node_analysis.paint_variance == PaintVariance::TimeVariant || composite_dirty;
    for child in &node.children {
        subtree_contains_dynamic |=
            mark_display_node_composite_dirty(child, analysis, invalidation, previous, next);
    }
    invalidation.insert(
        node.key,
        DisplayNodeInvalidation {
            composite_dirty,
            subtree_contains_dynamic,
        },
    );
    subtree_contains_dynamic
}
