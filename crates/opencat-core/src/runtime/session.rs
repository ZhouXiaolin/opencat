//! Backend-agnostic per-render session state.

use std::sync::Arc;

use crate::analyze::annotation::AnnotatedNodeHandle;
use crate::analyze::compositor::{OrderedSceneOp, OrderedSceneProgram};
use crate::analyze::invalidation::CompositeHistory;
use crate::ir::cache::RenderCache;
use crate::layout::LayoutSession;
use crate::resource::hash_map_catalog::HashMapResourceCatalog;
use crate::text::default_font_db;

const DEFAULT_SUBTREE_SNAPSHOT_CAP: usize = 256;
const DEFAULT_SEGMENT_CAP: usize = 256;
const DEFAULT_ITEM_RANGE_CAP: usize = 128;

pub struct RenderSession {
    /// per-render layout accumulator (node id -> measure cache)
    pub layout_session: LayoutSession,

    /// cross-frame composite dirty history
    pub composite_history: CompositeHistory,

    /// fontdb (platform-agnostic, cosmic-text reuses)
    pub font_db: Arc<fontdb::Database>,

    /// resource metadata (preflight writes; render reads only)
    pub catalog: HashMapResourceCatalog,

    /// last preflight root pointer, for skipping duplicate preflight
    pub prepared_root_ptr: Option<usize>,

    /// IR-based LRU caches (backend-agnostic).
    pub cache: RenderCache,

    /// last ordered scene program from the most recent render_frame call
    pub last_ordered_scene: OrderedSceneProgram,
}

impl RenderSession {
    pub fn new() -> Self {
        Self {
            layout_session: LayoutSession::new(),
            composite_history: CompositeHistory::default(),
            font_db: Arc::new(default_font_db(&[])),
            catalog: HashMapResourceCatalog::from_json("{}").expect("empty catalog must parse"),
            prepared_root_ptr: None,
            cache: RenderCache::new(
                DEFAULT_SUBTREE_SNAPSHOT_CAP,
                DEFAULT_SEGMENT_CAP,
                DEFAULT_ITEM_RANGE_CAP,
            ),
            last_ordered_scene: OrderedSceneProgram {
                root: OrderedSceneOp::LiveSubtree {
                    handle: AnnotatedNodeHandle(0),
                    children: Vec::new(),
                },
            },
        }
    }
}

impl Default for RenderSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_session_new_does_not_require_platform() {
        let session = RenderSession::new();
        assert!(session.prepared_root_ptr.is_none());
    }
}
