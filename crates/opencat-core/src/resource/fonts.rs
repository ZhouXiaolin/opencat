//! Document-level font declarations (`<fonts>` / `<font>`) and `fontdb` assembly.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use std::collections::HashSet;

/// Where to load a font binary from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FontSource {
    Path(PathBuf),
    Url(String),
}

/// Optional semantic role for fallback / default selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontRole {
    Sans,
    Emoji,
    Mono,
}

/// One `<font>` entry from markup.
#[derive(Debug, Clone)]
pub struct FontFaceDecl {
    pub id: String,
    /// PostScript / full name used by cosmic-text (`font-family`). When omitted, inferred after load.
    pub family: Option<String>,
    pub source: FontSource,
    pub role: Option<FontRole>,
}

/// Parsed `<fonts>` block.
#[derive(Debug, Clone, Default)]
pub struct FontManifest {
    /// `default` attribute on `<fonts>` — id of the default sans face.
    pub default_face_id: Option<String>,
    pub faces: Vec<FontFaceDecl>,
}

impl FontManifest {
    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// Family names for Tailwind `font-[id]` / `font-sans` resolution (no fontdb required).
    pub fn build_family_index(&self) -> FontFamilyIndex {
        let mut index = FontFamilyIndex::default();
        for face in &self.faces {
            let family = face.family.clone().unwrap_or_else(|| face.id.clone());
            index.id_to_family.insert(face.id.clone(), family);
        }
        index
    }

    pub fn face_by_id(&self, id: &str) -> Option<&FontFaceDecl> {
        self.faces.iter().find(|f| f.id == id)
    }

    /// Resolve a manifest face id to the family name used in shaping.
    pub fn family_for_id<'a>(&'a self, index: &'a FontFamilyIndex, id: &str) -> Option<&'a str> {
        index.id_to_family.get(id).map(String::as_str)
    }

    pub fn default_family<'a>(&'a self, index: &'a FontFamilyIndex) -> Option<&'a str> {
        let id = self.default_face_id.as_deref()?;
        self.family_for_id(index, id)
    }

    /// Apply `font-sans` / `font-[id]` on parsed element styles before tree build.
    pub fn apply_font_refs_to_styles(
        &self,
        index: &FontFamilyIndex,
        elements: &mut [crate::parse::document::ParsedElement],
    ) {
        for el in elements.iter_mut() {
            let style = &mut el.style;
            if style.use_document_default_font {
                if let Some(family) = self.default_family(index) {
                    style.font_family = Some(family.to_string());
                }
            }
            if let Some(id) = style.font_face_id.take() {
                if let Some(family) = self.family_for_id(index, &id) {
                    style.font_family = Some(family.to_string());
                } else {
                    style.font_face_id = Some(id);
                }
            }
        }
    }
}

/// Maps manifest face ids to loaded family names.
#[derive(Debug, Clone, Default)]
pub struct FontFamilyIndex {
    pub id_to_family: HashMap<String, String>,
}

/// Load font bytes into `db`. Returns updated db and id → family index.
pub fn load_faces_into_db(
    mut db: fontdb::Database,
    manifest: &FontManifest,
    bytes_by_id: &HashMap<String, Vec<u8>>,
) -> Result<(fontdb::Database, FontFamilyIndex)> {
    let mut index = FontFamilyIndex::default();

    for face in &manifest.faces {
        let bytes = bytes_by_id
            .get(&face.id)
            .ok_or_else(|| anyhow!("font `{}`: bytes not loaded", face.id))?;
        db.load_font_data(bytes.clone());
        let family = face.family.clone().unwrap_or_else(|| face.id.clone());
        index.id_to_family.insert(face.id.clone(), family);
    }

    if let Some(default_id) = manifest.default_face_id.as_deref() {
        if let Some(family) = index.id_to_family.get(default_id) {
            db.set_sans_serif_family(family);
        }
    } else if let Some(face) = manifest
        .faces
        .iter()
        .find(|f| f.role == Some(FontRole::Sans))
    {
        if let Some(family) = index.id_to_family.get(&face.id) {
            db.set_sans_serif_family(family);
        }
    } else if let Some((_, family)) = index.id_to_family.iter().next() {
        db.set_sans_serif_family(family);
    }

    Ok((db, index))
}

/// Merge manifest faces into an existing database (e.g. engine defaults).
pub fn merge_faces_into_db(
    db: fontdb::Database,
    manifest: &FontManifest,
    bytes_by_id: &HashMap<String, Vec<u8>>,
) -> Result<(fontdb::Database, FontFamilyIndex)> {
    if manifest.is_empty() {
        return Ok((db, FontFamilyIndex::default()));
    }
    load_faces_into_db(db, manifest, bytes_by_id)
}

