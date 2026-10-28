//! Explicit composition lifecycle: parse → draft → requirements → host inputs →
//! prepare → prepared composition → pipeline.
//!
//! Contract-phase surface for issue #12 / #14 / #24. Hosts still own all
//! fetch/cache/decode work; core only validates host-supplied inputs and opens a
//! pure derivation pipeline. Production open paths go through
//! [`PreparedComposition::open_pipeline`] only.

mod types;

pub use types::{
    CompositionDraft, HostInputs, HostRequirements, PrepareError, PreparedComposition,
    ResourceRequest,
};

// `ResourceKind` lives next to `AssetId` in `ir::asset_id` so identity rules
// have exactly one home (issue #39). Re-exported here as the lifecycle contract
// path that downstream crates already import (`opencat_core::lifecycle::...`).
pub use crate::ir::asset_id::ResourceKind;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;

use crate::ir::asset_id::{
    AssetId, asset_id_for_audio, asset_id_for_image, asset_id_for_lottie,
    asset_id_for_video,
};
// `ResourceKind` is brought into scope by the `pub use` above (single source in
// `ir::asset_id`); re-listing it here would shadow that and trigger E0252.
use crate::fonts::{font_asset_id, merge_document_over_base};
use crate::parse::ParsedComposition;
use crate::parse::preflight::collect_resource_requests_from_parsed;
use crate::parse::primitives::LottieSource;
use crate::pipeline::DefaultPipeline;
use crate::probe::catalog::PreparedResourceCatalog;
use crate::script::ScriptDriver;
use crate::script::js_context::JsContext;

impl CompositionDraft {
    /// Build a draft from an already-parsed composition.
    pub fn from_parsed(parsed: ParsedComposition) -> Self {
        let raw = collect_resource_requests_from_parsed(&parsed);
        let requirements = HostRequirements::from_raw(&parsed, &raw);
        Self {
            parsed,
            requirements,
        }
    }

    pub fn requirements(&self) -> &HostRequirements {
        &self.requirements
    }

    pub fn parsed(&self) -> &ParsedComposition {
        &self.parsed
    }

    /// Pure, synchronous preparation. Consumes the draft and host inputs; never
    /// accepts a loader, fetcher, `Future`, or host callback.
    pub fn prepare(self, inputs: HostInputs) -> Result<PreparedComposition, PrepareError> {
        prepare(self, inputs)
    }
}

impl HostRequirements {
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    pub fn requests(&self) -> &[ResourceRequest] {
        &self.requests
    }

    fn from_raw(parsed: &ParsedComposition, raw: &crate::probe::catalog::ResourceRequests) -> Self {
        let mut requests = Vec::new();
        let mut seen = HashSet::new();

        for src in &raw.images {
            let Some(id) = asset_id_for_image(src) else {
                continue;
            };
            if !seen.insert(id.clone()) {
                continue;
            }
            requests.push(ResourceRequest {
                asset_id: id,
                kind: ResourceKind::Image,
            });
        }

        for src in &raw.videos {
            let id = asset_id_for_video(src);
            if !seen.insert(id.clone()) {
                continue;
            }
            requests.push(ResourceRequest {
                asset_id: id,
                kind: ResourceKind::Video,
            });
        }

        for src in &raw.audios {
            let Some(id) = asset_id_for_audio(src) else {
                continue;
            };
            if !seen.insert(id.clone()) {
                continue;
            }
            requests.push(ResourceRequest {
                asset_id: id,
                kind: ResourceKind::Audio,
            });
        }

        for req in &raw.lotties {
            let Some(id) = asset_id_for_lottie(&req.source) else {
                continue;
            };
            if matches!(req.source, LottieSource::Unset) {
                continue;
            }
            if !seen.insert(id.clone()) {
                continue;
            }
            requests.push(ResourceRequest {
                asset_id: id,
                kind: ResourceKind::Lottie,
            });
        }

        // Document fonts: stable identity is font_asset_id(source). Face markup
        // id is not the resource identity — hosts fetch by locator/asset_id.
        for face in &parsed.font_manifest.faces {
            let id = AssetId::new(ResourceKind::Font, font_asset_id(&face.source));
            if !seen.insert(id.clone()) {
                continue;
            }
            requests.push(ResourceRequest {
                asset_id: id,
                kind: ResourceKind::Font,
            });
        }

        // External scripts declared on the tree (path/url). Inline scripts never
        // appear here — core already holds their text.
        collect_script_requirements(&parsed.root, &mut requests, &mut seen);

        // Stable order for tests/hosts: kind then asset id.
        requests.sort_by(|a, b| {
            a.kind
                .cmp(&b.kind)
                .then_with(|| a.asset_id.key.cmp(&b.asset_id.key))
        });

        Self { requests }
    }
}

