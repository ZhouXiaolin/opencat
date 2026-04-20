use std::collections::HashMap;

use crate::runtime::{
    analysis::{DisplayAnalysisTable, DisplayInvalidationTable, DisplayNodeInvalidation},
    annotation::{AnnotatedDisplayTree, AnnotatedNodeHandle, RenderNodeKey},
    compositor::SceneSlot,
    fingerprint::CompositeSig,
};

#[derive(Default)]
pub(crate) struct CompositeHistory {
    slots: HashMap<SceneSlot, HashMap<RenderNodeKey, CompositeSig>>,
}

impl CompositeHistory {
    pub(crate) fn history_for_slot(
        &self,
        slot: &SceneSlot,
    ) -> &HashMap<RenderNodeKey, CompositeSig> {
        static EMPTY: std::sync::LazyLock<HashMap<RenderNodeKey, CompositeSig>> =
            std::sync::LazyLock::new(HashMap::new);
        self.slots.get(slot).unwrap_or(&EMPTY)
    }

    pub(crate) fn history_for_slot_mut(
        &mut self,
        slot: SceneSlot,
    ) -> &mut HashMap<RenderNodeKey, CompositeSig> {
        self.slots.entry(slot).or_default()
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
        history.history_for_slot(&slot)
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
