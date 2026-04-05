use anyhow::{Context, Result, anyhow};
use ffmpeg_next as ffmpeg;
use ffmpeg_next::{
    Dictionary, codec,
    codec::packet::Packet,
    format,
    software::scaling::{context::Context as ScalingContext, flag::Flags as ScalingFlags},
    util::{format::pixel::Pixel, frame::video::Video, rational::Rational},
};
use skia_safe::{
    AlphaType, ColorType, EncodedImageFormat, ImageInfo, Picture, image::CachingHint, surfaces,
};
use std::{path::Path, ptr, time::Instant};

use crate::{
    Composition, FrameCtx, Node,
    assets::AssetsMap,
    backend::{
        skia::{
            BackendProfile, SkiaBackend, display_list_uses_video, new_image_cache,
            new_text_picture_cache, record_display_list_picture,
        },
        skia_transition,
    },
    display::{build::build_display_list, list::DisplayList},
    element::resolve::resolve_ui_tree,
    layout::{LayoutPassStats, LayoutSession},
    media::MediaContext,
    script::{ScriptRunner, StyleMutations},
    timeline::{FrameState, frame_state_for_root},
};
use std::sync::Arc;

pub enum OutputFormat {
    Mp4(Mp4Config),
    Png,
}

pub struct Mp4Config {
    pub crf: u8,
    pub preset: String,
}

impl Default for Mp4Config {
    fn default() -> Self {
        Self {
            crf: 18,
            preset: "fast".to_string(),
        }
    }
}

pub struct EncodingConfig {
    pub format: OutputFormat,
}

pub struct RenderSession {
    media_ctx: MediaContext,
    assets: AssetsMap,
    image_cache: crate::backend::skia::ImageCache,
    text_picture_cache: crate::backend::skia::TextPictureCache,
    script_runner: Option<ScriptRunner>,
    script_driver_ptr: Option<usize>,
    scene_layout: LayoutSession,
    transition_from_layout: LayoutSession,
    transition_to_layout: LayoutSession,
    scene_picture_cache: PictureSlotCache,
    transition_from_picture_cache: PictureSlotCache,
    transition_to_picture_cache: PictureSlotCache,
    profiler: RenderProfiler,
}

#[derive(Clone, Copy)]
enum SceneSlot {
    Scene,
    TransitionFrom,
    TransitionTo,
}

#[derive(Default)]
struct PictureSlotCache {
    picture: Option<Picture>,
}

#[derive(Default)]
struct SceneBuildStats {
    resolve_ms: f64,
    layout_ms: f64,
    display_ms: f64,
    layout_pass: LayoutPassStats,
    contains_video: bool,
}

#[derive(Default)]
struct FrameProfile {
    script_ms: f64,
    frame_state_ms: f64,
    resolve_ms: f64,
    layout_ms: f64,
    display_ms: f64,
    backend_ms: f64,
    transition_ms: f64,
    slide_transition_ms: f64,
    light_leak_transition_ms: f64,
    slide_transition_frames: usize,
    light_leak_transition_frames: usize,
    reused_nodes: usize,
    layout_dirty_nodes: usize,
    paint_only_nodes: usize,
    structure_rebuilds: usize,
    backend: BackendProfile,
}

#[derive(Default)]
struct RenderProfiler {
    frames: Vec<FrameProfile>,
}

impl EncodingConfig {
    pub fn mp4() -> Self {
        Self {
            format: OutputFormat::Mp4(Mp4Config::default()),
        }
    }

    pub fn mp4_with(config: Mp4Config) -> Self {
        Self {
            format: OutputFormat::Mp4(config),
        }
    }

    pub fn png() -> Self {
        Self {
            format: OutputFormat::Png,
        }
    }
}

impl RenderSession {
    pub fn new() -> Self {
        Self {
            media_ctx: MediaContext::new(),
            assets: AssetsMap::new(),
            image_cache: new_image_cache(),
            text_picture_cache: new_text_picture_cache(),
            script_runner: None,
            script_driver_ptr: None,
            scene_layout: LayoutSession::new(),
            transition_from_layout: LayoutSession::new(),
            transition_to_layout: LayoutSession::new(),
            scene_picture_cache: PictureSlotCache::default(),
            transition_from_picture_cache: PictureSlotCache::default(),
            transition_to_picture_cache: PictureSlotCache::default(),
            profiler: RenderProfiler::default(),
        }
    }

