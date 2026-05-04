use std::sync::Arc;

use opencat_core::text::{DefaultFontProvider, default_font_db_with_embedded_only};

pub fn default_font_db_with_system() -> fontdb::Database {
    let mut db = default_font_db_with_embedded_only();
    db.load_system_fonts();
    db
}

pub fn default_font_provider_with_system() -> DefaultFontProvider {
    DefaultFontProvider::from_arc(Arc::new(default_font_db_with_system()))
}
