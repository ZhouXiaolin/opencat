//! Host-owned resource metadata preparation.
//!
//! Image/video/Lottie probing has moved to hosts (issue #40). Hosts probe bytes
//! themselves and insert metadata directly via [`HostInputs::insert_*`].

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    // --- font-db host contract (regression guard) ----------------------------------

    #[test]
    fn font_database_is_built_from_host_bytes_core_owns_shaping() {
        use crate::fonts::{FontFaceDecl, FontManifest, FontRole, FontSource, load_faces_into_db};
        let bytes = include_bytes!("../../../../assets/NotoSansSC-Regular.otf").to_vec();
        let mut map = HashMap::new();
        map.insert("sans".to_string(), bytes);
        let manifest = FontManifest {
            default_face_id: Some("sans".into()),
            faces: vec![FontFaceDecl {
                id: "sans".into(),
                family: Some("Noto Sans SC".into()),
                source: FontSource::Path("NotoSansSC-Regular.otf".into()),
                role: Some(FontRole::Sans),
            }],
        };
        let (db, _index) =
            load_faces_into_db(fontdb::Database::new(), &manifest, &map).expect("load");
        assert_eq!(
            db.family_name(&fontdb::Family::SansSerif),
            "Noto Sans SC",
            "core must own matching/shaping after host supplies bytes"
        );
    }
}
