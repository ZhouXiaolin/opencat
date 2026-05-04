use std::collections::HashMap;

use crate::runtime::{
    analysis::{DisplayAnalysisTable, DisplayInvalidationTable, DisplayNodeInvalidation},
    annotation::{AnnotatedDisplayTree, AnnotatedNodeHandle, RenderNodeKey},
    fingerprint::CompositeSig,
};

#[derive(Default)]
pub struct CompositeHistory {
    entries: HashMap<RenderNodeKey, CompositeSig>,
}

impl CompositeHistory {
    pub fn history(&self) -> &HashMap<RenderNodeKey, CompositeSig> {
        static EMPTY: std::sync::LazyLock<HashMap<RenderNodeKey, CompositeSig>> =
            std::sync::LazyLock::new(HashMap::new);
        if self.entries.is_empty() {
            &EMPTY
        } else {
            &self.entries
        }
    }

    pub fn history_mut(&mut self) -> &mut HashMap<RenderNodeKey, CompositeSig> {
        &mut self.entries
    }
}

pub fn mark_display_tree_composite_dirty(
    history: &mut CompositeHistory,
    display_tree: &mut AnnotatedDisplayTree,
    structure_rebuild: bool,
) {
    let empty = HashMap::new();
    let previous = if structure_rebuild {
        &empty
    } else {
        history.history()
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
    *history.history_mut() = next;
}

#[allow(clippy::only_used_in_recursion)]
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
