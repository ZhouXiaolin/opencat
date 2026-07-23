//! wasm-bindgen bridge: build each frame as one binary DrawOp IR blob.
//!
//! Host-owned persistent pipeline (issue #8 / #11). The renderer holds a single
//! `DefaultPipeline<WebJsContext>` opened once via
//! [`WebRenderer::open_design`]; each [`WebRenderer::build_frame_ir`] call
//! just runs `pipeline.render_frame(frame)` and encodes the draw ops. Core
//! never fetches — web fetches all declared assets during `open_design`,
//! feeds host-probed metadata into core's explicit lifecycle
//! (`CompositionDraft` → `HostInputs` → `prepare` → `open_pipeline`). This
//! mirrors the engine's `open_parsed_host_owned` path (#7 / #15).

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

use opencat_core::ir::GeneratedImageId;
use opencat_core::ir::CompositionInfo;
use opencat_core::ir::RenderFrame;
use opencat_core::ir::asset_id::{asset_id_for_subtitle, AssetId};
use opencat_core::ir::draw_frame::DrawFrameScratch;
use opencat_core::ir::draw_types::ImageRef;
use opencat_core::ir::media_plan::{FrameGeneratedImage, FrameMediaPlan};
use opencat_core::lifecycle::{CompositionDraft, HostInputs, PrepareError, ResourceKind};
use opencat_core::pipeline::Pipeline;
use opencat_core::pipeline::default::DefaultPipeline;
use opencat_core::probe::catalog::{PreparedResourceCatalog, ResourceRequests};
use opencat_core::resource::fonts::font_asset_id;
use opencat_core::script::js_context::JsContext;

use crate::js_context::WebJsContext;
use crate::media::WebAudio;

#[wasm_bindgen]
pub struct WebRenderer {
    /// The persistent core pipeline. `None` until [`open_design`] is called;
    /// replaced (with epoch reset) each time a new design is opened.
    pipeline: Option<DefaultPipeline<WebJsContext>>,
    /// Cached composition metadata from the opened pipeline.
    info: Option<CompositionInfo>,
    pending_frame: Option<(u32, RenderFrame)>,
    scratch: DrawFrameScratch,
    audio: WebAudio,
    default_sans_sc: Option<Vec<u8>>,
    default_color_emoji: Option<Vec<u8>>,
    extra_fonts: Vec<Vec<u8>>,
}

#[wasm_bindgen]
impl WebRenderer {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WebRenderer, JsValue> {
        #[cfg(feature = "profile")]
        tracing_wasm::set_as_global_default();
        // Surface Rust panic messages in the browser console.
        console_error_panic_hook::set_once();

