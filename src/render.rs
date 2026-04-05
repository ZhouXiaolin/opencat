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
    AlphaType, ColorType, EncodedImageFormat, ImageInfo, image::CachingHint, surfaces,
};
use std::{path::Path, time::Instant};

use crate::{
    Composition, FrameCtx, Node,
    assets::AssetsMap,
    backend::{
        skia::{SkiaBackend, new_image_cache},
        skia_transition,
    },
    display::build::build_display_list,
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
    script_runner: Option<ScriptRunner>,
    script_driver_ptr: Option<usize>,
    scene_layout: LayoutSession,
    transition_from_layout: LayoutSession,
    transition_to_layout: LayoutSession,
    profiler: RenderProfiler,
}

#[derive(Clone, Copy)]
enum SceneSlot {
    Scene,
    TransitionFrom,
    TransitionTo,
}

#[derive(Default)]
struct SceneBuildStats {
    resolve_ms: f64,
    layout_ms: f64,
    display_ms: f64,
    layout_pass: LayoutPassStats,
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
    reused_nodes: usize,
    layout_dirty_nodes: usize,
    paint_only_nodes: usize,
    structure_rebuilds: usize,
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
            script_runner: None,
            script_driver_ptr: None,
            scene_layout: LayoutSession::new(),
            transition_from_layout: LayoutSession::new(),
            transition_to_layout: LayoutSession::new(),
            profiler: RenderProfiler::default(),
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
        let mut rgba = render_frame_rgb(composition, frame_index, &mut session)?;
        let mut rgb_frame = Video::new(
            Pixel::RGBA,
            composition.width as u32,
            composition.height as u32,
        );
        unsafe {
            (*rgb_frame.as_mut_ptr()).linesize[0] = composition.width as i32 * 4;
            (*rgb_frame.as_mut_ptr()).data[0] = rgba.as_mut_ptr();
        }

        let mut yuv_frame = Video::new(
            Pixel::YUV420P,
            composition.width as u32,
            composition.height as u32,
        );
        scaler.run(&rgb_frame, &mut yuv_frame)?;
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
            let mut backend = SkiaBackend::new_with_cache(
                canvas,
                composition.width,
                composition.height,
                &session.assets,
                session.image_cache.clone(),
                Some(&mut session.media_ctx),
                &frame_ctx,
            );
            backend.execute(&display_list)?;
            frame_profile.backend_ms = backend_started.elapsed().as_secs_f64() * 1000.0;
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
            let mut media_ctx = Some(&mut session.media_ctx);
            let transition_started = Instant::now();
            skia_transition::draw_transition(
                canvas,
                &from_display,
                &to_display,
                progress,
                kind,
                composition.width,
                composition.height,
                &session.assets,
                session.image_cache.clone(),
                &mut media_ctx,
                &frame_ctx,
            )?;
            frame_profile.transition_ms = transition_started.elapsed().as_secs_f64() * 1000.0;
        }
    }

    session.profiler.push(frame_profile);
    Ok(surface)
}

pub fn render_frame_rgb(
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

    // let mut rgb = vec![0_u8; (composition.width as usize) * (composition.height as usize) * 3];
    // for (src, dst) in bgra.chunks_exact(4).zip(rgb.chunks_exact_mut(3)) {
    //     dst[0] = src[2];
    //     dst[1] = src[1];
    //     dst[2] = src[0];
    // }

    Ok(rgba)
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
) -> Result<(crate::display::list::DisplayList, SceneBuildStats)> {
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

    Ok((display_list, stats))
}

fn copy_rgb_to_frame(rgb: &[u8], frame: &mut Video, width: usize, height: usize) {
    let stride = frame.stride(0);
    let row_len = width * 3;
    let data = frame.data_mut(0);

    for y in 0..height {
        let src_start = y * row_len;
        let src_end = src_start + row_len;
        let dst_start = y * stride;
        let dst_end = dst_start + row_len;

        data[dst_start..dst_end].copy_from_slice(&rgb[src_start..src_end]);
    }
}

impl RenderSession {
    fn layout_session_mut(&mut self, slot: SceneSlot) -> &mut LayoutSession {
        match slot {
            SceneSlot::Scene => &mut self.scene_layout,
            SceneSlot::TransitionFrom => &mut self.transition_from_layout,
            SceneSlot::TransitionTo => &mut self.transition_to_layout,
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
            "  avg nodes/frame: reused {:.1}, layout_dirty {:.1}, paint_only {:.1}, structure_rebuilds {:.2}",
            average_usize(&self.frames, |frame| frame.reused_nodes),
            average_usize(&self.frames, |frame| frame.layout_dirty_nodes),
            average_usize(&self.frames, |frame| frame.paint_only_nodes),
            average_usize(&self.frames, |frame| frame.structure_rebuilds),
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
