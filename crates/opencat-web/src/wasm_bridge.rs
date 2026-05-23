//! wasm-bindgen 桥：JS 端调用 `WebRenderer.build_frame(jsonl, frame, ck_canvas, resources_json)`.

use wasm_bindgen::prelude::*;

use opencat_core::parse::composition::Composition;
use opencat_core::resource::asset_id::AssetId;
use opencat_core::resource::hash_map_catalog::HashMapResourceCatalog;
use opencat_core::runtime::pipeline::render_frame;
use opencat_core::runtime::session::RenderSession;

use crate::canvaskit::CanvasKitCanvas2D;
use crate::canvaskit::bindings::CKCanvas;
use crate::codec::audio::WebAudio;
use crate::platform::WebPlatform;

#[wasm_bindgen]
pub struct WebRenderer {
    session: RenderSession<WebPlatform, CanvasKitCanvas2D>,
    audio: WebAudio,
    blobs: crate::resource::blob_store::BlobStore,
}

#[wasm_bindgen]
impl WebRenderer {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WebRenderer, JsValue> {
        #[cfg(feature = "profile")]
        tracing_wasm::set_as_global_default();
        let audio = WebAudio::new().map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Self {
            session: RenderSession::new(WebPlatform::new()),
            audio,
            blobs: crate::resource::blob_store::BlobStore::new(),
        })
    }

    pub fn build_frame(
        &mut self,
        jsonl: &str,
        frame: u32,
        ck_canvas: JsValue,
        resources_json: &str,
    ) -> Result<(), JsValue> {
        #[cfg(feature = "profile")]
        tracing::info!(frame, "build_frame start");
        let parsed = opencat_core::parse::jsonl::parse(jsonl)
            .map_err(|e| JsValue::from_str(&format!("parse: {e}")))?;
        let root_node = parsed.root.clone();
        let composition = Composition::new("web")
            .size(parsed.width, parsed.height)
            .fps(parsed.fps as u32)
            .frames(parsed.frames as u32)
            .audio_sources(parsed.audio_sources)
            .root(move |_ctx| root_node.clone())
            .build()
            .map_err(|e| JsValue::from_str(&format!("composition: {e}")))?;

        self.session.catalog = HashMapResourceCatalog::from_json(resources_json)
            .map_err(|e| JsValue::from_str(&format!("catalog: {e}")))?;

        let canvas: CKCanvas = ck_canvas.unchecked_into();
        let mut canvas2d = CanvasKitCanvas2D::new(canvas);

        let blob_store_ref: &dyn opencat_core::resource::BlobStore = &self.blobs;
        render_frame::<WebPlatform, CanvasKitCanvas2D>(
            &composition,
            frame,
            &mut self.session,
            &mut canvas2d,
            Some(blob_store_ref),
        )
        .map_err(|e| JsValue::from_str(&format!("render_frame: {e}")))?;

        Ok(())
    }

    // -- Video frame injection --

    pub fn inject_video_frame(
        &mut self,
        asset_id: String,
        frame: u32,
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    ) {
        self.session
            .platform
            .video
            .inject_frame(AssetId(asset_id), frame, rgba, width, height);
    }

    pub fn clear_video_cache(&mut self, asset_id: String) {
        if asset_id.is_empty() {
            self.session.platform.video.clear_cache(None);
        } else {
            self.session
                .platform
                .video
                .clear_cache(Some(&AssetId(asset_id)));
        }
    }

    /// Enumerate every video asset that will be drawn at `frame`, with the
    /// local media time at which it should be decoded. JS uses this list to
    /// decode and inject RGBA bytes before calling `build_frame`.
    ///
    /// Returned JSON shape: `[{"assetId": "...", "localTimeSecs": 1.5}]`.
    ///
    /// Time math mirrors `MediaContext::frame_rgba` + `VideoFrameRequest::resolve_time_secs`:
    /// `local_time = media_offset + composition_time * playback_rate`, with
    /// optional looping wrap (not applied here because the JS layer doesn't
    /// know the video duration; the worker re-applies it on decode).
    pub fn plan_video_frames(
        &self,
        jsonl: &str,
        frame: u32,
        resources_json: &str,
    ) -> Result<String, JsValue> {
        use opencat_core::frame_ctx::FrameCtx;
        use opencat_core::parse::node::NodeKind;
        use opencat_core::parse::primitives::VideoSource;
        use opencat_core::parse::time::{FrameState, frame_state_for_root};
        use opencat_core::resource::catalog::{ResourceCatalog, VideoInfoMeta};
        use opencat_core::resource::types::{VideoFrameRequest, VideoPreviewQuality};

        let parsed = opencat_core::parse::jsonl::parse(jsonl)
            .map_err(|e| JsValue::from_str(&format!("plan_video_frames parse: {e}")))?;
        let catalog = HashMapResourceCatalog::from_json(resources_json)
            .map_err(|e| JsValue::from_str(&format!("plan_video_frames catalog: {e}")))?;
        let frame_ctx = FrameCtx {
            frame,
            fps: parsed.fps as u32,
            width: parsed.width,
            height: parsed.height,
            frames: parsed.frames as u32,
        };

        let composition_time_secs = frame as f64 / (parsed.fps as f64).max(1.0);

        let mut plan: Vec<serde_json::Value> = Vec::new();

        fn walk(
            node: &opencat_core::parse::node::Node,
            ctx: &FrameCtx,
            composition_time_secs: f64,
            catalog: &HashMapResourceCatalog,
            out: &mut Vec<serde_json::Value>,
        ) {
            match node.kind() {
                NodeKind::Component(c) => {
                    walk(&c.render(ctx), ctx, composition_time_secs, catalog, out)
                }
                NodeKind::Div(div) => {
                    for child in div.children_ref() {
                        walk(child, ctx, composition_time_secs, catalog, out);
                    }
                }
                NodeKind::Video(video) => {
                    let timing = video.timing();
                    let asset_id = match video.source() {
                        VideoSource::Path(p) => AssetId(p.to_string_lossy().into_owned()),
                        VideoSource::Url(u) => {
                            opencat_core::resource::asset_id::asset_id_for_video_url(u)
                        }
                    };
                    let info = catalog.video_info(&asset_id).unwrap_or(VideoInfoMeta {
                        width: 0,
                        height: 0,
                        duration_secs: None,
                    });
                    let request = VideoFrameRequest {
                        composition_time_secs,
                        timing,
                        quality: VideoPreviewQuality::Exact,
                        target_size: None,
                    };
                    let local_time_secs = request.resolve_time_secs(&info);
                    out.push(serde_json::json!({
                        "assetId": asset_id.0,
                        "localTimeSecs": local_time_secs,
                    }));
                }
                NodeKind::Timeline(_) => {
                    let state = frame_state_for_root(node, ctx);
                    walk_state(&state, ctx, composition_time_secs, catalog, out);
                }
                _ => {}
            }
        }

        fn walk_state(
            state: &FrameState,
            ctx: &FrameCtx,
            composition_time_secs: f64,
            catalog: &HashMapResourceCatalog,
            out: &mut Vec<serde_json::Value>,
        ) {
            match state {
                FrameState::Scene { scene, .. } => {
                    walk(scene, ctx, composition_time_secs, catalog, out)
                }
                FrameState::Transition { from, to, .. } => {
                    walk(from, ctx, composition_time_secs, catalog, out);
                    walk(to, ctx, composition_time_secs, catalog, out);
                }
            }
        }

        let state = frame_state_for_root(&parsed.root, &frame_ctx);
        walk_state(
            &state,
            &frame_ctx,
            composition_time_secs,
            &catalog,
            &mut plan,
        );

        serde_json::to_string(&plan)
            .map_err(|e| JsValue::from_str(&format!("plan_video_frames json: {e}")))
    }

    // -- Audio API --

    pub async fn decode_audio_file(
        &mut self,
        asset_id: String,
        data: Vec<u8>,
    ) -> Result<(), JsValue> {
        self.audio
            .decode_file(&asset_id, &data)
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn get_audio_samples(
        &self,
        asset_id: String,
        start_secs: f64,
        duration_secs: f64,
        target_rate: u32,
    ) -> String {
        match self.audio.get_pcm(&asset_id) {
            Some(pcm) => {
                let samples =
                    WebAudio::extract_samples(pcm, start_secs, duration_secs, target_rate);
                serde_json::json!({
                    "sample_rate": pcm.sample_rate,
                    "channels": pcm.channels,
                    "samples": samples,
                })
                .to_string()
            }
            None => serde_json::json!({
                "sample_rate": 0,
                "channels": 0,
                "samples": [],
            })
            .to_string(),
        }
    }

    pub fn play_audio_at(
        &mut self,
        asset_id: String,
        offset_secs: f64,
        duration_secs: f64,
    ) -> Result<(), JsValue> {
        self.audio
            .play_at(&asset_id, offset_secs, duration_secs)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn set_audio_volume(&self, volume: f32) {
        self.audio.set_volume(volume);
    }

    pub fn clear_audio_cache(&mut self) -> Result<(), JsValue> {
        self.audio = WebAudio::new().map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(())
    }

    pub fn stop_audio(&mut self) -> Result<(), JsValue> {
        self.audio
            .stop_all()
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn audio_context_time(&self) -> f64 {
        self.audio.current_time()
    }

    // -- Image blob API --

    /// Inject image bytes from JS. Call before build_frame.
    /// asset_id must match catalog entry. Repeated injects overwrite.
    pub fn inject_image_bytes(&mut self, asset_id: String, bytes: Vec<u8>) {
        self.blobs
            .insert(AssetId(asset_id), std::sync::Arc::from(bytes));
    }

    /// Clear all injected image blobs (for switching compositions).
    pub fn clear_image_blobs(&mut self) {
        self.blobs.clear();
    }
}
