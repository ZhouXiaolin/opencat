#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("opencat-player 目前仅支持 macOS");
}

#[cfg(target_os = "macos")]
mod app {
    use std::ffi::c_void;
    use std::num::{NonZeroU16, NonZeroU32};
    use std::sync::mpsc::{self, Receiver};
    use std::time::{Duration, Instant};

    use anyhow::{Context, Result, anyhow};
    use cocoa::appkit::NSView;
    use cocoa::base::{YES, id};
    use core_graphics_types::geometry::CGSize;
    use foreign_types::{ForeignType, ForeignTypeRef};
    use metal::{CommandQueue, Device, MTLPixelFormat, MetalDrawable, MetalLayer};
    use opencat::{
        Composition, FrameCtx, RenderFrameViewKind, RenderSession, RenderTargetHandle,
        ScriptDriver, parse, render_audio_chunk,
    };
    use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
    use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, Source};
    use skia_safe::{
        ColorType,
        gpu::{self, SurfaceOrigin, backend_render_targets, mtl},
    };
    use winit::dpi::LogicalSize;
    use winit::event::{Event, WindowEvent};
    use winit::event_loop::{ControlFlow, EventLoop};
    use winit::window::WindowBuilder;

    const AUDIO_CHUNK_FRAMES: usize = 2048;
    const AUDIO_SAMPLE_RATE: u32 = 48_000;
    const AUDIO_CHANNELS: u16 = 2;

    enum AudioChunkMessage {
        Samples(Vec<f32>),
        Error(String),
    }

    struct AudioRenderSource {
        receiver: Receiver<AudioChunkMessage>,
        sample_rate: NonZeroU32,
        channels: NonZeroU16,
        chunk_sample_frames: usize,
        current_chunk: Vec<f32>,
        current_sample_index: usize,
        last_error: Option<String>,
        disconnected: bool,
    }

    impl AudioRenderSource {
        fn new(composition: Composition, has_audio: bool) -> Result<Option<Self>> {
            if !has_audio {
                return Ok(None);
            }

            let sample_rate = NonZeroU32::new(AUDIO_SAMPLE_RATE)
                .ok_or_else(|| anyhow!("audio chunk sample rate must be non-zero"))?;
            let channels = NonZeroU16::new(AUDIO_CHANNELS)
                .ok_or_else(|| anyhow!("audio chunk channel count must be non-zero"))?;
            let loop_sample_frames = composition_sample_frames(
                composition.frames.max(1),
                composition.fps.max(1),
                sample_rate,
            );
            let (sender, receiver) = mpsc::sync_channel(3);
            std::thread::spawn(move || {
                let mut session = RenderSession::new();
                let mut next_loop_sample_frame = 0;

                loop {
                    let chunk = match render_audio_chunk_looping(
                        &composition,
                        &mut session,
                        next_loop_sample_frame,
                        AUDIO_CHUNK_FRAMES,
                        sample_rate,
                        channels,
                        loop_sample_frames.max(1),
                    ) {
                        Ok(chunk) => {
                            next_loop_sample_frame = (next_loop_sample_frame + AUDIO_CHUNK_FRAMES)
                                % loop_sample_frames.max(1);
                            AudioChunkMessage::Samples(chunk)
                        }
                        Err(error) => {
                            let _ = sender.send(AudioChunkMessage::Error(format!("{error:#}")));
                            break;
                        }
                    };

                    if sender.send(chunk).is_err() {
                        break;
                    }
                }
            });

            Ok(Some(Self {
                receiver,
                sample_rate,
                channels,
                chunk_sample_frames: AUDIO_CHUNK_FRAMES,
                current_chunk: Vec::new(),
                current_sample_index: 0,
                last_error: None,
                disconnected: false,
            }))
        }

        fn refill_chunk(&mut self) {
            if self.disconnected {
                self.current_chunk =
                    vec![0.0; self.chunk_sample_frames * self.channels.get() as usize];
                self.current_sample_index = 0;
                return;
            }

            match self.receiver.recv() {
                Ok(AudioChunkMessage::Samples(chunk)) => {
                    self.current_chunk = if chunk.is_empty() {
                        vec![0.0; self.chunk_sample_frames * self.channels.get() as usize]
                    } else {
                        chunk
                    };
                    self.current_sample_index = 0;
                    self.last_error = None;
                }
                Ok(AudioChunkMessage::Error(message)) => {
                    if self.last_error.as_deref() != Some(message.as_str()) {
                        eprintln!("audio render error: {message}");
                        self.last_error = Some(message);
                    }
                    self.disconnected = true;
                    self.current_chunk =
                        vec![0.0; self.chunk_sample_frames * self.channels.get() as usize];
                    self.current_sample_index = 0;
                }
                Err(_) => {
                    self.disconnected = true;
                    self.current_chunk =
                        vec![0.0; self.chunk_sample_frames * self.channels.get() as usize];
                    self.current_sample_index = 0;
                }
            }
        }
    }

    impl Iterator for AudioRenderSource {
        type Item = f32;

        fn next(&mut self) -> Option<Self::Item> {
            if self.current_sample_index >= self.current_chunk.len() {
                self.refill_chunk();
            }

            let sample = self.current_chunk.get(self.current_sample_index).copied()?;
            self.current_sample_index += 1;
            Some(sample)
        }
    }

    impl Source for AudioRenderSource {
        fn current_span_len(&self) -> Option<usize> {
            None
        }

        fn channels(&self) -> rodio::ChannelCount {
            self.channels
        }

        fn sample_rate(&self) -> rodio::SampleRate {
            self.sample_rate
        }

        fn total_duration(&self) -> Option<Duration> {
            None
        }
    }

    struct AudioPlayback {
        _sink: MixerDeviceSink,
        player: Player,
    }

    impl AudioPlayback {
        fn new(composition: Composition, has_audio: bool) -> Result<Option<Self>> {
            let Some(source) = AudioRenderSource::new(composition, has_audio)? else {
                return Ok(None);
            };
            let sink = DeviceSinkBuilder::open_default_sink()
                .context("failed to open default audio output device")?;
            let player = Player::connect_new(&sink.mixer());
            player.append(source);
            Ok(Some(Self {
                _sink: sink,
                player,
            }))
        }

        fn frame_index(&self, total_frames: u32, fps: u32) -> u32 {
            if total_frames <= 1 {
                return 0;
            }

            let fps = fps.max(1);
            let loop_secs = total_frames as f64 / fps as f64;
            if loop_secs <= f64::EPSILON {
                return 0;
            }

            let elapsed_secs = self.player.get_pos().as_secs_f64();
            let loop_pos_secs = elapsed_secs % loop_secs;
            ((loop_pos_secs * fps as f64).floor() as u32) % total_frames
        }

        fn next_redraw_deadline(&self, fps: u32) -> Instant {
            let fps = fps.max(1);
            let frame_position = self.player.get_pos().as_secs_f64() * fps as f64;
            let fractional = frame_position.fract();
            let remaining_secs = if fractional <= f64::EPSILON {
                1.0 / fps as f64
            } else {
                (1.0 - fractional) / fps as f64
            };
            Instant::now() + Duration::from_secs_f64(remaining_secs.max(0.001))
        }
    }

    fn render_audio_chunk_looping(
        composition: &Composition,
        session: &mut RenderSession,
        start_loop_sample_frame: usize,
        chunk_sample_frames: usize,
        sample_rate: NonZeroU32,
        channels: NonZeroU16,
        loop_sample_frames: usize,
    ) -> Result<Vec<f32>> {
        let channel_count = channels.get() as usize;
        let mut samples = Vec::with_capacity(chunk_sample_frames * channel_count);
        let mut next_loop_sample_frame = start_loop_sample_frame % loop_sample_frames.max(1);

        while samples.len() < chunk_sample_frames * channel_count {
            let remaining_frames = chunk_sample_frames - (samples.len() / channel_count);
            let frames_until_loop_end = loop_sample_frames.saturating_sub(next_loop_sample_frame);
            let request_frames = remaining_frames.min(frames_until_loop_end.max(1));
            let start_time_secs = next_loop_sample_frame as f64 / sample_rate.get() as f64;
            let chunk = render_audio_chunk(composition, session, start_time_secs, request_frames)?;
            let chunk_samples = chunk
                .map(|chunk| chunk.samples)
                .unwrap_or_else(|| vec![0.0; request_frames * channel_count]);
            let rendered_frames = chunk_samples.len() / channel_count;
            samples.extend_from_slice(&chunk_samples);
            if rendered_frames < request_frames {
                samples.resize(
                    samples.len() + (request_frames - rendered_frames) * channel_count,
                    0.0,
                );
            }

            next_loop_sample_frame += request_frames;
            if next_loop_sample_frame >= loop_sample_frames {
                next_loop_sample_frame = 0;
            }
        }

        Ok(samples)
    }

    struct MetalSkiaRenderTarget {
        layer: MetalLayer,
        command_queue: CommandQueue,
        skia: gpu::DirectContext,
        current_drawable: Option<MetalDrawable>,
        current_surface: Option<skia_safe::Surface>,
    }

    impl MetalSkiaRenderTarget {
        fn new(ns_view: *mut c_void, width: i32, height: i32, scale_factor: f64) -> Result<Self> {
            let device =
                Device::system_default().ok_or_else(|| anyhow!("no Metal device found"))?;
            let command_queue = device.new_command_queue();

            let layer = MetalLayer::new();
            layer.set_device(&device);
            layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
            layer.set_presents_with_transaction(false);
            layer.set_display_sync_enabled(true);
            layer.set_framebuffer_only(false);
            layer.set_contents_scale(scale_factor);
            layer.set_drawable_size(CGSize::new(width as f64, height as f64));

            // SAFETY: ns_view 来自 winit AppKit 句柄，当前线程有效。
            unsafe {
                let view: id = ns_view as id;
                view.setWantsLayer(YES);
                view.setLayer(std::mem::transmute(layer.as_ref()));
            }

            let backend = unsafe {
                mtl::BackendContext::new(
                    device.as_ptr() as mtl::Handle,
                    command_queue.as_ptr() as mtl::Handle,
                )
            };
            let skia = gpu::direct_contexts::make_metal(&backend, None)
                .ok_or_else(|| anyhow!("failed to create Skia Metal direct context"))?;

            Ok(Self {
                layer,
                command_queue,
                skia,
                current_drawable: None,
                current_surface: None,
            })
        }

        fn resize(&mut self, width: i32, height: i32, scale_factor: f64) {
            self.layer.set_contents_scale(scale_factor);
            self.layer
                .set_drawable_size(CGSize::new(width as f64, height as f64));
        }

        fn begin_frame(&mut self, width: i32, height: i32) -> Result<*mut c_void> {
            if width <= 0 || height <= 0 {
                return Err(anyhow!("invalid render target size {width}x{height}"));
            }

            self.resize(width, height, self.layer.contents_scale());

            let drawable_ref = self
                .layer
                .next_drawable()
                .ok_or_else(|| anyhow!("CAMetalLayer failed to produce drawable"))?;
            let drawable = drawable_ref.to_owned();

            let drawable_size = self.layer.drawable_size();
            let (drawable_width, drawable_height) =
                (drawable_size.width as i32, drawable_size.height as i32);

            if drawable_width <= 0 || drawable_height <= 0 {
                return Err(anyhow!(
                    "invalid drawable size {}x{}",
                    drawable_width,
                    drawable_height
                ));
            }

            let texture_info =
                unsafe { mtl::TextureInfo::new(drawable.texture().as_ptr() as mtl::Handle) };

            let backend_render_target =
                backend_render_targets::make_mtl((drawable_width, drawable_height), &texture_info);

            let surface = gpu::surfaces::wrap_backend_render_target(
                &mut self.skia,
                &backend_render_target,
                SurfaceOrigin::TopLeft,
                ColorType::BGRA8888,
                None,
                None,
            )
            .ok_or_else(|| anyhow!("failed to wrap metal backend render target"))?;

            self.current_drawable = Some(drawable);
            self.current_surface = Some(surface);
            Ok(self as *mut Self as *mut c_void)
        }

        fn end_frame(&mut self) -> Result<()> {
            let surface = self
                .current_surface
                .take()
                .ok_or_else(|| anyhow!("end_frame called before begin_frame"))?;
            self.skia.flush_and_submit();
            drop(surface);
            Ok(())
        }

        fn present_frame(&mut self) -> Result<()> {
            let drawable = self
                .current_drawable
                .take()
                .ok_or_else(|| anyhow!("present_frame called before end_frame"))?;
            let command_buffer = self.command_queue.new_command_buffer();
            command_buffer.present_drawable(&drawable);
            command_buffer.commit();
            Ok(())
        }

        unsafe fn begin_frame_bridge(
            user_data: *mut c_void,
            width: i32,
            height: i32,
        ) -> Result<*mut c_void> {
            // SAFETY: user_data 来自 Box<MetalSkiaRenderTarget> 的稳定地址。
            unsafe { &mut *(user_data as *mut Self) }.begin_frame(width, height)
        }

        unsafe fn end_frame_bridge(user_data: *mut c_void) -> Result<()> {
            // SAFETY: user_data 来自 Box<MetalSkiaRenderTarget> 的稳定地址。
            unsafe { &mut *(user_data as *mut Self) }.end_frame()
        }

        unsafe fn resolve_skia_canvas_bridge(
            _user_data: *mut c_void,
            frame_surface: *mut c_void,
        ) -> Result<*mut c_void> {
            let target = unsafe { &mut *(frame_surface as *mut Self) };
            let surface = target.current_surface.as_mut().ok_or_else(|| {
                anyhow!("skia canvas requested before drawable surface was ready")
            })?;
            Ok(surface.canvas() as *const _ as *mut c_void)
        }

        unsafe fn present_frame_bridge(user_data: *mut c_void) -> Result<()> {
            // SAFETY: user_data 来自 Box<MetalSkiaRenderTarget> 的稳定地址。
            unsafe { &mut *(user_data as *mut Self) }.present_frame()
        }
    }

    pub fn run() -> Result<()> {
        let input_path = std::env::args()
            .nth(1)
            .ok_or_else(|| anyhow!("usage: cargo run -p opencat-player -- <input.jsonl>"))?;
        let input = std::fs::read_to_string(&input_path)
            .with_context(|| format!("failed to read jsonl file: {input_path}"))?;

        let parsed = parse(&input).context("failed to parse JSONL composition")?;
        let has_audio = !parsed.audio_sources.is_empty();
        let mut root = parsed.root;
        if let Some(script) = parsed.script.as_deref() {
            if !script.trim().is_empty() {
                let driver = ScriptDriver::from_source(script)
                    .context("failed to compile global script from JSONL")?;
                root = root.script_driver(driver);
            }
        }

        let composition = Composition::new("player")
            .size(parsed.width, parsed.height)
            .fps(parsed.fps as u32)
            .frames(parsed.frames as u32)
            .audio_sources(parsed.audio_sources.clone())
            .root(move |_ctx: &FrameCtx| root.clone())
            .build()
            .context("failed to build composition")?;

        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title("OpenCat Player")
            .with_inner_size(LogicalSize::new(parsed.width as f64, parsed.height as f64))
            .with_resizable(false)
            .build(&event_loop)
            .context("failed to create window")?;

        let raw = window.raw_window_handle();
        let (ns_window, ns_view) = match raw {
            RawWindowHandle::AppKit(handle) => (handle.ns_window, handle.ns_view),
            _ => return Err(anyhow!("expected AppKit window handle on macOS")),
        };
        if ns_window.is_null() || ns_view.is_null() {
            return Err(anyhow!("AppKit window handles are null"));
        }

        let mut metal_target = Box::new(MetalSkiaRenderTarget::new(
            ns_view,
            parsed.width,
            parsed.height,
            window.scale_factor(),
        )?);
        let target_ptr = metal_target.as_mut() as *mut MetalSkiaRenderTarget as *mut c_void;
        let mut render_target = RenderTargetHandle::new(
            RenderFrameViewKind::DrawContext2D,
            target_ptr,
            MetalSkiaRenderTarget::begin_frame_bridge,
            MetalSkiaRenderTarget::end_frame_bridge,
        )
        .with_frame_view_resolver(MetalSkiaRenderTarget::resolve_skia_canvas_bridge)
        .with_present_frame(MetalSkiaRenderTarget::present_frame_bridge);

        let mut session = RenderSession::new();
        let total_frames = composition.frames.max(1);
        let fps = composition.fps.max(1);

        let frame_duration = Duration::from_secs_f64(1.0 / fps as f64);
        if let Err(error) =
            composition.render_frame_with_target(0, &mut session, &mut render_target)
        {
            return Err(anyhow!("failed to render warmup frame: {error:#}"));
        }
        if let Err(error) = render_target.present_frame() {
            return Err(anyhow!("failed to present warmup frame: {error:#}"));
        }
        std::thread::sleep(Duration::from_millis(100));

        let audio_playback = AudioPlayback::new(composition.clone(), has_audio)?;
        let mut next_tick = audio_playback
            .as_ref()
            .map(|playback| playback.next_redraw_deadline(fps))
            .unwrap_or_else(|| Instant::now() + frame_duration);
        let mut fallback_frame_index = 1 % total_frames;

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::WaitUntil(next_tick);

            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    *control_flow = ControlFlow::Exit;
                }
                Event::MainEventsCleared => {
                    if Instant::now() >= next_tick {
                        window.request_redraw();
                    }
                }
                Event::RedrawRequested(_) => {
                    let frame_index = audio_playback
                        .as_ref()
                        .map(|playback| playback.frame_index(total_frames, fps))
                        .unwrap_or(fallback_frame_index);
                    if let Err(error) = composition.render_frame_with_target(
                        frame_index,
                        &mut session,
                        &mut render_target,
                    ) {
                        eprintln!("render error: {error:#}");
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                    if let Err(error) = render_target.present_frame() {
                        eprintln!("present error: {error:#}");
                        *control_flow = ControlFlow::Exit;
                        return;
                    }

                    if let Some(playback) = audio_playback.as_ref() {
                        next_tick = playback.next_redraw_deadline(fps);
                    } else {
                        fallback_frame_index = (frame_index + 1) % total_frames;
                        next_tick = Instant::now() + frame_duration;
                    }
                }
                _ => {}
            }
        });
    }

    fn composition_sample_frames(frames: u32, fps: u32, sample_rate: NonZeroU32) -> usize {
        ((frames as u64 * sample_rate.get() as u64) / fps.max(1) as u64) as usize
    }
}

#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    app::run()
}
