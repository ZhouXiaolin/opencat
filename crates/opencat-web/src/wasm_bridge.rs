//! wasm-bindgen bridge: build each frame as one binary DrawOp IR blob.

use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

use opencat_core::canvas::paint::{
    BlendMode, BlurStyle, ColorFilterSpec, FillSpec, ImageFilterSpec, MaskFilterSpec, PaintSpec,
    PaintStyle, PathEffectSpec, ShaderSpec as PaintShaderSpec, StrokeCap, StrokeJoin, TileMode,
};
use opencat_core::ir::draw_encoding::EncodedDrawFrame;
use opencat_core::ir::draw_frame::{DrawFrameScratch, DrawOpFrame};
use opencat_core::ir::draw_op::DrawOp;
use opencat_core::ir::draw_types::{
    EffectRef, EncodedPath, FillType, ImageRef, PathOp, RuntimeEffectChildRef, ShaderSpec,
    ShaderType, TableRange,
};
use opencat_core::parse::composition::Composition;
use opencat_core::resource::asset_id::AssetId;
use opencat_core::resource::hash_map_catalog::HashMapResourceCatalog;
use opencat_core::runtime::pipeline::render_frame;
use opencat_core::runtime::session::RenderSession;

use crate::codec::audio::WebAudio;
use crate::script::ScriptRuntimeCache;

const IR_MAGIC: &[u8; 4] = b"OCIR";
const IR_VERSION: u32 = 1;

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

