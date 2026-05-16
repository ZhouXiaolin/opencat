//! Generic per-render session: holds backend-agnostic state + platform state.
//!
//! engine / web each monomorphize this session with their concrete Platform
//! and Canvas2D types.

use std::sync::Arc;

use crate::canvas::Canvas2D;
use crate::layout::LayoutSession;
use crate::platform::platform::Platform;
use crate::render::RenderCache;
use crate::resource::hash_map_catalog::HashMapResourceCatalog;
use crate::runtime::compositor::ordered_scene::{OrderedSceneOp, OrderedSceneProgram};
use crate::runtime::compositor::reuse::LiveNodeItemExecution;
use crate::runtime::annotation::AnnotatedNodeHandle;
use crate::runtime::invalidation::CompositeHistory;
use crate::text::default_font_db;

/// Default cache capacity constants.
const DEFAULT_IMAGE_CAP: usize = 128;
const DEFAULT_SUBTREE_SNAPSHOT_CAP: usize = 256;
const DEFAULT_SUBTREE_IMAGE_CAP: usize = 128;
const DEFAULT_ITEM_PICTURE_CAP: usize = 64;
const DEFAULT_GLYPH_PATH_CAP: usize = 1024;
const DEFAULT_GLYPH_IMAGE_CAP: usize = 128;
const DEFAULT_RUNTIME_EFFECT_CAP: usize = 64;

pub struct RenderSession<P: Platform, C: Canvas2D> {
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

    /// LRU caches parameterised by the canvas backend.
    pub cache: RenderCache<C>,

    /// last ordered scene program from the most recent render_frame call
    pub last_ordered_scene: OrderedSceneProgram,

    /// platform's own stuff (script runtime, video source, IO etc)
    pub platform: P,
}

impl<P: Platform, C: Canvas2D> RenderSession<P, C> {
    pub fn new(platform: P) -> Self {
        Self {
            layout_session: LayoutSession::new(),
            composite_history: CompositeHistory::default(),
            font_db: Arc::new(default_font_db(&[])),
            catalog: HashMapResourceCatalog::from_json("{}").expect("empty catalog must parse"),
            prepared_root_ptr: None,
            cache: RenderCache::new(
                DEFAULT_IMAGE_CAP,
                DEFAULT_SUBTREE_SNAPSHOT_CAP,
                DEFAULT_SUBTREE_IMAGE_CAP,
                DEFAULT_ITEM_PICTURE_CAP,
                DEFAULT_GLYPH_PATH_CAP,
                DEFAULT_GLYPH_IMAGE_CAP,
                DEFAULT_RUNTIME_EFFECT_CAP,
            ),
            last_ordered_scene: OrderedSceneProgram {
                root: OrderedSceneOp::LiveSubtree {
                    handle: AnnotatedNodeHandle(0),
                    item_execution: LiveNodeItemExecution::Direct,
                    children: Vec::new(),
                },
            },
            platform,
        }
    }
}