/// Load document fonts first, then append fallback faces whose family is absent.
///
/// This keeps `<fonts>` authoritative for its declared/embedded families. For example,
/// when the document provides `Noto Sans SC` via URL, an embedded `Noto Sans SC`
/// fallback must not be added afterward, otherwise CSS-like weight matching may pick
/// the embedded face instead of the document face.
pub fn load_faces_with_fallbacks(
    manifest: &FontManifest,
    bytes_by_id: &HashMap<String, Vec<u8>>,
    fallback_faces: &[(&str, &[u8])],
) -> Result<(fontdb::Database, FontFamilyIndex)> {
    let (mut db, index) = load_faces_into_db(fontdb::Database::new(), manifest, bytes_by_id)?;
    let mut families = family_names_in_db(&db);

    for (family, bytes) in fallback_faces {
        if families.contains(*family) {
            continue;
        }
        db.load_font_data(bytes.to_vec());
        families = family_names_in_db(&db);
    }

    Ok((db, index))
}

fn family_names_in_db(db: &fontdb::Database) -> HashSet<String> {
    db.faces()
        .flat_map(|face| face.families.iter().map(|(family, _)| family.clone()))
        .collect()
}

pub fn resolve_font_source_path(path: &str, base_dir: Option<&Path>) -> Result<PathBuf> {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        return Ok(p);
    }
    if let Some(base) = base_dir {
        let joined = base.join(&p);
        if joined.exists() {
            return Ok(joined);
        }
    }
    if p.exists() {
        return Ok(p);
    }
    Err(anyhow!("font path not found: {path}"))
}

/// Load every face in `manifest` into memory.
pub fn fetch_manifest_bytes(
    manifest: &FontManifest,
    base_dir: Option<&Path>,
    mut read_path: impl FnMut(&Path) -> Result<Vec<u8>>,
    mut fetch_url: impl FnMut(&str) -> Result<Vec<u8>>,
) -> Result<HashMap<String, Vec<u8>>> {
    let mut out = HashMap::new();
    for face in &manifest.faces {
        let bytes = match &face.source {
            FontSource::Path(path) => {
                let resolved = resolve_font_source_path(&path.to_string_lossy(), base_dir)
                    .with_context(|| format!("font `{}`", face.id))?;
                read_path(&resolved)?
            }
            FontSource::Url(url) => {
                fetch_url(url).with_context(|| format!("font `{}` url `{url}`", face.id))?
            }
        };
        out.insert(face.id.clone(), bytes);
    }
    Ok(out)
}

pub fn font_asset_id(source: &FontSource) -> String {
    match source {
        FontSource::Path(p) => format!("font:path:{}", p.to_string_lossy()),
        FontSource::Url(u) => format!("font:url:{u}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_manifest_sets_sans_serif_from_default_id() {
        let bytes = include_bytes!("../../../../assets/NotoSansSC-Regular.otf").to_vec();
        let mut map = HashMap::new();
        map.insert("sans".to_string(), bytes);
        let manifest = FontManifest {
            default_face_id: Some("sans".to_string()),
            faces: vec![FontFaceDecl {
                id: "sans".to_string(),
                family: Some("Noto Sans SC".to_string()),
                source: FontSource::Path(PathBuf::from("NotoSansSC-Regular.otf")),
                role: Some(FontRole::Sans),
            }],
        };
        let (db, index) =
            load_faces_into_db(fontdb::Database::new(), &manifest, &map).expect("load");
        assert_eq!(
            index.id_to_family.get("sans").map(String::as_str),
            Some("Noto Sans SC")
        );
        assert_eq!(db.family_name(&fontdb::Family::SansSerif), "Noto Sans SC");
    }

    #[test]
    fn document_fonts_take_precedence_over_same_family_fallback() {
        let sans = include_bytes!("../../../../assets/NotoSansSC-Regular.otf").to_vec();
        let emoji = include_bytes!("../../../../assets/NotoColorEmoji.ttf").to_vec();
        let mut map = HashMap::new();
        map.insert("doc-sans".to_string(), sans.clone());
        let manifest = FontManifest {
            default_face_id: Some("doc-sans".to_string()),
            faces: vec![FontFaceDecl {
                id: "doc-sans".to_string(),
                family: Some("Noto Sans SC".to_string()),
                source: FontSource::Path(PathBuf::from("NotoSansSC-Regular.otf")),
                role: Some(FontRole::Sans),
            }],
        };

        let (db, _) = load_faces_with_fallbacks(
            &manifest,
            &map,
            &[
                ("Noto Sans SC", sans.as_slice()),
                ("Noto Color Emoji", emoji.as_slice()),
            ],
        )
        .expect("load");

        let noto_faces = db
            .faces()
            .filter(|face| {
                face.families
                    .iter()
                    .any(|(family, _)| family == "Noto Sans SC")
            })
            .count();
        let emoji_faces = db
            .faces()
            .filter(|face| {
                face.families
                    .iter()
                    .any(|(family, _)| family == "Noto Color Emoji")
            })
            .count();

        assert_eq!(noto_faces, 1);
        assert_eq!(emoji_faces, 1);
        assert_eq!(db.family_name(&fontdb::Family::SansSerif), "Noto Sans SC");
    }
}
