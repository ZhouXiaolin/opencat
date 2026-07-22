//! Host-driven resource metadata preparation chain.
//!
//! This module is the host-facing entry point for building a [`PreparedResourceCatalog`]
//! and hydrating a [`ParsedComposition`] **without** core ever touching a file
//! system, network, cache, or decoder. The split is:
//!
//! - **Host** owns fetch / cache / decode. After fetching bytes it calls core's
//!   *pure* probe functions and assembles a [`PreparedCatalog`] bundle.
//! - **Core** owns the pure metadata derivation: [`build_catalog`] maps
//!   host-supplied bytes to canonical [`AssetId`] metadata via the pure
//!   `probe_image` / `probe_video` / `parse_lottie_meta` functions; subtitles
//!   are hydrated into a `ParsedComposition` via [`hydrate_captions`]; fonts
//!   stay core-owned via the existing `load_faces_into_db`.
//!
//! Error boundary (issue #2 / #4 contract):
//! - A **missing** byte payload for a declared asset is treated as *probe
//!   failure* by core (the catalog simply omits that asset's metadata, leaving
//!   layout fallback / render-error policy to decide). The host is responsible
//!   for surfacing genuine fetch/cache/decode failures as host errors; core
//!   never observes them.
//! - A byte payload that is present but unparseable is likewise an omission, not
//!   a core panic: [`ProbeOutcome`] records it so a host can choose to fail
//!   loudly or fall back.

use std::collections::HashMap;

use anyhow::Result;

use crate::ir::asset_id::{
    asset_id_for_audio, asset_id_for_image, asset_id_for_lottie, asset_id_for_subtitle,
    asset_id_for_video, asset_id_for_url, AssetId,
};
use crate::parse::primitives::LottieSource;
use crate::probe::catalog::{PreparedResourceCatalog, ResourceRequests};
use crate::probe::probe::{probe_image, probe_video};

use super::catalog::LottieRequest;

/// Bytes a host has resolved for the declared [`ResourceRequests`].
///
/// Keys are canonical [`AssetId`](crate::ir::asset_id::AssetId) strings. A key
/// absent from this map means "the host did not (yet) provide bytes for this
/// declared asset" — see the module docs for how that is interpreted.
///
/// `B` lets callers borrow (`&HashMap<…>`) or own (`HashMap<…>`); the helpers
/// only need `B: ByteSource`.
pub trait ByteSource {
    /// Bytes for `id`, if the host has resolved them.
    fn bytes_for(&self, id: &str) -> Option<&[u8]>;
}

impl ByteSource for HashMap<String, Vec<u8>> {
    fn bytes_for(&self, id: &str) -> Option<&[u8]> {
        self.get(id).map(Vec::as_slice)
    }
}

impl ByteSource for HashMap<crate::ir::asset_id::AssetId, Vec<u8>> {
    fn bytes_for(&self, id: &str) -> Option<&[u8]> {
        // Legacy probe path: only the canonical wire string is known here, so
        // recover the kind best-effort (asset_id.rs `kind_from_canonical_str`)
        // purely to look the typed id up in the map. This is the one place that
        // round-trips a string id; production lifecycle never reconstructs ids.
        let kind = crate::ir::asset_id::kind_from_canonical_str(id);
        self.get(&crate::ir::asset_id::AssetId::new(kind, id))
            .map(Vec::as_slice)
    }
}

/// Per-asset outcome recorded while building a catalog. Lets a host distinguish
/// "probed" from "omitted" (missing bytes or unparseable bytes) without core
/// inventing a host failure category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeOutcome {
    /// Metadata was derived and stored in the catalog.
    Probed,
    /// No bytes were supplied for this declared asset.
    BytesMissing,
    /// Bytes were supplied but the pure probe rejected them.
    /// `reason` is a short, host-displayable diagnostic.
    ProbeFailed { reason: String },
}

/// Result of [`build_catalog`]: the assembled catalog plus a per-asset outcome
/// map (keyed by canonical `AssetId` string) so the host can report probe
/// failures or decide to abort.
#[derive(Debug, Default)]
pub struct PreparedCatalog {
    pub catalog: PreparedResourceCatalog,
    pub outcomes: HashMap<String, ProbeOutcome>,
}

