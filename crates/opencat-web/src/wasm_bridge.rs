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

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

use opencat_core::ir::GeneratedImageId;
use opencat_core::canvas::paint::{
    BlendMode, BlurStyle, ColorFilterSpec, FillSpec, ImageFilterSpec, MaskFilterSpec, PaintSpec,
    PaintStyle, PathEffectSpec, ShaderSpec as PaintShaderSpec, StrokeCap, StrokeJoin, TileMode,
};
use opencat_core::frame_ctx::duration_secs_to_frames;
use opencat_core::ir::CompositionInfo;
use opencat_core::ir::RenderFrame;
use opencat_core::ir::asset_id::asset_id_for_subtitle;
use opencat_core::ir::draw_encoding::EncodedDrawFrame;
use opencat_core::ir::draw_frame::{DrawFrameScratch, DrawOpFrame};
use opencat_core::ir::draw_op::DrawOp;
use opencat_core::ir::draw_types::{
    EffectRef, EncodedPath, FillType, ImageRef, PathOp, RuntimeEffectChildRef, ShaderSpec,
    ShaderType, TableRange,
};
use opencat_core::ir::media_plan::FrameMediaPlan;
use opencat_core::lifecycle::{CompositionDraft, HostInputs, PrepareError};
use opencat_core::pipeline::Pipeline;
use opencat_core::pipeline::default::DefaultPipeline;
use opencat_core::probe::catalog::ResourceRequests;
use opencat_core::probe::prepare::build_catalog;
use opencat_core::script::js_context::JsContext;

use crate::js_context::WebJsContext;
use crate::media::WebAudio;

const IR_MAGIC: &[u8; 4] = b"OCIR";
/// v3 (issue #8): persistent core pipeline, 11 draw-IR sections.
/// v4 (issue #10): adds `pipeline_epoch: u32` to the header and a
/// `SECTION_GENERATED_IMAGES` (12) carrying the per-frame generated-image
/// delta (core-rasterized color-emoji RGBA) keyed by `(epoch, id)` on the JS
/// side. Section 12 is always present in v4 (possibly `count: 0`).
const IR_VERSION: u32 = 4;

const SECTION_OPS: u32 = 1;
const SECTION_F32_POOL: u32 = 2;
const SECTION_BYTES: u32 = 3;
const SECTION_BYTE_RANGES: u32 = 4;
const SECTION_STRINGS_UTF8: u32 = 5;
const SECTION_STRING_RANGES: u32 = 6;
const SECTION_PAINTS: u32 = 7;
const SECTION_PATHS: u32 = 8;
const SECTION_CHILDREN: u32 = 9;
const SECTION_EFFECTS: u32 = 10;
const SECTION_SUBTREES: u32 = 11;
const SECTION_GENERATED_IMAGES: u32 = 12;

