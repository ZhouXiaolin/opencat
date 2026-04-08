pub(crate) mod invalidation;

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::Arc,
    time::Instant,
};

use anyhow::{Result, anyhow};
use skia_safe::{
    AlphaType, Canvas, ColorType, EncodedImageFormat, ImageInfo, image::CachingHint, surfaces,
};

#[cfg(target_os = "macos")]
use foreign_types::ForeignType;
#[cfg(target_os = "macos")]
use metal::{
    Device, MTLOrigin, MTLPixelFormat, MTLRegion, MTLSize, MTLStorageMode, MTLTextureType,
    MTLTextureUsage, Texture, TextureDescriptor,
};
#[cfg(target_os = "macos")]
use skia_safe::gpu::{self, backend_render_targets, mtl};

use crate::{
    Composition, FrameCtx, Node,
    assets::AssetsMap,
    backend::{resource_cache::BackendResourceCache, skia_transition},
    codec::decode::{AudioTrack, decode_audio_to_f32_stereo},
    display::{
        analysis::display_list_contains_video,
        build::{build_display_list_from_tree, build_display_tree},
        list::DisplayList,
        tree::DisplayTree,
    },
    element::resolve::resolve_ui_tree_with_script_cache,
    layout::LayoutSession,
    media::MediaContext,
    nodes::{AudioSource, ImageSource},
    profile::{BackendProfile, FrameProfile, RenderProfiler, SceneBuildStats},
    render_cache::{SceneSlot, SceneSnapshotCache},
    render_target::RenderTargetHandle,
    scene_snapshot::{SceneSnapshotRuntime, plan_for_scene, render_scene_slot},
    script::{ScriptRuntimeCache, StyleMutations},
    timeline::{FrameState, frame_state_for_root},
    view::NodeKind,
};

pub use crate::codec::encode::Mp4Config;

const AUDIO_SAMPLE_RATE: u32 = 48_000;

pub enum OutputFormat {
    Mp4(Mp4Config),
    Png,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderBackend {
    SkiaRaster,
    SkiaMetal,
}

pub struct EncodingConfig {
    pub format: OutputFormat,
}

pub struct RenderSession {
    media_ctx: MediaContext,
    assets: AssetsMap,
    scene_snapshots: SceneSnapshotCache,
    backend_resources: BackendResourceCache,
    script_runtime: ScriptRuntimeCache,
    scene_layout: LayoutSession,
    transition_from_layout: LayoutSession,
    transition_to_layout: LayoutSession,
    profiler: RenderProfiler,
    prepared_root_ptr: Option<usize>,
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
            scene_snapshots: SceneSnapshotCache::new(),
            backend_resources: BackendResourceCache::new(),
            script_runtime: ScriptRuntimeCache::default(),
            scene_layout: LayoutSession::new(),
            transition_from_layout: LayoutSession::new(),
            transition_to_layout: LayoutSession::new(),
            profiler: RenderProfiler::default(),
            prepared_root_ptr: None,
        }
    }

    pub fn print_profile_summary(&self) {
        self.profiler.print_summary();
    }

    fn layout_session_mut(&mut self, slot: SceneSlot) -> &mut LayoutSession {
        match slot {
            SceneSlot::Scene => &mut self.scene_layout,
            SceneSlot::TransitionFrom => &mut self.transition_from_layout,
            SceneSlot::TransitionTo => &mut self.transition_to_layout,
        }
    }
}

impl Composition {
    pub fn render(&self, output_path: impl AsRef<Path>, config: &EncodingConfig) -> Result<()> {
        self.render_with_backend(output_path, config, RenderBackend::SkiaRaster)
    }

    pub fn render_with_backend(
        &self,
        output_path: impl AsRef<Path>,
        config: &EncodingConfig,
        backend: RenderBackend,
    ) -> Result<()> {
        match &config.format {
            OutputFormat::Mp4(mp4_config) => render_mp4(self, output_path, mp4_config, backend),
            OutputFormat::Png => render_png(self, output_path, backend),
        }
    }

    pub fn render_frame_with_target(
        &self,
        frame_index: u32,
        session: &mut RenderSession,
        target: &mut RenderTargetHandle,
    ) -> Result<()> {
        render_frame_to_target(self, frame_index, session, target)
    }
}