impl PreparedCatalog {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Build a [`PreparedResourceCatalog`] purely from host-supplied bytes.
///
/// Iterates the declarative, order-independent [`ResourceRequests`] exactly as
/// the host received them; for each declared asset it looks up bytes via
/// `sources` and runs the matching *pure* probe. Core never fetches here. A
/// missing/unparseable asset is recorded in `outcomes` and omitted from the
/// catalog rather than aborting the whole build.
///
/// Subtitle *bytes* are **not** consumed here: per the #2/#4 contract, the host
/// parses SRT and hydrates caption entries into the `ParsedComposition` via
/// [`hydrate_captions`] before opening the pipeline, so the catalog never stores
/// a duplicate subtitle map's text. (Audio sources carry no core metadata, so
/// they are only registered by id.)
pub fn build_catalog<S: ByteSource>(requests: &ResourceRequests, sources: &S) -> PreparedCatalog {
    let mut prepared = PreparedCatalog::new();

    for src in &requests.images {
        let Some(id) = asset_id_for_image(src) else { continue };
        record_image(&mut prepared, &id, sources.bytes_for(&id.key));
    }

    for src in &requests.videos {
        let id = asset_id_for_video(src);
        record_video(&mut prepared, &id, sources.bytes_for(&id.key));
    }

    for src in &requests.audios {
        if let Some(id) = asset_id_for_audio(src) {
            prepared.catalog.audios.insert(id.clone());
            prepared.outcomes.insert(id.key, ProbeOutcome::Probed);
        }
    }

    // Subtitle source ids are registered (canonical id exists) but their *text*
    // lives in the parsed caption node after `hydrate_captions`; the catalog
    // intentionally does not duplicate it.
    for src in &requests.subtitles {
        let id = asset_id_for_subtitle(src);
        prepared
            .catalog
            .subtitles
            .entry(id.clone())
            .or_default();
        prepared.outcomes.insert(id.key, ProbeOutcome::Probed);
    }

    for req in &requests.lotties {
        let Some(bundle_id) = asset_id_for_lottie(&req.element_id, &req.source) else {
            continue;
        };
        if matches!(req.source, LottieSource::Unset) {
            continue;
        }
        let key = lottie_byte_key(req);
        record_lottie(&mut prepared, &bundle_id, sources.bytes_for(&key));
    }

    prepared
}

/// Shared outcome-recording for a single probed asset. `probe` runs the pure
/// derivation over the host-supplied bytes; the result is stored into `store`
/// (the matching catalog map) under `id`, and the per-asset outcome is always
/// recorded. This is the single shape the image/video/lottie recorders share,
/// so the "missing bytes → BytesMissing / parse error → ProbeFailed /
/// success → Probed" boundary lives in exactly one place.
fn record_probed<T>(
    prepared: &mut PreparedCatalog,
    id: &crate::ir::asset_id::AssetId,
    bytes: Option<&[u8]>,
    probe: impl FnOnce(&[u8]) -> anyhow::Result<T>,
    store: impl FnOnce(&mut PreparedResourceCatalog, &crate::ir::asset_id::AssetId, T),
) {
    let Some(bytes) = bytes else {
        prepared
            .outcomes
            .insert(id.key.clone(), ProbeOutcome::BytesMissing);
        return;
    };
    match probe(bytes) {
        Ok(meta) => {
            store(&mut prepared.catalog, id, meta);
            prepared
                .outcomes
                .insert(id.key.clone(), ProbeOutcome::Probed);
        }
        Err(err) => {
            prepared.outcomes.insert(
                id.key.clone(),
                ProbeOutcome::ProbeFailed {
                    reason: err.to_string(),
                },
            );
        }
    }
}

fn record_image(prepared: &mut PreparedCatalog, id: &crate::ir::asset_id::AssetId, bytes: Option<&[u8]>) {
    record_probed(prepared, id, bytes, probe_image, |catalog, id, meta| {
        catalog.images.insert(id.clone(), meta);
    });
}

fn record_video(prepared: &mut PreparedCatalog, id: &crate::ir::asset_id::AssetId, bytes: Option<&[u8]>) {
    record_probed(prepared, id, bytes, probe_video, |catalog, id, meta| {
        catalog.videos.insert(id.clone(), meta);
    });
}

/// The byte-map key a host would use for a Lottie primary JSON: the path for a
/// `Path` source, the url-derived `AssetId` for a `Url` source. `Unset` yields
/// an empty string (callers skip `Unset` first).
fn lottie_byte_key(req: &LottieRequest) -> String {
    match &req.source {
        LottieSource::Unset => String::new(),
        LottieSource::Path(p) => p.clone(),
        LottieSource::Url(u) => asset_id_for_url(u).key,
    }
}

fn record_lottie(
    prepared: &mut PreparedCatalog,
    bundle_id: &crate::ir::asset_id::AssetId,
    bytes: Option<&[u8]>,
) {
    record_probed(
        prepared,
        bundle_id,
        bytes,
        |b| {
            let json = std::str::from_utf8(b)?;
            crate::resource::lottie::parse_lottie_meta(json)
        },
        |catalog, id, meta| {
            catalog.lotties.insert(id.clone(), meta);
        },
    );
}

/// Scan a Lottie primary JSON (host-supplied) for external asset file names.
///
/// This surfaces the existing pure [`scan_lottie_dependencies`] through the
/// prep API so the host can learn what to fetch *next* without reaching into
/// `resource::lottie`. The host still owns fetching the dependency bytes; core
/// only reads the JSON it already has.
///
/// [`scan_lottie_dependencies`]: crate::resource::lottie::scan_lottie_dependencies
pub fn lottie_dependencies(json: &str) -> anyhow::Result<Vec<String>> {
    crate::resource::lottie::scan_lottie_dependencies(json)
}

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

/// Re-export of the pure SRT parser used by [`hydrate_captions`].
pub use crate::parse::primitives::parse_srt;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::asset_id::AssetId;
    use crate::parse::primitives::{
        LottieSource, SrtEntry, SubtitleSource, caption, div, video,
    };
    use std::collections::HashMap;

    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    // --- build_catalog: image path --------------------------------------------------