/// One core-rasterized glyph published to JS in the generated-image delta.
///
/// The delta only carries glyphs whose id was not already published in the
/// current [`WebRenderer::pipeline_epoch`]; JS caches the resulting CanvasKit
/// image under `(epoch, id)` and reuses it until the epoch bumps.
#[derive(Clone)]
pub(crate) struct GeneratedImageRecord {
    pub id: GeneratedImageId,
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

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
    /// Monotonically bumped on every [`open_design`]. Stamping this into each
    /// OCIR envelope lets JS key its generated-image cache by `(epoch, id)`
    /// and evict stale entries when a new design replaces the pipeline.
    pipeline_epoch: u32,
    /// Generated-image ids already published to JS in the current epoch. Drives
    /// the per-frame delta: a glyph's RGBA is emitted only on the first frame
    /// that references it. Cleared on each epoch bump.
    published_generated: HashSet<GeneratedImageId>,
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
            // epoch starts at 0; the first `open_design` bumps it to 1 so a
            // freshly-constructed renderer and one that has opened a design
            // are distinguishable.
            pipeline_epoch: 0,
            published_generated: HashSet::new(),
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
        // Bump the pipeline epoch and drop the set of ids published under the
        // previous design. JS keys its generated-image cache by `(epoch, id)`
        // and evicts stale entries when it observes the new epoch in the next
        // frame's envelope, so the first frame after a reopen republishes
        // every glyph the new pipeline needs.
        self.pipeline_epoch = self.pipeline_epoch.wrapping_add(1);
        self.published_generated.clear();
        Ok(catalog_json)
    }

    /// Render `frame` against the opened pipeline and encode its draw ops as a
    /// binary OCIR envelope, including the generated-image delta for any
    /// color-emoji glyphs newly referenced this epoch. Call [`open_design`]
    /// first.
    #[wasm_bindgen]
    pub fn build_frame_ir(&mut self, frame: u32) -> Result<Vec<u8>, JsValue> {
        let pipeline = self.pipeline.as_mut().ok_or_else(|| {
            JsValue::from_str("build_frame_ir: no design opened; call open_design first")
        })?;
        let info = self.info.as_ref().ok_or_else(|| {
            JsValue::from_str("build_frame_ir: composition info missing; call open_design first")
        })?;

        let render = match self.pending_frame.take() {
            Some((pending_index, render)) if pending_index == frame => render,
            _ => pipeline
                .render_frame(frame)
                .map_err(|e| JsValue::from_str(&format!("render_frame: {e}")))?,
        };
        let mut draw = render.draw;
        let media_plan = render.media;

        // Build the generated-image delta: every id the core needs visible this
        // frame that we have not already published in the current epoch. RGBA is
        // copied from the pipeline's table (populated during `render_frame`);
        // missing entries are skipped defensively (core always populates the
        // table before emitting `ImageRef::Generated`).
        let generated_table = pipeline.generated_images();
        let mut generated_delta: Vec<GeneratedImageRecord> = Vec::new();
        for id in &media_plan.generated_images {
            if self.published_generated.contains(id) {
                continue;
            }
            let Some(entry) = generated_table.get(id) else {
                continue;
            };
            generated_delta.push(GeneratedImageRecord {
                id: *id,
                width: entry.width,
                height: entry.height,
                rgba: Arc::clone(&entry.rgba),
            });
            self.published_generated.insert(*id);
        }

        use crate::consumer::FrameConsumer;

        let header = crate::consumer::RenderSessionHeader {
            composition_size: (info.width, info.height),
            fps: info.fps,
            frames: duration_secs_to_frames(info.duration, info.fps),
        };

        let mut consumer = crate::consumer::WebFrameConsumer {
            scratch: &mut self.scratch,
            pipeline_epoch: self.pipeline_epoch,
            generated_delta: &generated_delta,
        };
        consumer
            .consume_frame(&header, &mut draw, &media_plan)
            .map_err(JsValue::from)
    }

    /// Return the current frame's [`FrameMediaPlan`] as JSON so JS can drive
    /// its own video decoder window / readahead / Lottie / image fetching from
    /// the core-derived media contract (replaces the old `plan_video_frames`
    /// tree walk). Call after [`open_design`].
    ///
    /// Shape: `{ videoFrames: [{assetId, timeMicros}], images: [assetId],
    /// lottieBundles: [id], generatedImages: [id], runtimeEffects: [...] }`.
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
        if let Some(bytes) = crate::resource::wasm_api::blob_bytes_owned(&id.0)
            && let Ok(text) = std::str::from_utf8(&bytes)
        {
            srt.insert(id.0, text.to_string());
        }
    }
    srt
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
    let generated_images: Vec<Value> = plan
        .generated_images
        .iter()
        .map(|id| json!({ "id": id.0 }))
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

    // 2. Build the complete font database once (default fonts + document
    //    fonts). Start from the default fonts (NotoSansSC + NotoColorEmoji)
    //    like the old `load_default_fonts` did, then merge any document
    //    fonts declared in the source. `merge_preloaded_fonts` only applies
    //    document fonts from `<fonts>` manifests (markup); JSONL designs rely
    //    solely on the defaults, so they must be seeded here.
    let default_fonts = default_sans_sc.zip(default_color_emoji);
    let base_db = match default_fonts {
        Some((sans_sc, color_emoji)) => opencat_core::text::font_db_from_bytes(
            &[sans_sc.to_vec(), color_emoji.to_vec()],
            "Noto Sans SC",
        ),
        None => opencat_core::text::empty_font_db(),
    };
    let base_db = opencat_core::text::extend_font_db(&base_db, extra_fonts);
    let base_db = {
        let base = Arc::new(base_db);
        let merged = crate::source::merge_preloaded_fonts(&base, source, default_fonts)
            .map_err(|e| JsValue::from_str(&format!("open_design fonts: {e}")))?;
        (*merged).clone()
    };

    // 3. Parse → draft. Requirements carry canonical AssetId + logical locator.
    let parsed = crate::source::parse_source(source, &base_db)
        .map_err(|e| JsValue::from_str(&format!("open_design parse: {e}")))?;
    let draft = CompositionDraft::from_parsed(parsed);
    let requests = draft.requirements().resource_requests().clone();

    // 4. Pure probes over host-fetched bytes → metadata only.
    let bytes = crate::resource::wasm_api::blob_byte_map();
    let probed = build_catalog(&requests, &bytes).catalog;
    let srt = srt_text_by_subtitle_id(&requests);

    // 5. Build HostInputs from request AssetIds (never re-derive ids).
    let mut inputs = HostInputs::empty().with_font_db(Arc::new(base_db));
    inputs
        .fill_from_prepared_catalog(draft.requirements(), &probed, &srt)
        .map_err(prepare_js_err)?;

    // 6. prepare (sync pure) → open_pipeline. Subtitle hydration happens inside prepare.
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