impl HostInputs {
    /// Empty host inputs: no resource metadata, empty font db.
    /// Suitable only for compositions with no declared external resources (and
    /// no document fonts that need shaping beyond the empty database).
    pub fn empty() -> Self {
        Self {
            base_font_faces: Vec::new(),
            font_fallback_dirs: Vec::new(),
            sans_serif_family: String::new(),
            catalog: PreparedResourceCatalog::default(),
            document_fonts: HashMap::new(),
            script_texts: HashMap::new(),
            supplied: HashSet::new(),
            confirmed_absent: HashMap::new(),
        }
    }

    pub fn with_base_font_faces(mut self, faces: Vec<Vec<u8>>) -> Self {
        self.base_font_faces = faces;
        self
    }

    /// Append font directories to the glyph-fallback pool, loaded (best-effort,
    /// file-backed) after document and base faces so their families never win
    /// query matching — the pool only serves codepoints no declared face
    /// covers (browser fontconfig fallback parity).
    pub fn with_font_fallback_dirs(mut self, dirs: Vec<std::path::PathBuf>) -> Self {
        self.font_fallback_dirs = dirs;
        self
    }

    pub fn font_fallback_dirs(&self) -> &[std::path::PathBuf] {
        &self.font_fallback_dirs
    }

    pub fn base_font_faces(&self) -> &[Vec<u8>] {
        &self.base_font_faces
    }

    pub fn with_sans_serif_family(mut self, family: impl Into<String>) -> Self {
        self.sans_serif_family = family.into();
        self
    }

    pub fn sans_serif_family(&self) -> &str {
        &self.sans_serif_family
    }

    /// Insert image metadata for a declared asset. Duplicate ids error.
    pub fn insert_image(
        &mut self,
        id: AssetId,
        meta: crate::probe::catalog::ImageMeta,
    ) -> Result<(), PrepareError> {
        self.record_supply(&id)?;
        self.catalog.images.insert(id, meta);
        Ok(())
    }

    /// Insert video metadata for a declared asset. Duplicate ids error.
    pub fn insert_video(
        &mut self,
        id: AssetId,
        meta: crate::probe::catalog::VideoInfoMeta,
    ) -> Result<(), PrepareError> {
        self.record_supply(&id)?;
        self.catalog.videos.insert(id, meta);
        Ok(())
    }

    /// Register an audio asset id. Duplicate ids error.
    pub fn insert_audio(&mut self, id: AssetId) -> Result<(), PrepareError> {
        self.record_supply(&id)?;
        self.catalog.audios.insert(id);
        Ok(())
    }

    /// Insert Lottie metadata for a declared asset. Duplicate ids error.
    pub fn insert_lottie(
        &mut self,
        id: AssetId,
        meta: crate::lottie::LottieMeta,
    ) -> Result<(), PrepareError> {
        self.record_supply(&id)?;
        self.catalog.lotties.insert(id, meta);
        Ok(())
    }