    #[test]
    fn build_catalog_probes_image_bytes_and_records_outcome() {
        let mut req = ResourceRequests::default();
        req.images
            .insert(crate::parse::primitives::ImageSource::Path("/tmp/a.png".into()));
        let id = asset_id_for_image(req.images.iter().next().unwrap()).unwrap();

        let mut bytes = HashMap::<String, Vec<u8>>::new();
        bytes.insert(id.key.clone(), PNG_1X1.to_vec());

        let prepared = build_catalog(&req, &bytes);

        assert_eq!(
            prepared.catalog.images.get(&id).map(|m| (m.width, m.height)),
            Some((1, 1)),
            "image metadata should be probed and stored under canonical id"
        );
        assert_eq!(
            prepared.outcomes.get(&id.key),
            Some(&ProbeOutcome::Probed),
            "outcome should be recorded as Probed"
        );
    }

    #[test]
    fn build_catalog_omits_metadata_when_bytes_missing() {
        // Probe-failure boundary: a declared asset with no host bytes is simply
        // omitted; core does not invent a fetch error.
        let mut req = ResourceRequests::default();
        req.images
            .insert(crate::parse::primitives::ImageSource::Path("/tmp/missing.png".into()));
        let id = asset_id_for_image(req.images.iter().next().unwrap()).unwrap();

        let prepared = build_catalog(&req, &HashMap::<String, Vec<u8>>::new());

        assert!(
            !prepared.catalog.images.contains_key(&id),
            "missing bytes must not invent metadata"
        );
        assert_eq!(
            prepared.outcomes.get(&id.key),
            Some(&ProbeOutcome::BytesMissing)
        );
    }

    #[test]
    fn build_catalog_records_probe_failure_for_bad_bytes() {
        let mut req = ResourceRequests::default();
        req.images
            .insert(crate::parse::primitives::ImageSource::Path("/tmp/bad.png".into()));
        let id = asset_id_for_image(req.images.iter().next().unwrap()).unwrap();

        let mut bytes = HashMap::<String, Vec<u8>>::new();
        bytes.insert(id.key.clone(), b"not a png".to_vec());

        let prepared = build_catalog(&req, &bytes);

        assert!(
            !prepared.catalog.images.contains_key(&id),
            "unparseable bytes must not invent metadata"
        );
        match prepared.outcomes.get(&id.key) {
            Some(ProbeOutcome::ProbeFailed { .. }) => {}
            other => panic!("expected ProbeFailed, got {other:?}"),
        }
    }

    #[test]
    fn build_catalog_is_order_independent() {
        // Same requests + same bytes must yield equal catalogs regardless of
        // HashMap iteration order.
        let mut req = ResourceRequests::default();
        req.images
            .insert(crate::parse::primitives::ImageSource::Path("/tmp/a.png".into()));
        req.images
            .insert(crate::parse::primitives::ImageSource::Path("/tmp/b.png".into()));

        let mut bytes = HashMap::<String, Vec<u8>>::new();
        for src in &req.images {
            let id = asset_id_for_image(src).unwrap();
            bytes.insert(id.key, PNG_1X1.to_vec());
        }

        let a = build_catalog(&req, &bytes);
        let b = build_catalog(&req, &bytes);
        assert_eq!(a.catalog.images.len(), b.catalog.images.len());
        for (id, meta) in &a.catalog.images {
            assert_eq!(b.catalog.images.get(id), Some(meta));
        }
    }