    fn layout_session_mut(&mut self, slot: SceneSlot) -> &mut LayoutSession {
        match slot {
            SceneSlot::Scene => &mut self.scene_layout,
            SceneSlot::TransitionFrom => &mut self.transition_from_layout,
            SceneSlot::TransitionTo => &mut self.transition_to_layout,
        }
    }

    fn picture_cache(&self, slot: SceneSlot) -> &PictureSlotCache {
        match slot {
            SceneSlot::Scene => &self.scene_picture_cache,
            SceneSlot::TransitionFrom => &self.transition_from_picture_cache,
            SceneSlot::TransitionTo => &self.transition_to_picture_cache,
        }
    }

    fn picture_cache_mut(&mut self, slot: SceneSlot) -> &mut PictureSlotCache {
        match slot {
            SceneSlot::Scene => &mut self.scene_picture_cache,
            SceneSlot::TransitionFrom => &mut self.transition_from_picture_cache,
            SceneSlot::TransitionTo => &mut self.transition_to_picture_cache,
        }
    }
}

impl Composition {
    pub fn render(&self, output_path: impl AsRef<Path>, config: &EncodingConfig) -> Result<()> {
        match &config.format {
            OutputFormat::Mp4(mp4_config) => render_mp4(self, output_path, mp4_config),
            OutputFormat::Png => render_png(self, output_path),
        }
    }
}

fn render_png(composition: &Composition, output_path: impl AsRef<Path>) -> Result<()> {
    let mut session = RenderSession::new();
    let mut surface = render_frame_surface(composition, 0, &mut session)?;
    let image = surface.image_snapshot();
    let data = image
        .encode(None, EncodedImageFormat::PNG, 100)
        .ok_or_else(|| anyhow!("failed to encode PNG"))?;
    std::fs::write(output_path, &*data)?;
    session.profiler.print_summary();
    Ok(())
}

fn render_mp4(
    composition: &Composition,
    output_path: impl AsRef<Path>,
    config: &Mp4Config,
) -> Result<()> {
    ffmpeg::init()?;

    let mut session = RenderSession::new();

    let output_path = output_path.as_ref();
    let mut output = format::output(output_path).with_context(|| {
        format!(
            "failed to create output context for {}",
            output_path.display()
        )
    })?;

    let codec = ffmpeg::encoder::find(codec::Id::H264)
        .ok_or_else(|| anyhow!("H264 encoder not found in local ffmpeg"))?;

    let nominal_time_base = Rational(1, composition.fps as i32);
    let stream_time_base = Rational(1, 90_000);

    let mut encoder_ctx = ffmpeg::codec::context::Context::new_with_codec(codec)
        .encoder()
        .video()?;

    encoder_ctx.set_width(composition.width as u32);
    encoder_ctx.set_height(composition.height as u32);
    encoder_ctx.set_format(Pixel::YUV420P);
    encoder_ctx.set_time_base(nominal_time_base);
    encoder_ctx.set_frame_rate(Some(Rational(composition.fps as i32, 1)));

    if output
        .format()
        .flags()
        .contains(format::flag::Flags::GLOBAL_HEADER)
    {
        encoder_ctx.set_flags(codec::flag::Flags::GLOBAL_HEADER);
    }

    let mut encode_options = Dictionary::new();
    encode_options.set("crf", &config.crf.to_string());
    encode_options.set("preset", &config.preset);
    let mut encoder = encoder_ctx.open_as_with(codec, encode_options)?;
    let packet_time_base = nominal_time_base;
    let frame_duration = 1_i64;

    let stream_index = {
        let mut stream = output.add_stream(codec)?;
        stream.set_time_base(stream_time_base);
        stream.set_rate(Rational(composition.fps as i32, 1));
        stream.set_avg_frame_rate(Rational(composition.fps as i32, 1));
        stream.set_parameters(&encoder);
        stream.index()
    };

    output.write_header()?;

    let mut scaler = ScalingContext::get(
        Pixel::RGBA,
        composition.width as u32,
        composition.height as u32,
        Pixel::YUV420P,
        composition.width as u32,
        composition.height as u32,
        ScalingFlags::BILINEAR,
    )?;

    for frame_index in 0..composition.frames {
        let rgba = render_frame_rgba(composition, frame_index, &mut session)?;

        let mut rgba_frame = Video::new(
            Pixel::RGBA,
            composition.width as u32,
            composition.height as u32,
        );
        write_rgba_to_frame_ptr(
            &rgba,
            &mut rgba_frame,
            composition.width as usize,
            composition.height as usize,
        );

        let mut yuv_frame = Video::new(
            Pixel::YUV420P,
            composition.width as u32,
            composition.height as u32,
        );
        scaler.run(&rgba_frame, &mut yuv_frame)?;
        yuv_frame.set_pts(Some(frame_index as i64));

        encoder.send_frame(&yuv_frame)?;
        write_encoded_packets(
            &mut encoder,
            &mut output,
            stream_index,
            packet_time_base,
            stream_time_base,
            frame_duration,
        )?;
    }

    encoder.send_eof()?;
    write_encoded_packets(
        &mut encoder,
        &mut output,
        stream_index,
        packet_time_base,
        stream_time_base,
        frame_duration,
    )?;

    output.write_trailer()?;
    session.profiler.print_summary();
    Ok(())
}

