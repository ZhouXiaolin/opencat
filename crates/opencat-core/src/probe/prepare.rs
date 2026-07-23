//! Host-owned resource metadata preparation — caption hydration only.
//!
//! Image/video/Lottie probing has moved to hosts (issue #40). Hosts probe bytes
//! themselves and insert metadata directly via [`HostInputs::insert_*`].
//! Core retains SRT caption hydration ([`hydrate_captions`]) and the pure SRT
//! parser ([`parse_srt`]) per the spec: "Core retains XML, JSONL, Tailwind-class,
//! and SRT semantic parsing."

use std::collections::HashMap;

use anyhow::Result;

use crate::ir::asset_id::{AssetId, asset_id_for_subtitle};

/// Hydrate caption entries into a [`ParsedComposition`] using host-supplied
/// SRT text, keyed by canonical subtitle `AssetId`.
///
/// Contract (issue #2 / #4):
/// - No file system access — the host has already fetched and decoded the SRT
///   bytes; this only runs the pure [`parse_srt`] over the text.
/// - **Existing entries are never overwritten.** A caption node that already
///   carries entries (e.g. hydrated in a prior pass) is left untouched.
/// - **Missing entries stay empty.** A caption whose source id has no text in
///   `srt_by_id` keeps its (possibly empty) entry list; it is not an error.
/// - Returns the count of caption nodes that were hydrated this call.
///
/// `fps` must be the composition fps so SRT timestamps map to composition
/// frames.
pub fn hydrate_captions(
    root: crate::parse::node::Node,
    fps: u32,
    srt_by_id: &HashMap<AssetId, String>,
) -> Result<(crate::parse::node::Node, usize)> {
    let mut hydrated = 0usize;
    let node = walk_hydrate(root, fps, srt_by_id, &mut hydrated)?;
    Ok((node, hydrated))
}

fn walk_child_list(
    children: &[crate::parse::node::Node],
    fps: u32,
    srt_by_id: &HashMap<AssetId, String>,
    hydrated: &mut usize,
) -> Result<Vec<crate::parse::node::Node>> {
    children
        .iter()
        .cloned()
        .map(|c| walk_hydrate(c, fps, srt_by_id, hydrated))
        .collect()
}

fn walk_hydrate(
    node: crate::parse::node::Node,
    fps: u32,
    srt_by_id: &HashMap<AssetId, String>,
    hydrated: &mut usize,
) -> Result<crate::parse::node::Node> {
    use crate::parse::node::NodeKind;

    let mut kind = node.kind().clone();
    match &mut kind {
        NodeKind::Caption(caption) => {
            // Existing entries are authoritative — never overwrite.
            if !caption.entries_ref().is_empty() {
                return Ok(crate::parse::node::Node::new(kind));
            }
            let id = asset_id_for_subtitle(caption.source());
            if let Some(text) = srt_by_id.get(&id) {
                let entries = parse_srt(text, fps)?;
                caption.set_entries(entries);
                *hydrated += 1;
            }
            // Missing text => entries stay empty (not an error).
        }
        NodeKind::Div(div) => {
            let children = walk_child_list(div.children_ref(), fps, srt_by_id, hydrated)?;
            div.set_children(children);
        }
        NodeKind::Video(video) => {
            let children = walk_child_list(video.children_ref(), fps, srt_by_id, hydrated)?;
            video.set_children(children);
        }
        NodeKind::Timeline(tl) => {
            tl.map_scene_nodes(|scene| walk_hydrate(scene, fps, srt_by_id, hydrated))?;
        }
        NodeKind::Canvas(canvas) => {
            let hidden = walk_child_list(canvas.hidden_children_ref(), fps, srt_by_id, hydrated)?;
            canvas.set_hidden_children(hidden);
        }
        NodeKind::Image(_)
        | NodeKind::Text(_)
        | NodeKind::Lottie(_)
        | NodeKind::Lucide(_)
        | NodeKind::Path(_) => {}
    }

    Ok(crate::parse::node::Node::new(kind))
}

