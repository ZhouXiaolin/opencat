use std::collections::HashMap;

use crate::runtime::{
    analysis::{DisplayAnalysisTable, DisplayInvalidationTable, DisplayNodeInvalidation},
    annotation::{AnnotatedDisplayTree, AnnotatedNodeHandle, RenderNodeKey},
    compositor::SceneSlot,
    fingerprint::CompositeSig,
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
    let mut invalidation = DisplayInvalidationTable::with_len(display_tree.analysis.len());
    mark_display_node_composite_dirty(
        display_tree.root,
        display_tree,
        &display_tree.analysis,
        &mut invalidation,
        previous,
        &mut next,
    );
    display_tree.invalidation = invalidation;
    *history.history_for_slot_mut(slot) = next;
}

fn mark_display_node_composite_dirty(
    handle: AnnotatedNodeHandle,
    display_tree: &AnnotatedDisplayTree,
    analysis: &DisplayAnalysisTable,
    invalidation: &mut DisplayInvalidationTable,
    previous: &HashMap<RenderNodeKey, CompositeSig>,
    next: &mut HashMap<RenderNodeKey, CompositeSig>,
) {
    let node = display_tree.node(handle);
    let node_key = display_tree.key(handle);
    let current_sig = CompositeSig::from_annotated_node(node);
    let composite_dirty = previous
        .get(&node_key)
        .is_some_and(|previous_sig| *previous_sig != current_sig);
    next.insert(node_key, current_sig);

    for &child_handle in &node.children {
        mark_display_node_composite_dirty(
            child_handle,
            display_tree,
            analysis,
            invalidation,
            previous,
            next,
        );
    }
    invalidation.insert(handle, DisplayNodeInvalidation { composite_dirty });
}
