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
use crate::script::js_context::JsContext;

impl CompositionDraft {
    /// Build a draft from an already-parsed composition.
    pub fn from_parsed(parsed: ParsedComposition) -> Self {
        let requests = collect_resource_requests_from_parsed(&parsed);
        let requirements = HostRequirements::from_requests(&requests);
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

    fn from_requests(raw: &ResourceRequests) -> Self {
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

    /// Fill this inputs bag from a host-probed [`PreparedResourceCatalog`] and
    /// optional subtitle texts, using **only** the asset ids listed in
    /// `requirements` (never re-derived from locators). Soft-misses for subtitle
    /// text are allowed; missing image/video/lottie metadata returns
    /// [`PrepareError::MissingInput`].
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
                    let Some(meta) = probed.lotties.get(&req.asset_id).copied() else {
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

/// Pure prepare: validate host inputs against draft requirements, hydrate
/// captions, and produce a prepared composition ready for pipeline open.
pub fn prepare(
    draft: CompositionDraft,
    inputs: HostInputs,
) -> Result<PreparedComposition, PrepareError> {
    let CompositionDraft {
        mut parsed,
        requirements,
    } = draft;

    validate_inputs(&requirements, &inputs)?;

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
            ResourceKind::Image | ResourceKind::Video | ResourceKind::Lottie => {}
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
        font_db: inputs.font_db,
    })
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
    for id in inputs.supplied.iter().chain(inputs.catalog.images.keys()).chain(
        inputs
            .catalog
            .videos
            .keys()
            .chain(inputs.catalog.audios.iter())
            .chain(inputs.catalog.subtitles.keys())
            .chain(inputs.catalog.lotties.keys()),
    ) {
        if !required.contains_key(id) {
            return Err(PrepareError::UndeclaredInput {
                asset_id: id.clone(),
            });
        }
    }

    // Missing + layout-critical validation. Subtitle text is soft: missing SRT
    // leaves empty caption entries (same contract as hydrate_captions / #12).
    for req in requirements.requests() {
        match req.kind {
            ResourceKind::Image => {
                let Some(meta) = inputs.catalog.images.get(&req.asset_id) else {
                    // Kind mismatch: present only under another map.
                    if inputs.catalog.videos.contains_key(&req.asset_id)
                        || inputs.catalog.audios.contains(&req.asset_id)
                        || inputs.catalog.lotties.contains_key(&req.asset_id)
                        || inputs.catalog.subtitles.contains_key(&req.asset_id)
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
                if !inputs.catalog.videos.contains_key(&req.asset_id) {
                    return Err(PrepareError::MissingInput {
                        asset_id: req.asset_id.clone(),
                        kind: req.kind,
                    });
                }
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
                if !inputs.catalog.lotties.contains_key(&req.asset_id) {
                    return Err(PrepareError::MissingInput {
                        asset_id: req.asset_id.clone(),
                        kind: req.kind,
                    });
                }
            }
            ResourceKind::Subtitle => {}
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
            VideoSource::Path(p) => Self::LogicalPath(p.to_string_lossy().into_owned()),
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
            LottieSource::Path(p) => Self::LogicalPath(p.to_string_lossy().into_owned()),
            LottieSource::Url(u) => Self::Url(u.clone()),
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
                    duration_ms: None,
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
}