fn render_frame_surface(
    composition: &Composition,
    frame_index: u32,
    session: &mut RenderSession,
) -> Result<skia_safe::Surface> {
    let mut frame_profile = FrameProfile::default();
    let frame_ctx = FrameCtx {
        frame: frame_index,
        fps: composition.fps,
        width: composition.width,
        height: composition.height,
        frames: composition.frames,
    };

    let driver_ptr = composition
        .script_driver
        .as_ref()
        .map(|driver| Arc::as_ptr(driver) as usize);
    if session.script_driver_ptr != driver_ptr {
        session.script_runner = composition
            .script_driver
            .as_ref()
            .map(|driver| driver.create_runner())
            .transpose()?;
        session.script_driver_ptr = driver_ptr;
    }

    let script_started = Instant::now();
    let mutations = session
        .script_runner
        .as_mut()
        .map(|runner| runner.run(frame_index, composition.frames))
        .transpose()?;
    frame_profile.script_ms = script_started.elapsed().as_secs_f64() * 1000.0;

    let mut surface = surfaces::raster_n32_premul((composition.width, composition.height))
        .ok_or_else(|| anyhow!("failed to create skia raster surface"))?;
    let canvas = surface.canvas();
    let root = composition.root_node(&frame_ctx);

    let frame_state_started = Instant::now();
    let frame_state = frame_state_for_root(&root, &frame_ctx);
    frame_profile.frame_state_ms = frame_state_started.elapsed().as_secs_f64() * 1000.0;

    match frame_state {
        FrameState::Scene { scene } => {
            let (display_list, scene_stats) = build_scene_display_list_with_slot(
                &scene,
                &frame_ctx,
                session,
                mutations.as_ref(),
                SceneSlot::Scene,
            )?;
            frame_profile.merge_scene_stats(&scene_stats);

            let backend_started = Instant::now();
            let mut backend_profile = BackendProfile::default();

            if let Some(picture) = picture_for_slot(
                session,
                SceneSlot::Scene,
                &display_list,
                &scene_stats,
                composition.width,
                composition.height,
                &frame_ctx,
                &mut backend_profile,
                false,
            )? {
                let picture_draw_started = Instant::now();
                canvas.draw_picture(&picture, None, None);
                backend_profile.picture_draw_ms +=
                    picture_draw_started.elapsed().as_secs_f64() * 1000.0;
            } else {
                let mut backend = SkiaBackend::new_with_cache_and_profile(
                    canvas,
                    composition.width,
                    composition.height,
                    &session.assets,
                    session.image_cache.clone(),
                    session.text_picture_cache.clone(),
                    Some(&mut session.media_ctx),
                    &frame_ctx,
                    Some(&mut backend_profile),
                );
                backend.execute(&display_list)?;
            }

            frame_profile.backend_ms = backend_started.elapsed().as_secs_f64() * 1000.0;
            frame_profile.merge_backend_profile(&backend_profile);
        }
        FrameState::Transition {
            from,
            to,
            progress,
            kind,
        } => {
            let (from_display, from_stats) = build_scene_display_list_with_slot(
                &from,
                &frame_ctx,
                session,
                mutations.as_ref(),
                SceneSlot::TransitionFrom,
            )?;
            let (to_display, to_stats) = build_scene_display_list_with_slot(
                &to,
                &frame_ctx,
                session,
                mutations.as_ref(),
                SceneSlot::TransitionTo,
            )?;
            frame_profile.merge_scene_stats(&from_stats);
            frame_profile.merge_scene_stats(&to_stats);

            let backend_started = Instant::now();
            let mut backend_profile = BackendProfile::default();
            let from_picture = picture_for_slot(
                session,
                SceneSlot::TransitionFrom,
                &from_display,
                &from_stats,
                composition.width,
                composition.height,
                &frame_ctx,
                &mut backend_profile,
                true,
            )?
            .expect("transition source picture should exist");
            let to_picture = picture_for_slot(
                session,
                SceneSlot::TransitionTo,
                &to_display,
                &to_stats,
                composition.width,
                composition.height,
                &frame_ctx,
                &mut backend_profile,
                true,
            )?
            .expect("transition target picture should exist");
            frame_profile.backend_ms = backend_started.elapsed().as_secs_f64() * 1000.0;
            frame_profile.merge_backend_profile(&backend_profile);

            let transition_started = Instant::now();
            skia_transition::draw_transition(
                canvas,
                &from_picture,
                &to_picture,
                progress,
                kind,
                composition.width,
                composition.height,
            )?;
            let transition_ms = transition_started.elapsed().as_secs_f64() * 1000.0;
            frame_profile.transition_ms = transition_ms;
            match kind {
                crate::transitions::TransitionKind::Slide => {
                    frame_profile.slide_transition_ms = transition_ms;
                    frame_profile.slide_transition_frames = 1;
                }
                crate::transitions::TransitionKind::LightLeak(_) => {
                    frame_profile.light_leak_transition_ms = transition_ms;
                    frame_profile.light_leak_transition_frames = 1;
                }
            }
        }
    }

    session.profiler.push(frame_profile);
    Ok(surface)
}

