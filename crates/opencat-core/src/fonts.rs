//! Document-level font declarations (`<fonts>` / `<font>`) and `fontdb` assembly.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use std::collections::HashSet;

use crate::style::NodeStyle;

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

impl FontRole {
    /// The CSS generic-family class a role stands for. Emoji faces are
    /// presentation-selected, not generic-selected, so they map to `None`.
    pub fn generic_family(self) -> Option<crate::style::GenericFamily> {
        match self {
            Self::Sans => Some(crate::style::GenericFamily::SansSerif),
            Self::Mono => Some(crate::style::GenericFamily::Monospace),
            Self::Emoji => None,
        }
    }
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

    /// Apply `font-sans` / `font-[id]` on a resolved node style (DOM build time).
    ///
    /// A face whose `<font role>` declares a generic class (`mono`/`sans`) seeds
    /// `font_generic` unless the class already carried an explicit generic tail —
    /// the use-site stack wins, exactly as in CSS.
    pub fn apply_font_refs_to_style(&self, index: &FontFamilyIndex, style: &mut NodeStyle) {
        if style.use_document_default_font {
            if let Some(family) = self.default_family(index) {
                style.font_family = Some(family.to_string());
            }
        }
        if let Some(id) = style.font_face_id.take() {
            if let Some(family) = self.family_for_id(index, &id) {
                style.font_family = Some(family.to_string());
                if style.font_generic.is_none() {
                    style.font_generic = self
                        .face_by_id(&id)
                        .and_then(|face| face.role)
                        .and_then(FontRole::generic_family);
                }
            } else {
                style.font_face_id = Some(id);
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

/// Load document fonts first, then append faces from a host base database whose
/// families are not already present.
///
/// Used by lifecycle `prepare`: hosts supply base defaults (engine Noto, web
/// Noto/extra); core is the only place that merges document precedence, family
/// index, and sans-serif selection.
pub fn merge_document_over_base(
    base: &fontdb::Database,
    manifest: &FontManifest,
    bytes_by_face_id: &HashMap<String, Vec<u8>>,
) -> Result<(fontdb::Database, FontFamilyIndex)> {
    if manifest.is_empty() {
        return Ok((clone_font_db(base), FontFamilyIndex::default()));
    }

    let (mut db, index) = load_faces_into_db(fontdb::Database::new(), manifest, bytes_by_face_id)?;
    let mut families = family_names_in_db(&db);

    for face in base.faces() {
        if face
            .families
            .iter()
            .any(|(family, _)| families.contains(family))
        {
            continue;
        }
        match &face.source {
            fontdb::Source::Binary(data) => {
                db.load_font_data(data.as_ref().as_ref().to_vec());
            }
            fontdb::Source::File(path) => {
                // Best-effort; hosts that care about file-backed faces should
                // re-load them as binary into the base db they pass in.
                let _ = db.load_font_file(path);
            }
            fontdb::Source::SharedFile(_path, data) => {
                db.load_font_data(data.as_ref().as_ref().to_vec());
            }
        }
        families = family_names_in_db(&db);
    }

    Ok((db, index))
}

/// Shallow clone of a fontdb by reloading every face source into a new database.
/// Preserves sans-serif family when set.
pub fn clone_font_db(db: &fontdb::Database) -> fontdb::Database {
    let mut next = fontdb::Database::new();
    for face in db.faces() {
        match &face.source {
            fontdb::Source::Binary(data) => {
                next.load_font_data(data.as_ref().as_ref().to_vec());
            }
            fontdb::Source::File(path) => {
                let _ = next.load_font_file(path);
            }
            fontdb::Source::SharedFile(_path, data) => {
                next.load_font_data(data.as_ref().as_ref().to_vec());
            }
        }
    }
    // Re-apply sans-serif if the source db had a concrete family for it.
    let sans = db.family_name(&fontdb::Family::SansSerif).to_string();
    if !sans.is_empty() {
        next.set_sans_serif_family(sans);
    }
    next
}

fn family_names_in_db(db: &fontdb::Database) -> HashSet<String> {
    db.faces()
        .flat_map(|face| face.families.iter().map(|(family, _)| family.clone()))
        .collect()
}

pub fn font_asset_id(source: &FontSource) -> String {
    match source {
        FontSource::Path(p) => format!("font:path:{}", p.to_string_lossy()),
        FontSource::Url(u) => format!("font:url:{u}"),
    }
}


