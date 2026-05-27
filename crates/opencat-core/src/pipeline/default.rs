use std::sync::Arc;

use anyhow::Result;

use crate::analyze::annotation::{AnalyzeFingerprintHistory, AnnotatedNodeHandle};
use crate::analyze::compositor::{OrderedSceneOp, OrderedSceneProgram};
use crate::analyze::invalidation::CompositeHistory;
use crate::ir::asset_id::{
    asset_id_for_audio_url, asset_id_for_query, asset_id_for_url, asset_id_for_video_url,
};
use crate::ir::cache::RenderCache;
use crate::ir::{CompositionInfo, DrawOpFrame, FrameMediaPlan};
use crate::layout::LayoutSession;
use crate::parse::composition::Composition;
use crate::parse::preflight::collect_resource_requests;
use crate::parse::primitives::{AudioSource, ImageSource, SubtitleSource, VideoSource};
use crate::probe::catalog::ResourceCatalog;
use crate::probe::probe::{probe_image, probe_video};
use crate::probe::{AssetHandle, AssetId, AssetLoader};
use crate::script::js_context::JsContext;
use crate::text::default_font_db;

use super::Pipeline;

const DEFAULT_SUBTREE_SNAPSHOT_CAP: usize = 256;
const DEFAULT_SEGMENT_CAP: usize = 256;
const DEFAULT_ITEM_RANGE_CAP: usize = 128;

pub struct DefaultPipeline<L: AssetLoader, S: JsContext> {
    composition: Composition,
    info: CompositionInfo,
    catalog: ResourceCatalog,
    loader: L,
    scripts: crate::script::LiveScriptHost<S>,
    layout_session: LayoutSession,
    composite_history: CompositeHistory,
    analyze_fingerprint_history: AnalyzeFingerprintHistory,
    font_db: Arc<fontdb::Database>,
    cache: RenderCache,
    last_ordered_scene: OrderedSceneProgram,
}

impl<L: AssetLoader, S: JsContext> DefaultPipeline<L, S> {
    pub fn open(input: &str, mut loader: L, scripts: S) -> Result<Self> {
        let trimmed = input.trim();
        let parsed = if trimmed.starts_with('{') {
            crate::parse::jsonl::parse(input)?
        } else {
            crate::parse::markup::parse(input)?
        };

        let root_node = parsed.root;
        let composition = Composition::new("pipeline")
            .size(parsed.width, parsed.height)
            .fps(parsed.fps as u32)
            .frames(parsed.frames as u32)
            .root(move |_ctx| root_node.clone())
            .audio_sources(parsed.audio_sources)
            .build()?;

        let requests = collect_resource_requests(&composition);
        loader.load_all(&requests)?;

        let mut catalog = ResourceCatalog::default();
        probe_all(&loader, &requests, composition.fps, &mut catalog);

        let audio_plan = crate::parse::preflight::collect_audio_plan(&composition);

        let info = CompositionInfo {
            width: composition.width as u32,
            height: composition.height as u32,
            fps: composition.fps,
            frames: composition.frames,
            requests,
            audio_plan,
        };

        let live_host = crate::script::LiveScriptHost::new(scripts)?;

        Ok(Self {
            composition,
            info,
            catalog,
            loader,
            scripts: live_host,
            layout_session: LayoutSession::new(),
            composite_history: CompositeHistory::default(),
            analyze_fingerprint_history: AnalyzeFingerprintHistory::default(),
            font_db: Arc::new(default_font_db(&[])),
            cache: RenderCache::new(
                DEFAULT_SUBTREE_SNAPSHOT_CAP,
                DEFAULT_SEGMENT_CAP,
                DEFAULT_ITEM_RANGE_CAP,
            ),
            last_ordered_scene: OrderedSceneProgram {
                root: OrderedSceneOp::LiveSubtree {
                    handle: AnnotatedNodeHandle(0),
                    children: Vec::new(),
                },
            },
        })
    }