pub(crate) fn encode_ir_envelope(
    draw: &DrawOpFrame,
    encoded: &EncodedDrawFrame,
    pipeline_epoch: u32,
    generated_delta: &[GeneratedImageRecord],
) -> Result<Vec<u8>, String> {
    let generated_images_section = encode_generated_images(generated_delta);
    let sections = [
        (SECTION_OPS, encoded.ops.clone()),
        (SECTION_SUBTREES, encoded.subtrees.clone()),
        (SECTION_F32_POOL, encode_f32_slice(&encoded.f32_pool)),
        (SECTION_BYTES, draw.bytes.clone()),
        (SECTION_BYTE_RANGES, encode_ranges(&draw.byte_ranges)),
        (SECTION_STRINGS_UTF8, encoded.strings_utf8.clone()),
        (SECTION_STRING_RANGES, encode_ranges(&encoded.string_ranges)),
        (SECTION_PAINTS, encode_paints(&draw.paints)?),
        (SECTION_PATHS, encode_paths(&draw.paths)),
        (
            SECTION_CHILDREN,
            encode_children(&draw.children, &draw.strings)?,
        ),
        (SECTION_EFFECTS, encode_effects(&draw.effects)),
        (SECTION_GENERATED_IMAGES, generated_images_section),
    ];

    // v4 header: magic(4) + version(4) + section_count(4) + epoch(4) +
    // sections × 12.
    let header_len = 16 + sections.len() * 12;
    let mut offsets = Vec::with_capacity(sections.len());
    let mut cursor = header_len as u32;
    for (_, bytes) in &sections {
        cursor = align4(cursor);
        offsets.push(cursor);
        cursor = cursor
            .checked_add(bytes.len() as u32)
            .ok_or_else(|| "IR envelope too large".to_string())?;
    }

    let mut out = Vec::with_capacity(cursor as usize);
    out.extend_from_slice(IR_MAGIC);
    write_u32(&mut out, IR_VERSION);
    write_u32(&mut out, sections.len() as u32);
    write_u32(&mut out, pipeline_epoch);
    for ((id, bytes), offset) in sections.iter().zip(offsets.iter()) {
        write_u32(&mut out, *id);
        write_u32(&mut out, *offset);
        write_u32(&mut out, bytes.len() as u32);
    }

    for ((_, bytes), offset) in sections.iter().zip(offsets.iter()) {
        while out.len() < *offset as usize {
            out.push(0);
        }
        out.extend_from_slice(bytes);
    }

    Ok(out)
}

/// Encode the generated-image delta (section 12). Layout: `count: u32` then
/// `count × { id: u64, width: u32, height: u32, rgba_len: u32, rgba: [u8] }`.
/// An empty delta (no new glyphs this frame) serializes as a single `0` count.
fn encode_generated_images(delta: &[GeneratedImageRecord]) -> Vec<u8> {
    let mut out = Vec::new();
    write_u32(&mut out, delta.len() as u32);
    for record in delta {
        write_u64(&mut out, record.id.0);
        write_u32(&mut out, record.width);
        write_u32(&mut out, record.height);
        write_bytes_with_len(&mut out, &record.rgba);
    }
    out
}

pub(crate) fn intern_image_strings(draw: &mut DrawOpFrame) {
    fn push_unique(strings: &mut Vec<String>, asset_id: &str) {
        if !strings.iter().any(|item| item == asset_id) {
            strings.push(asset_id.to_string());
        }
    }

    for op in &draw.ops {
        intern_image_strings_in_ops(&mut draw.strings, std::slice::from_ref(op));
    }
    for subtree in &draw.subtrees {
        intern_image_strings_in_ops(&mut draw.strings, subtree);
    }
    for child in &draw.children {
        if let RuntimeEffectChildRef::Image(image) = child {
            intern_image_ref(&mut draw.strings, image);
        }
    }

    fn intern_image_strings_in_ops(strings: &mut Vec<String>, ops: &[DrawOp]) {
        for op in ops {
            match op {
                DrawOp::Image { image, .. } | DrawOp::ImageRect { image, .. } => {
                    intern_image_ref(strings, image);
                }
                DrawOp::LottieRect { bundle_id, .. } => push_unique(strings, bundle_id),
                _ => {}
            }
        }
    }

    fn intern_image_ref(strings: &mut Vec<String>, image: &ImageRef) {
        match image {
            ImageRef::Static { asset_id } | ImageRef::VideoFrame { asset_id, .. } => {
                push_unique(strings, asset_id);
            }
            // Generated images are addressed by numeric id, not a string
            // asset_id, so they contribute nothing to the IR string table.
            ImageRef::Generated { .. } => {}
        }
    }
}

fn encode_paints(paints: &[PaintSpec]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    write_u32(&mut out, paints.len() as u32);
    for paint in paints {
        let mut record = Vec::new();
        encode_paint(&mut record, paint)?;
        write_u32(&mut out, record.len() as u32);
        out.extend_from_slice(&record);
    }
    Ok(out)
}

