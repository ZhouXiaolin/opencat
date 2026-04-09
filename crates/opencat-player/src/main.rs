#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("opencat-player 目前仅支持 macOS");
}

#[cfg(target_os = "macos")]
mod app {
    use std::ffi::c_void;
    use std::num::{NonZeroU16, NonZeroU32};
    use std::time::{Duration, Instant};

    use anyhow::{Context, Result, anyhow};
    use cocoa::appkit::NSView;
    use cocoa::base::{YES, id};
    use core_graphics_types::geometry::CGSize;
    use foreign_types::{ForeignType, ForeignTypeRef};
    use metal::{CommandQueue, Device, MTLPixelFormat, MetalDrawable, MetalLayer};
    use opencat::{
        Composition, FrameCtx, RenderFrameViewKind, RenderSession, RenderTargetHandle,
        ScriptDriver, codec::decode::AudioTrack, parse,
    };
    use opencat::render::build_audio_track;
    use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
    use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, Source, buffer::SamplesBuffer};
    use skia_safe::{
        ColorType,
        gpu::{self, SurfaceOrigin, backend_render_targets, mtl},
    };
    use winit::dpi::LogicalSize;
    use winit::event::{Event, WindowEvent};
    use winit::event_loop::{ControlFlow, EventLoop};
    use winit::window::WindowBuilder;

    struct AudioPlayback {
        _sink: MixerDeviceSink,
        player: Player,
    }

    impl AudioPlayback {
        fn new(track: AudioTrack) -> Result<Self> {
            let sink = DeviceSinkBuilder::open_default_sink()
                .context("failed to open default audio output device")?;
            let player = Player::connect_new(&sink.mixer());
            let channels = NonZeroU16::new(track.channels)
                .ok_or_else(|| anyhow!("audio track channel count must be non-zero"))?;
            let sample_rate = NonZeroU32::new(track.sample_rate)
                .ok_or_else(|| anyhow!("audio track sample rate must be non-zero"))?;
            let source = SamplesBuffer::new(channels, sample_rate, track.samples).repeat_infinite();
            player.append(source);
            Ok(Self {
                _sink: sink,
                player,
            })
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
            .global_audio_sources(parsed.global_audio_sources.clone())
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

        let audio_playback = build_audio_track(&composition, &mut session)?
            .map(AudioPlayback::new)
            .transpose()?;
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
}

#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    app::run()
}
