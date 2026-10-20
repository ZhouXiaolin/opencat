use crate::fonts::{FontFamilyIndex, merge_faces_into_db};
use crate::parse::document::DeclaredScript;
use crate::script::ScriptDriver;

pub fn join_scripts(scripts: Vec<String>) -> Option<String> {
    if scripts.is_empty() {
        None
    } else {
        Some(scripts.join("\n"))
    }
}

/// Build a [`ScriptDriver`] from one or more declarations.
///
/// - All-inline → single driver with joined source.
/// - Any external → one driver (first external wins for asset id) with remaining
///   pieces recorded; prepare injects external text then re-joins.
///
/// Multiple externals on the same parent are concatenated at prepare in order.
pub fn script_driver_from_decls(
    scripts: Vec<DeclaredScript>,
) -> anyhow::Result<Option<ScriptDriver>> {
    if scripts.is_empty() {
        return Ok(None);
    }

    let all_inline = scripts
        .iter()
        .all(|s| matches!(s, DeclaredScript::Inline(_)));
    if all_inline {
        let joined = scripts
            .into_iter()
            .map(|s| match s {
                DeclaredScript::Inline(t) => t,
                DeclaredScript::External { .. } => unreachable!(),
            })
            .collect::<Vec<_>>()
            .join("\n");
        return Ok(Some(ScriptDriver::from_source(&joined)?));
    }

    // Mixed or pure-external: keep pieces. Use the first external locator as the
    // primary asset id for requirements; remaining externals are also required
    // via the driver's external list (stored as joined locators at prepare).
    // For install identity we hash the ordered locator list.
    let mut pieces_inline = Vec::new();
    let mut externals = Vec::new();
    for s in scripts {
        match s {
            DeclaredScript::Inline(t) => pieces_inline.push(t),
            DeclaredScript::External { locator } => externals.push(locator),
        }
    }

    if externals.len() == 1 && pieces_inline.is_empty() {
        return Ok(Some(ScriptDriver::from_external(externals.remove(0))));
    }

    // Multiple externals / mix: encode as a single synthetic external whose
    // host must supply the fully joined text under a compound key, plus we
    // also declare each path as a requirement. Simpler contract: require host
    // to supply each external piece; prepare joins with inlines.
    Ok(Some(ScriptDriver::from_pieces(pieces_inline, externals)))
}

/// Build fontdb + family index from manifest bytes merged into `base_db`.
pub fn build_font_resources(
    base_db: fontdb::Database,
    manifest: &crate::fonts::FontManifest,
    bytes_by_id: &std::collections::HashMap<String, Vec<u8>>,
) -> anyhow::Result<(fontdb::Database, FontFamilyIndex)> {
    merge_faces_into_db(base_db, manifest, bytes_by_id).map_err(|e| anyhow::anyhow!("{e}"))
}