fn render_png(
    composition: &Composition,
    output_path: impl AsRef<Path>,
    backend: RenderBackend,
) -> Result<()> {
    match backend {
        RenderBackend::SkiaRaster => {
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
        RenderBackend::SkiaMetal => {
            #[cfg(target_os = "macos")]
            {
                let mut session = RenderSession::new();
                let mut bridge = MetalEncodeBridge::new(composition.width, composition.height)?;
                let rgba = bridge.render_frame_rgba(composition, 0, &mut session)?;
                let image = image::RgbaImage::from_raw(
                    composition.width as u32,
                    composition.height as u32,
                    rgba,
                )
                .ok_or_else(|| anyhow!("failed to build PNG image from RGBA frame"))?;
                image.save(output_path)?;
                session.profiler.print_summary();
                return Ok(());
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err(anyhow!("SkiaMetal backend is only available on macOS"))
            }
        }
    }
}

fn render_mp4(
    composition: &Composition,
    output_path: impl AsRef<Path>,
    config: &Mp4Config,
    backend: RenderBackend,
) -> Result<()> {
    let mut session = RenderSession::new();
    ensure_assets_preloaded(composition, &mut session)?;
    let audio_track = build_audio_track(composition, &mut session.assets)?;
    let source_width = composition.width as u32;
    let source_height = composition.height as u32;
    let encoded_width = source_width + source_width % 2;
    let encoded_height = source_height + source_height % 2;
    match backend {
        RenderBackend::SkiaRaster => {
            crate::codec::encode::encode_rgba_frames(
                output_path,
                encoded_width,
                encoded_height,
                composition.fps,
                composition.frames,
                config,
                audio_track.as_ref(),
                |frame_index| {
                    let rgba = render_frame_rgba(composition, frame_index, &mut session)?;
                    Ok(pad_rgba_frame(
                        &rgba,
                        source_width,
                        source_height,
                        encoded_width,
                        encoded_height,
                    ))
                },
            )?;
        }
        RenderBackend::SkiaMetal => {
            #[cfg(target_os = "macos")]
            {
                let mut bridge = MetalEncodeBridge::new(composition.width, composition.height)?;
                crate::codec::encode::encode_rgba_frames(
                    output_path,
                    encoded_width,
                    encoded_height,
                    composition.fps,
                    composition.frames,
                    config,
                    audio_track.as_ref(),
                    |frame_index| {
                        let rgba =
                            bridge.render_frame_rgba(composition, frame_index, &mut session)?;
                        Ok(pad_rgba_frame(
                            &rgba,
                            source_width,
                            source_height,
                            encoded_width,
                            encoded_height,
                        ))
                    },
                )?;
            }
            #[cfg(not(target_os = "macos"))]
            {
                return Err(anyhow!("SkiaMetal backend is only available on macOS"));
            }
        }
    }
    session.profiler.print_summary();
    Ok(())
}

fn pad_rgba_frame(
    rgba: &[u8],
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
) -> Vec<u8> {
    if source_width == target_width && source_height == target_height {
        return rgba.to_vec();
    }

    let source_width = source_width as usize;
    let source_height = source_height as usize;
    let target_width = target_width as usize;
    let target_height = target_height as usize;
    let mut padded = vec![0_u8; target_width * target_height * 4];

    for y in 0..target_height {
        let source_y = y.min(source_height.saturating_sub(1));
        for x in 0..target_width {
            let source_x = x.min(source_width.saturating_sub(1));
            let source_index = (source_y * source_width + source_x) * 4;
            let target_index = (y * target_width + x) * 4;
            padded[target_index..target_index + 4]
                .copy_from_slice(&rgba[source_index..source_index + 4]);
        }
    }

    padded
}

fn render_frame_surface(
    composition: &Composition,
    frame_index: u32,
    session: &mut RenderSession,
) -> Result<skia_safe::Surface> {
    let mut surface = surfaces::raster_n32_premul((composition.width, composition.height))
        .ok_or_else(|| anyhow!("failed to create skia raster surface"))?;
    render_frame_on_canvas(composition, frame_index, session, surface.canvas())?;
    Ok(surface)
}

pub fn render_frame_to_target(
    composition: &Composition,
    frame_index: u32,
    session: &mut RenderSession,
    target: &mut RenderTargetHandle,
) -> Result<()> {
    target.require_skia_backend()?;
    let canvas_ptr = target.begin_frame(composition.width, composition.height)?;
    if canvas_ptr.is_null() {
        return Err(anyhow!(
            "render target begin_frame returned null canvas pointer"
        ));
    }
    // SAFETY: Skia 后端目标约定 begin_frame 返回 `skia_safe::Canvas` 有效指针。
    let canvas = unsafe { &*(canvas_ptr as *const Canvas) };
    let render_result = render_frame_on_canvas(composition, frame_index, session, canvas);
    let end_result = target.end_frame();
    render_result.and(end_result)
}

fn render_frame_on_canvas(
    composition: &Composition,
    frame_index: u32,
    session: &mut RenderSession,
    canvas: &Canvas,
) -> Result<()> {
    ensure_assets_preloaded(composition, session)?;

    let mut frame_profile = FrameProfile::default();
    let frame_ctx = FrameCtx {
        frame: frame_index,
        fps: composition.fps,
        width: composition.width,
        height: composition.height,
        frames: composition.frames,
    };

    let script_started = Instant::now();
    let mutations: Option<StyleMutations> = None;
    frame_profile.script_ms = script_started.elapsed().as_secs_f64() * 1000.0;

    let root = composition.root_node(&frame_ctx);
    let frame_state_started = Instant::now();
    let frame_state = frame_state_for_root(&root, &frame_ctx);
    frame_profile.frame_state_ms = frame_state_started.elapsed().as_secs_f64() * 1000.0;

    match frame_state {
        FrameState::Scene {
            scene,
            script_frame_ctx,
        } => {
            let (display_tree, display_list, scene_stats) = build_scene_display_list_with_slot(
                &scene,
                &frame_ctx,
                &script_frame_ctx,
                session,
                mutations.as_ref(),
                SceneSlot::Scene,
            )?;
            frame_profile.merge_scene_stats(&scene_stats);
            let snapshot_plan = plan_for_scene(&scene_stats);

            let backend_started = Instant::now();
            let mut backend_profile = BackendProfile::default();
            {
                let mut snapshot_runtime = SceneSnapshotRuntime {
                    assets: &session.assets,
                    scene_snapshots: &mut session.scene_snapshots,
                    backend_resources: &session.backend_resources,
                    media_ctx: &mut session.media_ctx,
                    frame_ctx: &frame_ctx,
                    backend_profile: &mut backend_profile,
                    width: composition.width,
                    height: composition.height,
                };
                render_scene_slot(
                    &mut snapshot_runtime,
                    SceneSlot::Scene,
                    &display_tree,
                    &display_list,
                    snapshot_plan,
                    false,
                    Some(canvas),
                )?;
            }

            frame_profile.backend_ms = backend_started.elapsed().as_secs_f64() * 1000.0;
            frame_profile.merge_backend_profile(&backend_profile);
        }
        FrameState::Transition {
            from,
            to,
            from_script_frame_ctx,
            to_script_frame_ctx,
            progress,
            kind,
        } => {
            let (from_tree, from_display, from_stats) = build_scene_display_list_with_slot(
                &from,
                &frame_ctx,
                &from_script_frame_ctx,
                session,
                mutations.as_ref(),
                SceneSlot::TransitionFrom,
            )?;
            let (to_tree, to_display, to_stats) = build_scene_display_list_with_slot(
                &to,
                &frame_ctx,
                &to_script_frame_ctx,
                session,
                mutations.as_ref(),
                SceneSlot::TransitionTo,
            )?;
            frame_profile.merge_scene_stats(&from_stats);
            frame_profile.merge_scene_stats(&to_stats);
            let from_plan = plan_for_scene(&from_stats);
            let to_plan = plan_for_scene(&to_stats);

            let backend_started = Instant::now();
            let mut backend_profile = BackendProfile::default();
            let (from_snapshot, to_snapshot) = {
                let mut snapshot_runtime = SceneSnapshotRuntime {
                    assets: &session.assets,
                    scene_snapshots: &mut session.scene_snapshots,
                    backend_resources: &session.backend_resources,
                    media_ctx: &mut session.media_ctx,
                    frame_ctx: &frame_ctx,
                    backend_profile: &mut backend_profile,
                    width: composition.width,
                    height: composition.height,
                };
                let from_snapshot = render_scene_slot(
                    &mut snapshot_runtime,
                    SceneSlot::TransitionFrom,
                    &from_tree,
                    &from_display,
                    from_plan,
                    true,
                    None,
                )?
                .expect("transition source scene snapshot should exist");
                let to_snapshot = render_scene_slot(
                    &mut snapshot_runtime,
                    SceneSlot::TransitionTo,
                    &to_tree,
                    &to_display,
                    to_plan,
                    true,
                    None,
                )?
                .expect("transition target scene snapshot should exist");
                (from_snapshot, to_snapshot)
            };
            frame_profile.backend_ms = backend_started.elapsed().as_secs_f64() * 1000.0;

            let transition_started = Instant::now();
            skia_transition::draw_transition(
                canvas,
                &from_snapshot,
                &to_snapshot,
                progress,
                kind,
                composition.width,
                composition.height,
                Some(&mut backend_profile),
            )?;
            let transition_ms = transition_started.elapsed().as_secs_f64() * 1000.0;
            frame_profile.transition_ms = transition_ms;
            frame_profile.merge_backend_profile(&backend_profile);
            match kind {
                crate::transitions::TransitionKind::Slide(_) => {
                    frame_profile.slide_transition_ms = transition_ms;
                    frame_profile.slide_transition_frames = 1;
                }
                crate::transitions::TransitionKind::LightLeak(_) => {
                    frame_profile.light_leak_transition_ms = transition_ms;
                    frame_profile.light_leak_transition_frames = 1;
                }
                _ => {}
            }
        }
    }

    session.profiler.push(frame_profile);
    Ok(())
}

fn ensure_assets_preloaded(composition: &Composition, session: &mut RenderSession) -> Result<()> {
    let root_ptr = Arc::as_ptr(&composition.root) as *const () as usize;
    if session.prepared_root_ptr == Some(root_ptr) {
        return Ok(());
    }

    let mut image_sources = HashSet::new();
    let mut audio_sources = HashSet::new();
    for frame in 0..composition.frames {
        let frame_ctx = FrameCtx {
            frame,
            fps: composition.fps,
            width: composition.width,
            height: composition.height,
            frames: composition.frames,
        };
        let root = composition.root_node(&frame_ctx);
        match frame_state_for_root(&root, &frame_ctx) {
            FrameState::Scene { scene, .. } => {
                collect_sources(&scene, &frame_ctx, &mut image_sources, &mut audio_sources);
            }
            FrameState::Transition { from, to, .. } => {
                collect_sources(&from, &frame_ctx, &mut image_sources, &mut audio_sources);
                collect_sources(&to, &frame_ctx, &mut image_sources, &mut audio_sources);
            }
        }
    }

    session.assets.preload_image_sources(image_sources)?;
    session.assets.preload_audio_sources(audio_sources)?;
    session.prepared_root_ptr = Some(root_ptr);
    Ok(())
}

fn collect_sources(
    node: &Node,
    frame_ctx: &FrameCtx,
    image_sources: &mut HashSet<ImageSource>,
    audio_sources: &mut HashSet<AudioSource>,
) {
    match node.kind() {
        NodeKind::Component(component) => {
            let rendered = component.render(frame_ctx);
            collect_sources(&rendered, frame_ctx, image_sources, audio_sources);
        }
        NodeKind::Div(div) => {
            for child in div.children_ref() {
                collect_sources(child, frame_ctx, image_sources, audio_sources);
            }
        }
        NodeKind::Canvas(canvas) => {
            for asset in canvas.assets_ref() {
                if !matches!(asset.source, ImageSource::Unset) {
                    image_sources.insert(asset.source.clone());
                }
            }
        }
        NodeKind::Image(image) => {
            if !matches!(image.source(), ImageSource::Unset) {
                image_sources.insert(image.source().clone());
            }
        }
        NodeKind::Audio(audio) => {
            if !matches!(audio.source(), AudioSource::Unset) {
                audio_sources.insert(audio.source().clone());
            }
        }
        NodeKind::Timeline(_) => match frame_state_for_root(node, frame_ctx) {
            FrameState::Scene { scene, .. } => {
                collect_sources(&scene, frame_ctx, image_sources, audio_sources);
            }
            FrameState::Transition { from, to, .. } => {
                collect_sources(&from, frame_ctx, image_sources, audio_sources);
                collect_sources(&to, frame_ctx, image_sources, audio_sources);
            }
        },
        NodeKind::Text(_) | NodeKind::Lucide(_) | NodeKind::Video(_) => {}
    }
}

#[derive(Clone)]
struct AudioInterval {
    source: AudioSource,
    start_frame: u32,
    end_frame: u32,
}

fn build_audio_track(
    composition: &Composition,
    assets: &mut AssetsMap,
) -> Result<Option<AudioTrack>> {
    let intervals = collect_audio_intervals(composition);
    if intervals.is_empty() {
        return Ok(None);
    }

    let total_frames =
        frame_to_audio_sample(composition.frames, composition.fps, AUDIO_SAMPLE_RATE);
    let mut mixed = vec![0.0_f32; total_frames * 2];
    let mut decoded = HashMap::new();

    for interval in intervals {
        let clip = if let Some(clip) = decoded.get(&interval.source) {
            clip
        } else {
            let asset_id = assets.register_audio_source(&interval.source)?;
            let path = assets
                .path(&asset_id)
                .ok_or_else(|| anyhow!("missing cached audio asset for {}", asset_id.0))?;
            let clip = decode_audio_to_f32_stereo(path, AUDIO_SAMPLE_RATE)?;
            decoded.insert(interval.source.clone(), clip);
            decoded
                .get(&interval.source)
                .expect("decoded audio clip should exist")
        };

        let start_sample =
            frame_to_audio_sample(interval.start_frame, composition.fps, AUDIO_SAMPLE_RATE);
        let end_sample =
            frame_to_audio_sample(interval.end_frame, composition.fps, AUDIO_SAMPLE_RATE);
        let available_frames = clip
            .sample_frames()
            .min(end_sample.saturating_sub(start_sample));

        for frame_offset in 0..available_frames {
            let mix_index = (start_sample + frame_offset) * 2;
            let clip_index = frame_offset * 2;
            mixed[mix_index] += clip.samples[clip_index];
            mixed[mix_index + 1] += clip.samples[clip_index + 1];
        }
    }

    for sample in &mut mixed {
        *sample = sample.clamp(-1.0, 1.0);
    }

    Ok(Some(AudioTrack::new(AUDIO_SAMPLE_RATE, 2, mixed)))
}

fn collect_audio_intervals(composition: &Composition) -> Vec<AudioInterval> {
    let mut active = HashMap::<AudioSource, u32>::new();
    let mut previous = HashSet::<AudioSource>::new();
    let mut intervals = Vec::new();

    for frame in 0..composition.frames {
        let frame_ctx = FrameCtx {
            frame,
            fps: composition.fps,
            width: composition.width,
            height: composition.height,
            frames: composition.frames,
        };
        let root = composition.root_node(&frame_ctx);
        let mut current = HashSet::new();
        let mut ignored_images = HashSet::new();

        match frame_state_for_root(&root, &frame_ctx) {
            FrameState::Scene { scene, .. } => {
                collect_sources(&scene, &frame_ctx, &mut ignored_images, &mut current);
            }
            FrameState::Transition { from, to, .. } => {
                collect_sources(&from, &frame_ctx, &mut ignored_images, &mut current);
                collect_sources(&to, &frame_ctx, &mut ignored_images, &mut current);
            }
        }

        for source in current.difference(&previous) {
            active.insert(source.clone(), frame);
        }

        for source in previous.difference(&current) {
            if let Some(start_frame) = active.remove(source) {
                intervals.push(AudioInterval {
                    source: source.clone(),
                    start_frame,
                    end_frame: frame,
                });
            }
        }

        previous = current;
    }

    for source in previous {
        if let Some(start_frame) = active.remove(&source) {
            intervals.push(AudioInterval {
                source,
                start_frame,
                end_frame: composition.frames,
            });
        }
    }

    intervals
}

fn frame_to_audio_sample(frame: u32, fps: u32, sample_rate: u32) -> usize {
    ((frame as u64 * sample_rate as u64) / fps as u64) as usize
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

#[cfg(target_os = "macos")]
struct MetalEncodeBridge {
    _target: Box<MetalOffscreenTarget>,
    handle: RenderTargetHandle,
}

#[cfg(target_os = "macos")]
impl MetalEncodeBridge {
    fn new(width: i32, height: i32) -> Result<Self> {
        let mut target = Box::new(MetalOffscreenTarget::new(width, height)?);
        let target_ptr = target.as_mut() as *mut MetalOffscreenTarget as *mut std::ffi::c_void;
        let handle = RenderTargetHandle::new(
            crate::render_target::RenderBackendKind::Skia,
            target_ptr,
            MetalOffscreenTarget::begin_frame_bridge,
            MetalOffscreenTarget::end_frame_bridge,
        )
        .with_readback_rgba(MetalOffscreenTarget::readback_rgba_bridge);
        Ok(Self {
            _target: target,
            handle,
        })
    }

    fn render_frame_rgba(
        &mut self,
        composition: &Composition,
        frame_index: u32,
        session: &mut RenderSession,
    ) -> Result<Vec<u8>> {
        render_frame_to_target(composition, frame_index, session, &mut self.handle)?;
        self.handle.readback_rgba()
    }
}

#[cfg(target_os = "macos")]
struct MetalOffscreenTarget {
    device: Device,
    skia: gpu::DirectContext,
    texture: Texture,
    width: i32,
    height: i32,
    current_surface: Option<skia_safe::Surface>,
}

#[cfg(target_os = "macos")]
impl MetalOffscreenTarget {
    fn new(width: i32, height: i32) -> Result<Self> {
        let device = Device::system_default().ok_or_else(|| anyhow!("no Metal device found"))?;
        let command_queue = device.new_command_queue();
        let backend = unsafe {
            mtl::BackendContext::new(
                device.as_ptr() as mtl::Handle,
                command_queue.as_ptr() as mtl::Handle,
            )
        };
        let skia = gpu::direct_contexts::make_metal(&backend, None)
            .ok_or_else(|| anyhow!("failed to create Skia Metal direct context"))?;
        let texture = Self::create_texture(&device, width, height);
        Ok(Self {
            device,
            skia,
            texture,
            width,
            height,
            current_surface: None,
        })
    }

    fn create_texture(device: &Device, width: i32, height: i32) -> Texture {
        let descriptor = TextureDescriptor::new();
        descriptor.set_texture_type(MTLTextureType::D2);
        descriptor.set_width(width.max(1) as u64);
        descriptor.set_height(height.max(1) as u64);
        descriptor.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        descriptor.set_storage_mode(MTLStorageMode::Shared);
        descriptor.set_usage(MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead);
        device.new_texture(&descriptor)
    }

    fn ensure_size(&mut self, width: i32, height: i32) {
        if self.width == width && self.height == height {
            return;
        }
        self.texture = Self::create_texture(&self.device, width, height);
        self.width = width;
        self.height = height;
    }

    fn begin_frame(&mut self, width: i32, height: i32) -> Result<*mut std::ffi::c_void> {
        if width <= 0 || height <= 0 {
            return Err(anyhow!("invalid offscreen target size {width}x{height}"));
        }
        self.ensure_size(width, height);

        let texture_info = unsafe { mtl::TextureInfo::new(self.texture.as_ptr() as mtl::Handle) };
        let backend_render_target =
            backend_render_targets::make_mtl((self.width, self.height), &texture_info);
        let mut surface = gpu::surfaces::wrap_backend_render_target(
            &mut self.skia,
            &backend_render_target,
            skia_safe::gpu::SurfaceOrigin::TopLeft,
            ColorType::BGRA8888,
            None,
            None,
        )
        .ok_or_else(|| anyhow!("failed to wrap metal offscreen render target"))?;
        let canvas_ptr = surface.canvas() as *const _ as *mut std::ffi::c_void;
        self.current_surface = Some(surface);
        Ok(canvas_ptr)
    }

    fn end_frame(&mut self) -> Result<()> {
        let surface = self
            .current_surface
            .take()
            .ok_or_else(|| anyhow!("offscreen end_frame called before begin_frame"))?;
        self.skia.flush_and_submit();
        drop(surface);
        Ok(())
    }

    fn readback_rgba(&self) -> Result<Vec<u8>> {
        let width = self.width.max(1) as u64;
        let height = self.height.max(1) as u64;
        let mut bgra = vec![0_u8; (width * height * 4) as usize];
        let region = MTLRegion {
            origin: MTLOrigin { x: 0, y: 0, z: 0 },
            size: MTLSize {
                width,
                height,
                depth: 1,
            },
        };
        self.texture
            .as_ref()
            .get_bytes(bgra.as_mut_ptr().cast(), width * 4, region, 0);
        for px in bgra.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
        Ok(bgra)
    }

    unsafe fn begin_frame_bridge(
        user_data: *mut std::ffi::c_void,
        width: i32,
        height: i32,
    ) -> Result<*mut std::ffi::c_void> {
        unsafe { &mut *(user_data as *mut Self) }.begin_frame(width, height)
    }

    unsafe fn end_frame_bridge(user_data: *mut std::ffi::c_void) -> Result<()> {
        unsafe { &mut *(user_data as *mut Self) }.end_frame()
    }

    unsafe fn readback_rgba_bridge(user_data: *mut std::ffi::c_void) -> Result<Vec<u8>> {
        unsafe { &mut *(user_data as *mut Self) }.readback_rgba()
    }
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
    script_frame_ctx: &crate::frame_ctx::ScriptFrameCtx,
    session: &mut RenderSession,
    mutations: Option<&StyleMutations>,
    slot: SceneSlot,
) -> Result<(DisplayTree, DisplayList, SceneBuildStats)> {
    let mut stats = SceneBuildStats::default();

    let resolve_started = Instant::now();
    let element_root = resolve_ui_tree_with_script_cache(
        scene,
        frame_ctx,
        script_frame_ctx,
        &mut session.media_ctx,
        &mut session.assets,
        mutations,
        &mut session.script_runtime,
    )?;
    stats.resolve_ms = resolve_started.elapsed().as_secs_f64() * 1000.0;

    let layout_started = Instant::now();
    let (layout_tree, layout_pass) = session
        .layout_session_mut(slot)
        .compute_layout(&element_root, frame_ctx)?;
    stats.layout_ms = layout_started.elapsed().as_secs_f64() * 1000.0;
    stats.layout_pass = layout_pass;

    let display_started = Instant::now();
    let display_tree = build_display_tree(&element_root, &layout_tree)?;
    let display_list = build_display_list_from_tree(&display_tree);
    stats.display_ms = display_started.elapsed().as_secs_f64() * 1000.0;
    stats.contains_video = display_list_contains_video(&display_list, &session.assets);

    Ok((display_tree, display_list, stats))
}

#[cfg(test)]
mod tests {
    use super::{RenderSession, pad_rgba_frame, render_frame_rgba};
    use crate::{
        Composition, FrameCtx,
        nodes::{canvas, div},
        style::ColorToken,
    };

    fn write_test_png(path: &std::path::Path) {
        let mut image = image::RgbaImage::new(2, 1);
        image.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        image.put_pixel(1, 0, image::Rgba([0, 255, 0, 255]));
        image.save(path).expect("test image should save");
    }

    fn pixel_rgba(frame: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
        let index = (y * width + x) * 4;
        [
            frame[index],
            frame[index + 1],
            frame[index + 2],
            frame[index + 3],
        ]
    }

    #[test]
    fn subtree_cache_does_not_apply_node_opacity_twice() {
        let scene = div()
            .id("root")
            .w_full()
            .h_full()
            .bg_black()
            .script_source(r#"ctx.getNode("box").opacity(ctx.frame === 0 ? 1 : 0.5);"#)
            .expect("script should compile")
            .child(
                div()
                    .id("box")
                    .absolute()
                    .left(0.0)
                    .top(0.0)
                    .w(10.0)
                    .h(10.0)
                    .bg_white(),
            );

        let composition = Composition::new("opacity_cache")
            .size(20, 20)
            .fps(30)
            .frames(2)
            .root(move |_ctx: &FrameCtx| scene.clone().into())
            .build()
            .expect("composition should build");

        let mut session = RenderSession::new();
        let first =
            render_frame_rgba(&composition, 0, &mut session).expect("frame 0 should render");
        let second =
            render_frame_rgba(&composition, 1, &mut session).expect("frame 1 should render");

        let first_pixel = pixel_rgba(&first, 20, 5, 5);
        let second_pixel = pixel_rgba(&second, 20, 5, 5);

        assert!(first_pixel[0] >= 250, "frame 0 should stay fully white");
        assert!(
            (120..=136).contains(&second_pixel[0]),
            "frame 1 should be roughly 50% white, got {:?}",
            second_pixel
        );
    }

    #[test]
    fn display_list_and_subtree_cache_both_preserve_overflow_clipping() {
        let scene = div()
            .id("root")
            .w_full()
            .h_full()
            .bg(ColorToken::Black)
            .script_source(r#"ctx.getNode("mover").translateY(ctx.frame);"#)
            .expect("script should compile")
            .child(
                div()
                    .id("card")
                    .absolute()
                    .left(4.0)
                    .top(4.0)
                    .w(12.0)
                    .h(12.0)
                    .rounded(6.0)
                    .overflow_hidden()
                    .child(
                        div()
                            .id("card-fill")
                            .w_full()
                            .h_full()
                            .bg(ColorToken::White),
                    ),
            )
            .child(
                div()
                    .id("mover")
                    .absolute()
                    .left(0.0)
                    .top(0.0)
                    .w(2.0)
                    .h(2.0)
                    .bg(ColorToken::Red500),
            );

        let composition = Composition::new("clip_consistency")
            .size(24, 24)
            .fps(30)
            .frames(2)
            .root(move |_ctx: &FrameCtx| scene.clone().into())
            .build()
            .expect("composition should build");

        let mut session = RenderSession::new();
        let first =
            render_frame_rgba(&composition, 0, &mut session).expect("frame 0 should render");
        let second =
            render_frame_rgba(&composition, 1, &mut session).expect("frame 1 should render");

        assert_eq!(
            pixel_rgba(&first, 24, 4, 4),
            [0, 0, 0, 255],
            "frame 0 should keep the clipped corner transparent to the black background"
        );
        assert_eq!(
            pixel_rgba(&second, 24, 4, 4),
            [0, 0, 0, 255],
            "frame 1 should match frame 0 after subtree caching kicks in"
        );
    }

    #[test]
    fn canvas_node_draw_image_uses_asset_alias_in_backend() {
        let image_path =
            std::env::temp_dir().join(format!("opencat-canvas-test-{}.png", std::process::id()));
        write_test_png(&image_path);

        let scene = canvas()
            .id("canvas")
            .size(2.0, 1.0)
            .asset_path("hero", &image_path)
            .script_source(
                r#"
                const CK = ctx.CanvasKit;
                const image = ctx.getImage("hero");
                ctx.getCanvas().drawImageRect(
                    image,
                    CK.XYWHRect(0, 0, 1, 1),
                    CK.XYWHRect(0, 0, 2, 1),
                );
                "#,
            )
            .expect("script should compile");

        let composition = Composition::new("canvas_asset_alias")
            .size(2, 1)
            .fps(30)
            .frames(1)
            .root(move |_ctx: &FrameCtx| scene.clone().into())
            .build()
            .expect("composition should build");

        let mut session = RenderSession::new();
        let frame = render_frame_rgba(&composition, 0, &mut session).expect("frame should render");

        let _ = std::fs::remove_file(&image_path);

        assert_eq!(pixel_rgba(&frame, 2, 0, 0), [255, 0, 0, 255]);
        assert_eq!(pixel_rgba(&frame, 2, 1, 0), [0, 255, 0, 255]);
    }

    #[test]
    fn pad_rgba_frame_extends_edge_pixels_for_even_dimensions() {
        let rgba = vec![
            1, 0, 0, 255, 2, 0, 0, 255, 3, 0, 0, 255, 4, 0, 0, 255, 5, 0, 0, 255, 6, 0, 0, 255, 7,
            0, 0, 255, 8, 0, 0, 255, 9, 0, 0, 255,
        ];

        let padded = pad_rgba_frame(&rgba, 3, 3, 4, 4);

        assert_eq!(pixel_rgba(&padded, 4, 0, 0), [1, 0, 0, 255]);
        assert_eq!(pixel_rgba(&padded, 4, 2, 2), [9, 0, 0, 255]);
        assert_eq!(pixel_rgba(&padded, 4, 3, 0), [3, 0, 0, 255]);
        assert_eq!(pixel_rgba(&padded, 4, 0, 3), [7, 0, 0, 255]);
        assert_eq!(pixel_rgba(&padded, 4, 3, 3), [9, 0, 0, 255]);
    }
}
