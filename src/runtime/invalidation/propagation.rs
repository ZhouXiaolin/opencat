use crate::{
    display::tree::{DisplayNode, DisplayTree},
    runtime::{
        compositor::SceneSlot,
        fingerprint::{CompositeSig, PaintVariance},
    },
};

#[derive(Default)]
pub(crate) struct CompositeHistory {
    scene: Vec<CompositeSig>,
    transition_from: Vec<CompositeSig>,
    transition_to: Vec<CompositeSig>,
}

impl CompositeHistory {
    fn history_for_slot(&self, slot: SceneSlot) -> &[CompositeSig] {
        match slot {
            SceneSlot::Scene => &self.scene,
            SceneSlot::TransitionFrom => &self.transition_from,
            SceneSlot::TransitionTo => &self.transition_to,
        }
    }

    fn history_for_slot_mut(&mut self, slot: SceneSlot) -> &mut Vec<CompositeSig> {
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
    display_tree: &mut DisplayTree,
    structure_rebuild: bool,
) {
    let previous = if structure_rebuild {
        &[][..]
    } else {
        history.history_for_slot(slot)
    };
    let mut next = Vec::new();
    let mut index = 0;
    mark_display_node_composite_dirty(&mut display_tree.root, previous, &mut index, &mut next);
    *history.history_for_slot_mut(slot) = next;
}

fn mark_display_node_composite_dirty(
    node: &mut DisplayNode,
    previous: &[CompositeSig],
    index: &mut usize,
    next: &mut Vec<CompositeSig>,
) -> bool {
    let current_index = *index;
    *index += 1;

    let current_sig = CompositeSig::from_node(node);
    let composite_dirty = previous
        .get(current_index)
        .is_some_and(|previous_sig| *previous_sig != current_sig);
    next.push(current_sig);
    node.composite_dirty = composite_dirty;

    let mut subtree_contains_dynamic =
        node.paint_variance == PaintVariance::TimeVariant || composite_dirty;
    for child in &mut node.children {
        subtree_contains_dynamic |= mark_display_node_composite_dirty(child, previous, index, next);
    }
    node.subtree_contains_dynamic = subtree_contains_dynamic;
    subtree_contains_dynamic
}
