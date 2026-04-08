use std::{cell::RefCell, collections::HashMap, rc::Rc};

use skia_safe::{Image as SkiaImage, Picture};

pub(crate) type SkiaImageCache = Rc<RefCell<HashMap<String, Option<SkiaImage>>>>;
pub(crate) type SkiaTextSnapshotCache = Rc<RefCell<HashMap<u64, Picture>>>;
pub(crate) type SkiaSubtreeSnapshotCache = Rc<RefCell<HashMap<u64, Picture>>>;

pub(crate) fn new_image_cache() -> SkiaImageCache {
    Rc::new(RefCell::new(HashMap::new()))
}

pub(crate) fn new_text_snapshot_cache() -> SkiaTextSnapshotCache {
    Rc::new(RefCell::new(HashMap::new()))
}

pub(crate) fn new_subtree_snapshot_cache() -> SkiaSubtreeSnapshotCache {
    Rc::new(RefCell::new(HashMap::new()))
}