pub fn render_frame_rgba(
    composition: &Composition,
    frame_index: u32,
    session: &mut RenderSession,
) -> Result<Vec<u8>> {
    let mut surface = render_frame_surface(composition, frame_index, session)?;
    let image = surface.image_snapshot();
    let image_info = ImageInfo::new(
        (composition.width, composition.height),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );

    let mut rgba = vec![0_u8; (composition.width as usize) * (composition.height as usize) * 4];
    let read_ok = image.read_pixels(
        &image_info,
        rgba.as_mut_slice(),
        (composition.width as usize) * 4,
        (0, 0),
        CachingHint::Allow,
    );

    if !read_ok {
        return Err(anyhow!("failed to read pixels from skia surface"));
    }

    Ok(rgba)
}

pub fn render_frame_rgb(
    composition: &Composition,
    frame_index: u32,
    session: &mut RenderSession,
) -> Result<Vec<u8>> {
    let rgba = render_frame_rgba(composition, frame_index, session)?;
    let mut rgb = vec![0_u8; (composition.width as usize) * (composition.height as usize) * 3];
    for (src, dst) in rgba.chunks_exact(4).zip(rgb.chunks_exact_mut(3)) {
        dst[0] = src[0];
        dst[1] = src[1];
        dst[2] = src[2];
    }
    Ok(rgb)
}

impl Default for RenderSession {
    fn default() -> Self {
        Self::new()
    }
}

