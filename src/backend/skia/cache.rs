use std::{cell::RefCell, collections::HashMap, rc::Rc};

use skia_safe::{Image as SkiaImage, Picture};

pub(crate) type SkiaImageCache = Rc<RefCell<HashMap<String, Option<SkiaImage>>>>;
pub(crate) type SkiaTextPictureCache = Rc<RefCell<HashMap<u64, Picture>>>;
pub(crate) type SkiaSubtreePictureCache = Rc<RefCell<HashMap<u64, Picture>>>;

pub(crate) fn new_image_cache() -> SkiaImageCache {
    Rc::new(RefCell::new(HashMap::new()))
}

pub(crate) fn new_text_picture_cache() -> SkiaTextPictureCache {
    Rc::new(RefCell::new(HashMap::new()))
}

pub(crate) fn new_subtree_picture_cache() -> SkiaSubtreePictureCache {
    Rc::new(RefCell::new(HashMap::new()))
}