    /// Provide raw document font face bytes for a declared font asset. Duplicate
    /// ids error. Core merges faces, family index, fallback precedence and
    /// shaping database during prepare; hosts must not interpret manifest semantics.
    ///
    /// `id` must be the canonical font AssetId from requirements
    /// (`font:path:…` / `font:url:…`).
    pub fn insert_document_font(
        &mut self,
        id: AssetId,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<(), PrepareError> {
        self.record_supply(&id)?;
        self.document_fonts.insert(id, bytes.into());
        Ok(())
    }

    /// Provide external script source text for a declared script asset.
    /// Duplicate ids error. Core injects the text into `ScriptDriver`s during
    /// prepare; hosts must not rewrite composition input strings.
    ///
    /// `id` must be the canonical script AssetId from requirements
    /// (`script:path:…` / `script:url:…`).
    pub fn insert_script_text(
        &mut self,
        id: AssetId,
        text: impl Into<String>,
    ) -> Result<(), PrepareError> {
        self.record_supply(&id)?;
        self.script_texts.insert(id, text.into());
        Ok(())
    }

    /// Explicitly confirm a declared asset as absent with a structured reason.
    /// The host has checked and determined the resource is definitively
    /// unavailable (e.g. "not found", "access denied", "timeout"). Core
    /// distinguishes this from "host has not responded" — prepare returns
    /// [`PrepareError::AbsentInput`] for absent assets, not [`PrepareError::MissingInput`].
    ///
    /// Duplicate entries error with [`PrepareError::DuplicateInput`] like all
    /// other `insert_*` methods.
    pub fn mark_absent(
        &mut self,
        id: AssetId,
        reason: impl Into<String>,
    ) -> Result<(), PrepareError> {
        self.record_supply(&id)?;
        self.confirmed_absent.insert(id, reason.into());
        Ok(())
    }

    fn record_supply(&mut self, id: &AssetId) -> Result<(), PrepareError> {
        if !self.supplied.insert(id.clone()) {
            return Err(PrepareError::DuplicateInput {
                asset_id: id.clone(),
            });
        }
        Ok(())
    }
}

impl PreparedComposition {
    /// Access the internal catalog. Crate-private — hosts interact with
    /// resource metadata through the lifecycle API only.
    #[allow(dead_code)]
    pub(crate) fn catalog(&self) -> &PreparedResourceCatalog {
        &self.catalog
    }

    pub fn parsed(&self) -> &ParsedComposition {
        &self.parsed
    }

    pub fn font_db(&self) -> &Arc<fontdb::Database> {
        &self.font_db
    }

