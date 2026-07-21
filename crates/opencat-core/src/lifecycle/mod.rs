//! Explicit composition lifecycle: parse → draft → requirements → host inputs →
//! prepare → prepared composition → pipeline.
//!
//! This is the expand-phase surface for issue #12 / #14. Hosts still own all
//! fetch/cache/decode work; core only validates host-supplied inputs and opens a
//! pure derivation pipeline. The legacy
//! [`crate::pipeline::DefaultPipeline::open_with_prepared_catalog`] entry remains
//! available for existing callers.

mod types;

pub use types::{
    CompositionDraft, HostInputs, HostRequirements, PrepareError, PreparedComposition,
    ResourceKind, ResourceLocator, ResourceRequest,
};

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;

use crate::ir::asset_id::{
    asset_id_for_audio, asset_id_for_image, asset_id_for_lottie, asset_id_for_subtitle,
    asset_id_for_video, AssetId,
};
use crate::parse::primitives::LottieSource;
use crate::parse::preflight::collect_resource_requests_from_parsed;
use crate::parse::ParsedComposition;
use crate::pipeline::DefaultPipeline;
use crate::probe::catalog::{PreparedResourceCatalog, ResourceRequests};
use crate::probe::prepare::hydrate_captions;
use crate::resource::fonts::{font_asset_id, merge_document_over_base, FontSource};
use crate::script::js_context::JsContext;

impl CompositionDraft {
    /// Build a draft from an already-parsed composition.
    pub fn from_parsed(parsed: ParsedComposition) -> Self {
        let requests = collect_resource_requests_from_parsed(&parsed);
        let requirements = HostRequirements::from_parsed(&parsed, &requests);
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

    /// Underlying declarative collection kept for back-compat with the probe
    /// chain (`build_catalog`, engine/web loaders).
    pub fn resource_requests(&self) -> &ResourceRequests {
        &self.raw
    }

    fn from_parsed(parsed: &ParsedComposition, raw: &ResourceRequests) -> Self {
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
                locator: ResourceLocator::from_image(src),
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
                locator: ResourceLocator::from_video(src),
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
                locator: ResourceLocator::from_audio(src),
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
                locator: ResourceLocator::from_subtitle(src),
            });
        }

        for req in &raw.lotties {
            let Some(id) = asset_id_for_lottie(&req.element_id, &req.source) else {
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
                locator: ResourceLocator::from_lottie(&req.source),
            });
        }

        // Document fonts: stable identity is font_asset_id(source). Face markup
        // id is not the resource identity — hosts fetch by locator/asset_id.
        for face in &parsed.font_manifest.faces {
            let id = AssetId(font_asset_id(&face.source));
            if !seen.insert(id.clone()) {
                continue;
            }
            requests.push(ResourceRequest {
                asset_id: id,
                kind: ResourceKind::Font,
                locator: ResourceLocator::from_font_source(&face.source),
            });
        }

        // Stable order for tests/hosts: kind then asset id.
        requests.sort_by(|a, b| {
            a.kind
                .cmp(&b.kind)
                .then_with(|| a.asset_id.0.cmp(&b.asset_id.0))
        });

        Self {
            requests,
            raw: raw.clone(),
        }
    }
}

impl HostInputs {
    /// Empty host inputs: no resource metadata, no subtitle text, empty font db.
    /// Suitable only for compositions with no declared external resources (and
    /// no document fonts that need shaping beyond the empty database).
    pub fn empty() -> Self {
        Self {
            font_db: Arc::new(crate::text::empty_font_db()),
            catalog: PreparedResourceCatalog::default(),
            subtitle_texts: HashMap::new(),
            document_fonts: HashMap::new(),
            supplied: HashSet::new(),
        }
    }

    pub fn with_font_db(mut self, font_db: Arc<fontdb::Database>) -> Self {
        self.font_db = font_db;
        self
    }