    pub fn composition(&self) -> &Composition {
        &self.composition
    }

    pub fn catalog(&self) -> &ResourceCatalog {
        &self.catalog
    }

    pub fn scripts(&self) -> &crate::script::LiveScriptHost<S> {
        &self.scripts
    }
}

fn source_to_image_id(src: &ImageSource) -> Option<AssetId> {
    match src {
        ImageSource::Unset => None,
        ImageSource::Path(p) => Some(AssetId(p.to_string_lossy().into_owned())),
        ImageSource::Url(u) => Some(asset_id_for_url(u)),
        ImageSource::Query(q) => Some(asset_id_for_query(q)),
    }
}

fn source_to_video_id(src: &VideoSource) -> AssetId {
    match src {
        VideoSource::Path(p) => AssetId(format!("video:path:{}", p.to_string_lossy())),
        VideoSource::Url(u) => asset_id_for_video_url(u),
    }
}

fn source_to_audio_id(src: &AudioSource) -> Option<AssetId> {
    match src {
        AudioSource::Unset => None,
        AudioSource::Path(p) => Some(AssetId(format!("audio:path:{}", p.to_string_lossy()))),
        AudioSource::Url(u) => Some(asset_id_for_audio_url(u)),
    }
}

fn source_to_subtitle_id(src: &SubtitleSource) -> AssetId {
    match src {
        SubtitleSource::Path(p) => AssetId(format!("subtitle:path:{}", p.to_string_lossy())),
        SubtitleSource::Url(u) => AssetId(format!("subtitle:url:{u}")),
    }
}

fn probe_all<L: AssetLoader>(
    loader: &L,
    requests: &crate::probe::catalog::ResourceRequests,
    fps: u32,
    catalog: &mut ResourceCatalog,
) where
    <L as AssetLoader>::Handle: AssetHandle,
{
    for src in &requests.images {
        if let Some(id) = source_to_image_id(src) {
            if let Some(handle) = loader.handle(&id) {
                if let Ok(bytes) = handle.read_bytes() {
                    if let Ok(meta) = probe_image(&bytes) {
                        catalog.images.insert(id, meta);
                    }
                }
            }
        }
    }

    for src in &requests.videos {
        let id = source_to_video_id(src);
        if let Some(handle) = loader.handle(&id) {
            if let Ok(bytes) = handle.read_bytes() {
                if let Ok(meta) = probe_video(&bytes) {
                    catalog.videos.insert(id, meta);
                }
            }
        }
    }

    for src in &requests.audios {
        if let Some(id) = source_to_audio_id(src) {
            catalog.audios.insert(id);
        }
    }

    for src in &requests.subtitles {
        let id = source_to_subtitle_id(src);
        if let Some(handle) = loader.handle(&id) {
            if let Ok(bytes) = handle.read_bytes() {
                if let Ok(entries) = crate::probe::probe::parse_srt_bytes(&bytes, fps) {
                    catalog.subtitles.insert(id, entries);
                }
            }
        }
    }
}

impl<L: AssetLoader, S: JsContext> Pipeline for DefaultPipeline<L, S> {
    type Loader = L;
    type Scripts = S;

    fn info(&self) -> &CompositionInfo {
        &self.info
    }

    fn render_frame(&mut self, frame_index: u32) -> Result<(DrawOpFrame, FrameMediaPlan)> {
        super::frame::render_frame_with_state(
            &self.composition,
            frame_index,
            &mut self.layout_session,
            &mut self.composite_history,
            &mut self.analyze_fingerprint_history,
            &self.font_db,
            &mut self.catalog,
            &mut self.cache,
            &mut self.last_ordered_scene,
            &mut self.scripts,
            None,
        )
    }