fn build_scene_display_list_with_slot(
    scene: &Node,
    frame_ctx: &FrameCtx,
    session: &mut RenderSession,
    mutations: Option<&StyleMutations>,
    slot: SceneSlot,
) -> Result<(DisplayList, SceneBuildStats)> {
    let mut stats = SceneBuildStats::default();

    let resolve_started = Instant::now();
    let element_root = resolve_ui_tree(
        scene,
        frame_ctx,
        &mut session.media_ctx,
        &mut session.assets,
        mutations,
    );
    stats.resolve_ms = resolve_started.elapsed().as_secs_f64() * 1000.0;

    let layout_started = Instant::now();
    let (layout_tree, layout_pass) = session
        .layout_session_mut(slot)
        .compute_layout(&element_root, frame_ctx)?;
    stats.layout_ms = layout_started.elapsed().as_secs_f64() * 1000.0;
    stats.layout_pass = layout_pass;

    let display_started = Instant::now();
    let display_list = build_display_list(&layout_tree)?;
    stats.display_ms = display_started.elapsed().as_secs_f64() * 1000.0;
    stats.contains_video = display_list_uses_video(&display_list, &session.assets);

    Ok((display_list, stats))
}

fn picture_for_slot(
    session: &mut RenderSession,
    slot: SceneSlot,
    display_list: &DisplayList,
    scene_stats: &SceneBuildStats,
    width: i32,
    height: i32,
    frame_ctx: &FrameCtx,
    backend_profile: &mut BackendProfile,
    require_picture: bool,
) -> Result<Option<Picture>> {
    if scene_stats.contains_video {
        session.picture_cache_mut(slot).picture = None;
        if !require_picture {
            return Ok(None);
        }
        let picture = record_display_list_picture(
            display_list,
            width,
            height,
            &session.assets,
            session.image_cache.clone(),
            session.text_picture_cache.clone(),
            Some(&mut session.media_ctx),
            frame_ctx,
            Some(backend_profile),
        )?;
        return Ok(Some(picture));
    }

    if layout_pass_is_clean(&scene_stats.layout_pass) {
        if let Some(picture) = session.picture_cache(slot).picture.clone() {
            backend_profile.picture_cache_hits += 1;
            return Ok(Some(picture));
        }

        let picture = record_display_list_picture(
            display_list,
            width,
            height,
            &session.assets,
            session.image_cache.clone(),
            session.text_picture_cache.clone(),
            Some(&mut session.media_ctx),
            frame_ctx,
            Some(backend_profile),
        )?;
        backend_profile.picture_cache_misses += 1;
        session.picture_cache_mut(slot).picture = Some(picture.clone());
        return Ok(Some(picture));
    }

    session.picture_cache_mut(slot).picture = None;
    if !require_picture {
        return Ok(None);
    }

    let picture = record_display_list_picture(
        display_list,
        width,
        height,
        &session.assets,
        session.image_cache.clone(),
        session.text_picture_cache.clone(),
        Some(&mut session.media_ctx),
        frame_ctx,
        Some(backend_profile),
    )?;
    Ok(Some(picture))
}

fn layout_pass_is_clean(stats: &LayoutPassStats) -> bool {
    !stats.structure_rebuild && stats.layout_dirty_nodes == 0 && stats.paint_only_nodes == 0
}

fn write_rgba_to_frame_ptr(rgba: &[u8], frame: &mut Video, width: usize, height: usize) {
    let stride = frame.stride(0);
    let row_len = width * 4;
    let data_ptr = frame.data_mut(0).as_mut_ptr();

    for y in 0..height {
        let src_start = y * row_len;
        let dst_start = y * stride;
        unsafe {
            ptr::copy_nonoverlapping(
                rgba.as_ptr().add(src_start),
                data_ptr.add(dst_start),
                row_len,
            );
        }
    }
}

impl FrameProfile {
    fn merge_scene_stats(&mut self, stats: &SceneBuildStats) {
        self.resolve_ms += stats.resolve_ms;
        self.layout_ms += stats.layout_ms;
        self.display_ms += stats.display_ms;
        self.reused_nodes += stats.layout_pass.reused_nodes;
        self.layout_dirty_nodes += stats.layout_pass.layout_dirty_nodes;
        self.paint_only_nodes += stats.layout_pass.paint_only_nodes;
        self.structure_rebuilds += usize::from(stats.layout_pass.structure_rebuild);
    }