    pub fn font_db(&self) -> &Arc<fontdb::Database> {
        &self.font_db
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
        meta: crate::resource::lottie::LottieMeta,
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
        self.subtitle_texts.insert(id.0, text.into());
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

    /// Fill this inputs bag from a host-probed [`PreparedResourceCatalog`] and
    /// optional subtitle texts, using **only** the asset ids listed in
    /// `requirements` (never re-derived from locators). Soft-misses for subtitle
    /// text are allowed; missing image/video/lottie metadata returns
    /// [`PrepareError::MissingInput`]. Font requirements are skipped here —
    /// hosts must call [`HostInputs::insert_document_font`] with the raw face
    /// bytes (fonts are content-level inputs, not probe metadata).
    pub fn fill_from_prepared_catalog(
        &mut self,
        requirements: &HostRequirements,
        probed: &PreparedResourceCatalog,
        subtitle_texts: &HashMap<String, String>,
    ) -> Result<(), PrepareError> {
        for req in requirements.requests() {
            match req.kind {
                ResourceKind::Image => {
                    let Some(meta) = probed.images.get(&req.asset_id).copied() else {
                        return Err(PrepareError::MissingInput {
                            asset_id: req.asset_id.clone(),
                            kind: req.kind,
                        });
                    };
                    self.insert_image(req.asset_id.clone(), meta)?;
                }
                ResourceKind::Video => {
                    let Some(meta) = probed.videos.get(&req.asset_id).copied() else {
                        return Err(PrepareError::MissingInput {
                            asset_id: req.asset_id.clone(),
                            kind: req.kind,
                        });
                    };
                    self.insert_video(req.asset_id.clone(), meta)?;
                }
                ResourceKind::Audio => {
                    self.insert_audio(req.asset_id.clone())?;
                }
                ResourceKind::Lottie => {
                    let Some(meta) = probed.lotties.get(&req.asset_id).cloned() else {
                        return Err(PrepareError::MissingInput {
                            asset_id: req.asset_id.clone(),
                            kind: req.kind,
                        });
                    };
                    self.insert_lottie(req.asset_id.clone(), meta)?;
                }
                ResourceKind::Subtitle => {
                    if let Some(text) = subtitle_texts.get(&req.asset_id.0) {
                        self.insert_subtitle_text(req.asset_id.clone(), text.clone())?;
                    }
                }
                ResourceKind::Font => {
                    // Host supplies document font bytes via insert_document_font.
                }
            }
        }
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
            | ResourceKind::Font => {}
        }
    }

    let fps = parsed.fps.max(1) as u32;
    parsed.root = hydrate_captions(parsed.root, fps, &inputs.subtitle_texts)
        .map_err(|err| PrepareError::Internal {
            message: err.to_string(),
        })?
        .0;

    Ok(PreparedComposition {
        parsed,
        catalog,
        font_db,
    })
}

