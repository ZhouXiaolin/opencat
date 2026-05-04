#![cfg(feature = "host-default")]

use std::sync::Arc;

use crate::core::text::{DefaultFontProvider, default_font_db_with_embedded_only};

pub fn default_font_db_with_system() -> fontdb::Database {
    let mut db = default_font_db_with_embedded_only();
    db.load_system_fonts();
    db
}

impl DefaultFontProvider {
    pub fn with_system_fonts() -> Self {
        Self::from_arc(Arc::new(default_font_db_with_system()))
    }
}