    fn loader(&self) -> &Self::Loader {
        &self.loader
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::collections::HashMap;
    use std::sync::Arc;

    use super::*;
    use crate::probe::{AssetHandle, AssetLoader as AssetLoaderTrait};
    use crate::script::js_context::JsContext;
    use crate::script::recorder::MutationStore;

    #[derive(Clone)]
    struct ByteHandle(Arc<Vec<u8>>);
    impl AssetHandle for ByteHandle {
        fn read_bytes(&self) -> Result<Cow<'_, [u8]>> {
            Ok(Cow::Borrowed(&self.0))
        }
    }

    #[derive(Default)]
    struct InMemoryLoader {
        map: HashMap<AssetId, ByteHandle>,
    }
    impl AssetLoaderTrait for InMemoryLoader {
        type Handle = ByteHandle;
        fn load_all(&mut self, _: &crate::probe::catalog::ResourceRequests) -> Result<()> {
            Ok(())
        }
        fn handle(&self, id: &AssetId) -> Option<&Self::Handle> {
            self.map.get(id)
        }
    }

    struct NoopJsContext {
        store: std::cell::RefCell<MutationStore>,
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

    #[test]
    fn open_empty_composition_returns_info() {
        let jsonl = r#"{"type":"composition","width":100,"height":200,"fps":30,"frames":1}
{"type":"div","id":"root","parentId":null}"#;

        let loader = InMemoryLoader::default();
        let ctx = NoopJsContext::new().expect("js context");

        let pipeline = DefaultPipeline::open(jsonl, loader, ctx).expect("open");

        assert_eq!(pipeline.info().width, 100);
        assert_eq!(pipeline.info().height, 200);
        assert_eq!(pipeline.info().fps, 30);
        assert_eq!(pipeline.info().frames, 1);
    }

    #[test]
    fn render_frame_produces_draw_ops() {
        let jsonl = r##"{"type":"composition","width":320,"height":240,"fps":30,"frames":3}
{"type":"div","id":"root","parentId":null}
{"type":"div","id":"child","parentId":"root","bg":"#ff0000","w":100,"h":50}"##;

        let loader = InMemoryLoader::default();
        let ctx = NoopJsContext::new().expect("js context");

        let mut pipeline = DefaultPipeline::open(jsonl, loader, ctx).expect("open");

        let (frame, media_plan) = pipeline.render_frame(0).expect("render frame 0");

        assert!(
            !frame.ops.is_empty(),
            "render_frame should produce at least one DrawOp"
        );
        let _ = media_plan;
    }

    #[test]
    fn render_frame_multi_frame_is_deterministic() {
        let jsonl = r##"{"type":"composition","width":100,"height":100,"fps":10,"frames":5}
{"type":"div","id":"root","parentId":null,"bg":"#00ff00","w":100,"h":100}"##;

        let loader = InMemoryLoader::default();
        let ctx1 = NoopJsContext::new().expect("js context 1");
        let ctx2 = NoopJsContext::new().expect("js context 2");

        let mut p1 = DefaultPipeline::open(jsonl, InMemoryLoader::default(), ctx1).expect("open 1");
        let mut p2 = DefaultPipeline::open(jsonl, InMemoryLoader::default(), ctx2).expect("open 2");

        for i in 0..5 {
            let (f1, _) = p1.render_frame(i).expect("render p1");
            let (f2, _) = p2.render_frame(i).expect("render p2");
            assert_eq!(f1.ops.len(), f2.ops.len(), "frame {i} op count mismatch");
        }
    }

