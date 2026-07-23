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
    asset_id_for_audio, asset_id_for_image, asset_id_for_lottie, asset_id_for_subtitle,
    asset_id_for_video, AssetId,
};
// `ResourceKind` is brought into scope by the `pub use` above (single source in
// `ir::asset_id`); re-listing it here would shadow that and trigger E0252.
use crate::parse::primitives::LottieSource;
use crate::parse::preflight::collect_resource_requests_from_parsed;
use crate::parse::ParsedComposition;
use crate::pipeline::DefaultPipeline;
use crate::probe::catalog::PreparedResourceCatalog;
use crate::probe::prepare::hydrate_captions;
use crate::fonts::{font_asset_id, merge_document_over_base};
use crate::script::js_context::JsContext;
use crate::script::ScriptDriver;

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

    /// Parse XML markup or JSONL into a draft. Format is auto-detected from the
    /// first non-whitespace character (`{` ⇒ JSONL, otherwise markup).
    pub fn parse(input: &str) -> Result<Self> {
        let trimmed = input.trim_start();
        let parsed = if trimmed.starts_with('{') {
            crate::parse::jsonl::parse(input)?
        } else {
            crate::parse::markup::parse(input)?
        };
        Ok(Self::from_parsed(parsed))
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

        for src in &raw.subtitles {
            let id = asset_id_for_subtitle(src);
            if !seen.insert(id.clone()) {
                continue;
            }
            requests.push(ResourceRequest {
                asset_id: id,
                kind: ResourceKind::Subtitle,
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
    /// Empty host inputs: no resource metadata, no subtitle text, empty font db.
    /// Suitable only for compositions with no declared external resources (and
    /// no document fonts that need shaping beyond the empty database).
    pub fn empty() -> Self {
        Self {
            base_font_faces: Vec::new(),
            sans_serif_family: String::new(),
            catalog: PreparedResourceCatalog::default(),
            subtitle_texts: HashMap::new(),
            document_fonts: HashMap::new(),
            script_texts: HashMap::new(),
            supplied: HashSet::new(),
        }
    }

    pub fn with_base_font_faces(mut self, faces: Vec<Vec<u8>>) -> Self {
        self.base_font_faces = faces;
        self
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

    /// Provide raw subtitle text for a declared subtitle asset. Duplicate ids error.
    /// Core parses SRT during prepare; hosts must not interpret captions themselves.
    pub fn insert_subtitle_text(
        &mut self,
        id: AssetId,
        text: impl Into<String>,
    ) -> Result<(), PrepareError> {
        self.record_supply(&id)?;
        // Register the canonical id so undeclared checks and catalog presence align.
        self.catalog.subtitles.entry(id.clone()).or_default();
        self.subtitle_texts.insert(id, text.into());
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

    fn record_supply(&mut self, id: &AssetId) -> Result<(), PrepareError> {
        if !self.supplied.insert(id.clone()) {
            return Err(PrepareError::DuplicateInput { asset_id: id.clone() });
        }
        Ok(())
    }
}

impl PreparedComposition {
    pub fn catalog(&self) -> &PreparedResourceCatalog {
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
/// document fonts over the host base font database, hydrate captions, and
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

    // Ensure every declared audio/subtitle id is present in the catalog maps
    // even when the host only registered presence (audio) or text (subtitle).
    let mut catalog = inputs.catalog;
    for req in requirements.requests() {
        match req.kind {
            ResourceKind::Audio => {
                catalog.audios.insert(req.asset_id.clone());
            }
            ResourceKind::Subtitle => {
                catalog.subtitles.entry(req.asset_id.clone()).or_default();
            }
            ResourceKind::Image
            | ResourceKind::Video
            | ResourceKind::Lottie
            | ResourceKind::Font
            | ResourceKind::Script => {}
        }
    }

    let fps = parsed.fps.max(1) as u32;
    parsed.root = hydrate_captions(parsed.root, fps, &inputs.subtitle_texts)
        .map_err(|err| PrepareError::Internal {
            message: err.to_string(),
        })?
        .0;

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
        return Ok(Arc::new(base));
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

    let (db, _index) = merge_document_over_base(&base, manifest, &bytes_by_face_id)
        .map_err(|err| PrepareError::Internal {
            message: format!("font merge failed: {err}"),
        })?;
    Ok(Arc::new(db))
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
                .chain(inputs.catalog.subtitles.keys())
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

    // Missing + layout-critical validation. Subtitle text is soft: missing SRT
    // leaves empty caption entries (same contract as hydrate_captions / #12).
    // Document fonts are hard: missing/empty bytes fail prepare.
    for req in requirements.requests() {
        match req.kind {
            ResourceKind::Image => {
                let Some(meta) = inputs.catalog.images.get(&req.asset_id) else {
                    // Kind mismatch: present only under another map.
                    if inputs.catalog.videos.contains_key(&req.asset_id)
                        || inputs.catalog.audios.contains(&req.asset_id)
                        || inputs.catalog.lotties.contains_key(&req.asset_id)
                        || inputs.catalog.subtitles.contains_key(&req.asset_id)
                        || inputs.document_fonts.contains_key(&req.asset_id)
                    {
                        return Err(PrepareError::InvalidMetadata {
                            asset_id: req.asset_id.clone(),
                            kind: req.kind,
                            reason: "kind mismatch: image required but metadata is not image"
                                .into(),
                        });
                    }
                    return Err(PrepareError::MissingInput {
                        asset_id: req.asset_id.clone(),
                        kind: req.kind,
                    });
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
                        || inputs.catalog.subtitles.contains_key(&req.asset_id)
                        || inputs.document_fonts.contains_key(&req.asset_id)
                    {
                        return Err(PrepareError::InvalidMetadata {
                            asset_id: req.asset_id.clone(),
                            kind: req.kind,
                            reason: "kind mismatch: video required but metadata is not video"
                                .into(),
                        });
                    }
                    return Err(PrepareError::MissingInput {
                        asset_id: req.asset_id.clone(),
                        kind: req.kind,
                    });
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
                if !inputs.catalog.audios.contains(&req.asset_id)
                    && !inputs.supplied.contains(&req.asset_id)
                {
                    return Err(PrepareError::MissingInput {
                        asset_id: req.asset_id.clone(),
                        kind: req.kind,
                    });
                }
            }
            ResourceKind::Lottie => {
                let Some(meta) = inputs.catalog.lotties.get(&req.asset_id) else {
                    if inputs.catalog.images.contains_key(&req.asset_id)
                        || inputs.catalog.videos.contains_key(&req.asset_id)
                        || inputs.catalog.audios.contains(&req.asset_id)
                        || inputs.catalog.subtitles.contains_key(&req.asset_id)
                        || inputs.document_fonts.contains_key(&req.asset_id)
                    {
                        return Err(PrepareError::InvalidMetadata {
                            asset_id: req.asset_id.clone(),
                            kind: req.kind,
                            reason: "kind mismatch: lottie required but metadata is not lottie"
                                .into(),
                        });
                    }
                    return Err(PrepareError::MissingInput {
                        asset_id: req.asset_id.clone(),
                        kind: req.kind,
                    });
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
            ResourceKind::Subtitle => {
                // Subtitle must be present in subtitle_texts or catalog.subtitles.
                // Empty string is valid (explicit empty captions).
                if !inputs.subtitle_texts.contains_key(&req.asset_id)
                    && !inputs.catalog.subtitles.contains_key(&req.asset_id)
                {
                    return Err(PrepareError::MissingInput {
                        asset_id: req.asset_id.clone(),
                        kind: req.kind,
                    });
                }
            }
            ResourceKind::Font => {
                match inputs.document_fonts.get(&req.asset_id) {
                    None => {
                        return Err(PrepareError::MissingInput {
                            asset_id: req.asset_id.clone(),
                            kind: req.kind,
                        });
                    }
                    Some(bytes) if bytes.is_empty() => {
                        return Err(PrepareError::InvalidMetadata {
                            asset_id: req.asset_id.clone(),
                            kind: req.kind,
                            reason: "empty font bytes; document font faces must be loadable"
                                .into(),
                        });
                    }
                    Some(_) => {}
                }
            }
            ResourceKind::Script => match inputs.script_texts.get(&req.asset_id) {
                None => {
                    return Err(PrepareError::MissingInput {
                        asset_id: req.asset_id.clone(),
                        kind: req.kind,
                    });
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
                                .unwrap_or_else(|| {
                                    AssetId::new(ResourceKind::Script, "script:?")
                                })
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
        | NodeKind::Lucide(_)
        | NodeKind::Path(_)
        | NodeKind::Caption(_) => {}
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
        | NodeKind::Lucide(_)
        | NodeKind::Path(_)
        | NodeKind::Caption(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::Pipeline;
    use crate::probe::catalog::ImageMeta;
    use crate::script::js_context::JsContext;
    use crate::script::recorder::MutationStore;
    use std::cell::RefCell;

    struct NoopJsContext {
        store: RefCell<MutationStore>,
    }

    impl JsContext for NoopJsContext {
        fn new() -> Result<Self> {
            Ok(Self {
                store: MutationStore::default().into(),
            })
        }
        fn eval(&self, _code: &str) -> Result<()> {
            Ok(())
        }
        fn set_ctx_field(&self, _name: &str, _v: serde_json::Value) -> Result<()> {
            Ok(())
        }
        fn call_global_fn(&self, _name: &str) -> Result<()> {
            Ok(())
        }
        fn install_dispatcher<F>(&self, _dispatcher: F) -> Result<()>
        where
            F: Fn(&mut MutationStore, &str, &[serde_json::Value]) -> Result<serde_json::Value>
                + 'static,
        {
            Ok(())
        }
        fn rebind_dispatcher(&self) -> Result<()> {
            Ok(())
        }
        fn with_store_mut<R>(&self, f: impl FnOnce(&mut MutationStore) -> R) -> R {
            f(&mut *self.store.borrow_mut())
        }
    }

    fn open_lifecycle(input: &str) -> DefaultPipeline<NoopJsContext> {
        let draft = CompositionDraft::parse(input).expect("parse draft");
        let inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
        let prepared = draft.prepare(inputs).expect("prepare");
        prepared
            .open_pipeline(NoopJsContext::new().expect("js"))
            .expect("open pipeline")
    }

    #[test]
    fn xml_without_resources_opens_and_renders_via_lifecycle() {
        let xml = r#"
            <opencat width="320" height="240" fps="30" duration="0.1">
              <div id="root" class="w-full h-full">
                <div id="child" class="w-[100px] h-[50px] bg-red-500" />
              </div>
            </opencat>
        "#;

        let mut pipeline = open_lifecycle(xml);
        assert_eq!(pipeline.info().width, 320);
        assert_eq!(pipeline.info().height, 240);
        assert_eq!(pipeline.info().fps, 30);

        let frame = pipeline.render_frame(0).expect("render");
        assert!(
            !frame.draw.ops.is_empty(),
            "lifecycle path should produce DrawOps"
        );
    }

    #[test]
    fn jsonl_without_resources_opens_and_renders_via_lifecycle() {
        let jsonl = r##"{"type":"composition","width":100,"height":200,"fps":30,"duration":0.033333333333}
{"type":"div","id":"root","parentId":null}
{"type":"div","id":"child","parentId":"root","bg":"#ff0000","w":100,"h":50}"##;

        let mut pipeline = open_lifecycle(jsonl);
        assert_eq!(pipeline.info().width, 100);
        assert_eq!(pipeline.info().height, 200);

        let frame = pipeline.render_frame(0).expect("render");
        assert!(!frame.draw.ops.is_empty());
    }

    #[test]
    fn draft_requirements_are_empty_for_no_resource_composition() {
        let jsonl = r#"{"type":"composition","width":10,"height":10,"fps":30,"duration":0.1}
{"type":"div","id":"root","parentId":null}"#;
        let draft = CompositionDraft::parse(jsonl).unwrap();
        assert!(draft.requirements().is_empty());
        assert!(draft.requirements().requests().is_empty());
    }

    #[test]
    fn draft_requirements_list_declared_images() {
        let jsonl = r#"{"type":"composition","width":10,"height":10,"fps":30,"duration":0.1}
{"type":"div","id":"root","parentId":null}
{"type":"image","id":"pic","parentId":"root","path":"photos/a.png"}"#;
        let draft = CompositionDraft::parse(jsonl).unwrap();
        let reqs = draft.requirements().requests();
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].kind, ResourceKind::Image);
        assert_eq!(reqs[0].asset_id.key, "photos/a.png");
    }

    #[test]
    fn prepare_errors_on_missing_image_metadata() {
        let jsonl = r#"{"type":"composition","width":10,"height":10,"fps":30,"duration":0.1}
{"type":"div","id":"root","parentId":null}
{"type":"image","id":"pic","parentId":"root","path":"photos/a.png"}"#;
        let draft = CompositionDraft::parse(jsonl).unwrap();
        let err = draft
            .prepare(HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC"))
            .expect_err("missing image must fail prepare");
        assert!(matches!(
            err,
            PrepareError::MissingInput {
                kind: ResourceKind::Image,
                ..
            }
        ));
    }

    #[test]
    fn prepare_errors_on_undeclared_host_input() {
        let jsonl = r#"{"type":"composition","width":10,"height":10,"fps":30,"duration":0.1}
{"type":"div","id":"root","parentId":null}"#;
        let draft = CompositionDraft::parse(jsonl).unwrap();
        let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
        inputs
            .insert_image(
                AssetId::new(ResourceKind::Image, "ghost.png"),
                ImageMeta {
                    width: 1,
                    height: 1,
                },
            )
            .unwrap();
        let err = draft.prepare(inputs).expect_err("undeclared must fail");
        assert!(matches!(err, PrepareError::UndeclaredInput { .. }));
    }

    #[test]
    fn prepare_errors_on_duplicate_host_input() {
        let mut inputs = HostInputs::empty();
        let id = AssetId::new(ResourceKind::Image, "photos/a.png");
        inputs
            .insert_image(
                id.clone(),
                ImageMeta {
                    width: 1,
                    height: 1,
                },
            )
            .unwrap();
        let err = inputs
            .insert_image(
                id,
                ImageMeta {
                    width: 2,
                    height: 2,
                },
            )
            .expect_err("duplicate");
        assert!(matches!(err, PrepareError::DuplicateInput { .. }));
    }

    #[test]
    fn prepare_accepts_declared_image_metadata_and_opens() {
        let jsonl = r#"{"type":"composition","width":10,"height":10,"fps":30,"duration":0.1}
{"type":"div","id":"root","parentId":null}
{"type":"image","id":"pic","parentId":"root","path":"photos/a.png","w":8,"h":8}"#;
        let draft = CompositionDraft::parse(jsonl).unwrap();
        let id = draft.requirements().requests()[0].asset_id.clone();
        let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
        inputs
            .insert_image(
                id,
                ImageMeta {
                    width: 8,
                    height: 8,
                },
            )
            .unwrap();
        let prepared = draft.prepare(inputs).expect("prepare with image meta");
        let mut pipeline = prepared
            .open_pipeline(NoopJsContext::new().unwrap())
            .expect("open");
        let frame = pipeline.render_frame(0).expect("render");
        assert!(!frame.draw.ops.is_empty());
    }

    #[test]
    fn prepare_errors_on_zero_size_image_metadata() {
        let jsonl = r#"{"type":"composition","width":10,"height":10,"fps":30,"duration":0.1}
{"type":"div","id":"root","parentId":null}
{"type":"image","id":"pic","parentId":"root","path":"photos/a.png"}"#;
        let draft = CompositionDraft::parse(jsonl).unwrap();
        let id = draft.requirements().requests()[0].asset_id.clone();
        let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
        inputs
            .insert_image(
                id,
                ImageMeta {
                    width: 0,
                    height: 0,
                },
            )
            .unwrap();
        let err = draft
            .prepare(inputs)
            .expect_err("zero-size image must fail prepare");
        assert!(
            matches!(
                err,
                PrepareError::InvalidMetadata {
                    kind: ResourceKind::Image,
                    ..
                }
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn prepare_errors_on_image_kind_mismatch() {
        // Host supplies video metadata under the image asset id via insert_video —
        // but the id is declared as image. Using insert_video marks the id supplied
        // without putting it in images map; prepare must fail (missing or invalid).
        let jsonl = r#"{"type":"composition","width":10,"height":10,"fps":30,"duration":0.1}
{"type":"div","id":"root","parentId":null}
{"type":"image","id":"pic","parentId":"root","path":"photos/a.png"}"#;
        let draft = CompositionDraft::parse(jsonl).unwrap();
        let id = draft.requirements().requests()[0].asset_id.clone();
        let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
        inputs
            .insert_video(
                id,
                crate::probe::catalog::VideoInfoMeta {
                    width: 8,
                    height: 8,
                    duration_micros: None,
                },
            )
            .unwrap();
        let err = draft.prepare(inputs).expect_err("kind mismatch must fail");
        assert!(
            matches!(
                err,
                PrepareError::InvalidMetadata {
                    kind: ResourceKind::Image,
                    ..
                }
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn markup_image_path_stays_logical_even_with_base_dir() {
        // parse_with_base_dir must not join base into the AST locator.
        let xml = r#"
            <opencat width="10" height="10" fps="30" duration="0.1">
              <div id="root">
                <image id="pic" path="photos/a.png" class="w-[8px] h-[8px]" />
              </div>
            </opencat>
        "#;
        let base = std::path::Path::new("/host/doc/root");
        let parsed = crate::parse::markup::parse_with_base_dir(xml, Some(base)).unwrap();
        let draft = CompositionDraft::from_parsed(parsed);
        let req = &draft.requirements().requests()[0];
        assert_eq!(req.asset_id.key, "photos/a.png");
    }

    #[test]
    fn render_frame_media_plan_uses_request_asset_id_for_image() {
        let jsonl = r#"{"type":"composition","width":64,"height":64,"fps":30,"duration":0.1}
{"type":"div","id":"root","parentId":null,"className":"w-full h-full"}
{"type":"image","id":"pic","parentId":"root","path":"photos/a.png","className":"w-[32px] h-[32px]"}"#;
        let draft = CompositionDraft::parse(jsonl).unwrap();
        let id = draft.requirements().requests()[0].asset_id.clone();
        assert_eq!(id.key, "photos/a.png");
        let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
        inputs
            .insert_image(
                id.clone(),
                ImageMeta {
                    width: 32,
                    height: 32,
                },
            )
            .unwrap();
        let prepared = draft.prepare(inputs).expect("prepare");
        // Catalog must expose the same canonical id hosts supplied.
        assert!(prepared.catalog().images.contains_key(&id));
        let mut pipeline = prepared
            .open_pipeline(NoopJsContext::new().unwrap())
            .expect("open");
        let frame = pipeline.render_frame(0).expect("render");
        // FrameMediaPlan should list the image under the request's AssetId.
        use crate::ir::draw_types::ImageRef;
        let has = frame.media.images.iter().any(|img| match img {
            ImageRef::Static { asset_id } => asset_id == &id.key,
            _ => false,
        });
        assert!(
            has,
            "media plan images={:?}, expected static {}",
            frame.media.images,
            id.key
        );
    }



    /// Contract both engine and web must satisfy for video (#16): logical locator,
    /// host-supplied `VideoInfoMeta` with microsecond duration, prepare fail-fast
    /// on zero size, and FrameMediaPlan video requests carrying only
    /// `(canonical AssetId, authoritative time_micros)`.
    #[test]
    fn host_agnostic_video_metadata_and_time_contract() {
        use crate::ir::draw_types::ImageRef;
        use crate::probe::catalog::VideoInfoMeta;
        use crate::time::{secs_to_micros, DurationMicros};

        // JSONL path/locator contract (timing attrs are markup-only today).
        {
            let jsonl = r#"{"type":"composition","width":320,"height":180,"fps":30,"duration":1}
{"type":"div","id":"root","parentId":null,"className":"w-full h-full"}
{"type":"video","id":"vid","parentId":"root","path":"clips/hero.mp4","className":"w-[320px] h-[180px]"}"#;
            let draft = CompositionDraft::parse(jsonl).expect("parse jsonl");
            let reqs = draft.requirements().requests();
            assert_eq!(reqs.len(), 1);
            assert_eq!(reqs[0].kind, ResourceKind::Video);
            assert_eq!(reqs[0].asset_id.key, "video:path:clips/hero.mp4");
            let id = reqs[0].asset_id.clone();
            let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
            inputs
                .insert_video(
                    id.clone(),
                    VideoInfoMeta {
                        width: 320,
                        height: 180,
                        duration_micros: Some(DurationMicros(secs_to_micros(60.0))),
                    },
                )
                .unwrap();
            let prepared = draft.prepare(inputs).expect("prepare metadata-only");
            assert!(prepared.catalog().videos.contains_key(&id));
            assert_eq!(
                prepared.catalog().videos[&id].duration_micros,
                Some(DurationMicros(60_000_000))
            );
        }

        // Markup: host metadata + authoritative microsecond media request.
        let xml = r#"
            <opencat width="320" height="180" fps="30" duration="4">
              <div id="root" class="w-[320px] h-[180px]">
                <video id="vid" path="clips/hero.mp4" class="w-[320px] h-[180px]" data-media-start="12" />
              </div>
            </opencat>
            "#;
        let draft = CompositionDraft::parse(xml).expect("parse markup");
        let reqs = draft.requirements().requests();
        assert_eq!(reqs[0].asset_id.key, "video:path:clips/hero.mp4");
        let id = reqs[0].asset_id.clone();
        let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
        // Host supplies only metadata — never video bytes.
        inputs
            .insert_video(
                id.clone(),
                VideoInfoMeta {
                    width: 320,
                    height: 180,
                    duration_micros: Some(DurationMicros(secs_to_micros(60.0))),
                },
            )
            .unwrap();
        let prepared = draft.prepare(inputs).expect("prepare metadata-only");
        let mut pipeline = prepared
            .open_pipeline(NoopJsContext::new().unwrap())
            .expect("open");
        // Frame 0 → media time 12s → 12_000_000 µs.
        let frame = pipeline.render_frame(0).expect("render");
        assert!(
            frame.media.video_frames.iter().any(|img| matches!(
                img,
                ImageRef::VideoFrame { asset_id, time_micros }
                    if asset_id == &id.key && *time_micros == 12_000_000
            )),
            "media plan must use request AssetId + authoritative micros; got {:?}",
            frame.media.video_frames
        );
        // No guessed source frame index in the contract — only asset_id + time_micros.
        for vf in &frame.media.video_frames {
            assert!(matches!(vf, ImageRef::VideoFrame { .. }));
        }
    }

    #[test]
    fn prepare_errors_on_missing_video_metadata() {
        let jsonl = r#"{"type":"composition","width":10,"height":10,"fps":30,"duration":0.1}
{"type":"div","id":"root","parentId":null}
{"type":"video","id":"vid","parentId":"root","path":"clip.mp4"}"#;
        let draft = CompositionDraft::parse(jsonl).unwrap();
        let err = draft
            .prepare(HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC"))
            .expect_err("missing video must fail prepare");
        assert!(matches!(
            err,
            PrepareError::MissingInput {
                kind: ResourceKind::Video,
                ..
            }
        ));
    }

    #[test]
    fn prepare_errors_on_zero_size_video_metadata() {
        use crate::probe::catalog::VideoInfoMeta;
        let jsonl = r#"{"type":"composition","width":10,"height":10,"fps":30,"duration":0.1}
{"type":"div","id":"root","parentId":null}
{"type":"video","id":"vid","parentId":"root","path":"clip.mp4"}"#;
        let draft = CompositionDraft::parse(jsonl).unwrap();
        let id = draft.requirements().requests()[0].asset_id.clone();
        let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
        inputs
            .insert_video(
                id,
                VideoInfoMeta {
                    width: 0,
                    height: 0,
                    duration_micros: None,
                },
            )
            .unwrap();
        let err = draft
            .prepare(inputs)
            .expect_err("zero-size video must fail prepare");
        assert!(matches!(
            err,
            PrepareError::InvalidMetadata {
                kind: ResourceKind::Video,
                ..
            }
        ));
    }

    #[test]
    fn markup_video_path_stays_logical_even_with_base_dir() {
        let xml = r#"
            <opencat width="10" height="10" fps="30" duration="0.1">
              <div id="root">
                <video id="vid" path="clips/a.mp4" class="w-[8px] h-[8px]" />
              </div>
            </opencat>
        "#;
        let base = std::path::Path::new("/host/doc/root");
        let parsed = crate::parse::markup::parse_with_base_dir(xml, Some(base)).unwrap();
        let draft = CompositionDraft::from_parsed(parsed);
        let req = &draft.requirements().requests()[0];
        assert_eq!(req.asset_id.key, "video:path:clips/a.mp4");
    }

    // --- Lottie lifecycle (#17) ----------------------------------------------------

    #[test]
    fn draft_requirements_list_declared_lottie_with_canonical_id() {
        // Markup only: JSONL has no lottie line type yet.
        let markup = r#"
            <opencat width="10" height="10" fps="30" duration="0.1">
              <div id="root">
                <lottie id="loader" path="anim/loader.json" />
              </div>
            </opencat>
        "#;
        let draft = CompositionDraft::parse(markup).unwrap();
        let reqs = draft.requirements().requests();
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].kind, ResourceKind::Lottie);
        assert_eq!(reqs[0].asset_id.key, "lottie:path:anim/loader.json");
    }

    #[test]
    fn prepare_errors_on_missing_lottie_metadata() {
        let markup = r#"
            <opencat width="10" height="10" fps="30" duration="0.1">
              <div id="root">
                <lottie id="loader" path="anim/loader.json" />
              </div>
            </opencat>
        "#;
        let draft = CompositionDraft::parse(markup).unwrap();
        let err = draft
            .prepare(HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC"))
            .expect_err("missing lottie must fail prepare");
        assert!(matches!(
            err,
            PrepareError::MissingInput {
                kind: ResourceKind::Lottie,
                ..
            }
        ));
    }

    #[test]
    fn prepare_errors_on_invalid_lottie_layout_or_timing_metadata() {
        use crate::lottie::LottieMeta;
        let markup = r#"
            <opencat width="10" height="10" fps="30" duration="0.1">
              <div id="root">
                <lottie id="loader" path="anim/loader.json" />
              </div>
            </opencat>
        "#;
        let draft = CompositionDraft::parse(markup).unwrap();
        let id = draft.requirements().requests()[0].asset_id.clone();

        let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
        inputs
            .insert_lottie(
                id.clone(),
                LottieMeta {
                    width: 0,
                    height: 100,
                    fps: 25.0,
                    in_frame: 0.0,
                    out_frame: 10.0,
                    dependencies: vec![],
                },
            )
            .unwrap();
        let err = draft
            .clone()
            .prepare(inputs)
            .expect_err("zero width must fail");
        assert!(matches!(
            err,
            PrepareError::InvalidMetadata {
                kind: ResourceKind::Lottie,
                ..
            }
        ));

        let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
        inputs
            .insert_lottie(
                id.clone(),
                LottieMeta {
                    width: 100,
                    height: 100,
                    fps: 0.0,
                    in_frame: 0.0,
                    out_frame: 10.0,
                    dependencies: vec![],
                },
            )
            .unwrap();
        let err = draft
            .clone()
            .prepare(inputs)
            .expect_err("zero fps must fail");
        assert!(matches!(
            err,
            PrepareError::InvalidMetadata {
                kind: ResourceKind::Lottie,
                ..
            }
        ));

        let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
        inputs
            .insert_lottie(
                id,
                LottieMeta {
                    width: 100,
                    height: 100,
                    fps: 25.0,
                    in_frame: 10.0,
                    out_frame: 10.0,
                    dependencies: vec![],
                },
            )
            .unwrap();
        let err = draft.prepare(inputs).expect_err("empty frame range must fail");
        assert!(matches!(
            err,
            PrepareError::InvalidMetadata {
                kind: ResourceKind::Lottie,
                ..
            }
        ));
    }

    /// Contract both engine and web must satisfy for Lottie (#17):
    /// requirements emit canonical bundle AssetId + kind + logical locator;
    /// prepare only consumes LottieMeta (width/height/fps/frame range/deps);
    /// RenderFrame emits Lottie DrawOp + deduped lottie_bundles (not as image).
    #[test]
    fn host_agnostic_lottie_contract() {
        use crate::ir::draw_op::DrawOp;
        use crate::lottie::LottieMeta;

        let sources = [
            r#"
            <opencat width="64" height="64" fps="30" duration="0.1">
              <div id="root" class="w-full h-full">
                <lottie id="loader" path="anim/loader.json" class="w-[32px] h-[32px]" />
              </div>
            </opencat>
            "#,
        ];
        for input in sources {
            let draft = CompositionDraft::parse(input).expect("parse");
            let reqs = draft.requirements().requests();
            assert_eq!(reqs.len(), 1);
            assert_eq!(reqs[0].kind, ResourceKind::Lottie);
            assert_eq!(reqs[0].asset_id.key, "lottie:path:anim/loader.json");

            let id = reqs[0].asset_id.clone();
            let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
            // Host supplies only metadata — never Lottie JSON/bytes — under request id.
            inputs
                .insert_lottie(
                    id.clone(),
                    LottieMeta {
                        width: 280,
                        height: 200,
                        fps: 25.0,
                        in_frame: 0.0,
                        out_frame: 32.0,
                        dependencies: vec!["image_0.png".into()],
                    },
                )
                .unwrap();
            let prepared = draft.prepare(inputs).expect("prepare metadata-only");
            let stored = prepared.catalog().lotties.get(&id).expect("catalog lottie");
            assert_eq!(stored.width, 280);
            assert_eq!(stored.height, 200);
            assert_eq!(stored.fps, 25.0);
            assert_eq!(stored.dependencies, vec!["image_0.png".to_string()]);

            let mut pipeline = prepared
                .open_pipeline(NoopJsContext::new().unwrap())
                .expect("open");
            let frame = pipeline.render_frame(0).expect("render");
            assert!(
                frame.media.lottie_bundles.iter().any(|b| b == &id.key),
                "media plan must list Lottie bundle under request AssetId; got {:?}",
                frame.media.lottie_bundles
            );
            // Not disguised as a static image.
            assert!(
                frame.media.images.is_empty(),
                "Lottie must not appear as ordinary image; got {:?}",
                frame.media.images
            );
            let lottie_frame = frame.draw.ops.iter().find_map(|op| match op {
                DrawOp::LottieRect { bundle_id, frame, .. } if bundle_id == &id.key => Some(*frame),
                _ => None,
            });
            let lottie_frame = lottie_frame.unwrap_or_else(|| {
                panic!(
                    "draw ops must include LottieRect for {}; ops={:?}",
                    id.key, frame.draw.ops
                )
            });
            // frame 0 @ 30fps, meta fps 25, media_start 0 → local frame 0
            assert!(
                (lottie_frame - 0.0).abs() < 0.01,
                "frame mapping for t=0 must be in_frame; got {lottie_frame}"
            );
        }
    }

    /// Contract both engine and web must satisfy for static images (#15):
    /// requirements emit opaque AssetId + kind; prepare only consumes
    /// ImageMeta under that id; RenderFrame media plan echoes the same id.
    #[test]
    fn host_agnostic_static_image_contract() {
        let sources = [
            r#"{"type":"composition","width":32,"height":32,"fps":30,"duration":0.1}
{"type":"div","id":"root","parentId":null}
{"type":"image","id":"pic","parentId":"root","path":"assets/hero.png","className":"w-[16px] h-[16px]"}"#,
            r#"
            <opencat width="32" height="32" fps="30" duration="0.1">
              <div id="root" class="w-full h-full">
                <image id="pic" path="assets/hero.png" class="w-[16px] h-[16px]" />
              </div>
            </opencat>
            "#,
        ];
        for input in sources {
            let draft = CompositionDraft::parse(input).expect("parse");
            let reqs = draft.requirements().requests();
            assert_eq!(reqs.len(), 1);
            assert_eq!(reqs[0].kind, ResourceKind::Image);
            assert_eq!(reqs[0].asset_id.key, "assets/hero.png");

            let id = reqs[0].asset_id.clone();
            let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
            // Host supplies only metadata — never image bytes — under the request id.
            inputs
                .insert_image(
                    id.clone(),
                    ImageMeta {
                        width: 16,
                        height: 16,
                    },
                )
                .unwrap();
            let prepared = draft.prepare(inputs).expect("prepare metadata-only");
            assert!(prepared.catalog().images.contains_key(&id));
            assert_eq!(
                prepared.catalog().images[&id],
                ImageMeta {
                    width: 16,
                    height: 16
                }
            );

            let mut pipeline = prepared
                .open_pipeline(NoopJsContext::new().unwrap())
                .expect("open");
            let frame = pipeline.render_frame(0).expect("render");
            use crate::ir::draw_types::ImageRef;
            assert!(
                frame.media.images.iter().any(|img| matches!(
                    img,
                    ImageRef::Static { asset_id } if asset_id == &id.key
                )),
                "media plan must echo request AssetId; got {:?}",
                frame.media.images
            );
        }
    }


    // --- fonts & subtitles (#19) -------------------------------------------------

    #[test]
    fn draft_requirements_list_document_fonts_with_stable_identity() {
        let xml = r#"
            <opencat width="64" height="64" fps="30" duration="0.1">
              <fonts default="sans">
                <font id="sans" family="Noto Sans SC" path="fonts/NotoSansSC-Regular.otf" role="sans" />
              </fonts>
              <div id="root" class="w-full h-full font-sans">
                <text id="t" class="text-white" data-text="Hi" />
              </div>
            </opencat>
        "#;
        let draft = CompositionDraft::parse(xml).expect("parse");
        let fonts: Vec<_> = draft
            .requirements()
            .requests()
            .iter()
            .filter(|r| r.kind == ResourceKind::Font)
            .collect();
        assert_eq!(fonts.len(), 1);
        assert_eq!(
            fonts[0].asset_id.key,
            "font:path:fonts/NotoSansSC-Regular.otf"
        );

        // base_dir must not leak into the stable identity (parity with images).
        let base = std::path::Path::new("/host/doc/root");
        let parsed = crate::parse::markup::parse_with_base_dir(xml, Some(base)).unwrap();
        let draft = CompositionDraft::from_parsed(parsed);
        let fonts: Vec<_> = draft
            .requirements()
            .requests()
            .iter()
            .filter(|r| r.kind == ResourceKind::Font)
            .collect();
        assert_eq!(
            fonts[0].asset_id.key,
            "font:path:fonts/NotoSansSC-Regular.otf"
        );
    }

    #[test]
    fn prepare_errors_on_missing_document_font() {
        let xml = r#"
            <opencat width="64" height="64" fps="30" duration="0.1">
              <fonts default="sans">
                <font id="sans" family="Noto Sans SC" path="fonts/NotoSansSC-Regular.otf" role="sans" />
              </fonts>
              <div id="root" class="w-full h-full" />
            </opencat>
        "#;
        let draft = CompositionDraft::parse(xml).expect("parse");
        let err = draft
            .prepare(HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC"))
            .expect_err("missing document font must fail prepare");
        assert!(
            matches!(
                err,
                PrepareError::MissingInput {
                    kind: ResourceKind::Font,
                    ..
                }
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn prepare_errors_on_empty_document_font_bytes() {
        let xml = r#"
            <opencat width="64" height="64" fps="30" duration="0.1">
              <fonts default="sans">
                <font id="sans" family="Noto Sans SC" path="fonts/NotoSansSC-Regular.otf" role="sans" />
              </fonts>
              <div id="root" class="w-full h-full" />
            </opencat>
        "#;
        let draft = CompositionDraft::parse(xml).expect("parse");
        let id = draft
            .requirements()
            .requests()
            .iter()
            .find(|r| r.kind == ResourceKind::Font)
            .unwrap()
            .asset_id
            .clone();
        let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
        inputs.insert_document_font(id, Vec::new()).unwrap();
        let err = draft
            .prepare(inputs)
            .expect_err("empty font bytes must fail");
        assert!(
            matches!(
                err,
                PrepareError::InvalidMetadata {
                    kind: ResourceKind::Font,
                    ..
                }
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn prepare_merges_document_font_and_applies_font_sans() {
        let xml = r#"
            <opencat width="320" height="180" fps="30" duration="0.1">
              <fonts default="sans">
                <font id="sans" family="Noto Sans SC" path="fonts/NotoSansSC-Regular.otf" role="sans" />
              </fonts>
              <div id="root" class="w-full h-full">
                <text id="t" class="font-sans text-white text-[24px]" data-text="你好" />
              </div>
            </opencat>
        "#;
        let draft = CompositionDraft::parse(xml).expect("parse");
        let font_id = draft
            .requirements()
            .requests()
            .iter()
            .find(|r| r.kind == ResourceKind::Font)
            .unwrap()
            .asset_id
            .clone();

        // Empty base so document face is the sole sans source.
        let mut inputs = HostInputs::empty();
        let face_bytes = include_bytes!("../../../../assets/NotoSansSC-Regular.otf").to_vec();
        inputs
            .insert_document_font(font_id, face_bytes)
            .expect("insert font");

        let prepared = draft.prepare(inputs).expect("prepare with document font");
        assert_eq!(
            prepared.font_db().family_name(&fontdb::Family::SansSerif),
            "Noto Sans SC"
        );
        assert!(
            prepared.font_db().faces().count() >= 1,
            "document face must load"
        );

        // font-sans must have resolved to a concrete family at parse/build time.
        // Walk the prepared tree for the text node style.
        use crate::parse::node::NodeKind;
        fn find_text_family(node: &crate::parse::node::Node) -> Option<String> {
            match node.kind() {
                NodeKind::Text(t) => t.style_ref().font_family.clone(),
                NodeKind::Div(d) => d
                    .children_ref()
                    .iter()
                    .find_map(|c| find_text_family(c)),
                _ => None,
            }
        }
        let family = find_text_family(&prepared.parsed().root);
        assert_eq!(family.as_deref(), Some("Noto Sans SC"));

        let mut pipeline = prepared
            .open_pipeline(NoopJsContext::new().unwrap())
            .expect("open");
        let frame = pipeline.render_frame(0).expect("render");
        assert!(!frame.draw.ops.is_empty());
    }

    #[test]
    fn prepare_document_font_takes_precedence_over_same_family_base() {
        let xml = r#"
            <opencat width="64" height="64" fps="30" duration="0.1">
              <fonts default="doc">
                <font id="doc" family="Noto Sans SC" path="fonts/NotoSansSC-Regular.otf" role="sans" />
              </fonts>
              <div id="root" class="w-full h-full" />
            </opencat>
        "#;
        let draft = CompositionDraft::parse(xml).expect("parse");
        let font_id = draft
            .requirements()
            .requests()
            .iter()
            .find(|r| r.kind == ResourceKind::Font)
            .unwrap()
            .asset_id
            .clone();

        // Base already has Noto Sans SC (engine-like defaults).
        let face_bytes = include_bytes!("../../../../assets/NotoSansSC-Regular.otf").to_vec();
        let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
        inputs
            .insert_document_font(font_id, face_bytes)
            .expect("insert");

        let prepared = draft.prepare(inputs).expect("prepare");
        // Document wins: only one Noto Sans SC face (no duplicate base face).
        let noto_faces = prepared
            .font_db()
            .faces()
            .filter(|face| {
                face.families
                    .iter()
                    .any(|(family, _)| family == "Noto Sans SC")
            })
            .count();
        assert_eq!(noto_faces, 1, "document face must replace same-family base");
        assert_eq!(
            prepared.font_db().family_name(&fontdb::Family::SansSerif),
            "Noto Sans SC"
        );
    }

    #[test]
    fn prepare_font_face_id_resolves_to_family() {
        let xml = r#"
            <opencat width="320" height="180" fps="30" duration="0.1">
              <fonts>
                <font id="display" family="Noto Sans SC" path="fonts/display.otf" />
              </fonts>
              <div id="root" class="w-full h-full">
                <text id="t" class="font-[display] text-white text-[20px]" data-text="Aa" />
              </div>
            </opencat>
        "#;
        let draft = CompositionDraft::parse(xml).expect("parse");
        let font_id = draft
            .requirements()
            .requests()
            .iter()
            .find(|r| r.kind == ResourceKind::Font)
            .unwrap()
            .asset_id
            .clone();
        let mut inputs = HostInputs::empty();
        let face_bytes = include_bytes!("../../../../assets/NotoSansSC-Regular.otf").to_vec();
        inputs
            .insert_document_font(font_id, face_bytes)
            .expect("insert font");
        let prepared = draft.prepare(inputs).expect("prepare");

        use crate::parse::node::NodeKind;
        fn find_text_family(node: &crate::parse::node::Node) -> Option<String> {
            match node.kind() {
                NodeKind::Text(t) => t.style_ref().font_family.clone(),
                NodeKind::Div(d) => d.children_ref().iter().find_map(|c| find_text_family(c)),
                _ => None,
            }
        }
        assert_eq!(
            find_text_family(&prepared.parsed().root).as_deref(),
            Some("Noto Sans SC")
        );
    }

    #[test]
    fn prepare_hydrates_subtitle_active_text() {
        let jsonl = r#"{"type":"composition","width":320,"height":180,"fps":30,"duration":1}
{"id":"root","parentId":null,"type":"div","className":"relative w-[320px] h-[180px]"}
{"id":"subs","parentId":"root","type":"caption","className":"absolute left-[0px] top-[0px] text-white","path":"sub.srt"}"#;
        let draft = CompositionDraft::parse(jsonl).expect("parse");
        let sub_id = draft
            .requirements()
            .requests()
            .iter()
            .find(|r| r.kind == ResourceKind::Subtitle)
            .expect("subtitle req")
            .asset_id
            .clone();

        let srt = "1\n00:00:00,000 --> 00:00:00,500\nHello Core\n";
        let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
        inputs
            .insert_subtitle_text(sub_id, srt)
            .expect("insert srt");

        let prepared = draft.prepare(inputs).expect("prepare");
        use crate::parse::node::NodeKind;
        use crate::parse::primitives::CaptionNode;
        fn find_caption<'a>(node: &'a crate::parse::node::Node, id: &str) -> Option<&'a CaptionNode> {
            match node.kind() {
                NodeKind::Caption(c) if c.style_ref().id == id => Some(c),
                NodeKind::Div(d) => d.children_ref().iter().find_map(|c| find_caption(c, id)),
                _ => None,
            }
        }
        let caption = find_caption(&prepared.parsed().root, "subs").expect("caption");
        assert_eq!(caption.entries_ref().len(), 1);
        assert_eq!(caption.active_text(0), Some("Hello Core"));
        // Past the cue end: no active text.
        assert_eq!(caption.active_text(30), None);
    }

    #[test]
    fn prepare_missing_subtitle_text_keeps_empty_entries() {
        let jsonl = r#"{"type":"composition","width":64,"height":64,"fps":30,"duration":0.1}
{"id":"root","parentId":null,"type":"div"}
{"id":"subs","parentId":"root","type":"caption","path":"missing.srt"}"#;
        let draft = CompositionDraft::parse(jsonl).expect("parse");
        let sub_id = draft
            .requirements()
            .requests()
            .iter()
            .find(|r| r.kind == ResourceKind::Subtitle)
            .expect("subtitle req")
            .asset_id
            .clone();
        // Empty text is valid (explicit empty caption entries).
        let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
        inputs
            .insert_subtitle_text(sub_id, "")
            .expect("insert empty srt");
        let prepared = draft.prepare(inputs).expect("empty SRT is valid");
        use crate::parse::node::NodeKind;
        fn find_caption_entries(node: &crate::parse::node::Node) -> Option<usize> {
            match node.kind() {
                NodeKind::Caption(c) => Some(c.entries_ref().len()),
                NodeKind::Div(d) => d.children_ref().iter().find_map(|c| find_caption_entries(c)),
                _ => None,
            }
        }
        assert_eq!(find_caption_entries(&prepared.parsed().root), Some(0));
    }

    #[test]
    fn xml_caption_hydrate_via_lifecycle() {
        let xml = r#"
            <opencat width="320" height="180" fps="30" duration="1">
              <div id="root" class="w-full h-full">
                <caption id="subs" path="sub.srt" />
              </div>
            </opencat>
        "#;
        let draft = CompositionDraft::parse(xml).expect("parse xml");
        let sub_id = draft
            .requirements()
            .requests()
            .iter()
            .find(|r| r.kind == ResourceKind::Subtitle)
            .expect("subtitle req")
            .asset_id
            .clone();
        assert!(
            sub_id.key.contains("sub.srt"),
            "subtitle asset id should reference the path: {sub_id:?}"
        );

        let srt = "1\n00:00:00,000 --> 00:00:00,500\nXML Caption\n";
        let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
        inputs
            .insert_subtitle_text(sub_id, srt)
            .expect("insert srt");
        let prepared = draft.prepare(inputs).expect("prepare");
        use crate::parse::node::NodeKind;
        use crate::parse::primitives::CaptionNode;
        fn find_caption<'a>(node: &'a crate::parse::node::Node, id: &str) -> Option<&'a CaptionNode> {
            match node.kind() {
                NodeKind::Caption(c) if c.style_ref().id == id => Some(c),
                NodeKind::Div(d) => d.children_ref().iter().find_map(|c| find_caption(c, id)),
                _ => None,
            }
        }
        let caption = find_caption(&prepared.parsed().root, "subs").expect("caption node");
        assert_eq!(caption.entries_ref().len(), 1);
        assert_eq!(caption.active_text(0), Some("XML Caption"));
    }

    #[test]
    fn xml_caption_missing_text_fails_prepare() {
        let xml = r#"
            <opencat width="320" height="180" fps="30" duration="1">
              <div id="root" class="w-full h-full">
                <caption id="subs" path="sub.srt" />
              </div>
            </opencat>
        "#;
        let draft = CompositionDraft::parse(xml).expect("parse xml");
        // No subtitle text supplied → MissingInput fail-fast.
        let err = draft
            .prepare(HostInputs::empty()
                .with_base_font_faces(crate::test_support::test_font_faces())
                .with_sans_serif_family("Noto Sans SC"))
            .expect_err("missing subtitle text must fail");
        assert!(
            matches!(
                err,
                PrepareError::MissingInput {
                    kind: ResourceKind::Subtitle,
                    ..
                }
            ),
            "got {err:?}"
        );
    }

    /// Host-agnostic AudioPlan contract (#18): core derives typed ranges from
    /// timeline/scene/transition/explicit duration; hosts only consume the plan.
    /// Missing non-critical audio metadata must not block prepare.
    #[test]
    fn host_agnostic_audio_plan_contract() {
        use crate::time::{DurationMicros, TimestampMicros};

        // Two scenes + transition; scene-a audio, scene-b audio with trim, timeline BGM.
        // Scene A: 10/30s, transition: 5/30s, scene B: 20/30s → total 35 frames @ 30fps.
        let jsonl = r#"{"type":"composition","width":100,"height":100,"fps":30,"duration":1.166666666666}
{"type":"div","id":"root","parentId":null}
{"type":"tl","id":"main-tl","parentId":"root"}
{"type":"div","id":"scene-a","parentId":"main-tl","duration":0.333333333333}
{"type":"transition","parentId":"main-tl","from":"scene-a","to":"scene-b","effect":"fade","duration":0.166666666666,"timing":"linear"}
{"type":"div","id":"scene-b","parentId":"main-tl","duration":0.666666666666}
{"type":"audio","id":"bgm","attach":"main-tl","url":"https://example.com/bgm.mp3"}
{"type":"audio","id":"a","attach":"scene-a","url":"https://example.com/a.mp3"}
{"type":"audio","id":"b","attach":"scene-b","url":"https://example.com/b.mp3","duration":0.1}"#;

        let draft = CompositionDraft::parse(jsonl).expect("parse");
        let reqs = draft.requirements().requests();
        let audio_reqs: Vec<_> = reqs
            .iter()
            .filter(|r| r.kind == ResourceKind::Audio)
            .collect();
        assert_eq!(audio_reqs.len(), 3, "three audio requirements");
        for r in &audio_reqs {
            assert!(r.asset_id.key.starts_with("audio:url:"));
        }

        // Audio needs only presence registration — no layout metadata (non-critical).
        let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
        for r in &audio_reqs {
            inputs.insert_audio(r.asset_id.clone()).unwrap();
        }
        let prepared = draft.prepare(inputs).expect("prepare without audio probe meta");
        let pipeline = prepared
            .open_pipeline(NoopJsContext::new().unwrap())
            .expect("open");
        let plan = &pipeline.info().audio_plan;
        assert_eq!(plan.segments.len(), 3, "plan segments={:?}", plan.segments);

        // Order follows composition audio_sources declaration: bgm, a, b.
        let bgm = plan
            .segments
            .iter()
            .find(|s| s.asset.key.contains("bgm"))
            .expect("bgm");
        let a = plan
            .segments
            .iter()
            .find(|s| s.asset.key.ends_with("/a.mp3"))
            .expect("a");
        let b = plan
            .segments
            .iter()
            .find(|s| s.asset.key.ends_with("/b.mp3"))
            .expect("b");

        // Timeline BGM spans full composition frame count.
        assert_eq!(bgm.start_micros(), TimestampMicros(0));
        assert!(
            bgm.end_micros().0 > 1_000_000,
            "bgm should span composition duration, end={}",
            bgm.end_micros().0
        );

        // Scene-a starts at 0.
        assert_eq!(a.start_micros(), TimestampMicros(0));
        assert!(a.end_micros().0 > 300_000 && a.end_micros().0 < 400_000);

        // Scene-b starts after scene-a + transition (~500ms); explicit duration trims to 100ms.
        assert!(
            b.start_micros().0 >= 490_000 && b.start_micros().0 <= 510_000,
            "scene-b start should include transition offset, got {}",
            b.start_micros().0
        );
        assert_eq!(b.duration_micros(), DurationMicros(100_000));
        assert_eq!(
            b.end_micros().0,
            b.start_micros().0 + 100_000,
            "explicit duration trims segment"
        );
    }

    #[test]
    fn audio_plan_prepare_does_not_require_audio_probe_fields() {
        // AC: missing non-critical audio metadata must not fail prepare.
        let jsonl = r#"{"type":"composition","width":10,"height":10,"fps":30,"duration":0.1}
{"type":"div","id":"root","parentId":null}
{"type":"audio","id":"bgm","attach":"root","url":"https://example.com/bgm.mp3"}"#;
        let draft = CompositionDraft::parse(jsonl).unwrap();
        let id = draft
            .requirements()
            .requests()
            .iter()
            .find(|r| r.kind == ResourceKind::Audio)
            .unwrap()
            .asset_id
            .clone();
        let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
        inputs.insert_audio(id).unwrap();
        let prepared = draft.prepare(inputs).expect("audio presence-only ok");
        assert!(!prepared
            .open_pipeline(NoopJsContext::new().unwrap())
            .unwrap()
            .info()
            .audio_plan
            .segments
            .is_empty());
    }

    #[test]
    fn draft_requirements_list_external_scripts() {
        let jsonl = r#"{"type":"composition","width":64,"height":36,"fps":30,"duration":0.1}
{"id":"root","parentId":null,"type":"div","className":"flex"}
{"type":"script","path":"anim/main.js"}"#;
        let draft = CompositionDraft::parse(jsonl).unwrap();
        let scripts: Vec<_> = draft
            .requirements()
            .requests()
            .iter()
            .filter(|r| r.kind == ResourceKind::Script)
            .collect();
        assert_eq!(scripts.len(), 1);
        assert_eq!(scripts[0].asset_id.key, "script:path:anim/main.js");
    }

    #[test]
    fn prepare_errors_on_missing_external_script_text() {
        let jsonl = r#"{"type":"composition","width":64,"height":36,"fps":30,"duration":0.1}
{"id":"root","parentId":null,"type":"div","className":"flex"}
{"type":"script","path":"missing.js"}"#;
        let draft = CompositionDraft::parse(jsonl).unwrap();
        let err = draft
            .prepare(HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC"))
            .expect_err("missing script text must fail");
        assert!(matches!(
            err,
            PrepareError::MissingInput {
                kind: ResourceKind::Script,
                ..
            }
        ));
    }

    #[test]
    fn prepare_injects_external_script_text_into_driver() {
        let jsonl = r#"{"type":"composition","width":64,"height":36,"fps":30,"duration":0.1}
{"id":"root","parentId":null,"type":"div","className":"flex"}
{"type":"script","parentId":"root","path":"node.js"}"#;
        let draft = CompositionDraft::parse(jsonl).unwrap();
        let id = draft
            .requirements()
            .requests()
            .iter()
            .find(|r| r.kind == ResourceKind::Script)
            .unwrap()
            .asset_id
            .clone();
        let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
        inputs
            .insert_script_text(id, "ctx.getNode('root').opacity(0.5);")
            .unwrap();
        let prepared = draft.prepare(inputs).expect("prepare");
        let driver = prepared
            .parsed()
            .root
            .style_ref()
            .script_driver
            .as_ref()
            .expect("script on root");
        assert_eq!(driver.source, "ctx.getNode('root').opacity(0.5);");
        assert!(!driver.is_external_pending());
    }

    #[test]
    fn prepare_rejects_empty_script_text() {
        let jsonl = r#"{"type":"composition","width":64,"height":36,"fps":30,"duration":0.1}
{"id":"root","parentId":null,"type":"div","className":"flex"}
{"type":"script","path":"empty.js"}"#;
        let draft = CompositionDraft::parse(jsonl).unwrap();
        let id = draft
            .requirements()
            .requests()
            .iter()
            .find(|r| r.kind == ResourceKind::Script)
            .unwrap()
            .asset_id
            .clone();
        let mut inputs = HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC");
        inputs.insert_script_text(id, "").unwrap();
        let err = draft.prepare(inputs).expect_err("empty script text");
        assert!(matches!(
            err,
            PrepareError::InvalidMetadata {
                kind: ResourceKind::Script,
                ..
            }
        ));
    }

    #[test]
    fn two_pipelines_own_independent_script_realms() {
        // AC: each pipeline creates an independent script realm; hosts pass a
        // fresh JsContext per open. NoopJsContext has no shared process global.
        let xml = r#"
            <opencat width="64" height="36" fps="30" duration="0.1">
              <div id="root" class="w-full h-full" />
            </opencat>
        "#;
        let a = open_lifecycle(xml);
        let mut b = open_lifecycle(xml);
        // Distinct host contexts are wrapped into distinct ScriptRealm instances.
        let _ = a.scripts();
        let _ = b.scripts();
        // Rendering either must not require rebinding a shared dispatcher.
        let _ = a;
        let frame = b.render_frame(0).expect("render");
        let _ = frame;
    }

    #[test]
    fn inline_scripts_do_not_create_script_requirements() {
        let xml = r#"
            <opencat width="64" height="36" fps="30" duration="0.1">
              <script>ctx.getNode("root").opacity(1);</script>
              <div id="root" class="w-full h-full" />
            </opencat>
        "#;
        let draft = CompositionDraft::parse(xml).unwrap();
        assert!(
            draft
                .requirements()
                .requests()
                .iter()
                .all(|r| r.kind != ResourceKind::Script),
            "inline markup scripts are not host requirements"
        );
        let prepared = draft
            .prepare(HostInputs::empty()
            .with_base_font_faces(crate::test_support::test_font_faces())
            .with_sans_serif_family("Noto Sans SC"))
            .expect("inline scripts need no host text");
        assert!(prepared.parsed().root.style_ref().script_driver.is_some());
    }
}
