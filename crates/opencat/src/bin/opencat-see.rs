#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn main() {
    eprintln!("opencat-see 目前仅支持 macOS / Windows");
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod app {
    use std::ffi::c_void;
    use std::num::{NonZeroU16, NonZeroU32};
    use std::sync::mpsc::{self, Receiver};
    use std::time::{Duration, Instant};

    use anyhow::{Context, Result, anyhow};
    use opencat::{
        Composition, FrameCtx, RenderFrameViewKind, RenderSession, RenderTargetHandle,
        ScriptDriver, parse_file, render_audio_chunk,
    };
    use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
    use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, Source};
    use winit::dpi::LogicalSize;
    use winit::event::{Event, WindowEvent};
    use winit::event_loop::{ControlFlow, EventLoop};
    use winit::window::{Window, WindowBuilder};

    #[cfg(target_os = "macos")]
    use cocoa::appkit::NSView;
    #[cfg(target_os = "macos")]
    use cocoa::base::{YES, id};
    #[cfg(target_os = "macos")]
    use core_graphics_types::geometry::CGSize;
    #[cfg(target_os = "macos")]
    use foreign_types::{ForeignType, ForeignTypeRef};
    #[cfg(target_os = "macos")]
    use metal::{CommandQueue, Device, MTLPixelFormat, MetalDrawable, MetalLayer};
    #[cfg(target_os = "macos")]
    use skia_safe::{
        ColorType,
        gpu::{self, SurfaceOrigin, backend_render_targets, mtl},
    };

    #[cfg(target_os = "windows")]
    use skia_safe::{
        ColorType,
        gpu::{self, SurfaceOrigin, backend_render_targets, gl},
    };
    #[cfg(target_os = "windows")]
    use std::ffi::CString;
    #[cfg(target_os = "windows")]
    use windows_sys::Win32::{
        Foundation::{FreeLibrary, HMODULE, HWND},
        Graphics::{
            Gdi::{GetDC, HDC, ReleaseDC},
            OpenGL::{
                ChoosePixelFormat, HGLRC, PFD_DOUBLEBUFFER, PFD_DRAW_TO_WINDOW, PFD_MAIN_PLANE,
                PFD_SUPPORT_OPENGL, PFD_TYPE_RGBA, PIXELFORMATDESCRIPTOR, SetPixelFormat,
                SwapBuffers, wglCreateContext, wglDeleteContext, wglGetProcAddress, wglMakeCurrent,
            },
        },
        System::LibraryLoader::{GetProcAddress, LoadLibraryA},
    };

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

    #[cfg(target_os = "macos")]
    struct MetalSkiaRenderTarget {
        layer: MetalLayer,
        command_queue: CommandQueue,
        skia: gpu::DirectContext,
        current_drawable: Option<MetalDrawable>,
        current_surface: Option<skia_safe::Surface>,
    }

    #[cfg(target_os = "macos")]
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
            unsafe { &mut *(user_data as *mut Self) }.begin_frame(width, height)
        }

        unsafe fn end_frame_bridge(user_data: *mut c_void) -> Result<()> {
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
            unsafe { &mut *(user_data as *mut Self) }.present_frame()
        }
    }

    #[cfg(target_os = "macos")]
    fn build_platform_render_target(
        window: &Window,
        width: i32,
        height: i32,
    ) -> Result<(Box<MetalSkiaRenderTarget>, RenderTargetHandle)> {
        let raw = window.raw_window_handle();
        let (ns_window, ns_view) = match raw {
            RawWindowHandle::AppKit(handle) => (handle.ns_window, handle.ns_view),
            _ => return Err(anyhow!("expected AppKit window handle on macOS")),
        };
        if ns_window.is_null() || ns_view.is_null() {
            return Err(anyhow!("AppKit window handles are null"));
        }

        let mut target = Box::new(MetalSkiaRenderTarget::new(
            ns_view,
            width,
            height,
            window.scale_factor(),
        )?);
        let target_ptr = target.as_mut() as *mut MetalSkiaRenderTarget as *mut c_void;
        let render_target = RenderTargetHandle::new(
            RenderFrameViewKind::DrawContext2D,
            target_ptr,
            MetalSkiaRenderTarget::begin_frame_bridge,
            MetalSkiaRenderTarget::end_frame_bridge,
        )
        .with_frame_view_resolver(MetalSkiaRenderTarget::resolve_skia_canvas_bridge)
        .with_present_frame(MetalSkiaRenderTarget::present_frame_bridge);
        Ok((target, render_target))
    }

    #[cfg(target_os = "windows")]
    struct WglSkiaRenderTarget {
        hwnd: HWND,
        hdc: HDC,
        glrc: HGLRC,
        opengl32: HMODULE,
        skia: gpu::DirectContext,
        current_surface: Option<skia_safe::Surface>,
    }

    #[cfg(target_os = "windows")]
    impl WglSkiaRenderTarget {
        fn new(hwnd: HWND, _width: i32, _height: i32) -> Result<Self> {
            if hwnd.is_null() {
                return Err(anyhow!("Win32 window handle is null"));
            }

            let hdc = unsafe { GetDC(hwnd) };
            if hdc.is_null() {
                return Err(anyhow!("GetDC returned null for Win32 window"));
            }

            let pixel_format_descriptor = PIXELFORMATDESCRIPTOR {
                nSize: std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16,
                nVersion: 1,
                dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER,
                iPixelType: PFD_TYPE_RGBA,
                cColorBits: 32,
                cAlphaBits: 8,
                cDepthBits: 24,
                cStencilBits: 8,
                iLayerType: PFD_MAIN_PLANE as u8,
                ..Default::default()
            };

            let pixel_format = unsafe { ChoosePixelFormat(hdc, &pixel_format_descriptor) };
            if pixel_format == 0 {
                unsafe {
                    let _ = ReleaseDC(hwnd, hdc);
                }
                return Err(anyhow!("ChoosePixelFormat failed for Win32 OpenGL window"));
            }

            if unsafe { SetPixelFormat(hdc, pixel_format, &pixel_format_descriptor) } == 0 {
                unsafe {
                    let _ = ReleaseDC(hwnd, hdc);
                }
                return Err(anyhow!("SetPixelFormat failed for Win32 OpenGL window"));
            }

            let glrc = unsafe { wglCreateContext(hdc) };
            if glrc.is_null() {
                unsafe {
                    let _ = ReleaseDC(hwnd, hdc);
                }
                return Err(anyhow!("wglCreateContext failed for Win32 window"));
            }

            if unsafe { wglMakeCurrent(hdc, glrc) } == 0 {
                unsafe {
                    let _ = wglDeleteContext(glrc);
                    let _ = ReleaseDC(hwnd, hdc);
                }
                return Err(anyhow!(
                    "wglMakeCurrent failed while creating Win32 context"
                ));
            }

            let opengl32 = unsafe { LoadLibraryA(b"opengl32.dll\0".as_ptr()) };
            if opengl32.is_null() {
                unsafe {
                    let _ = wglMakeCurrent(hdc, std::ptr::null_mut());
                    let _ = wglDeleteContext(glrc);
                    let _ = ReleaseDC(hwnd, hdc);
                }
                return Err(anyhow!("LoadLibraryA(opengl32.dll) failed"));
            }

            let interface =
                gl::Interface::new_load_with(|name| load_gl_proc_address(opengl32, name))
                    .ok_or_else(|| anyhow!("failed to create Skia OpenGL interface"))?;
            let skia = gpu::direct_contexts::make_gl(interface, None)
                .ok_or_else(|| anyhow!("failed to create Skia OpenGL direct context"))?;

            Ok(Self {
                hwnd,
                hdc,
                glrc,
                opengl32,
                skia,
                current_surface: None,
            })
        }

        fn make_current(&self) -> Result<()> {
            if unsafe { wglMakeCurrent(self.hdc, self.glrc) } == 0 {
                return Err(anyhow!("wglMakeCurrent failed for Win32 render target"));
            }
            Ok(())
        }

        fn begin_frame(&mut self, width: i32, height: i32) -> Result<*mut c_void> {
            if width <= 0 || height <= 0 {
                return Err(anyhow!("invalid render target size {width}x{height}"));
            }

            self.make_current()?;

            let backend_render_target = backend_render_targets::make_gl(
                (width, height),
                0,
                8,
                gl::FramebufferInfo {
                    fboid: 0,
                    format: gl::Format::RGBA8.into(),
                    ..Default::default()
                },
            );

            let surface = gpu::surfaces::wrap_backend_render_target(
                &mut self.skia,
                &backend_render_target,
                SurfaceOrigin::BottomLeft,
                ColorType::RGBA8888,
                None,
                None,
            )
            .ok_or_else(|| anyhow!("failed to wrap Win32 OpenGL framebuffer for Skia"))?;

            self.current_surface = Some(surface);
            Ok(self as *mut Self as *mut c_void)
        }

        fn end_frame(&mut self) -> Result<()> {
            let mut surface = self
                .current_surface
                .take()
                .ok_or_else(|| anyhow!("end_frame called before begin_frame"))?;
            self.skia.flush_and_submit_surface(&mut surface, None);
            drop(surface);
            Ok(())
        }

        fn present_frame(&mut self) -> Result<()> {
            self.make_current()?;
            if unsafe { SwapBuffers(self.hdc) } == 0 {
                return Err(anyhow!("SwapBuffers failed for Win32 window"));
            }
            Ok(())
        }

        unsafe fn begin_frame_bridge(
            user_data: *mut c_void,
            width: i32,
            height: i32,
        ) -> Result<*mut c_void> {
            unsafe { &mut *(user_data as *mut Self) }.begin_frame(width, height)
        }

        unsafe fn end_frame_bridge(user_data: *mut c_void) -> Result<()> {
            unsafe { &mut *(user_data as *mut Self) }.end_frame()
        }

        unsafe fn resolve_skia_canvas_bridge(
            _user_data: *mut c_void,
            frame_surface: *mut c_void,
        ) -> Result<*mut c_void> {
            let target = unsafe { &mut *(frame_surface as *mut Self) };
            let surface = target.current_surface.as_mut().ok_or_else(|| {
                anyhow!("skia canvas requested before Win32 framebuffer surface was ready")
            })?;
            Ok(surface.canvas() as *const _ as *mut c_void)
        }

        unsafe fn present_frame_bridge(user_data: *mut c_void) -> Result<()> {
            unsafe { &mut *(user_data as *mut Self) }.present_frame()
        }
    }

    #[cfg(target_os = "windows")]
    impl Drop for WglSkiaRenderTarget {
        fn drop(&mut self) {
            self.current_surface.take();
            self.skia.release_resources_and_abandon();

            unsafe {
                let _ = wglMakeCurrent(self.hdc, std::ptr::null_mut());
                if !self.glrc.is_null() {
                    let _ = wglDeleteContext(self.glrc);
                }
                if !self.hdc.is_null() {
                    let _ = ReleaseDC(self.hwnd, self.hdc);
                }
                if !self.opengl32.is_null() {
                    let _ = FreeLibrary(self.opengl32);
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn build_platform_render_target(
        window: &Window,
        width: i32,
        height: i32,
    ) -> Result<(Box<WglSkiaRenderTarget>, RenderTargetHandle)> {
        let raw = window.raw_window_handle();
        let hwnd = match raw {
            RawWindowHandle::Win32(handle) => handle.hwnd as HWND,
            _ => return Err(anyhow!("expected Win32 window handle on Windows")),
        };
        if hwnd.is_null() {
            return Err(anyhow!("Win32 window handle is null"));
        }

        let mut target = Box::new(WglSkiaRenderTarget::new(hwnd, width, height)?);
        let target_ptr = target.as_mut() as *mut WglSkiaRenderTarget as *mut c_void;
        let render_target = RenderTargetHandle::new(
            RenderFrameViewKind::DrawContext2D,
            target_ptr,
            WglSkiaRenderTarget::begin_frame_bridge,
            WglSkiaRenderTarget::end_frame_bridge,
        )
        .with_frame_view_resolver(WglSkiaRenderTarget::resolve_skia_canvas_bridge)
        .with_present_frame(WglSkiaRenderTarget::present_frame_bridge);
        Ok((target, render_target))
    }

    #[cfg(target_os = "windows")]
    fn load_gl_proc_address(opengl32: HMODULE, name: &str) -> *const c_void {
        let Ok(name) = CString::new(name) else {
            return std::ptr::null();
        };

        let wgl_proc = unsafe { wglGetProcAddress(name.as_ptr() as *const u8) };
        if let Some(proc) = wgl_proc {
            let raw = proc as *const () as usize;
            if !matches!(raw, 0 | 1 | 2 | 3 | usize::MAX) {
                return proc as *const () as *const c_void;
            }
        }

        unsafe { GetProcAddress(opengl32, name.as_ptr() as *const u8) }
            .map(|proc| proc as *const () as *const c_void)
            .unwrap_or(std::ptr::null())
    }

    pub fn run() -> Result<()> {
        let input_path = std::env::args()
            .nth(1)
            .ok_or_else(|| anyhow!("usage: opencat-see <input.jsonl>"))?;
        let parsed = parse_file(&input_path).context("failed to parse JSONL composition")?;
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
            .with_title("OpenCat See")
            .with_inner_size(LogicalSize::new(parsed.width as f64, parsed.height as f64))
            .with_resizable(false)
            .build(&event_loop)
            .context("failed to create window")?;

        let (mut _platform_target, mut render_target) =
            build_platform_render_target(&window, parsed.width, parsed.height)?;

        let mut session = RenderSession::new();
        let total_frames = composition.frames.max(1);
        let fps = composition.fps.max(1);

        let frame_duration = Duration::from_secs_f64(1.0 / fps as f64);
        if let Err(error) =
            opencat::render_frame_with_target(&composition, 0, &mut session, &mut render_target)
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
                    if let Err(error) = opencat::render_frame_with_target(
                        &composition,
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

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn main() -> anyhow::Result<()> {
    app::run()
}