    #[cfg(feature = "profile")]
    #[test]
    fn render_frame_emits_profile_events_for_each_frame() {
        let jsonl = r##"{"type":"composition","width":100,"height":100,"fps":10,"frames":2}
{"type":"div","id":"root","parentId":null,"bg":"#00ff00","w":100,"h":100}"##;

        let config = crate::profile::ProfileConfig { enabled: true };
        let (_, summary) = crate::profile::profile_render(&config, || {
            let mut pipeline = DefaultPipeline::open(
                jsonl,
                InMemoryLoader::default(),
                NoopJsContext::new().expect("js context"),
            )
            .expect("open");

            for frame_index in 0..2 {
                let _ = pipeline.render_frame(frame_index)?;
            }
            Ok::<_, anyhow::Error>(())
        })
        .expect("profile render");

        let summary = summary.expect("summary should exist");
        assert!(
            summary.frames.contains_key(&0),
            "frame 0 profile should be present, got {:?}",
            summary.frames.keys().collect::<Vec<_>>()
        );
        assert!(
            summary.frames.contains_key(&1),
            "frame 1 profile should be present, got {:?}",
            summary.frames.keys().collect::<Vec<_>>()
        );
        assert_eq!(summary.frames[&0].structure_rebuilds, 1);
        assert_eq!(summary.frames[&1].structure_rebuilds, 0);
        assert!(
            summary.frames[&1].reused_nodes > 0,
            "second frame should record layout reuse stats"
        );
    }

    #[cfg(feature = "profile")]
    #[test]
    fn profile_showcase_jsonl_records_split_merkle_profile() {
        let jsonl = include_str!("../../../../json/profile-showcase.jsonl");

        let config = crate::profile::ProfileConfig { enabled: true };
        let (_, summary) = crate::profile::profile_render(&config, || {
            let mut pipeline = DefaultPipeline::open(
                jsonl,
                InMemoryLoader::default(),
                NoopJsContext::new().expect("js context"),
            )
            .expect("open profile showcase jsonl");

            for frame_index in 0..pipeline.info().frames {
                let _ = pipeline.render_frame(frame_index)?;
            }
            Ok::<_, anyhow::Error>(())
        })
        .expect("profile render");

        let summary = summary.expect("summary should exist");
        let full_hit_nodes = summary
            .frames
            .values()
            .map(|frame| frame.input_merkle_full_hit_nodes)
            .sum::<usize>();
        let layout_skipped_nodes = summary
            .frames
            .values()
            .map(|frame| frame.layout_merkle_skipped_nodes)
            .sum::<usize>();
        let analyze_skipped_nodes = summary
            .frames
            .values()
            .map(|frame| frame.analyze_merkle_skipped_nodes)
            .sum::<usize>();

        assert!(
            full_hit_nodes > 0,
            "profile-showcase jsonl should exercise full input Merkle hits"
        );
        assert!(
            layout_skipped_nodes >= full_hit_nodes,
            "layout Merkle skip should include full hits and layout-only clean subtrees"
        );
        assert!(
            analyze_skipped_nodes > 0,
            "profile-showcase jsonl should exercise analyze Merkle fingerprint skips"
        );
    }

    #[test]
    fn open_pipeline_populates_audio_plan() {
        let jsonl = r##"{"type":"composition","width":100,"height":100,"fps":30,"frames":1}
{"type":"div","id":"root","parentId":null}"##;
        let pipeline = DefaultPipeline::<InMemoryLoader, NoopJsContext>::open(
            jsonl,
            InMemoryLoader::default(),
            NoopJsContext::new().unwrap(),
        )
        .expect("open");
        assert!(
            pipeline.info().audio_plan.segments.is_empty(),
            "no audio sources => empty plan"
        );
    }

    #[test]
    fn open_from_xml() {
        let xml = r#"<opencat width="200" height="100" fps="30" frames="1">
  <div id="root" />
</opencat>"#;
        let loader = InMemoryLoader::default();
        let ctx = NoopJsContext::new().expect("js context");
        let pipeline = DefaultPipeline::open(xml, loader, ctx).expect("open xml");
        assert_eq!(pipeline.info().width, 200);
        assert_eq!(pipeline.info().height, 100);
        assert_eq!(pipeline.info().fps, 30);
        assert_eq!(pipeline.info().frames, 1);
    }
}