    // --- build_catalog: audio & subtitle registration -------------------------------

    #[test]
    fn build_catalog_registers_audio_id_without_bytes() {
        // Audio has no core metadata; it is registered by canonical id only,
        // and needs no host bytes.
        let mut req = ResourceRequests::default();
        req.audios
            .insert(crate::parse::primitives::AudioSource::Url("m.mp3".into()));
        let id = asset_id_for_audio(req.audios.iter().next().unwrap()).unwrap();

        let prepared = build_catalog(&req, &HashMap::<String, Vec<u8>>::new());
        assert!(prepared.catalog.audios.contains(&id));
        assert_eq!(prepared.outcomes.get(&id.key), Some(&ProbeOutcome::Probed));
    }

    #[test]
    fn build_catalog_does_not_store_subtitle_text() {
        // Per contract, the catalog holds a canonical subtitle slot but the
        // caption *text* lives in the parsed node after hydrate_captions.
        let mut req = ResourceRequests::default();
        req.subtitles
            .insert(SubtitleSource::Path("/tmp/sub.srt".into()));
        let id = asset_id_for_subtitle(req.subtitles.iter().next().unwrap());

        let prepared = build_catalog(&req, &HashMap::<String, Vec<u8>>::new());
        assert!(prepared.catalog.subtitles.contains_key(&id));
        assert!(
            prepared.catalog.subtitles.get(&id).unwrap().is_empty(),
            "catalog must not duplicate subtitle text"
        );
    }

    // --- build_catalog: lottie ------------------------------------------------------

    #[test]
    fn build_catalog_probes_lottie_meta_from_host_bytes() {
        let json = r#"{"w":280,"h":200,"fr":25,"ip":0,"op":32,"assets":[]}"#;
        let mut req = ResourceRequests::default();
        req.lotties.insert(LottieRequest {
            element_id: "hero".into(),
            source: LottieSource::Url("https://e.com/a.json".into()),
        });
        let bundle_id = AssetId::new(crate::ir::asset_id::ResourceKind::Lottie, "lottie:hero");

        let mut bytes = HashMap::<String, Vec<u8>>::new();
        bytes.insert(
            asset_id_for_url("https://e.com/a.json").key,
            json.as_bytes().to_vec(),
        );

        let prepared = build_catalog(&req, &bytes);
        let meta = prepared.catalog.lotties.get(&bundle_id).expect("lottie meta");
        assert_eq!((meta.width, meta.height), (280, 200));
        assert_eq!(meta.fps, 25.0);
        assert_eq!(prepared.outcomes.get(&bundle_id.key), Some(&ProbeOutcome::Probed));
    }

    #[test]
    fn build_catalog_lottie_meta_includes_dependencies_from_json() {
        let json = r#"{"w":10,"h":10,"fr":30,"ip":0,"op":5,"assets":[{"u":"images/a.png"}]}"#;
        let mut req = ResourceRequests::default();
        req.lotties.insert(LottieRequest {
            element_id: "badge".into(),
            source: LottieSource::Path("badge.json".into()),
        });
        let bundle_id = AssetId::new(crate::ir::asset_id::ResourceKind::Lottie, "lottie:badge");
        let mut bytes = HashMap::<String, Vec<u8>>::new();
        bytes.insert("badge.json".into(), json.as_bytes().to_vec());
        let prepared = build_catalog(&req, &bytes);
        let meta = prepared.catalog.lotties.get(&bundle_id).expect("meta");
        assert_eq!(meta.dependencies, vec!["a.png".to_string()]);
    }

    #[test]
    fn build_catalog_lottie_missing_bytes_omitted() {
        let mut req = ResourceRequests::default();
        req.lotties.insert(LottieRequest {
            element_id: "hero".into(),
            source: LottieSource::Url("https://e.com/a.json".into()),
        });
        let bundle_id = AssetId::new(crate::ir::asset_id::ResourceKind::Lottie, "lottie:hero");

        let prepared = build_catalog(&req, &HashMap::<String, Vec<u8>>::new());
        assert!(!prepared.catalog.lotties.contains_key(&bundle_id));
        assert_eq!(
            prepared.outcomes.get(&bundle_id.key),
            Some(&ProbeOutcome::BytesMissing)
        );
    }

