mod propagation;

#[cfg(feature = "host-default")]
pub(crate) use propagation::{CompositeHistory, mark_display_tree_composite_dirty};
