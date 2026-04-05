use std::{cell::RefCell, collections::HashMap, rc::Rc};

use skia_safe::{Image as SkiaImage, Picture};

pub(crate) type ImageCache = Rc<RefCell<HashMap<String, Option<SkiaImage>>>>;
pub(crate) type TextPictureCache = Rc<RefCell<HashMap<u64, Picture>>>;
pub(crate) type SubtreePictureCache = Rc<RefCell<HashMap<u64, Picture>>>;

pub(crate) fn new_image_cache() -> ImageCache {
    Rc::new(RefCell::new(HashMap::new()))
}

pub(crate) fn new_text_picture_cache() -> TextPictureCache {
    Rc::new(RefCell::new(HashMap::new()))
}

pub(crate) fn new_subtree_picture_cache() -> SubtreePictureCache {
    Rc::new(RefCell::new(HashMap::new()))
}