    #[test]
    fn build_catalog_video_probe_failure_omits_metadata() {
        // record_video path: bytes present but unparseable -> ProbeFailed, no
        // metadata invented. (A success path needs real mp4 bytes + nom-exif;
        // the omission boundary is the contract this issue cares about.)
        let mut req = ResourceRequests::default();
        req.videos
            .insert(crate::parse::primitives::VideoSource::Path("/tmp/bad.mp4".into()));
        let id = asset_id_for_video(req.videos.iter().next().unwrap());

        let mut bytes = HashMap::<String, Vec<u8>>::new();
        bytes.insert(id.key.clone(), b"not a video".to_vec());

        let prepared = build_catalog(&req, &bytes);
        assert!(
            !prepared.catalog.videos.contains_key(&id),
            "unparseable video bytes must not invent metadata"
        );
        match prepared.outcomes.get(&id.key) {
            Some(ProbeOutcome::ProbeFailed { .. }) => {}
            other => panic!("expected ProbeFailed, got {other:?}"),
        }
    }

    #[test]
    fn build_catalog_video_missing_bytes_omitted() {
        let mut req = ResourceRequests::default();
        req.videos
            .insert(crate::parse::primitives::VideoSource::Path("/tmp/missing.mp4".into()));
        let id = asset_id_for_video(req.videos.iter().next().unwrap());

        let prepared = build_catalog(&req, &HashMap::<String, Vec<u8>>::new());
        assert!(!prepared.catalog.videos.contains_key(&id));
        assert_eq!(
            prepared.outcomes.get(&id.key),
            Some(&ProbeOutcome::BytesMissing)
        );
    }

    #[test]
    fn lottie_dependencies_surfaces_pure_scan_for_host_fetch() {
        // AC: Lottie primary JSON can be scanned by core for dependencies;
        // dependency bytes are still fetched by host. The host calls this to
        // learn what to fetch next; it does not fetch here.
        let json = r#"{
          "assets": [
            { "p": "data:image/png;base64,AAAA" },
            { "u": "images/photo.png", "e": "images/" }
          ]
        }"#;
        let deps = lottie_dependencies(json).expect("scan");
        assert_eq!(deps, vec!["photo.png".to_string()]);
    }

    // --- hydrate_captions ------------------------------------------------------------

    fn caption_in_div() -> crate::parse::node::Node {
        let cap: crate::parse::node::Node = caption()
            .id("subs")
            .path("/tmp/sub.srt")
            .into();
        div().id("root").child(cap).into()
    }

    #[test]
    fn hydrate_captions_parses_srt_without_fs_access() {
        let root = caption_in_div();
        let id = asset_id_for_subtitle(&SubtitleSource::Path("/tmp/sub.srt".into()));
        let mut srt: HashMap<AssetId, String> = HashMap::new();
        srt.insert(
            id,
            "1\n00:00:00,000 --> 00:00:01,000\nHello\n".to_string(),
        );

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
        // AC: already-hydrated entries must not be overwritten.
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
        let mut srt: HashMap<AssetId, String> = HashMap::new();
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
        // AC: missing entries stay empty; not an error.
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
        let mut srt: HashMap<AssetId, String> = HashMap::new();
        srt.insert(
            id,
            "1\n00:00:00,000 --> 00:00:01,000\nHi\n".to_string(),
        );

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
        // Contract guard: host supplies bytes, core builds the fontdb. This
        // documents that the existing pure `load_faces_into_db` is the host
        // entry point and stays core-owned.
        use crate::resource::fonts::{
            FontFaceDecl, FontManifest, FontRole, FontSource, load_faces_into_db,
        };
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

    // --- ByteSource impl over AssetId-keyed map ------------------------------------

    #[test]
    fn byte_source_over_assetid_keyed_map() {
        let mut req = ResourceRequests::default();
        req.images
            .insert(crate::parse::primitives::ImageSource::Path("/tmp/a.png".into()));
        let id = asset_id_for_image(req.images.iter().next().unwrap()).unwrap();

        let mut bytes: HashMap<AssetId, Vec<u8>> = HashMap::new();
        bytes.insert(id.clone(), PNG_1X1.to_vec());

        let prepared = build_catalog(&req, &bytes);
        assert_eq!(
            prepared.catalog.images.get(&id).map(|m| (m.width, m.height)),
            Some((1, 1))
        );
    }
}