fn encode_paint(out: &mut Vec<u8>, paint: &PaintSpec) -> Result<(), String> {
    encode_fill(out, &paint.fill);
    write_u8(out, encode_paint_style(paint.style));
    write_u8(out, if paint.anti_alias { 1 } else { 0 });
    write_u8(out, encode_blend_mode(paint.blend_mode));
    match &paint.stroke {
        Some(stroke) => {
            write_u8(out, 1);
            write_f32(out, stroke.width);
            write_u8(out, encode_stroke_cap(stroke.cap));
            write_u8(out, encode_stroke_join(stroke.join));
            write_f32(out, stroke.miter_limit);
        }
        None => write_u8(out, 0),
    }
    encode_optional(out, paint.image_filter.as_ref(), encode_image_filter)?;
    encode_optional(out, paint.color_filter.as_ref(), encode_color_filter)?;
    encode_optional(out, paint.mask_filter.as_ref(), encode_mask_filter)?;
    encode_optional(out, paint.path_effect.as_ref(), encode_path_effect)?;
    Ok(())
}

fn encode_fill(out: &mut Vec<u8>, fill: &FillSpec) {
    match fill {
        FillSpec::Solid(color) => {
            write_u8(out, 0);
            write_f32_array(out, color);
        }
        FillSpec::Shader(shader) => {
            write_u8(out, 1);
            encode_paint_shader(out, shader);
        }
    }
}

fn encode_paint_shader(out: &mut Vec<u8>, shader: &PaintShaderSpec) {
    match shader {
        PaintShaderSpec::LinearGradient {
            from,
            to,
            stops,
            colors,
            tile_mode,
            local_matrix,
        } => {
            write_u8(out, 0);
            write_u8(out, encode_tile_mode(*tile_mode));
            write_f32_array(out, from);
            write_f32_array(out, to);
            write_f32_vec(out, stops);
            write_color_vec(out, colors);
            encode_optional_matrix(out, local_matrix);
        }
        PaintShaderSpec::RadialGradient {
            center,
            radius,
            stops,
            colors,
            tile_mode,
            local_matrix,
        } => {
            write_u8(out, 1);
            write_u8(out, encode_tile_mode(*tile_mode));
            write_f32_array(out, center);
            write_f32(out, *radius);
            write_f32_vec(out, stops);
            write_color_vec(out, colors);
            encode_optional_matrix(out, local_matrix);
        }
    }
}

/// Encode an optional 3×3 row-major matrix: presence byte (1 = Some) + 9×f32.
fn encode_optional_matrix(out: &mut Vec<u8>, matrix: &Option<[f32; 9]>) {
    match matrix {
        Some(m) => {
            write_u8(out, 1);
            for v in m {
                write_f32(out, *v);
            }
        }
        None => write_u8(out, 0),
    }
}

fn encode_image_filter(out: &mut Vec<u8>, filter: &ImageFilterSpec) -> Result<(), String> {
    match filter {
        ImageFilterSpec::Blur {
            sigma_x,
            sigma_y,
            crop_rect,
        } => {
            write_u8(out, 0);
            write_f32(out, *sigma_x);
            write_f32(out, *sigma_y);
            match crop_rect {
                Some(rect) => {
                    write_u8(out, 1);
                    write_f32(out, rect.x0 as f32);
                    write_f32(out, rect.y0 as f32);
                    write_f32(out, rect.x1 as f32);
                    write_f32(out, rect.y1 as f32);
                }
                None => write_u8(out, 0),
            }
        }
        ImageFilterSpec::DropShadow {
            dx,
            dy,
            sigma_x,
            sigma_y,
            color,
        } => {
            write_u8(out, 1);
            write_f32(out, *dx);
            write_f32(out, *dy);
            write_f32(out, *sigma_x);
            write_f32(out, *sigma_y);
            write_f32_array(out, color);
        }
        ImageFilterSpec::ColorFilter(filter) => {
            write_u8(out, 2);
            encode_color_filter(out, filter)?;
        }
        ImageFilterSpec::Compose(outer, inner) => {
            write_u8(out, 3);
            encode_image_filter(out, outer)?;
            encode_image_filter(out, inner)?;
        }
    }
    Ok(())
}

fn encode_color_filter(out: &mut Vec<u8>, filter: &ColorFilterSpec) -> Result<(), String> {
    match filter {
        ColorFilterSpec::Matrix(matrix) => {
            write_u8(out, 0);
            write_f32_array(out, matrix);
        }
        ColorFilterSpec::BlendColor { color, mode } => {
            write_u8(out, 1);
            write_f32_array(out, color);
            write_u8(out, encode_blend_mode(*mode));
        }
        ColorFilterSpec::LinearToSrgbGamma => write_u8(out, 2),
        ColorFilterSpec::SrgbToLinearGamma => write_u8(out, 3),
    }
    Ok(())
}