    fn merge_backend_profile(&mut self, profile: &BackendProfile) {
        self.backend.rect_draw_ms += profile.rect_draw_ms;
        self.backend.text_draw_ms += profile.text_draw_ms;
        self.backend.text_picture_record_ms += profile.text_picture_record_ms;
        self.backend.text_picture_draw_ms += profile.text_picture_draw_ms;
        self.backend.bitmap_draw_ms += profile.bitmap_draw_ms;
        self.backend.image_decode_ms += profile.image_decode_ms;
        self.backend.video_decode_ms += profile.video_decode_ms;
        self.backend.picture_record_ms += profile.picture_record_ms;
        self.backend.picture_draw_ms += profile.picture_draw_ms;
        self.backend.picture_cache_hits += profile.picture_cache_hits;
        self.backend.picture_cache_misses += profile.picture_cache_misses;
        self.backend.text_cache_hits += profile.text_cache_hits;
        self.backend.text_cache_misses += profile.text_cache_misses;
        self.backend.image_cache_hits += profile.image_cache_hits;
        self.backend.image_cache_misses += profile.image_cache_misses;
        self.backend.video_frame_decodes += profile.video_frame_decodes;
        self.backend.draw_rect_count += profile.draw_rect_count;
        self.backend.draw_text_count += profile.draw_text_count;
        self.backend.draw_bitmap_count += profile.draw_bitmap_count;
        self.backend.save_layer_count += profile.save_layer_count;
    }
}

impl RenderProfiler {
    fn push(&mut self, frame: FrameProfile) {
        self.frames.push(frame);
    }