#[wasm_bindgen]
pub struct WebRenderer {
    session: RenderSession,
    script: ScriptRuntimeCache,
    scratch: DrawFrameScratch,
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
            session: RenderSession::new(),
            script: ScriptRuntimeCache::default(),
            scratch: DrawFrameScratch::default(),
            audio,
            blobs: crate::resource::blob_store::BlobStore::new(),
        })
    }

    pub fn build_frame_ir(
        &mut self,
        jsonl: &str,
        frame: u32,
        resources_json: &str,
    ) -> Result<Vec<u8>, JsValue> {
        #[cfg(feature = "profile")]
        tracing::info!(frame, "build_frame_ir start");

        let parsed = opencat_core::parse::jsonl::parse_with_base_dir(jsonl, None)
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

        let blob_store_ref: &dyn opencat_core::resource::BlobStore = &self.blobs;
        let (mut draw, media_plan) = render_frame(
            &composition,
            frame,
            &mut self.session,
            &mut self.script,
            Some(blob_store_ref),
        )
        .map_err(|e| JsValue::from_str(&format!("render_frame: {e}")))?;

        use opencat_core::platform::frame_consumer::FrameConsumer;

        let header = opencat_core::platform::frame_consumer::RenderSessionHeader {
            composition_size: (parsed.width as u32, parsed.height as u32),
            fps: parsed.fps as u32,
            frames: parsed.frames as u32,
        };

        let mut consumer = crate::consumer::WebFrameConsumer { scratch: &mut self.scratch };
        consumer.consume_frame(&header, &mut draw, &media_plan)
            .map_err(JsValue::from)
    }

    // Retained for API compatibility. Video frames now live in JS-side caches
    // and are consumed by the DrawOp IR renderer.
    pub fn inject_video_frame(
        &mut self,
        _asset_id: String,
        _frame: u32,
        _rgba: Vec<u8>,
        _width: u32,
        _height: u32,
    ) {
    }

    pub fn clear_video_cache(&mut self, _asset_id: String) {}

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

        let parsed = opencat_core::parse::jsonl::parse_with_base_dir(jsonl, None)
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
        let mut plan: Vec<Value> = Vec::new();

        fn walk(
            node: &opencat_core::parse::node::Node,
            ctx: &FrameCtx,
            composition_time_secs: f64,
            catalog: &HashMapResourceCatalog,
            out: &mut Vec<Value>,
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
                    out.push(json!({
                        "assetId": asset_id.0,
                        "localTimeSecs": request.resolve_time_secs(&info),
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
            out: &mut Vec<Value>,
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

    pub fn inject_image_bytes(&mut self, asset_id: String, bytes: Vec<u8>) {
        self.blobs
            .insert(AssetId(asset_id), std::sync::Arc::from(bytes));
    }

    pub fn clear_image_blobs(&mut self) {
        self.blobs.clear();
    }
}

pub(crate) fn encode_ir_envelope(
    draw: &DrawOpFrame,
    encoded: &EncodedDrawFrame,
) -> Result<Vec<u8>, JsValue> {
    let sections = [
        (SECTION_OPS, encoded.ops.clone()),
        (SECTION_F32_POOL, encode_f32_slice(&encoded.f32_pool)),
        (SECTION_BYTES, draw.bytes.clone()),
        (SECTION_BYTE_RANGES, encode_ranges(&draw.byte_ranges)),
        (SECTION_STRINGS_UTF8, encoded.strings_utf8.clone()),
        (SECTION_STRING_RANGES, encode_ranges(&encoded.string_ranges)),
        (SECTION_PAINTS, encode_paints(&draw.paints)?),
        (SECTION_PATHS, encode_paths(&draw.paths)),
        (SECTION_CHILDREN, encode_children(&draw.children, &draw.strings)?),
        (SECTION_EFFECTS, encode_effects(&draw.effects)),
    ];

    let header_len = 12 + sections.len() * 12;
    let mut offsets = Vec::with_capacity(sections.len());
    let mut cursor = header_len as u32;
    for (_, bytes) in &sections {
        cursor = align4(cursor);
        offsets.push(cursor);
        cursor = cursor
            .checked_add(bytes.len() as u32)
            .ok_or_else(|| JsValue::from_str("IR envelope too large"))?;
    }

    let mut out = Vec::with_capacity(cursor as usize);
    out.extend_from_slice(IR_MAGIC);
    write_u32(&mut out, IR_VERSION);
    write_u32(&mut out, sections.len() as u32);
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

pub(crate) fn intern_image_strings(draw: &mut DrawOpFrame) {
    fn push_unique(strings: &mut Vec<String>, asset_id: &str) {
        if !strings.iter().any(|item| item == asset_id) {
            strings.push(asset_id.to_string());
        }
    }

    for op in &draw.ops {
        match op {
            DrawOp::Image { image, .. } | DrawOp::ImageRect { image, .. } => {
                intern_image_ref(&mut draw.strings, image);
            }
            _ => {}
        }
    }
    for child in &draw.children {
        if let RuntimeEffectChildRef::Image(image) = child {
            intern_image_ref(&mut draw.strings, image);
        }
    }

    fn intern_image_ref(strings: &mut Vec<String>, image: &ImageRef) {
        match image {
            ImageRef::Static { asset_id } | ImageRef::VideoFrame { asset_id, .. } => {
                push_unique(strings, asset_id);
            }
        }
    }
}

fn encode_paints(paints: &[PaintSpec]) -> Result<Vec<u8>, JsValue> {
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

fn encode_paint(out: &mut Vec<u8>, paint: &PaintSpec) -> Result<(), JsValue> {
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
        } => {
            write_u8(out, 0);
            write_u8(out, encode_tile_mode(*tile_mode));
            write_f32_array(out, from);
            write_f32_array(out, to);
            write_f32_vec(out, stops);
            write_color_vec(out, colors);
        }
        PaintShaderSpec::RadialGradient {
            center,
            radius,
            stops,
            colors,
            tile_mode,
        } => {
            write_u8(out, 1);
            write_u8(out, encode_tile_mode(*tile_mode));
            write_f32_array(out, center);
            write_f32(out, *radius);
            write_f32_vec(out, stops);
            write_color_vec(out, colors);
        }
    }
}

fn encode_image_filter(out: &mut Vec<u8>, filter: &ImageFilterSpec) -> Result<(), JsValue> {
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

fn encode_color_filter(out: &mut Vec<u8>, filter: &ColorFilterSpec) -> Result<(), JsValue> {
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

fn encode_mask_filter(out: &mut Vec<u8>, filter: &MaskFilterSpec) -> Result<(), JsValue> {
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

fn encode_path_effect(out: &mut Vec<u8>, effect: &PathEffectSpec) -> Result<(), JsValue> {
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

fn encode_children(children: &[RuntimeEffectChildRef], strings: &[String]) -> Result<Vec<u8>, JsValue> {
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

fn encode_image_ref(out: &mut Vec<u8>, image: &ImageRef, strings: &[String]) -> Result<(), JsValue> {
    match image {
        ImageRef::Static { asset_id } => {
            write_u8(out, 0);
            write_u32(out, lookup_string_id(strings, asset_id)?);
            write_u32(out, 0);
        }
        ImageRef::VideoFrame {
            asset_id,
            frame_index,
        } => {
            write_u8(out, 1);
            write_u32(out, lookup_string_id(strings, asset_id)?);
            write_u32(out, *frame_index);
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
    encode: fn(&mut Vec<u8>, &T) -> Result<(), JsValue>,
) -> Result<(), JsValue> {
    match value {
        Some(value) => {
            write_u8(out, 1);
            encode(out, value)?;
        }
        None => write_u8(out, 0),
    }
    Ok(())
}

fn lookup_string_id(strings: &[String], s: &str) -> Result<u32, JsValue> {
    strings
        .iter()
        .position(|item| item == s)
        .map(|idx| idx as u32)
        .ok_or_else(|| JsValue::from_str(&format!("asset_id not interned in IR strings: {s}")))
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