fn encode_mask_filter(out: &mut Vec<u8>, filter: &MaskFilterSpec) -> Result<(), String> {
    match filter {
        MaskFilterSpec::Blur {
            sigma,
            style,
            respect_ctm,
        } => {
            write_u8(out, 0);
            write_f32(out, *sigma);
            write_u8(out, encode_blur_style(*style));
            write_u8(out, if *respect_ctm { 1 } else { 0 });
        }
    }
    Ok(())
}

fn encode_path_effect(out: &mut Vec<u8>, effect: &PathEffectSpec) -> Result<(), String> {
    match effect {
        PathEffectSpec::Dash { intervals, phase } => {
            write_u8(out, 0);
            write_f32_vec(out, intervals);
            write_f32(out, *phase);
        }
    }
    Ok(())
}

fn encode_paths(paths: &[EncodedPath]) -> Vec<u8> {
    let mut out = Vec::new();
    write_u32(&mut out, paths.len() as u32);
    for path in paths {
        let mut record = Vec::new();
        write_u8(&mut record, encode_fill_type(path.fill_type));
        write_u32(&mut record, path.ops.len() as u32);
        for op in &path.ops {
            encode_path_op(&mut record, op);
        }
        write_u32(&mut out, record.len() as u32);
        out.extend_from_slice(&record);
    }
    out
}

fn encode_path_op(out: &mut Vec<u8>, op: &PathOp) {
    match op {
        PathOp::MoveTo { x, y } => {
            write_u16(out, 0);
            write_f32(out, *x);
            write_f32(out, *y);
        }
        PathOp::LineTo { x, y } => {
            write_u16(out, 1);
            write_f32(out, *x);
            write_f32(out, *y);
        }
        PathOp::QuadTo { cx, cy, x, y } => {
            write_u16(out, 2);
            write_f32(out, *cx);
            write_f32(out, *cy);
            write_f32(out, *x);
            write_f32(out, *y);
        }
        PathOp::CubicTo {
            c1x,
            c1y,
            c2x,
            c2y,
            x,
            y,
        } => {
            write_u16(out, 3);
            write_f32(out, *c1x);
            write_f32(out, *c1y);
            write_f32(out, *c2x);
            write_f32(out, *c2y);
            write_f32(out, *x);
            write_f32(out, *y);
        }
        PathOp::Close => write_u16(out, 4),
        PathOp::AddRect {
            x,
            y,
            width,
            height,
        } => {
            write_u16(out, 5);
            write_f32(out, *x);
            write_f32(out, *y);
            write_f32(out, *width);
            write_f32(out, *height);
        }
        PathOp::AddRRect {
            x,
            y,
            width,
            height,
            radius,
        } => {
            write_u16(out, 6);
            write_f32(out, *x);
            write_f32(out, *y);
            write_f32(out, *width);
            write_f32(out, *height);
            write_f32(out, *radius);
        }
        PathOp::AddOval {
            x,
            y,
            width,
            height,
        } => {
            write_u16(out, 7);
            write_f32(out, *x);
            write_f32(out, *y);
            write_f32(out, *width);
            write_f32(out, *height);
        }
        PathOp::AddArc {
            x,
            y,
            width,
            height,
            start_angle,
            sweep_angle,
        } => {
            write_u16(out, 8);
            write_f32(out, *x);
            write_f32(out, *y);
            write_f32(out, *width);
            write_f32(out, *height);
            write_f32(out, *start_angle);
            write_f32(out, *sweep_angle);
        }
    }
}

fn encode_children(
    children: &[RuntimeEffectChildRef],
    strings: &[String],
) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    write_u32(&mut out, children.len() as u32);
    for child in children {
        let mut record = Vec::new();
        match child {
            RuntimeEffectChildRef::Image(image) => {
                write_u8(&mut record, 0);
                encode_image_ref(&mut record, image, strings)?;
            }
            RuntimeEffectChildRef::Picture(range) => {
                write_u8(&mut record, 1);
                write_u32(&mut record, range.start_op);
                write_u32(&mut record, range.op_len);
            }
            RuntimeEffectChildRef::SubtreePicture(subtree) => {
                write_u8(&mut record, 3);
                write_u32(&mut record, subtree.0);
            }
            RuntimeEffectChildRef::Shader(shader) => {
                write_u8(&mut record, 2);
                encode_ir_shader(&mut record, shader);
            }
        }
        write_u32(&mut out, record.len() as u32);
        out.extend_from_slice(&record);
    }
    Ok(out)
}