    fn print_summary(&self) {
        if self.frames.is_empty() {
            return;
        }

        eprintln!("Render profile:");
        eprintln!("  frames: {}", self.frames.len());
        eprintln!(
            "  avg ms/frame: script {:.2}, frame_state {:.2}, resolve {:.2}, layout {:.2}, display {:.2}, backend {:.2}, transition {:.2}",
            average(&self.frames, |frame| frame.script_ms),
            average(&self.frames, |frame| frame.frame_state_ms),
            average(&self.frames, |frame| frame.resolve_ms),
            average(&self.frames, |frame| frame.layout_ms),
            average(&self.frames, |frame| frame.display_ms),
            average(&self.frames, |frame| frame.backend_ms),
            average(&self.frames, |frame| frame.transition_ms),
        );
        eprintln!(
            "  p95 ms/frame: resolve {:.2}, layout {:.2}, display {:.2}, backend {:.2}, transition {:.2}",
            percentile_95(&self.frames, |frame| frame.resolve_ms),
            percentile_95(&self.frames, |frame| frame.layout_ms),
            percentile_95(&self.frames, |frame| frame.display_ms),
            percentile_95(&self.frames, |frame| frame.backend_ms),
            percentile_95(&self.frames, |frame| frame.transition_ms),
        );
        eprintln!(
            "  transition avg ms/active-frame: slide {:.2} ({} frames), light_leak {:.2} ({} frames)",
            average_when_counted(
                &self.frames,
                |frame| frame.slide_transition_ms,
                |frame| frame.slide_transition_frames,
            ),
            self.frames
                .iter()
                .map(|frame| frame.slide_transition_frames)
                .sum::<usize>(),
            average_when_counted(
                &self.frames,
                |frame| frame.light_leak_transition_ms,
                |frame| frame.light_leak_transition_frames,
            ),
            self.frames
                .iter()
                .map(|frame| frame.light_leak_transition_frames)
                .sum::<usize>(),
        );
        eprintln!(
            "  avg nodes/frame: reused {:.1}, layout_dirty {:.1}, paint_only {:.1}, structure_rebuilds {:.2}",
            average_usize(&self.frames, |frame| frame.reused_nodes),
            average_usize(&self.frames, |frame| frame.layout_dirty_nodes),
            average_usize(&self.frames, |frame| frame.paint_only_nodes),
            average_usize(&self.frames, |frame| frame.structure_rebuilds),
        );
        eprintln!(
            "  backend avg ms/frame: rect {:.2}, text {:.2}, text_record {:.2}, text_pic_draw {:.2}, bitmap {:.2}, image_decode {:.2}, video_decode {:.2}, picture_record {:.2}, picture_draw {:.2}",
            average(&self.frames, |frame| frame.backend.rect_draw_ms),
            average(&self.frames, |frame| frame.backend.text_draw_ms),
            average(&self.frames, |frame| frame.backend.text_picture_record_ms),
            average(&self.frames, |frame| frame.backend.text_picture_draw_ms),
            average(&self.frames, |frame| frame.backend.bitmap_draw_ms),
            average(&self.frames, |frame| frame.backend.image_decode_ms),
            average(&self.frames, |frame| frame.backend.video_decode_ms),
            average(&self.frames, |frame| frame.backend.picture_record_ms),
            average(&self.frames, |frame| frame.backend.picture_draw_ms),
        );
        eprintln!(
            "  backend avg counts/frame: rect {:.1}, text {:.1}, bitmap {:.1}, save_layer {:.1}, text_hit {:.2}, text_miss {:.2}, pic_hit {:.2}, pic_miss {:.2}, img_hit {:.2}, img_miss {:.2}, video_decode {:.2}",
            average_usize(&self.frames, |frame| frame.backend.draw_rect_count),
            average_usize(&self.frames, |frame| frame.backend.draw_text_count),
            average_usize(&self.frames, |frame| frame.backend.draw_bitmap_count),
            average_usize(&self.frames, |frame| frame.backend.save_layer_count),
            average_usize(&self.frames, |frame| frame.backend.text_cache_hits),
            average_usize(&self.frames, |frame| frame.backend.text_cache_misses),
            average_usize(&self.frames, |frame| frame.backend.picture_cache_hits),
            average_usize(&self.frames, |frame| frame.backend.picture_cache_misses),
            average_usize(&self.frames, |frame| frame.backend.image_cache_hits),
            average_usize(&self.frames, |frame| frame.backend.image_cache_misses),
            average_usize(&self.frames, |frame| frame.backend.video_frame_decodes),
        );
    }
}

fn average(frames: &[FrameProfile], map: impl Fn(&FrameProfile) -> f64) -> f64 {
    frames.iter().map(map).sum::<f64>() / frames.len() as f64
}

fn average_usize(frames: &[FrameProfile], map: impl Fn(&FrameProfile) -> usize) -> f64 {
    frames.iter().map(map).sum::<usize>() as f64 / frames.len() as f64
}

fn percentile_95(frames: &[FrameProfile], map: impl Fn(&FrameProfile) -> f64) -> f64 {
    let mut values = frames.iter().map(map).collect::<Vec<_>>();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let index = ((values.len() as f64 * 0.95).ceil() as usize)
        .saturating_sub(1)
        .min(values.len() - 1);
    values[index]
}

fn average_when_counted(
    frames: &[FrameProfile],
    value: impl Fn(&FrameProfile) -> f64,
    count: impl Fn(&FrameProfile) -> usize,
) -> f64 {
    let total_count = frames.iter().map(count).sum::<usize>();
    if total_count == 0 {
        return 0.0;
    }
    frames.iter().map(value).sum::<f64>() / total_count as f64
}

fn write_encoded_packets(
    encoder: &mut ffmpeg::codec::encoder::video::Encoder,
    output: &mut format::context::Output,
    stream_index: usize,
    packet_time_base: Rational,
    stream_time_base: Rational,
    frame_duration: i64,
) -> Result<()> {
    let mut packet = Packet::empty();
    while encoder.receive_packet(&mut packet).is_ok() {
        if packet.duration() == 0 {
            packet.set_duration(frame_duration);
        }
        packet.rescale_ts(packet_time_base, stream_time_base);
        packet.set_stream(stream_index);
        packet.write_interleaved(output)?;
    }

    Ok(())
}
