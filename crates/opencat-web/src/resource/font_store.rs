//! Thread-local storage for font bytes preloaded from markup `<fonts>`.

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static FONT_BYTES: RefCell<HashMap<String, Vec<u8>>> = RefCell::new(HashMap::new());
}

pub fn insert(id: String, bytes: Vec<u8>) {
    FONT_BYTES.with(|store| {
        store.borrow_mut().insert(id, bytes);
    });
}

pub fn take_all() -> HashMap<String, Vec<u8>> {
    FONT_BYTES.with(|store| std::mem::take(&mut *store.borrow_mut()))
}

pub fn get_manifest_bytes(manifest: &opencat_core::resource::fonts::FontManifest) -> HashMap<String, Vec<u8>> {
    FONT_BYTES.with(|store| {
        let store = store.borrow();
        manifest
            .faces
            .iter()
            .filter_map(|face| {
                store
                    .get(&face.id)
                    .map(|b| (face.id.clone(), b.clone()))
            })
            .collect()
    })
}

pub fn clear() {
    FONT_BYTES.with(|store| store.borrow_mut().clear());
}

pub fn len() -> usize {
    FONT_BYTES.with(|store| store.borrow().len())
}