/// Merge document font bytes over the host base database. Fail-fast when a
/// declared face has no bytes or zero-length bytes. Empty manifests leave the
/// base database unchanged (cloned).
fn prepare_font_db(
    parsed: &ParsedComposition,
    inputs: &HostInputs,
) -> Result<Arc<fontdb::Database>, PrepareError> {
    let manifest = &parsed.font_manifest;
    if manifest.is_empty() {
        return Ok(inputs.font_db.clone());
    }

    // Remap asset-id → face-id for load_faces_into_db / merge_document_over_base.
    let mut bytes_by_face_id: HashMap<String, Vec<u8>> = HashMap::new();
    for face in &manifest.faces {
        let asset_id = AssetId(font_asset_id(&face.source));
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

    let (db, _index) = merge_document_over_base(inputs.font_db.as_ref(), manifest, &bytes_by_face_id)
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
                .chain(inputs.document_fonts.keys()),
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
            ResourceKind::Subtitle => {}
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
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// ResourceLocator helpers (logical locators — no host base-dir resolution)
// ---------------------------------------------------------------------------

impl ResourceLocator {
    fn from_image(src: &crate::parse::primitives::ImageSource) -> Self {
        use crate::parse::primitives::ImageSource;
        match src {
            ImageSource::Unset => Self::Unset,
            ImageSource::Path(p) => Self::LogicalPath(p.clone()),
            ImageSource::Url(u) => Self::Url(u.clone()),
            ImageSource::Query(q) => Self::Query {
                query: q.query.clone(),
                count: q.count,
                aspect_ratio: q.aspect_ratio.clone(),
            },
        }
    }

    fn from_video(src: &crate::parse::primitives::VideoSource) -> Self {
        use crate::parse::primitives::VideoSource;
        match src {
            VideoSource::Path(p) => Self::LogicalPath(p.clone()),
            VideoSource::Url(u) => Self::Url(u.clone()),
        }
    }

    fn from_audio(src: &crate::parse::primitives::AudioSource) -> Self {
        use crate::parse::primitives::AudioSource;
        match src {
            AudioSource::Unset => Self::Unset,
            AudioSource::Path(p) => Self::LogicalPath(p.to_string_lossy().into_owned()),
            AudioSource::Url(u) => Self::Url(u.clone()),
        }
    }

    fn from_subtitle(src: &crate::parse::primitives::SubtitleSource) -> Self {
        use crate::parse::primitives::SubtitleSource;
        match src {
            SubtitleSource::Path(p) => Self::LogicalPath(p.to_string_lossy().into_owned()),
            SubtitleSource::Url(u) => Self::Url(u.clone()),
        }
    }

    fn from_lottie(src: &LottieSource) -> Self {
        match src {
            LottieSource::Unset => Self::Unset,
            LottieSource::Path(p) => Self::LogicalPath(p.clone()),
            LottieSource::Url(u) => Self::Url(u.clone()),
        }
    }

    fn from_font_source(src: &FontSource) -> Self {
        match src {
            // Markup may still join a host base into Path at parse time for
            // engine path resolution; the logical locator string is whatever
            // the manifest source carries (hosts interpret against document base).
            FontSource::Path(p) => Self::LogicalPath(p.to_string_lossy().into_owned()),
            FontSource::Url(u) => Self::Url(u.clone()),
        }
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

    fn test_font_db() -> Arc<fontdb::Database> {
        Arc::new(crate::text::test_default_font_db())
    }

    fn open_lifecycle(input: &str) -> DefaultPipeline<NoopJsContext> {
        let draft = CompositionDraft::parse(input).expect("parse draft");
        let inputs = HostInputs::empty().with_font_db(test_font_db());
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
        assert_eq!(reqs[0].asset_id.0, "photos/a.png");
        assert!(matches!(
            &reqs[0].locator,
            ResourceLocator::LogicalPath(p) if p == "photos/a.png"
        ));
    }

    #[test]
    fn prepare_errors_on_missing_image_metadata() {
        let jsonl = r#"{"type":"composition","width":10,"height":10,"fps":30,"duration":0.1}
{"type":"div","id":"root","parentId":null}
{"type":"image","id":"pic","parentId":"root","path":"photos/a.png"}"#;
        let draft = CompositionDraft::parse(jsonl).unwrap();
        let err = draft
            .prepare(HostInputs::empty().with_font_db(test_font_db()))
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
        let mut inputs = HostInputs::empty().with_font_db(test_font_db());
        inputs
            .insert_image(
                AssetId("ghost.png".into()),
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
        let id = AssetId("photos/a.png".into());
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
        let mut inputs = HostInputs::empty().with_font_db(test_font_db());
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
        let mut inputs = HostInputs::empty().with_font_db(test_font_db());
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
        let mut inputs = HostInputs::empty().with_font_db(test_font_db());
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
        assert_eq!(req.asset_id.0, "photos/a.png");
        assert!(matches!(
            &req.locator,
            ResourceLocator::LogicalPath(p) if p == "photos/a.png"
        ));
    }

    #[test]
    fn render_frame_media_plan_uses_request_asset_id_for_image() {
        let jsonl = r#"{"type":"composition","width":64,"height":64,"fps":30,"duration":0.1}
{"type":"div","id":"root","parentId":null,"className":"w-full h-full"}
{"type":"image","id":"pic","parentId":"root","path":"photos/a.png","className":"w-[32px] h-[32px]"}"#;
        let draft = CompositionDraft::parse(jsonl).unwrap();
        let id = draft.requirements().requests()[0].asset_id.clone();
        assert_eq!(id.0, "photos/a.png");
        let mut inputs = HostInputs::empty().with_font_db(test_font_db());
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
            ImageRef::Static { asset_id } => asset_id == &id.0,
            _ => false,
        });
        assert!(
            has,
            "media plan images={:?}, expected static {}",
            frame.media.images,
            id.0
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
            assert_eq!(reqs[0].asset_id.0, "video:path:clips/hero.mp4");
            assert!(matches!(
                &reqs[0].locator,
                ResourceLocator::LogicalPath(p) if p == "clips/hero.mp4"
            ));
            let id = reqs[0].asset_id.clone();
            let mut inputs = HostInputs::empty().with_font_db(test_font_db());
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
        assert_eq!(reqs[0].asset_id.0, "video:path:clips/hero.mp4");
        let id = reqs[0].asset_id.clone();
        let mut inputs = HostInputs::empty().with_font_db(test_font_db());
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
                    if asset_id == &id.0 && *time_micros == 12_000_000
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
            .prepare(HostInputs::empty().with_font_db(test_font_db()))
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
        let mut inputs = HostInputs::empty().with_font_db(test_font_db());
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
        assert_eq!(req.asset_id.0, "video:path:clips/a.mp4");
        assert!(matches!(
            &req.locator,
            ResourceLocator::LogicalPath(p) if p == "clips/a.mp4"
        ));
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
        assert_eq!(reqs[0].asset_id.0, "lottie:loader");
        assert!(matches!(
            &reqs[0].locator,
            ResourceLocator::LogicalPath(p) if p == "anim/loader.json"
        ));
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
            .prepare(HostInputs::empty().with_font_db(test_font_db()))
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
        use crate::resource::lottie::LottieMeta;
        let markup = r#"
            <opencat width="10" height="10" fps="30" duration="0.1">
              <div id="root">
                <lottie id="loader" path="anim/loader.json" />
              </div>
            </opencat>
        "#;
        let draft = CompositionDraft::parse(markup).unwrap();
        let id = draft.requirements().requests()[0].asset_id.clone();

        let mut inputs = HostInputs::empty().with_font_db(test_font_db());
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

        let mut inputs = HostInputs::empty().with_font_db(test_font_db());
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

        let mut inputs = HostInputs::empty().with_font_db(test_font_db());
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
        use crate::resource::lottie::LottieMeta;

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
            assert_eq!(reqs[0].asset_id.0, "lottie:loader");
            assert!(matches!(
                &reqs[0].locator,
                ResourceLocator::LogicalPath(p) if p == "anim/loader.json"
            ));

            let id = reqs[0].asset_id.clone();
            let mut inputs = HostInputs::empty().with_font_db(test_font_db());
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
                frame.media.lottie_bundles.iter().any(|b| b == &id.0),
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
                DrawOp::LottieRect { bundle_id, frame, .. } if bundle_id == &id.0 => Some(*frame),
                _ => None,
            });
            let lottie_frame = lottie_frame.unwrap_or_else(|| {
                panic!(
                    "draw ops must include LottieRect for {}; ops={:?}",
                    id.0, frame.draw.ops
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
    /// requirements emit opaque AssetId + kind + logical locator; prepare only
    /// consumes ImageMeta under that id; RenderFrame media plan echoes the same id.
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
            assert_eq!(reqs[0].asset_id.0, "assets/hero.png");
            assert!(matches!(
                &reqs[0].locator,
                ResourceLocator::LogicalPath(p) if p == "assets/hero.png"
            ));

            let id = reqs[0].asset_id.clone();
            let mut inputs = HostInputs::empty().with_font_db(test_font_db());
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
                    ImageRef::Static { asset_id } if asset_id == &id.0
                )),
                "media plan must echo request AssetId; got {:?}",
                frame.media.images
            );
        }
    }


    #[test]
    fn legacy_open_with_prepared_catalog_still_works() {
        // Expand-contract: old entry remains for existing callers/tests.
        let jsonl = r#"{"type":"composition","width":10,"height":10,"fps":30,"duration":0.1}
{"type":"div","id":"root","parentId":null}"#;
        let parsed = crate::parse::jsonl::parse(jsonl).unwrap();
        let pipeline = DefaultPipeline::open_with_prepared_catalog(
            parsed,
            PreparedResourceCatalog::default(),
            NoopJsContext::new().unwrap(),
            test_font_db(),
        )
        .expect("legacy open");
        assert_eq!(pipeline.info().width, 10);
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
            fonts[0].asset_id.0,
            "font:path:fonts/NotoSansSC-Regular.otf"
        );
        assert!(matches!(
            &fonts[0].locator,
            ResourceLocator::LogicalPath(p) if p == "fonts/NotoSansSC-Regular.otf"
        ));

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
            fonts[0].asset_id.0,
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
            .prepare(HostInputs::empty().with_font_db(test_font_db()))
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
        let mut inputs = HostInputs::empty().with_font_db(test_font_db());
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
        let mut inputs = HostInputs::empty().with_font_db(Arc::new(crate::text::empty_font_db()));
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
        let base = test_font_db();
        let face_bytes = include_bytes!("../../../../assets/NotoSansSC-Regular.otf").to_vec();
        let mut inputs = HostInputs::empty().with_font_db(base);
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
        let mut inputs = HostInputs::empty().with_font_db(Arc::new(crate::text::empty_font_db()));
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
        let mut inputs = HostInputs::empty().with_font_db(test_font_db());
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
        // Soft-miss: no insert_subtitle_text.
        let prepared = draft
            .prepare(HostInputs::empty().with_font_db(test_font_db()))
            .expect("missing SRT is not an error");
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
}