    /// Open a pipeline from this prepared composition. Consumes the prepared
    /// state so the unprepared/prepared distinction stays type-enforced.
    pub fn open_pipeline<S: JsContext>(self, scripts: S) -> Result<DefaultPipeline<S>> {
        DefaultPipeline::open_with_prepared_catalog(
            self.parsed,
            self.catalog,
            scripts,
            self.font_db,
        )
    }
}

/// Pure prepare: validate host inputs against draft requirements, merge
/// document fonts over the host base font database, and
/// produce a prepared composition ready for pipeline open.
pub fn prepare(
    draft: CompositionDraft,
    inputs: HostInputs,
) -> Result<PreparedComposition, PrepareError> {
    let CompositionDraft {
        mut parsed,
        requirements,
    } = draft;

    validate_inputs(&requirements, &inputs)?;

    let font_db = prepare_font_db(&parsed, &inputs)?;

    // Ensure every declared audio id is present in the catalog map even when
    // the host only registered presence.
    let mut catalog = inputs.catalog;
    for req in requirements.requests() {
        match req.kind {
            ResourceKind::Audio => {
                catalog.audios.insert(req.asset_id.clone());
            }
            ResourceKind::Image
            | ResourceKind::Video
            | ResourceKind::Lottie
            | ResourceKind::Font
            | ResourceKind::Script => {}
        }
    }

    // Inject host-supplied external script texts into ScriptDrivers (issue #20).
    // Keyed by AssetId string; pure tree walk, no FS/network.
    let script_texts_by_id: HashMap<String, String> = inputs
        .script_texts
        .iter()
        .map(|(id, text)| (id.key.clone(), text.clone()))
        .collect();
    parsed.root = inject_script_texts(parsed.root, &script_texts_by_id)?;

    Ok(PreparedComposition {
        parsed,
        catalog,
        font_db,
    })
}

/// Merge document font bytes over the host base font database. Fail-fast when a
/// declared face has no bytes or zero-length bytes. Empty manifests leave the
/// base database unchanged (constructed from base_font_faces).
fn prepare_font_db(
    parsed: &ParsedComposition,
    inputs: &HostInputs,
) -> Result<Arc<fontdb::Database>, PrepareError> {
    let manifest = &parsed.font_manifest;

    let base = if inputs.base_font_faces.is_empty() {
        crate::text::empty_font_db()
    } else {
        crate::text::font_db_from_bytes(&inputs.base_font_faces, &inputs.sans_serif_family)
    };

    if manifest.is_empty() {
        return Ok(Arc::new(extend_db_with_fallback_dirs(base, inputs)));
    }

    // Remap asset-id → face-id for load_faces_into_db / merge_document_over_base.
    let mut bytes_by_face_id: HashMap<String, Vec<u8>> = HashMap::new();
    for face in &manifest.faces {
        let asset_id = AssetId::new(ResourceKind::Font, font_asset_id(&face.source));
        let Some(bytes) = inputs.document_fonts.get(&asset_id) else {
            return Err(PrepareError::MissingInput {
                asset_id,
                kind: ResourceKind::Font,
            });
        };
        if bytes.is_empty() {
            return Err(PrepareError::InvalidMetadata {
                asset_id,
                kind: ResourceKind::Font,
                reason: "empty font bytes; document font faces must be loadable".into(),
            });
        }
        // Probe loadability without permanently mutating a db.
        {
            let mut probe = fontdb::Database::new();
            let before = probe.faces().count();
            probe.load_font_data(bytes.clone());
            if probe.faces().count() <= before {
                return Err(PrepareError::InvalidMetadata {
                    asset_id,
                    kind: ResourceKind::Font,
                    reason: "font bytes could not be loaded by fontdb".into(),
                });
            }
        }
        bytes_by_face_id.insert(face.id.clone(), bytes.clone());
    }

    let (db, _index) =
        merge_document_over_base(&base, manifest, &bytes_by_face_id).map_err(|err| {
            PrepareError::Internal {
                message: format!("font merge failed: {err}"),
            }
        })?;
    Ok(Arc::new(extend_db_with_fallback_dirs(db, inputs)))
}

/// Load the host's font-fallback directories into `db` (best-effort): unreadable
/// or non-font entries are skipped silently — the pool exists so codepoints no
/// declared face covers still shape to a real glyph instead of tofu. Loaded
/// last, so document/base families keep priority in query matching. Emoji
/// presentation stays on the color face via `BrowserEmojiFirstFallback`
/// (text/mod.rs), which the shaper consults for every run shaped from this db.
fn extend_db_with_fallback_dirs(
    db: fontdb::Database,
    inputs: &HostInputs,
) -> fontdb::Database {
    let mut db = db;
    for dir in &inputs.font_fallback_dirs {
        db.load_fonts_dir(dir);
    }
    db
}

fn validate_inputs(
    requirements: &HostRequirements,
    inputs: &HostInputs,
) -> Result<(), PrepareError> {
    let required: HashMap<&AssetId, ResourceKind> = requirements
        .requests()
        .iter()
        .map(|r| (&r.asset_id, r.kind))
        .collect();

    // Undeclared: any host-supplied id (insert tracking *or* catalog keys) that
    // is not in draft requirements. DuplicateInput is raised at HostInputs::insert_*
    // so prepare never observes a second insert for the same id.
    for id in inputs
        .supplied
        .iter()
        .chain(inputs.catalog.images.keys())
        .chain(
            inputs
                .catalog
                .videos
                .keys()
                .chain(inputs.catalog.audios.iter())
                .chain(inputs.catalog.lotties.keys())
                .chain(inputs.document_fonts.keys())
                .chain(inputs.script_texts.keys()),
        )
    {
        if !required.contains_key(id) {
            return Err(PrepareError::UndeclaredInput {
                asset_id: id.clone(),
            });
        }
    }

    // Missing + layout-critical validation. Document fonts are hard:
    // missing/empty bytes fail prepare.
    for req in requirements.requests() {
        match req.kind {
            ResourceKind::Image => {
                let Some(meta) = inputs.catalog.images.get(&req.asset_id) else {
                    // Kind mismatch: present only under another map.
                    if inputs.catalog.videos.contains_key(&req.asset_id)
                        || inputs.catalog.audios.contains(&req.asset_id)
                        || inputs.catalog.lotties.contains_key(&req.asset_id)
                        || inputs.document_fonts.contains_key(&req.asset_id)
                    {
                        return Err(PrepareError::InvalidMetadata {
                            asset_id: req.asset_id.clone(),
                            kind: req.kind,
                            reason: "kind mismatch: image required but metadata is not image"
                                .into(),
                        });
                    }
                    return Err(absent_or_missing(&req.asset_id, req.kind, inputs));
                };
                if meta.width == 0 || meta.height == 0 {
                    return Err(PrepareError::InvalidMetadata {
                        asset_id: req.asset_id.clone(),
                        kind: req.kind,
                        reason: format!(
                            "zero dimensions ({}x{}); layout requires positive size",
                            meta.width, meta.height
                        ),
                    });
                }
            }
            ResourceKind::Video => {
                let Some(meta) = inputs.catalog.videos.get(&req.asset_id) else {
                    if inputs.catalog.images.contains_key(&req.asset_id)
                        || inputs.catalog.audios.contains(&req.asset_id)
                        || inputs.catalog.lotties.contains_key(&req.asset_id)
                        || inputs.document_fonts.contains_key(&req.asset_id)
                    {
                        return Err(PrepareError::InvalidMetadata {
                            asset_id: req.asset_id.clone(),
                            kind: req.kind,
                            reason: "kind mismatch: video required but metadata is not video"
                                .into(),
                        });
                    }
                    return Err(absent_or_missing(&req.asset_id, req.kind, inputs));
                };
                if meta.width == 0 || meta.height == 0 {
                    return Err(PrepareError::InvalidMetadata {
                        asset_id: req.asset_id.clone(),
                        kind: req.kind,
                        reason: format!(
                            "zero dimensions ({}x{}); layout requires positive size",
                            meta.width, meta.height
                        ),
                    });
                }
                // duration_micros may be None (unknown length); that is non-critical
                // and must not fail prepare — hosts still decode via time_micros.
            }
            ResourceKind::Audio => {
                if inputs.confirmed_absent.contains_key(&req.asset_id) {
                    return Err(absent_or_missing(&req.asset_id, req.kind, inputs));
                }
                if !inputs.catalog.audios.contains(&req.asset_id)
                    && !inputs.supplied.contains(&req.asset_id)
                {
                    return Err(absent_or_missing(&req.asset_id, req.kind, inputs));
                }
            }
            ResourceKind::Lottie => {
                let Some(meta) = inputs.catalog.lotties.get(&req.asset_id) else {
                    if inputs.catalog.images.contains_key(&req.asset_id)
                        || inputs.catalog.videos.contains_key(&req.asset_id)
                        || inputs.catalog.audios.contains(&req.asset_id)
                        || inputs.document_fonts.contains_key(&req.asset_id)
                    {
                        return Err(PrepareError::InvalidMetadata {
                            asset_id: req.asset_id.clone(),
                            kind: req.kind,
                            reason: "kind mismatch: lottie required but metadata is not lottie"
                                .into(),
                        });
                    }
                    return Err(absent_or_missing(&req.asset_id, req.kind, inputs));
                };
                if meta.width == 0 || meta.height == 0 {
                    return Err(PrepareError::InvalidMetadata {
                        asset_id: req.asset_id.clone(),
                        kind: req.kind,
                        reason: format!(
                            "zero dimensions ({}x{}); layout requires positive size",
                            meta.width, meta.height
                        ),
                    });
                }
                if !meta.fps.is_finite() || meta.fps <= 0.0 {
                    return Err(PrepareError::InvalidMetadata {
                        asset_id: req.asset_id.clone(),
                        kind: req.kind,
                        reason: format!(
                            "invalid fps {}; timing requires positive finite fps",
                            meta.fps
                        ),
                    });
                }
                if !meta.in_frame.is_finite() || !meta.out_frame.is_finite() {
                    return Err(PrepareError::InvalidMetadata {
                        asset_id: req.asset_id.clone(),
                        kind: req.kind,
                        reason: "non-finite frame range".into(),
                    });
                }
                if meta.out_frame <= meta.in_frame {
                    return Err(PrepareError::InvalidMetadata {
                        asset_id: req.asset_id.clone(),
                        kind: req.kind,
                        reason: format!(
                            "invalid frame range [{}, {}]; out_frame must be greater than in_frame",
                            meta.in_frame, meta.out_frame
                        ),
                    });
                }
            }
            ResourceKind::Font => match inputs.document_fonts.get(&req.asset_id) {
                None => {
                    return Err(absent_or_missing(&req.asset_id, req.kind, inputs));
                }
                Some(bytes) if bytes.is_empty() => {
                    return Err(PrepareError::InvalidMetadata {
                        asset_id: req.asset_id.clone(),
                        kind: req.kind,
                        reason: "empty font bytes; document font faces must be loadable".into(),
                    });
                }
                Some(_) => {}
            },
            ResourceKind::Script => match inputs.script_texts.get(&req.asset_id) {
                None => {
                    return Err(absent_or_missing(&req.asset_id, req.kind, inputs));
                }
                Some(text) if text.is_empty() => {
                    return Err(PrepareError::InvalidMetadata {
                        asset_id: req.asset_id.clone(),
                        kind: req.kind,
                        reason: "empty script text; external scripts must be non-empty".into(),
                    });
                }
                Some(_) => {}
            },
        }
    }

    Ok(())
}

/// Return [`PrepareError::AbsentInput`] if the asset is in `confirmed_absent`,
/// otherwise [`PrepareError::MissingInput`]. This is the core distinction
/// between "host has checked and the resource is unavailable" and "host has
/// not responded."
fn absent_or_missing(id: &AssetId, kind: ResourceKind, inputs: &HostInputs) -> PrepareError {
    if let Some(reason) = inputs.confirmed_absent.get(id) {
        PrepareError::AbsentInput {
            asset_id: id.clone(),
            kind,
            reason: reason.clone(),
        }
    } else {
        PrepareError::MissingInput {
            asset_id: id.clone(),
            kind,
        }
    }
}

fn collect_script_requirements(
    root: &crate::parse::node::Node,
    requests: &mut Vec<ResourceRequest>,
    seen: &mut HashSet<AssetId>,
) {
    walk_script_drivers(root, &mut |driver: &ScriptDriver| {
        for ext in &driver.externals {
            if !seen.insert(ext.asset_id.clone()) {
                continue;
            }
            requests.push(ResourceRequest {
                asset_id: ext.asset_id.clone(),
                kind: ResourceKind::Script,
            });
        }
    });
}

fn inject_script_texts(
    root: crate::parse::node::Node,
    texts: &HashMap<String, String>,
) -> Result<crate::parse::node::Node, PrepareError> {
    walk_inject_scripts(root, texts)
}

fn walk_inject_scripts(
    node: crate::parse::node::Node,
    texts: &HashMap<String, String>,
) -> Result<crate::parse::node::Node, PrepareError> {
    use crate::parse::node::NodeKind;

    let mut kind = node.kind().clone();
    {
        let style = kind.style_mut();
        if let Some(driver) = style.script_driver.as_mut() {
            let mut next = (**driver).clone();
            if next.is_external_pending() {
                next.resolve_with_host_texts(texts);
                if next.source.is_empty() {
                    let missing = next
                        .externals
                        .iter()
                        .find(|e| !texts.contains_key(&e.asset_id.key))
                        .map(|e| e.asset_id.clone())
                        .unwrap_or_else(|| {
                            next.externals
                                .first()
                                .map(|e| e.asset_id.clone())
                                .unwrap_or_else(|| AssetId::new(ResourceKind::Script, "script:?"))
                        });
                    return Err(PrepareError::MissingInput {
                        asset_id: missing,
                        kind: ResourceKind::Script,
                    });
                }
            }
            *driver = std::sync::Arc::new(next);
        }
    }

    match &mut kind {
        NodeKind::Div(div) => {
            let children = div
                .children_ref()
                .iter()
                .cloned()
                .map(|c| walk_inject_scripts(c, texts))
                .collect::<Result<Vec<_>, _>>()?;
            div.set_children(children);
        }
        NodeKind::Video(video) => {
            let children = video
                .children_ref()
                .iter()
                .cloned()
                .map(|c| walk_inject_scripts(c, texts))
                .collect::<Result<Vec<_>, _>>()?;
            video.set_children(children);
        }
        NodeKind::Timeline(tl) => {
            tl.map_scene_nodes(|scene| {
                walk_inject_scripts(scene, texts).map_err(|e| anyhow::anyhow!(e))
            })
            .map_err(|err| PrepareError::Internal {
                message: err.to_string(),
            })?;
        }
        NodeKind::Canvas(canvas) => {
            let hidden = canvas
                .hidden_children_ref()
                .iter()
                .cloned()
                .map(|c| walk_inject_scripts(c, texts))
                .collect::<Result<Vec<_>, _>>()?;
            canvas.set_hidden_children(hidden);
        }
        NodeKind::Image(_)
        | NodeKind::Text(_)
        | NodeKind::Lottie(_)
        | NodeKind::Path(_)
        | NodeKind::Svg(_) => {}
    }

    Ok(crate::parse::node::Node::new(kind))
}

fn walk_script_drivers(node: &crate::parse::node::Node, f: &mut dyn FnMut(&ScriptDriver)) {
    use crate::parse::node::NodeKind;

    if let Some(driver) = node.style_ref().script_driver.as_deref() {
        f(driver);
    }

    match node.kind() {
        NodeKind::Div(div) => {
            for child in div.children_ref() {
                walk_script_drivers(child, f);
            }
        }
        NodeKind::Video(video) => {
            for child in video.children_ref() {
                walk_script_drivers(child, f);
            }
        }
        NodeKind::Timeline(tl) => {
            for segment in tl.segments() {
                match segment {
                    crate::parse::time::TimelineSegment::Scene { scene, .. } => {
                        walk_script_drivers(scene, f);
                    }
                    crate::parse::time::TimelineSegment::Transition { from, to, .. } => {
                        walk_script_drivers(from, f);
                        walk_script_drivers(to, f);
                    }
                }
            }
        }
        NodeKind::Canvas(canvas) => {
            for child in canvas.hidden_children_ref() {
                walk_script_drivers(child, f);
            }
        }
        NodeKind::Image(_)
        | NodeKind::Text(_)
        | NodeKind::Lottie(_)
        | NodeKind::Path(_)
        | NodeKind::Svg(_) => {}
    }
}