/// Re-export of the pure SRT parser.
pub use crate::parse::primitives::parse_srt;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::asset_id::AssetId;
    use crate::parse::primitives::{SrtEntry, SubtitleSource, caption, div, video};
    use std::collections::HashMap;

    fn caption_in_div() -> crate::parse::node::Node {
        let cap: crate::parse::node::Node = caption().id("subs").path("/tmp/sub.srt").into();
        div().id("root").child(cap).into()
    }

    #[test]
    fn hydrate_captions_parses_srt_without_fs_access() {
        let root = caption_in_div();
        let id = asset_id_for_subtitle(&SubtitleSource::Path("/tmp/sub.srt".into()));
        let mut srt = HashMap::new();
        srt.insert(id, "1\n00:00:00,000 --> 00:00:01,000\nHello\n".to_string());

        let (root, count) = hydrate_captions(root, 30, &srt).unwrap();
        assert_eq!(count, 1, "one caption node should be hydrated");

        use crate::parse::node::NodeKind;
        let NodeKind::Div(div) = root.kind() else {
            panic!("expected div root");
        };
        let NodeKind::Caption(cap) = div.children_ref()[0].kind() else {
            panic!("expected caption child");
        };
        assert_eq!(cap.entries_ref().len(), 1);
        assert_eq!(cap.active_text(0), Some("Hello"));
    }

    #[test]
    fn hydrate_captions_does_not_overwrite_existing_entries() {
        let pre = caption()
            .id("subs")
            .path("/tmp/sub.srt")
            .entries(vec![SrtEntry {
                index: 99,
                start_frame: 0,
                end_frame: 10,
                text: "pre-existing".into(),
            }]);
        let root: crate::parse::node::Node = div().id("root").child(pre).into();

        let id = asset_id_for_subtitle(&SubtitleSource::Path("/tmp/sub.srt".into()));
        let mut srt = HashMap::new();
        srt.insert(
            id,
            "1\n00:00:00,000 --> 00:00:01,000\nSHOULD NOT WIN\n".to_string(),
        );

        let (root, count) = hydrate_captions(root, 30, &srt).unwrap();
        assert_eq!(count, 0, "pre-existing entries must not be touched");

        use crate::parse::node::NodeKind;
        let NodeKind::Div(div) = root.kind() else {
            panic!("expected div root");
        };
        let NodeKind::Caption(cap) = div.children_ref()[0].kind() else {
            panic!("expected caption child");
        };
        assert_eq!(cap.entries_ref().len(), 1);
        assert_eq!(cap.entries_ref()[0].text, "pre-existing");
        assert_eq!(cap.active_text(0), Some("pre-existing"));
    }

    #[test]
    fn hydrate_captions_missing_text_keeps_entries_empty() {
        let root = caption_in_div();
        let (root, count) =
            hydrate_captions(root, 30, &HashMap::new()).expect("missing text is not an error");
        assert_eq!(count, 0);

        use crate::parse::node::NodeKind;
        let NodeKind::Div(div) = root.kind() else {
            panic!("expected div root");
        };
        let NodeKind::Caption(cap) = div.children_ref()[0].kind() else {
            panic!("expected caption child");
        };
        assert!(
            cap.entries_ref().is_empty(),
            "missing srt must leave entries empty"
        );
    }

    #[test]
    fn hydrate_captions_recurses_into_video_children() {
        let cap: crate::parse::node::Node = caption().id("subs").path("/tmp/sub.srt").into();
        let vid: crate::parse::node::Node = video("/clip.mp4").id("vid").child(cap).into();
        let root: crate::parse::node::Node = div().id("root").child(vid).into();

        let id = asset_id_for_subtitle(&SubtitleSource::Path("/tmp/sub.srt".into()));
        let mut srt = HashMap::new();
        srt.insert(id, "1\n00:00:00,000 --> 00:00:01,000\nHi\n".to_string());

        let (root, count) = hydrate_captions(root, 30, &srt).unwrap();
        assert_eq!(count, 1);

        use crate::parse::node::NodeKind;
        let NodeKind::Div(div) = root.kind() else {
            panic!("expected div root");
        };
        let NodeKind::Video(vid) = div.children_ref()[0].kind() else {
            panic!("expected video child");
        };
        let NodeKind::Caption(cap) = vid.children_ref()[0].kind() else {
            panic!("expected caption under video");
        };
        assert_eq!(cap.active_text(0), Some("Hi"));
    }

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