fn encode_image_ref(
    out: &mut Vec<u8>,
    image: &ImageRef,
    strings: &[String],
) -> Result<(), String> {
    match image {
        ImageRef::Static { asset_id } => {
            write_u8(out, 0);
            write_u32(out, lookup_string_id(strings, asset_id)?);
            write_u64(out, 0); // time_micros = 0
        }
        ImageRef::VideoFrame {
            asset_id,
            time_micros,
        } => {
            write_u8(out, 1);
            write_u32(out, lookup_string_id(strings, asset_id)?);
            write_u64(out, *time_micros);
        }
        // Generated images carry a numeric id, not an interned asset string.
        // The RGBA is published separately via the generated-image delta
        // (issue #10); the JS decoder resolves it from (pipeline_epoch, id).
        // Layout mirrors the core encoder: tag(1) + id_u64(8) + reserved(4).
        ImageRef::Generated { id } => {
            write_u8(out, 2);
            write_u64(out, id.0);
            write_u32(out, 0); // reserved
        }
    }
    Ok(())
}

fn encode_ir_shader(out: &mut Vec<u8>, shader: &ShaderSpec) {
    match &shader.shader_type {
        ShaderType::LinearGradient { start, end, colors } => {
            write_u8(out, 0);
            write_f32(out, start.0);
            write_f32(out, start.1);
            write_f32(out, end.0);
            write_f32(out, end.1);
            encode_ir_gradient_colors(out, colors);
        }
        ShaderType::RadialGradient {
            center,
            radius,
            colors,
        } => {
            write_u8(out, 1);
            write_f32(out, center.0);
            write_f32(out, center.1);
            write_f32(out, *radius);
            encode_ir_gradient_colors(out, colors);
        }
    }
}

fn encode_ir_gradient_colors(out: &mut Vec<u8>, colors: &[(f32, [f32; 4])]) {
    write_u32(out, colors.len() as u32);
    for (stop, color) in colors {
        write_f32(out, *stop);
        write_f32_array(out, color);
    }
}

fn encode_effects(effects: &[EffectRef]) -> Vec<u8> {
    let mut out = Vec::new();
    write_u32(&mut out, effects.len() as u32);
    for effect in effects {
        write_u64(&mut out, effect.hash);
        write_bytes_with_len(&mut out, effect.sksl.as_bytes());
    }
    out
}

fn encode_optional<T>(
    out: &mut Vec<u8>,
    value: Option<&T>,
    encode: fn(&mut Vec<u8>, &T) -> Result<(), String>,
) -> Result<(), String> {
    match value {
        Some(value) => {
            write_u8(out, 1);
            encode(out, value)?;
        }
        None => write_u8(out, 0),
    }
    Ok(())
}

fn lookup_string_id(strings: &[String], s: &str) -> Result<u32, String> {
    strings
        .iter()
        .position(|item| item == s)
        .map(|idx| idx as u32)
        .ok_or_else(|| format!("asset_id not interned in IR strings: {s}"))
}

fn encode_f32_slice(values: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * 4);
    for value in values {
        write_f32(&mut out, *value);
    }
    out
}

fn encode_ranges(ranges: &[TableRange]) -> Vec<u8> {
    let mut out = Vec::with_capacity(ranges.len() * 8);
    for range in ranges {
        write_u32(&mut out, range.start);
        write_u32(&mut out, range.len);
    }
    out
}

fn write_color_vec(out: &mut Vec<u8>, colors: &[[f32; 4]]) {
    write_u32(out, colors.len() as u32);
    for color in colors {
        write_f32_array(out, color);
    }
}

fn write_f32_vec(out: &mut Vec<u8>, values: &[f32]) {
    write_u32(out, values.len() as u32);
    for value in values {
        write_f32(out, *value);
    }
}

fn write_f32_array<const N: usize>(out: &mut Vec<u8>, values: &[f32; N]) {
    for value in values {
        write_f32(out, *value);
    }
}

fn write_bytes_with_len(out: &mut Vec<u8>, bytes: &[u8]) {
    write_u32(out, bytes.len() as u32);
    out.extend_from_slice(bytes);
}

fn write_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

