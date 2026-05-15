//! wasm-bindgen bridge — cache query API for JS side.
//!
//! Compiles on both native and wasm32. wasm-bindgen attributes are
//! conditionally applied via `#[cfg_attr(target_arch = "wasm32", ...)]`.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use std::any::Any;

use opencat_core::display::list::DisplayRect;
use opencat_core::platform::render_engine::{FrameView, FrameViewKind};
use opencat_core::resource::hash_map_catalog::HashMapResourceCatalog;
use opencat_core::runtime::cache::CachedSubtreeSnapshot;
use opencat_core::runtime::pipeline::render_frame;
use opencat_core::runtime::session::RenderSession;
use opencat_core::scene::composition::Composition;
use opencat_core::scene::script::precomputed_host::PrecomputedScriptHost;

use crate::backend::WebPicture;
use crate::platform::WebPlatform;

// ── Exported types ──

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct WebRenderer {
    session: RenderSession<WebPlatform>,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
#[derive(Clone)]
pub struct SubtreeCacheResult {
    pub found: bool,
    pub secondary_fingerprint: u64,
    pub recorded_bounds_x: f32,
    pub recorded_bounds_y: f32,
    pub recorded_bounds_w: f32,
    pub recorded_bounds_h: f32,
    pub consecutive_hits: u32,
    pub render_mode: String,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter_with_clone))]
pub struct BuildFrameResult {
    pub ops_json: String,
    pub frame_width: u32,
    pub frame_height: u32,
}

// ── Helpers ──

fn default_platform() -> WebPlatform {
    use crate::codec::audio::WebAudio;
    use crate::engine::WebRenderEngine;
    use crate::video::WebVideoSource;
    use opencat_core::scene::path_bounds::DefaultPathBounds;
    use opencat_core::scene::script::precomputed_host::PrecomputedScriptHost;

    WebPlatform {
        backend: WebRenderEngine::new(),
        script: PrecomputedScriptHost::new(),
        video: WebVideoSource::default(),
        audio: WebAudio::new().expect("WebAudio initialization"),
        path_bounds: DefaultPathBounds,
    }
}

// ── WebRenderer impl ──