        let audio = WebAudio::new().map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Self {
            pipeline: None,
            info: None,
            pending_frame: None,
            scratch: DrawFrameScratch::default(),
            audio,
            default_sans_sc: None,
            default_color_emoji: None,
            extra_fonts: Vec::new(),
        })
    }

    /// Open (or replace) the persistent core pipeline for `source`.
    ///
    /// This is the host-owned open flow mirroring the engine's
    /// `open_parsed_host_owned` (#7): web fetches all declared resources,
    /// builds a prepared `PreparedResourceCatalog` via core's pure `probe::prepare`
    /// chain, hydrates captions, injects the font database, then opens the
    /// pipeline. Subsequent [`build_frame_ir`] calls render against this
    /// pipeline until the next `open_design`.
    ///
    /// The async fetch runs in the free [`open_design_pipeline`] helper; this
    /// method only touches `self` synchronously, before the `.await` (snapshot
    /// default fonts) and after it (store the opened pipeline). wasm-bindgen
    /// keeps a borrow guard alive across `.await` on `&mut self`, which would
    /// re-entrantly panic later `&mut self` methods — so `self` is never held
    /// across the await.
    #[wasm_bindgen]
    pub async fn open_design(&mut self, source: String) -> Result<String, JsValue> {
        let default_sans_sc = self.default_sans_sc.clone();
        let default_color_emoji = self.default_color_emoji.clone();
        let extra_fonts = self.extra_fonts.clone();

        let result = open_design_pipeline(
            &source,
            default_sans_sc.as_deref(),
            default_color_emoji.as_deref(),
            &extra_fonts,
        )
        .await;

        let (info, pipeline, catalog_json) = result?;
        self.info = Some(info);
        self.pipeline = Some(pipeline);
        self.pending_frame = None;
        Ok(catalog_json)
    }

    /// Render `frame` against the opened pipeline and encode its draw ops as a
    /// self-contained OCIR envelope (issue #45). All generated-image RGBA is
    /// fully encoded every frame — no epoch/delta/history dependency. Call
    /// [`open_design`] first.
    #[wasm_bindgen]
    pub fn build_frame_ir(&mut self, frame: u32) -> Result<Vec<u8>, JsValue> {
        let pipeline = self.pipeline.as_mut().ok_or_else(|| {
            JsValue::from_str("build_frame_ir: no design opened; call open_design first")
        })?;

        let render = match self.pending_frame.take() {
            Some((pending_index, render)) if pending_index == frame => render,
            _ => pipeline
                .render_frame(frame)
                .map_err(|e| JsValue::from_str(&format!("render_frame: {e}")))?,
        };

        crate::consumer::encode_render_frame_envelope(
            &mut RenderFrame {
                draw: render.draw,
                media: render.media,
            },
            &mut self.scratch,
        )
        .map_err(|e| JsValue::from_str(&e.0))
    }

    /// Return the current frame's [`FrameMediaPlan`] as JSON so JS can drive
    /// its own video decoder window / readahead / Lottie / image fetching from
    /// the core-derived media contract (replaces the old `plan_video_frames`
    /// tree walk). Call after [`open_design`].
    ///
    /// Shape: `{ videoFrames: [{assetId, timeMicros}], images: [assetId],
    /// lottieBundles: [id], generatedImages: [{id,width,height}], ... }`.
    /// Full RGBA for generated images is on the OCIR envelope, not this JSON.
    #[wasm_bindgen]
    pub fn prepare_frame(&mut self, frame: u32) -> Result<String, JsValue> {
        let pipeline = self.pipeline.as_mut().ok_or_else(|| {
            JsValue::from_str("prepare_frame: no design opened; call open_design first")
        })?;
        let render = pipeline
            .render_frame(frame)
            .map_err(|e| JsValue::from_str(&format!("prepare_frame render: {e}")))?;
        let plan_json = media_plan_to_json(&render.media);
        self.pending_frame = Some((frame, render));
        Ok(plan_json)
    }

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
                json!({
                    "sample_rate": pcm.sample_rate,
                    "channels": pcm.channels,
                    "samples": samples,
                })
                .to_string()
            }
            None => json!({
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

    /// Core-derived audio schedule for the opened design.
    ///
    /// Shape: `{ segments: [{ assetId, startMicros, endMicros, durationMicros }] }`.
    /// Hosts must use this for preview/export placement instead of treating every
    /// audio id as a full-composition track (issue #18).
    #[wasm_bindgen]
    pub fn audio_plan(&self) -> Result<String, JsValue> {
        let info = self.info.as_ref().ok_or_else(|| {
            JsValue::from_str("audio_plan: no design opened; call open_design first")
        })?;
        Ok(audio_plan_to_json(&info.audio_plan))
    }

    /// Load default CJK + emoji fonts. Call before [`open_design`]; the bytes
    /// are merged into the pipeline's font database when a design is opened.
    pub fn load_default_fonts(
        &mut self,
        sans_sc: Vec<u8>,
        color_emoji: Vec<u8>,
    ) -> Result<(), JsValue> {
        self.default_sans_sc = Some(sans_sc);
        self.default_color_emoji = Some(color_emoji);
        Ok(())
    }

    /// Append a single font file. Call before [`open_design`]; each face is
    /// loaded independently into the pipeline's font database.
    pub fn load_font_data(&mut self, bytes: Vec<u8>) -> Result<(), JsValue> {
        self.extra_fonts.push(bytes);
        Ok(())
    }
}

/// Read the SRT text for each declared subtitle from the thread-local
/// `BlobStore`, keyed by canonical subtitle `AssetId`. Mirrors the engine's
/// `EngineLoader::srt_text_by_subtitle_id`.
fn srt_text_by_subtitle_id(req: &ResourceRequests) -> HashMap<String, String> {
    let mut srt = HashMap::new();
    for src in &req.subtitles {
        let id = asset_id_for_subtitle(src);
        if let Some(bytes) = crate::resource::wasm_api::blob_bytes_owned(&id.key)
            && let Ok(text) = std::str::from_utf8(&bytes)
        {
            srt.insert(id.key, text.to_string());
        }
    }
    srt
}

/// Serialize core [`opencat_core::AudioPlan`] for web preview/export.
fn audio_plan_to_json(plan: &opencat_core::AudioPlan) -> String {
    let segments: Vec<Value> = plan
        .segments
        .iter()
        .map(|seg| {
            json!({
                "assetId": seg.asset.key,
                "startMicros": seg.start_micros().0,
                "endMicros": seg.end_micros().0,
                "durationMicros": seg.duration_micros().0,
            })
        })
        .collect();
    json!({ "segments": segments }).to_string()
}

/// Serialize a [`FrameMediaPlan`] to the JSON shape JS consumes to drive its
/// own video decoder / image / Lottie fetching.
fn media_plan_to_json(plan: &FrameMediaPlan) -> String {
    let video_frames: Vec<Value> = plan
        .video_frames
        .iter()
        .filter_map(|r| match r {
            ImageRef::VideoFrame {
                asset_id,
                time_micros,
            } => Some(json!({
                "assetId": asset_id,
                "timeMicros": time_micros,
            })),
            _ => None,
        })
        .collect();
    let images: Vec<Value> = plan
        .images
        .iter()
        .filter_map(|r| match r {
            ImageRef::Static { asset_id } => Some(json!({ "assetId": asset_id })),
            _ => None,
        })
        .collect();
    let lottie_bundles: Vec<Value> = plan
        .lottie_bundles
        .iter()
        .map(|id| json!({ "bundleId": id }))
        .collect();
    // JS prepare_frame only needs ids for cache bookkeeping; full RGBA rides
    // the OCIR envelope (section 12). Width/height are still useful for
    // allocation hints without decoding the binary delta.
    let generated_images: Vec<Value> = plan
        .generated_images
        .iter()
        .map(|g| {
            json!({
                "id": g.id.0,
                "width": g.width,
                "height": g.height,
            })
        })
        .collect();
    json!({
        "videoFrames": video_frames,
        "images": images,
        "lottieBundles": lottie_bundles,
        "generatedImages": generated_images,
    })
    .to_string()
}

/// Best-effort `JsValue` → string for error formatting.
fn js_err(v: &JsValue) -> String {
    v.as_string().unwrap_or_else(|| format!("{:?}", v))
}

/// Free async helper behind [`WebRenderer::open_design`]: does every step that
/// does not need `&mut self`, so the `&mut self` on the wasm-bindgen method is
/// only held across synchronous assignment — never across an `.await`. This
/// avoids wasm-bindgen's "recursive use of an object" borrow-guard panic.
///
/// Returns the composition info and the opened pipeline for the caller to store.
async fn open_design_pipeline(
    source: &str,
    default_sans_sc: Option<&[u8]>,
    default_color_emoji: Option<&[u8]>,
    extra_fonts: &[Vec<u8>],
) -> Result<
    (
        CompositionInfo,
        DefaultPipeline<WebJsContext>,
        String,
    ),
    JsValue,
> {
    // 1. Fetch all declared resources into the thread-local BlobStore and the
    //    Skottie provider store. `preload_assets` is the existing web fetch
    //    path (fetch/Blob/cache, font_store, Lottie bundle hydration, Openverse
    //    queries). It populates BLOB_STORE keyed by canonical AssetId;
    //    draw-ir.ts reads static images back via get_blob_bytes.
    let catalog_json = crate::resource::wasm_api::preload_assets(source)
        .await
        .map_err(|e| JsValue::from_str(&format!("open_design preload: {}", js_err(&e))))?;

    // 2. Build base font faces from host-supplied fonts. Host provides raw bytes;
    //    core constructs the fontdb internally.
    let mut font_faces: Vec<Vec<u8>> = Vec::new();
    if let Some(sans_sc) = default_sans_sc {
        font_faces.push(sans_sc.to_vec());
    }
    if let Some(color_emoji) = default_color_emoji {
        font_faces.push(color_emoji.to_vec());
    }
    font_faces.extend(extra_fonts.iter().cloned());

    // 3. Parse → draft. The font_db arg is unused (kept for signature compat).
    let parsed = crate::source::parse_source(source, &opencat_core::text::empty_font_db())
        .map_err(|e| JsValue::from_str(&format!("open_design parse: {e}")))?;
    let draft = CompositionDraft::from_parsed(parsed);
    let requests = opencat_core::parse::preflight::collect_resource_requests_from_parsed(draft.parsed());

    // 4. Read the catalog built by preload_assets (host-probed metadata).
    //    No re-probing needed — the host already extracted metadata during fetch.
    let probed = crate::resource::wasm_api::take_catalog()
        .ok_or_else(|| JsValue::from_str("open_design: no catalog from preload_assets"))?;
    let srt = srt_text_by_subtitle_id(&requests);

    // 5. Build HostInputs from raw font faces + family (core builds fontdb).
    let mut inputs = HostInputs::empty()
        .with_base_font_faces(font_faces)
        .with_sans_serif_family("Noto Sans SC");

    // Copy host-probed metadata into inputs, keyed by requirement AssetId.
    for req in draft.requirements().requests() {
        match req.kind {
            ResourceKind::Image => {
                if let Some(meta) = probed.images.get(&req.asset_id).copied() {
                    inputs.insert_image(req.asset_id.clone(), meta).map_err(prepare_js_err)?;
                }
            }
            ResourceKind::Video => {
                if let Some(meta) = probed.videos.get(&req.asset_id).copied() {
                    inputs.insert_video(req.asset_id.clone(), meta).map_err(prepare_js_err)?;
                }
            }
            ResourceKind::Audio => {
                inputs.insert_audio(req.asset_id.clone()).map_err(prepare_js_err)?;
            }
            ResourceKind::Lottie => {
                if let Some(meta) = probed.lotties.get(&req.asset_id).cloned() {
                    inputs.insert_lottie(req.asset_id.clone(), meta).map_err(prepare_js_err)?;
                }
            }
            ResourceKind::Subtitle => {
                if let Some(text) = srt.get(&req.asset_id.key) {
                    inputs.insert_subtitle_text(req.asset_id.clone(), text).map_err(prepare_js_err)?;
                }
            }
            ResourceKind::Font | ResourceKind::Script => {}
        }
    }

    // External scripts: host fetches text (path via VFS reader / url via fetch)
    // and injects via HostInputs — core never rewrites input strings (#20).
    for req in draft.requirements().requests() {
        if req.kind != ResourceKind::Script {
            continue;
        }
        let key = &req.asset_id.key;
        let text = if let Some(path) = key.strip_prefix("script:path:") {
            let bytes = crate::resource::asset_reader::read_path(path)
                .await
                .map_err(|e| {
                    JsValue::from_str(&format!("open_design script path `{path}`: {e}"))
                })?;
            String::from_utf8(bytes).map_err(|e| {
                JsValue::from_str(&format!("open_design script path `{path}` utf8: {e}"))
            })?
        } else if key.starts_with("script:url:") {
            let url = &key["script:url:".len()..];
            let bytes = crate::resource::fetch::fetch_bytes(url)
                .await
                .map_err(|e| {
                    JsValue::from_str(&format!("open_design script url `{url}`: {e}"))
                })?;
            String::from_utf8(bytes).map_err(|e| {
                JsValue::from_str(&format!("open_design script url `{url}` utf8: {e}"))
            })?
        } else {
            continue;
        };
        inputs
            .insert_script_text(req.asset_id.clone(), text)
            .map_err(prepare_js_err)?;
    }

    // Document fonts: face bytes from font_store → stable font AssetId.
    let font_bytes_by_face =
        crate::resource::font_store::get_manifest_bytes(&draft.parsed().font_manifest);
    for req in draft.requirements().requests() {
        if req.kind != ResourceKind::Font {
            continue;
        }
        let face_id = draft
            .parsed()
            .font_manifest
            .faces
            .iter()
            .find(|f| font_asset_id(&f.source) == req.asset_id.key)
            .map(|f| f.id.as_str());
        let Some(face_id) = face_id else {
            continue;
        };
        let Some(font_bytes) = font_bytes_by_face.get(face_id) else {
            continue;
        };
        inputs
            .insert_document_font(req.asset_id.clone(), font_bytes.clone())
            .map_err(prepare_js_err)?;
    }

    // 6. prepare (sync pure) → open_pipeline. Font merge + subtitle hydration
    //    happen inside prepare.
    let prepared = draft.prepare(inputs).map_err(prepare_js_err)?;
    let ctx = <WebJsContext as JsContext>::new()
        .map_err(|e| JsValue::from_str(&format!("open_design js context: {e}")))?;
    let pipeline = prepared
        .open_pipeline(ctx)
        .map_err(|e| JsValue::from_str(&format!("open_design pipeline: {e}")))?;

    let info = pipeline.info().clone();
    Ok((info, pipeline, catalog_json))
}

fn prepare_js_err(err: PrepareError) -> JsValue {
    JsValue::from_str(&format!("open_design prepare: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencat_core::ir::draw_encoding::{encode_ir_envelope, section, IR_VERSION};
    use opencat_core::ir::draw_frame::{DrawFrameScratch, DrawOpFrame};
    use opencat_core::ir::RenderFrame;
    use opencat_core::ir::media_plan::FrameMediaPlan;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn empty_render_frame() -> RenderFrame {
        RenderFrame {
            draw: DrawOpFrame::default(),
            media: FrameMediaPlan::default(),
        }
    }

    fn rgba(value: u8, width: u32, height: u32) -> Arc<[u8]> {
        Arc::from(vec![value; width as usize * height as usize * 4])
    }

    /// Decode just enough of a v5 envelope to read the generated-image section (12),
    /// mirroring the JS decoder. v5 has no pipeline_epoch in the header.
    struct DecodedEnvelope {
        generated: Vec<FrameGeneratedImage>,
    }

    fn decode_envelope(bytes: &[u8]) -> DecodedEnvelope {
        assert_eq!(&bytes[0..4], b"OCIR", "magic");
        let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        assert_eq!(version, IR_VERSION, "version");
        let section_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;

        let mut sections: HashMap<u32, (usize, usize)> = HashMap::new();
        for i in 0..section_count {
            let base = 12 + i * 12;
            let id = u32::from_le_bytes(bytes[base..base + 4].try_into().unwrap());
            let offset =
                u32::from_le_bytes(bytes[base + 4..base + 8].try_into().unwrap()) as usize;
            let len = u32::from_le_bytes(bytes[base + 8..base + 12].try_into().unwrap()) as usize;
            sections.insert(id, (offset, len));
        }

        let generated = match sections.get(&section::GENERATED_IMAGES) {
            Some((offset, len)) => decode_generated_images(&bytes[*offset..*offset + *len]),
            None => Vec::new(),
        };

        DecodedEnvelope { generated }
    }

    fn decode_generated_images(bytes: &[u8]) -> Vec<FrameGeneratedImage> {
        let mut cursor = 0;
        let count = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
        cursor += 4;
        let mut out = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let id = u64::from_le_bytes(bytes[cursor..cursor + 8].try_into().unwrap());
            cursor += 8;
            let width = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
            cursor += 4;
            let height = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
            cursor += 4;
            let rgba_len = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            let rgba = Arc::from(bytes[cursor..cursor + rgba_len].to_vec());
            cursor += rgba_len;
            out.push(FrameGeneratedImage {
                id: GeneratedImageId(id),
                width,
                height,
                rgba,
            });
        }
        out
    }

    #[test]
    fn envelope_header_has_no_pipeline_epoch() {
        let frame = empty_render_frame();
        let mut scratch = DrawFrameScratch::default();
        let bytes = encode_ir_envelope(&frame, &mut scratch).unwrap();

        // v5 header is only 12 bytes — no pipeline_epoch.
        assert_eq!(&bytes[0..4], b"OCIR");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), IR_VERSION);
        let section_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        assert!(section_count > 0);
        // Directory starts at offset 12
        let _first_id = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
        // Generated section is empty (zero count)
        let decoded = decode_envelope(&bytes);
        assert!(decoded.generated.is_empty(), "no images => empty section 12");
    }

    #[test]
    fn generated_images_fully_encoded_every_frame() {
        let delta = vec![
            FrameGeneratedImage {
                id: GeneratedImageId(0x0123_4567_89ab_cdef),
                width: 3,
                height: 2,
                rgba: rgba(0xAB, 3, 2),
            },
            FrameGeneratedImage {
                id: GeneratedImageId(42),
                width: 1,
                height: 1,
                rgba: rgba(0x11, 1, 1),
            },
        ];
        let frame = RenderFrame {
            draw: DrawOpFrame::default(),
            media: FrameMediaPlan {
                generated_images: delta,
                ..Default::default()
            },
        };
        let mut scratch = DrawFrameScratch::default();
        let bytes = encode_ir_envelope(&frame, &mut scratch).unwrap();
        let decoded = decode_envelope(&bytes);

        assert_eq!(decoded.generated.len(), 2);
        assert_eq!(decoded.generated[0].id, GeneratedImageId(0x0123_4567_89ab_cdef));
        assert_eq!(decoded.generated[0].width, 3);
        assert_eq!(decoded.generated[0].height, 2);
        assert_eq!(decoded.generated[0].rgba.as_ref(), rgba(0xAB, 3, 2).as_ref());
        assert_eq!(decoded.generated[1].id, GeneratedImageId(42));
        assert_eq!(decoded.generated[1].width, 1);
        assert_eq!(decoded.generated[1].height, 1);
        assert_eq!(decoded.generated[1].rgba.as_ref(), rgba(0x11, 1, 1).as_ref());
    }

    #[test]
    fn empty_delta_serializes_as_zero_count_section() {
        let frame = empty_render_frame();
        let mut scratch = DrawFrameScratch::default();
        let bytes = encode_ir_envelope(&frame, &mut scratch).unwrap();
        let section_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let mut section_12 = None;
        for i in 0..section_count {
            let base = 12 + i * 12;
            let id = u32::from_le_bytes(bytes[base..base + 4].try_into().unwrap());
            if id == section::GENERATED_IMAGES {
                let offset =
                    u32::from_le_bytes(bytes[base + 4..base + 8].try_into().unwrap()) as usize;
                let len =
                    u32::from_le_bytes(bytes[base + 8..base + 12].try_into().unwrap()) as usize;
                section_12 = Some(&bytes[offset..offset + len]);
            }
        }
        let section = section_12.expect("section 12 present");
        assert_eq!(section.len(), 4, "empty generated is exactly one u32 count");
        assert_eq!(u32::from_le_bytes(section.try_into().unwrap()), 0);
    }

    #[test]
    fn same_glyph_encoded_on_every_frame() {
        // In v5, every frame carries the full generated-image RGBA — there is no
        // delta or epoch-based suppression. The same glyph is fully encoded
        // regardless of whether it appeared on a previous frame.
        let glyph = FrameGeneratedImage {
            id: GeneratedImageId(99),
            width: 2,
            height: 2,
            rgba: rgba(0x55, 2, 2),
        };
        let frame = RenderFrame {
            draw: DrawOpFrame::default(),
            media: FrameMediaPlan {
                generated_images: vec![glyph.clone()],
                ..Default::default()
            },
        };
        let mut scratch = DrawFrameScratch::default();
        let bytes = encode_ir_envelope(&frame, &mut scratch).unwrap();
        assert_eq!(decode_envelope(&bytes).generated.len(), 1);

        scratch.clear();
        // Second encoding of the same frame still carries the glyph
        let bytes2 = encode_ir_envelope(&frame, &mut scratch).unwrap();
        assert_eq!(
            decode_envelope(&bytes2).generated.len(),
            1,
            "same glyph fully encoded every frame"
        );
    }

    #[test]
    fn generated_image_cache_keyed_by_id_only() {
        // Since there is no epoch, the same id always carries the same RGBA.
        // Verify by encoding two identical frames: the OCIR is byte-identical.
        let glyph = FrameGeneratedImage {
            id: GeneratedImageId(7),
            width: 1,
            height: 1,
            rgba: rgba(0xFF, 1, 1),
        };
        let frame = RenderFrame {
            draw: DrawOpFrame::default(),
            media: FrameMediaPlan {
                generated_images: vec![glyph],
                ..Default::default()
            },
        };
        let mut scratch = DrawFrameScratch::default();
        let bytes = encode_ir_envelope(&frame, &mut scratch).unwrap();
        scratch.clear();
        let bytes2 = encode_ir_envelope(&frame, &mut scratch).unwrap();
        assert_eq!(bytes, bytes2, "same RenderFrame => byte-identical OCIR");
    }

    /// Web host contract (#15): draft requirements' AssetId is the only id used
    /// when inserting image metadata — never re-derived from locator.
    #[test]
    fn web_host_static_image_uses_request_asset_id() {
        use opencat_core::lifecycle::{CompositionDraft, HostInputs, ResourceKind};
        use opencat_core::pipeline::Pipeline;
        use opencat_core::probe::catalog::ImageMeta;
        use opencat_core::script::js_context::JsContext;
        use opencat_core::ir::draw_types::ImageRef;

        let jsonl = r#"{"type":"composition","width":32,"height":32,"fps":30,"duration":0.1}
{"type":"div","id":"root","parentId":null,"className":"w-full h-full"}
{"type":"image","id":"pic","parentId":"root","path":"hero.png","className":"w-[16px] h-[16px]"}"#;

        let draft = CompositionDraft::parse(jsonl).expect("parse draft");
        let req = &draft.requirements().requests()[0];
        assert_eq!(req.kind, ResourceKind::Image);
        assert_eq!(req.asset_id.key, "hero.png");

        let id = req.asset_id.clone();
        let mut inputs = HostInputs::empty();
        inputs
            .insert_image(id.clone(), ImageMeta { width: 16, height: 16 })
            .expect("insert under request id");
        let prepared = draft.prepare(inputs).expect("prepare");
        let ctx = <WebJsContext as JsContext>::new().expect("js");
        let mut pipeline = prepared.open_pipeline(ctx).expect("open via lifecycle");
        let frame = pipeline.render_frame(0).expect("render");
        assert!(
            frame.media.images.iter().any(|img| matches!(
                img,
                ImageRef::Static { asset_id } if asset_id == "hero.png"
            )),
            "web host media plan must use request AssetId; got {:?}",
            frame.media.images
        );
    }

    /// Web host contract (#17): draft requirements' bundle AssetId is the only id
    /// used when inserting Lottie metadata — never re-derived; media plan lists
    /// Lottie bundles distinctly from ordinary images.
    #[test]
    fn web_host_lottie_uses_request_bundle_asset_id() {
        use opencat_core::ir::draw_op::DrawOp;
        use opencat_core::lifecycle::{CompositionDraft, HostInputs, ResourceKind};
        use opencat_core::pipeline::Pipeline;
        use opencat_core::resource::lottie::LottieMeta;
        use opencat_core::script::js_context::JsContext;

        let markup = r#"
            <opencat width="32" height="32" fps="30" duration="0.1">
              <div id="root" class="w-full h-full">
                <lottie id="loader" path="anim/loader.json" class="w-[16px] h-[16px]" />
              </div>
            </opencat>
        "#;

        let draft = CompositionDraft::parse(markup).expect("parse draft");
        let req = &draft.requirements().requests()[0];
        assert_eq!(req.kind, ResourceKind::Lottie);
        assert_eq!(req.asset_id.key, "lottie:path:anim/loader.json");

        let id = req.asset_id.clone();
        let mut inputs = HostInputs::empty();
        inputs
            .insert_lottie(
                id.clone(),
                LottieMeta {
                    width: 40,
                    height: 30,
                    fps: 25.0,
                    in_frame: 0.0,
                    out_frame: 10.0,
                    dependencies: vec!["dep.png".into()],
                },
            )
            .expect("insert under request id");
        let prepared = draft.prepare(inputs).expect("prepare");
        assert_eq!(
            prepared.catalog().lotties[&id].dependencies,
            vec!["dep.png".to_string()]
        );
        let ctx = <WebJsContext as JsContext>::new().expect("js");
        let mut pipeline = prepared.open_pipeline(ctx).expect("open via lifecycle");
        let frame = pipeline.render_frame(0).expect("render");
        assert!(
            frame
                .media
                .lottie_bundles
                .iter()
                .any(|b| b == "lottie:path:anim/loader.json"),
            "web host media plan must list Lottie bundle; got {:?}",
            frame.media.lottie_bundles
        );
        assert!(
            frame.media.images.is_empty(),
            "Lottie must not be disguised as image"
        );
        assert!(
            frame.draw.ops.iter().any(|op| {
                matches!(op, DrawOp::LottieRect { bundle_id, .. } if bundle_id == "lottie:path:anim/loader.json")
            }),
            "draw must emit LottieRect"
        );
    }

}