fn write_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_f32(out: &mut Vec<u8>, value: f32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn align4(value: u32) -> u32 {
    (value + 3) & !3
}

fn encode_paint_style(style: PaintStyle) -> u8 {
    match style {
        PaintStyle::Fill => 0,
        PaintStyle::Stroke => 1,
    }
}

fn encode_stroke_cap(cap: StrokeCap) -> u8 {
    match cap {
        StrokeCap::Butt => 0,
        StrokeCap::Round => 1,
        StrokeCap::Square => 2,
    }
}

fn encode_stroke_join(join: StrokeJoin) -> u8 {
    match join {
        StrokeJoin::Miter => 0,
        StrokeJoin::Round => 1,
        StrokeJoin::Bevel => 2,
    }
}

fn encode_blend_mode(mode: BlendMode) -> u8 {
    match mode {
        BlendMode::Clear => 0,
        BlendMode::Src => 1,
        BlendMode::Dst => 2,
        BlendMode::SrcOver => 3,
        BlendMode::DstOver => 4,
        BlendMode::SrcIn => 5,
        BlendMode::DstIn => 6,
        BlendMode::SrcOut => 7,
        BlendMode::DstOut => 8,
        BlendMode::SrcATop => 9,
        BlendMode::DstATop => 10,
        BlendMode::Xor => 11,
        BlendMode::Plus => 12,
        BlendMode::Modulate => 13,
        BlendMode::Screen => 14,
        BlendMode::Overlay => 15,
        BlendMode::Darken => 16,
        BlendMode::Lighten => 17,
        BlendMode::ColorDodge => 18,
        BlendMode::ColorBurn => 19,
        BlendMode::HardLight => 20,
        BlendMode::SoftLight => 21,
        BlendMode::Difference => 22,
        BlendMode::Exclusion => 23,
        BlendMode::Multiply => 24,
        BlendMode::Hue => 25,
        BlendMode::Saturation => 26,
        BlendMode::Color => 27,
        BlendMode::Luminosity => 28,
    }
}

fn encode_tile_mode(mode: TileMode) -> u8 {
    match mode {
        TileMode::Clamp => 0,
        TileMode::Repeat => 1,
        TileMode::Mirror => 2,
        TileMode::Decal => 3,
    }
}

fn encode_blur_style(style: BlurStyle) -> u8 {
    match style {
        BlurStyle::Normal => 0,
        BlurStyle::Inner => 1,
        BlurStyle::Solid => 2,
        BlurStyle::Outer => 3,
    }
}

fn encode_fill_type(fill_type: FillType) -> u8 {
    match fill_type {
        FillType::Winding => 0,
        FillType::EvenOdd => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencat_core::ir::draw_encoding::encode_draw_frame;
    use opencat_core::ir::draw_frame::DrawFrameScratch;

    /// Build a no-op draw frame and its binary encoding — enough to exercise
    /// the envelope layout through the real core `encode_draw_frame` path.
    fn empty_frames() -> (DrawOpFrame, EncodedDrawFrame) {
        let draw = DrawOpFrame::default();
        let mut scratch = DrawFrameScratch::default();
        let encoded = encode_draw_frame(&draw, &mut scratch);
        (draw, encoded)
    }

    fn rgba(value: u8, width: u32, height: u32) -> Arc<[u8]> {
        Arc::from(vec![value; width as usize * height as usize * 4])
    }

    /// Decode just enough of a v4 envelope to read the header epoch and the
    /// generated-image section (12), mirroring the JS decoder.
    struct DecodedEnvelope {
        epoch: u32,
        generated: Vec<GeneratedImageRecord>,
    }

    fn decode_envelope(bytes: &[u8]) -> DecodedEnvelope {
        assert_eq!(&bytes[0..4], b"OCIR", "magic");
        let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        assert_eq!(version, IR_VERSION, "version");
        let section_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let epoch = u32::from_le_bytes(bytes[12..16].try_into().unwrap());

        let mut sections: HashMap<u32, (usize, usize)> = HashMap::new();
        for i in 0..section_count {
            let base = 16 + i * 12;
            let id = u32::from_le_bytes(bytes[base..base + 4].try_into().unwrap());
            let offset =
                u32::from_le_bytes(bytes[base + 4..base + 8].try_into().unwrap()) as usize;
            let len = u32::from_le_bytes(bytes[base + 8..base + 12].try_into().unwrap()) as usize;
            sections.insert(id, (offset, len));
        }

        let generated = match sections.get(&SECTION_GENERATED_IMAGES) {
            Some((offset, len)) => decode_generated_images(&bytes[*offset..*offset + *len]),
            None => Vec::new(),
        };

        DecodedEnvelope { epoch, generated }
    }

    fn decode_generated_images(bytes: &[u8]) -> Vec<GeneratedImageRecord> {
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
            let rgba = Arc::from(
                bytes[cursor..cursor + rgba_len].to_vec(),
            );
            cursor += rgba_len;
            out.push(GeneratedImageRecord {
                id: GeneratedImageId(id),
                width,
                height,
                rgba,
            });
        }
        out
    }

    #[test]
    fn envelope_stamps_pipeline_epoch_in_header() {
        let (draw, encoded) = empty_frames();
        let bytes =
            encode_ir_envelope(&draw, &encoded, 7, &[]).expect("encode empty delta");
        let decoded = decode_envelope(&bytes);
        assert_eq!(decoded.epoch, 7);
        assert!(decoded.generated.is_empty(), "no delta => empty section 12");
    }

    #[test]
    fn generated_image_delta_round_trips_field_by_field() {
        let (draw, encoded) = empty_frames();
        let delta = vec![
            GeneratedImageRecord {
                id: GeneratedImageId(0x0123_4567_89ab_cdef),
                width: 3,
                height: 2,
                rgba: rgba(0xAB, 3, 2),
            },
            GeneratedImageRecord {
                id: GeneratedImageId(42),
                width: 1,
                height: 1,
                rgba: rgba(0x11, 1, 1),
            },
        ];
        let bytes = encode_ir_envelope(&draw, &encoded, 3, &delta).expect("encode delta");
        let decoded = decode_envelope(&bytes);

        assert_eq!(decoded.epoch, 3);
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
        // Section 12 is always present in v4 — even with no new glyphs it
        // carries a `count: 0` so the JS decoder can rely on its presence.
        let (draw, encoded) = empty_frames();
        let bytes = encode_ir_envelope(&draw, &encoded, 1, &[]).expect("encode");

        // Locate section 12 raw bytes and confirm a single zero u32.
        let section_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let mut section_12 = None;
        for i in 0..section_count {
            let base = 16 + i * 12;
            let id = u32::from_le_bytes(bytes[base..base + 4].try_into().unwrap());
            if id == SECTION_GENERATED_IMAGES {
                let offset =
                    u32::from_le_bytes(bytes[base + 4..base + 8].try_into().unwrap()) as usize;
                let len =
                    u32::from_le_bytes(bytes[base + 8..base + 12].try_into().unwrap()) as usize;
                section_12 = Some(&bytes[offset..offset + len]);
            }
        }
        let section = section_12.expect("section 12 present");
        assert_eq!(section.len(), 4, "empty delta is exactly one u32 count");
        assert_eq!(u32::from_le_bytes(section.try_into().unwrap()), 0);
    }

    /// Delta semantics: a glyph published in frame 0 must not be re-published
    /// in frame 1. This mirrors the bookkeeping `build_frame_ir` does via the
    /// `published_generated` set, but tested at the pure encoder level by
    /// feeding a shrinking delta.
    #[test]
    fn delta_only_carries_newly_published_glyphs() {
        let (draw, encoded) = empty_frames();
        let glyph = GeneratedImageRecord {
            id: GeneratedImageId(99),
            width: 2,
            height: 2,
            rgba: rgba(0x55, 2, 2),
        };

        // Frame 0: glyph 99 is new → published.
        let frame0 = encode_ir_envelope(&draw, &encoded, 1, &[glyph.clone()]).unwrap();
        assert_eq!(decode_envelope(&frame0).generated.len(), 1);

        // Frame 1: glyph 99 already published → delta is empty.
        let frame1 = encode_ir_envelope(&draw, &encoded, 1, &[]).unwrap();
        assert!(decode_envelope(&frame1).generated.is_empty());
    }

    /// A new epoch republishes: even though the glyph id is the same, a
    /// different epoch means JS has discarded its cache, so the encoder must
    /// be free to (and the decoder must correctly read) a fresh delta.
    #[test]
    fn epoch_change_allows_republish_of_same_glyph() {
        let (draw, encoded) = empty_frames();
        let glyph = GeneratedImageRecord {
            id: GeneratedImageId(7),
            width: 1,
            height: 1,
            rgba: rgba(0xFF, 1, 1),
        };
        let bytes = encode_ir_envelope(&draw, &encoded, 5, &[glyph]).unwrap();
        let decoded = decode_envelope(&bytes);
        assert_eq!(decoded.epoch, 5);
        assert_eq!(decoded.generated.len(), 1);
        assert_eq!(decoded.generated[0].id, GeneratedImageId(7));
    }

    /// Web host contract (#15): draft requirements' AssetId is the only id used
    /// when inserting image metadata — never re-derived from locator.
    #[test]
    fn web_host_static_image_uses_request_asset_id() {
        use opencat_core::lifecycle::{CompositionDraft, HostInputs, ResourceKind, ResourceLocator};
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
        assert_eq!(req.asset_id.0, "hero.png");
        assert!(matches!(
            &req.locator,
            ResourceLocator::LogicalPath(p) if p == "hero.png"
        ));

        let id = req.asset_id.clone();
        let mut inputs = HostInputs::empty().with_font_db(Arc::new(
            opencat_core::text::empty_font_db(),
        ));
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
        use opencat_core::lifecycle::{CompositionDraft, HostInputs, ResourceKind, ResourceLocator};
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
        assert_eq!(req.asset_id.0, "lottie:loader");
        assert!(matches!(
            &req.locator,
            ResourceLocator::LogicalPath(p) if p == "anim/loader.json"
        ));

        let id = req.asset_id.clone();
        let mut inputs = HostInputs::empty().with_font_db(Arc::new(
            opencat_core::text::empty_font_db(),
        ));
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
                .any(|b| b == "lottie:loader"),
            "web host media plan must list Lottie bundle; got {:?}",
            frame.media.lottie_bundles
        );
        assert!(
            frame.media.images.is_empty(),
            "Lottie must not be disguised as image"
        );
        assert!(
            frame.draw.ops.iter().any(|op| {
                matches!(op, DrawOp::LottieRect { bundle_id, .. } if bundle_id == "lottie:loader")
            }),
            "draw must emit LottieRect"
        );
    }

}