impl Default for WebRenderer {
    fn default() -> Self {
        Self {
            session: RenderSession::new(default_platform()),
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl WebRenderer {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn query_subtree_snapshot(&self, key: u64) -> SubtreeCacheResult {
        let cache = self.session.cache_registry.subtree_snapshot_cache();
        let mut cache_ref = cache.borrow_mut();
        if let Some(entry) = cache_ref.get_cloned(&key) {
            SubtreeCacheResult {
                found: true,
                secondary_fingerprint: entry.secondary_fingerprint,
                recorded_bounds_x: entry.recorded_bounds.x,
                recorded_bounds_y: entry.recorded_bounds.y,
                recorded_bounds_w: entry.recorded_bounds.width,
                recorded_bounds_h: entry.recorded_bounds.height,
                consecutive_hits: entry.consecutive_hits as u32,
                render_mode: "draw_picture".to_string(),
            }
        } else {
            SubtreeCacheResult {
                found: false,
                secondary_fingerprint: 0,
                recorded_bounds_x: 0.0,
                recorded_bounds_y: 0.0,
                recorded_bounds_w: 0.0,
                recorded_bounds_h: 0.0,
                consecutive_hits: 0,
                render_mode: String::new(),
            }
        }
    }

    pub fn report_subtree_snapshot_hit(&mut self, key: u64) {
        let cache = self.session.cache_registry.subtree_snapshot_cache();
        let mut cache_ref = cache.borrow_mut();
        if let Some(mut entry) = cache_ref.get_cloned(&key) {
            entry.consecutive_hits += 1;
            cache_ref.insert(key, entry);
        }
    }

    pub fn store_subtree_snapshot(
        &mut self,
        key: u64,
        secondary: u64,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) {
        let entry = CachedSubtreeSnapshot {
            picture: WebPicture { fingerprint: key },
            secondary_fingerprint: secondary,
            consecutive_hits: 1,
            recorded_bounds: DisplayRect {
                x,
                y,
                width: w,
                height: h,
            },
        };
        self.session
            .cache_registry
            .subtree_snapshot_cache()
            .borrow_mut()
            .insert(key, entry);
    }

    /// Returns glyph path data as JSON string, or None if not cached.
    /// JS side parses the JSON to reconstruct path commands.
    pub fn query_glyph_path(&self, key: u64) -> Option<String> {
        use crate::backend::GlyphPathData;
        let data: GlyphPathData = self
            .session
            .cache_registry
            .glyph_path_cache()
            .borrow_mut()
            .get_cloned(&key)?;
        serde_json::to_string(&data).ok()
    }

    pub fn query_glyph_image(&self, key: u64) -> Option<Vec<u8>> {
        self.session
            .cache_registry
            .glyph_image_cache()
            .borrow_mut()
            .get_cloned(&key)
            .map(|arc| (*arc).clone())
    }

    pub fn query_image(&self, url: String) -> Option<Vec<u8>> {
        self.session
            .cache_registry
            .image_cache()
            .borrow_mut()
            .get_cloned(&url)
            .flatten()
            .map(|arc| (*arc).clone())
    }

    /// Inject a pre-decoded video frame from JS.
    /// JS side calls this after `prepareVideoFrames()` completes.
    pub fn inject_video_frame(
        &mut self,
        asset_id: String,
        frame: u32,
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    ) {
        use opencat_core::resource::asset_id::AssetId;
        self.session
            .platform
            .video
            .inject_frame(AssetId(asset_id), frame, rgba, width, height);
    }

    /// Clear cached video frames for a specific asset (or all if empty string).
    pub fn clear_video_cache(&mut self, asset_id: String) {
        use opencat_core::resource::asset_id::AssetId;
        if asset_id.is_empty() {
            self.session.platform.video.clear_cache(None);
        } else {
            self.session.platform.video.clear_cache(Some(&AssetId(asset_id)));
        }
    }

    // ── Audio API ──

    /// Decode an audio file (bytes) via Web Audio API.
    /// `asset_id` is used as the cache key for subsequent play/get_samples calls.
    pub async fn decode_audio_file(
        &mut self,
        asset_id: String,
        data: Vec<u8>,
    ) -> Result<(), JsValue> {
        self.session
            .platform
            .audio
            .decode_file(&asset_id, &data)
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get decoded PCM audio samples for a time range (for export).
    /// Returns JSON: `{ sample_rate, channels, samples: [f32...] }`
    pub fn get_audio_samples(
        &self,
        asset_id: String,
        start_secs: f64,
        duration_secs: f64,
        target_rate: u32,
    ) -> String {
        use crate::codec::audio::WebAudio;

        let pcm = self.session.platform.audio.get_pcm(&asset_id);
        match pcm {
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

    /// Play audio from `offset_secs` for `duration_secs` (for preview).
    pub fn play_audio_at(
        &mut self,
        asset_id: String,
        offset_secs: f64,
        duration_secs: f64,
    ) -> Result<(), JsValue> {
        self.session
            .platform
            .audio
            .play_at(&asset_id, offset_secs, duration_secs)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Set master audio volume (0.0 ~ 1.0).
    pub fn set_audio_volume(&self, volume: f32) {
        self.session.platform.audio.set_volume(volume);
    }

    /// Clear decoded audio cache.
    pub fn clear_audio_cache(&mut self) {
        self.session.platform.audio = crate::codec::audio::WebAudio::new()
            .expect("WebAudio re-initialization");
    }

    /// Stop all audio playback.
    pub fn stop_audio(&mut self) -> Result<(), JsValue> {
        self.session
            .platform
            .audio
            .stop_all()
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get AudioContext.currentTime for audio-driven frame sync.
    pub fn audio_context_time(&self) -> f64 {
        self.session.platform.audio.current_time()
    }

    pub fn build_frame(
        &mut self,
        jsonl: String,
        frame: u32,
        resources: String,
        mutations: String,
    ) -> BuildFrameResult {
        match self.build_frame_inner(&jsonl, frame, &resources, &mutations) {
            Ok(result) => result,
            Err(e) => BuildFrameResult {
                ops_json: serde_json::json!({"error": e.to_string()}).to_string(),
                frame_width: 0,
                frame_height: 0,
            },
        }
    }
}

impl WebRenderer {
    fn build_frame_inner(
        &mut self,
        jsonl: &str,
        frame: u32,
        resources: &str,
        mutations: &str,
    ) -> anyhow::Result<BuildFrameResult> {
        // 1. Parse JSONL
        let parsed = opencat_core::jsonl::parse(jsonl)?;

        // 2. Update session catalog from JS-provided resource metadata
        self.session.catalog = HashMapResourceCatalog::from_json(resources)?;

        // 3. Update session platform script from JS-provided mutations
        self.session.platform.script = PrecomputedScriptHost::from_json(mutations)?;

        // 4. Build a Composition from the parsed JSONL
        let root_node = parsed.root;
        let width = parsed.width;
        let height = parsed.height;
        let composition = Composition::new("web")
            .size(width, height)
            .fps(parsed.fps as u32)
            .frames(parsed.frames as u32)
            .audio_sources(parsed.audio_sources)
            .root(move |_ctx| root_node.clone())
            .build()?;

        // 5. Render frame through core pipeline
        let mut frame_view_data: Box<dyn Any> = Box::new(());
        let frame_view = FrameView {
            width: width as u32,
            height: height as u32,
            kind: FrameViewKind::Opaque(&mut *frame_view_data),
        };
        let mut platform_data: Box<dyn Any> = Box::new(());

        render_frame(
            &composition,
            frame,
            &mut self.session,
            frame_view,
            &mut *platform_data,
        )?;

        // 6. Serialize ordered scene program to JSON
        let ops_json = serde_json::to_string(&self.session.last_ordered_scene)?;

        Ok(BuildFrameResult {
            ops_json,
            frame_width: width as u32,
            frame_height: height as u32,
        })
    }
}
