#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn main() {
    eprintln!("opencat-see 目前仅支持 macOS / Windows / Linux");
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
mod app {
    use std::ffi::c_void;
    use std::num::{NonZeroU16, NonZeroU32};
    use std::time::{Duration, Instant};

    use anyhow::{Context, Result, anyhow};
    use std::path::Path;
    use std::sync::Arc;

    use opencat::{
        EngineDrawExecutor, EngineLoader, MediaContext, RqJsContext, build_audio_track_from_pipeline,
        duration_secs_to_frames, execute_render_frame, open,
    };
    use opencat_core::ir::GeneratedImageId;
    use skia_safe::Image;
    use std::collections::HashMap;
    use opencat_core::script::js_context::JsContext;
    use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle};
    use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, Source};
    use winit::dpi::PhysicalSize;
    use winit::event::{Event, WindowEvent};
    use winit::event_loop::ControlFlow;
    #[cfg(not(target_os = "linux"))]
    use winit::event_loop::EventLoop;
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

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    use skia_safe::{
        ColorType,
        gpu::{self, SurfaceOrigin, backend_render_targets, gl},
    };
    #[cfg(any(target_os = "windows", target_os = "linux"))]
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
    #[cfg(target_os = "linux")]
    use khronos_egl as egl;

    const AUDIO_SAMPLE_RATE: u32 = 48_000;
    const AUDIO_CHANNELS: u16 = 2;

    struct AudioRenderSource {
        samples: Arc<Vec<f32>>,
        sample_rate: NonZeroU32,
        channels: NonZeroU16,
        position: usize,
        loop_sample_frames: usize,
    }

    impl Iterator for AudioRenderSource {
        type Item = f32;

        fn next(&mut self) -> Option<Self::Item> {
            let total = self.loop_sample_frames.saturating_mul(self.channels.get() as usize);
            if total == 0 {
                return Some(0.0);
            }
            if self.position >= total {
                self.position = 0;
            }
            let sample = self.samples.get(self.position).copied().unwrap_or(0.0);
            self.position += 1;
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
        fn new(
            samples: Option<Arc<Vec<f32>>>,
            loop_sample_frames: usize,
        ) -> Result<Option<Self>> {
            let Some(samples) = samples else {
                return Ok(None);
            };
            if samples.is_empty() || loop_sample_frames == 0 {
                return Ok(None);
            }

            let sample_rate = NonZeroU32::new(AUDIO_SAMPLE_RATE)
                .ok_or_else(|| anyhow!("audio sample rate must be non-zero"))?;
            let channels = NonZeroU16::new(AUDIO_CHANNELS)
                .ok_or_else(|| anyhow!("audio channel count must be non-zero"))?;
            let source = AudioRenderSource {
                samples,
                sample_rate,
                channels,
                position: 0,
                loop_sample_frames,
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

        fn canvas_mut(&mut self) -> Result<&mut skia_safe::Canvas> {
            let surface = self
                .current_surface
                .as_mut()
                .ok_or_else(|| anyhow!("skia canvas requested before drawable surface was ready"))?;
            // SAFETY: skia_safe::Canvas has interior mutability; surface owns the canvas.
            #[allow(invalid_reference_casting)]
            Ok(unsafe {
                &mut *(surface.canvas() as *const skia_safe::Canvas as *mut skia_safe::Canvas)
            })
        }

    }

    #[cfg(target_os = "macos")]
    fn build_platform_render_target(
        window: &Window,
        width: i32,
        height: i32,
    ) -> Result<Box<MetalSkiaRenderTarget>> {
        let raw = window.raw_window_handle();
        let (ns_window, ns_view) = match raw {
            RawWindowHandle::AppKit(handle) => (handle.ns_window, handle.ns_view),
            _ => return Err(anyhow!("expected AppKit window handle on macOS")),
        };
        if ns_window.is_null() || ns_view.is_null() {
            return Err(anyhow!("AppKit window handles are null"));
        }

        Ok(Box::new(MetalSkiaRenderTarget::new(
            ns_view,
            width,
            height,
            window.scale_factor(),
        )?))
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

        fn canvas_mut(&mut self) -> Result<&mut skia_safe::Canvas> {
            let surface = self.current_surface.as_mut().ok_or_else(|| {
                anyhow!("skia canvas requested before Win32 framebuffer surface was ready")
            })?;
            #[allow(invalid_reference_casting)]
            Ok(unsafe {
                &mut *(surface.canvas() as *const skia_safe::Canvas as *mut skia_safe::Canvas)
            })
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
    ) -> Result<Box<WglSkiaRenderTarget>> {
        let raw = window.raw_window_handle();
        let hwnd = match raw {
            RawWindowHandle::Win32(handle) => handle.hwnd as HWND,
            _ => return Err(anyhow!("expected Win32 window handle on Windows")),
        };
        if hwnd.is_null() {
            return Err(anyhow!("Win32 window handle is null"));
        }

        Ok(Box::new(WglSkiaRenderTarget::new(hwnd, width, height)?))
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

    #[cfg(target_os = "linux")]
    struct EglSkiaRenderTarget {
        egl: egl::DynamicInstance<egl::EGL1_5>,
        display: egl::Display,
        // Kept for diagnostics; not read after construction.
        _config: egl::Config,
        context: egl::Context,
        surface: egl::Surface,
        /// libGL handle for core GL 1.x symbol fallback. EGL's `eglGetProcAddress`
        /// does not reliably expose core entry points (glGetString, glGetIntegerv,
        /// ...) on Mesa; Skia needs them while assembling the GL interface, so
        /// resolve them from libGL itself.
        libgl: *mut c_void,
        skia: gpu::DirectContext,
        current_surface: Option<skia_safe::Surface>,
    }

    // SAFETY: EGL handles are only touched on the winit event-loop thread.
    #[cfg(target_os = "linux")]
    unsafe impl Send for EglSkiaRenderTarget {}

    #[cfg(target_os = "linux")]
    impl EglSkiaRenderTarget {
        fn new(display_handle: *mut c_void, window: u64) -> Result<Self> {
            if display_handle.is_null() {
                return Err(anyhow!("X11 display handle is null"));
            }
            if window == 0 {
                return Err(anyhow!("X11 window id is zero"));
            }

            // Skia's binary-cache build links EGL internally (its GL proc resolver
            // calls `eglGetProcAddress`), so EGL — not GLX — is the required
            // backend on Linux. `load_required()` fails if libEGL.so.1 is missing
            // or predates EGL 1.5.
            let egl = unsafe { egl::DynamicInstance::<egl::EGL1_5>::load_required() }
                .map_err(|e| anyhow!("failed to load libEGL.so.1: {e}"))?;

            // EGL defaults to OpenGL ES; bind desktop GL so `make_gl` succeeds.
            egl.bind_api(egl::OPENGL_API)
                .map_err(|e| anyhow!("eglBindAPI(OPENGL) failed: {e}"))?;

            let display = unsafe { egl.get_display(display_handle as egl::NativeDisplayType) }
                .ok_or_else(|| anyhow!("eglGetDisplay returned EGL_NO_DISPLAY"))?;
            let (_major, _minor) = egl
                .initialize(display)
                .map_err(|e| anyhow!("eglInitialize failed: {e}"))?;

            // RGBA8 window surface, OpenGL-desktop renderable.
            let config_attrs: [egl::Int; 13] = [
                egl::SURFACE_TYPE,
                egl::WINDOW_BIT,
                egl::RENDERABLE_TYPE,
                egl::OPENGL_BIT,
                egl::RED_SIZE,
                8,
                egl::GREEN_SIZE,
                8,
                egl::BLUE_SIZE,
                8,
                egl::ALPHA_SIZE,
                8,
                egl::NONE,
            ];
            let config = egl
                .choose_first_config(display, &config_attrs)
                .map_err(|e| anyhow!("eglChooseConfig failed: {e}"))?
                .ok_or_else(|| anyhow!("no matching EGL config (need RGBA8 + OPENGL_BIT)"))?;

            // OpenGL 3.3 core profile — the lowest desktop core context the
            // binary-cache Skia targets.
            let context_attrs: [egl::Int; 7] = [
                egl::CONTEXT_MAJOR_VERSION,
                3,
                egl::CONTEXT_MINOR_VERSION,
                3,
                egl::CONTEXT_OPENGL_PROFILE_MASK,
                egl::CONTEXT_OPENGL_CORE_PROFILE_BIT,
                egl::NONE,
            ];
            let context = egl
                .create_context(display, config, None, &context_attrs)
                .map_err(|e| anyhow!("eglCreateContext failed: {e}"))?;

            let surface = unsafe {
                egl.create_window_surface(
                    display,
                    config,
                    window as egl::NativeWindowType,
                    None,
                )
                .map_err(|e| anyhow!("eglCreateWindowSurface failed: {e}"))?
            };

            egl.make_current(display, Some(surface), Some(surface), Some(context))
                .map_err(|e| anyhow!("eglMakeCurrent failed: {e}"))?;

            // Resolve core GL 1.x/2.x entry points from libGL itself. Assign to
            // the outer binding — a `let` here would shadow it and leave
            // `self.libgl` null, which makes Skia dereference bad pointers and
            // segfault inside GrGLExtensions::init.
            let mut libgl = unsafe {
                libc::dlopen(
                    b"libGL.so.1\0".as_ptr() as *const _,
                    libc::RTLD_LAZY | libc::RTLD_GLOBAL,
                )
            };
            if libgl.is_null() {
                libgl = unsafe {
                    libc::dlopen(
                        b"libGL.so\0".as_ptr() as *const _,
                        libc::RTLD_LAZY | libc::RTLD_GLOBAL,
                    )
                };
            }
            if libgl.is_null() {
                let _ = egl.make_current(display, None, None, None);
                let _ = egl.destroy_context(display, context);
                let _ = egl.destroy_surface(display, surface);
                let _ = egl.terminate(display);
                return Err(anyhow!("dlopen(libGL.so.1 / libGL.so) failed"));
            }

            let interface = gl::Interface::new_load_with(|name| {
                load_egl_proc_address(&egl, libgl, name)
            })
            .ok_or_else(|| anyhow!("failed to create Skia OpenGL interface (EGL)"))?;
            let skia = gpu::direct_contexts::make_gl(interface, None)
                .ok_or_else(|| anyhow!("failed to create Skia OpenGL direct context (EGL)"))?;

            Ok(Self {
                egl,
                display,
                _config: config,
                context,
                surface,
                libgl,
                skia,
                current_surface: None,
            })
        }

        fn make_current(&self) -> Result<()> {
            self.egl
                .make_current(
                    self.display,
                    Some(self.surface),
                    Some(self.surface),
                    Some(self.context),
                )
                .map_err(|e| anyhow!("eglMakeCurrent failed for Linux render target: {e}"))
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
            .ok_or_else(|| anyhow!("failed to wrap Linux OpenGL framebuffer for Skia"))?;

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
            self.egl
                .swap_buffers(self.display, self.surface)
                .map_err(|e| anyhow!("eglSwapBuffers failed: {e}"))?;
            Ok(())
        }

        fn canvas_mut(&mut self) -> Result<&mut skia_safe::Canvas> {
            let surface = self.current_surface.as_mut().ok_or_else(|| {
                anyhow!("skia canvas requested before Linux framebuffer surface was ready")
            })?;
            #[allow(invalid_reference_casting)]
            Ok(unsafe {
                &mut *(surface.canvas() as *const skia_safe::Canvas as *mut skia_safe::Canvas)
            })
        }
    }

    #[cfg(target_os = "linux")]
    impl Drop for EglSkiaRenderTarget {
        fn drop(&mut self) {
            self.current_surface.take();
            self.skia.release_resources_and_abandon();
            // Unbind the context before tearing it down.
            let _ = self
                .egl
                .make_current(self.display, None, None, None);
            let _ = self.egl.destroy_context(self.display, self.context);
            let _ = self.egl.destroy_surface(self.display, self.surface);
            let _ = self.egl.terminate(self.display);
            if !self.libgl.is_null() {
                unsafe {
                    let _ = libc::dlclose(self.libgl);
                }
                self.libgl = std::ptr::null_mut();
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn build_platform_render_target(
        window: &Window,
        _width: i32,
        _height: i32,
    ) -> Result<Box<EglSkiaRenderTarget>> {
        let display_handle = match window.raw_display_handle() {
            RawDisplayHandle::Xlib(handle) => {
                if handle.display.is_null() {
                    return Err(anyhow!("Xlib display handle is null"));
                }
                handle.display
            }
            _ => {
                return Err(anyhow!(
                    "opencat-see on Linux currently requires X11 (force the winit X11 backend)"
                ));
            }
        };
        let xwindow = match window.raw_window_handle() {
            RawWindowHandle::Xlib(handle) => handle.window,
            _ => {
                return Err(anyhow!(
                    "expected Xlib window handle on Linux; Wayland is not supported yet"
                ));
            }
        };
        Ok(Box::new(EglSkiaRenderTarget::new(display_handle, xwindow)?))
    }

    #[cfg(target_os = "linux")]
    fn load_egl_proc_address(
        egl: &egl::DynamicInstance<egl::EGL1_5>,
        libgl: *mut c_void,
        name: &str,
    ) -> *const c_void {
        let Ok(cname) = CString::new(name) else {
            return std::ptr::null();
        };

        // Core GL entry points (glGetString, glGetIntegerv, ...) must be resolved
        // from libGL first. Mesa's `eglGetProcAddress` historically returns a
        // non-null but un-callable pointer for them; Skia then dereferences the
        // junk pointer and segfaults inside `GrGLExtensions::init` while it probes
        // GL_NUM_EXTENSIONS. Only extension / GL-version-specific entry points
        // (glGetStringi, ...) legitimately come from `eglGetProcAddress`.
        if !libgl.is_null() {
            let sym = unsafe { libc::dlsym(libgl, cname.as_ptr()) };
            if !sym.is_null() {
                return sym as *const c_void;
            }
        }

        if let Some(proc) = egl.get_proc_address(name) {
            let raw = proc as *const () as usize;
            // Treat Mesa's small error sentinels for missing procs as null.
            if !matches!(raw, 0 | 1 | 2 | 3 | usize::MAX) {
                return proc as *const () as *const c_void;
            }
        }

        std::ptr::null()
    }


    fn render_pipeline_frame_to_gpu_target(
        pipeline: &mut opencat::EnginePipeline,
        media_ctx: &mut MediaContext,
        executor: &mut EngineDrawExecutor,
        generated_cache: &mut HashMap<GeneratedImageId, Image>,
        gpu_target: &mut impl GpuRenderTarget,
        width: i32,
        height: i32,
        frame_index: u32,
    ) -> Result<()> {
        gpu_target.begin_frame(width, height)?;
        let canvas = gpu_target.canvas_mut()?;
        let render = pipeline.render_frame(frame_index)?;
        let mut frame = render.draw;
        let media_plan = render.media;
        execute_render_frame(
            &mut frame,
            &media_plan,
            executor,
            pipeline.loader(),
            media_ctx,
            generated_cache,
            canvas,
        )?;
        gpu_target.end_frame()?;
        gpu_target.present_frame()?;
        Ok(())
    }

    trait GpuRenderTarget {
        fn begin_frame(&mut self, width: i32, height: i32) -> Result<*mut c_void>;
        fn end_frame(&mut self) -> Result<()>;
        fn present_frame(&mut self) -> Result<()>;
        fn canvas_mut(&mut self) -> Result<&mut skia_safe::Canvas>;
    }

    #[cfg(target_os = "macos")]
    impl GpuRenderTarget for MetalSkiaRenderTarget {
        fn begin_frame(&mut self, width: i32, height: i32) -> Result<*mut c_void> {
            MetalSkiaRenderTarget::begin_frame(self, width, height)
        }
        fn end_frame(&mut self) -> Result<()> {
            MetalSkiaRenderTarget::end_frame(self)
        }
        fn present_frame(&mut self) -> Result<()> {
            MetalSkiaRenderTarget::present_frame(self)
        }
        fn canvas_mut(&mut self) -> Result<&mut skia_safe::Canvas> {
            MetalSkiaRenderTarget::canvas_mut(self)
        }
    }

    #[cfg(target_os = "windows")]
    impl GpuRenderTarget for WglSkiaRenderTarget {
        fn begin_frame(&mut self, width: i32, height: i32) -> Result<*mut c_void> {
            WglSkiaRenderTarget::begin_frame(self, width, height)
        }
        fn end_frame(&mut self) -> Result<()> {
            WglSkiaRenderTarget::end_frame(self)
        }
        fn present_frame(&mut self) -> Result<()> {
            WglSkiaRenderTarget::present_frame(self)
        }
        fn canvas_mut(&mut self) -> Result<&mut skia_safe::Canvas> {
            WglSkiaRenderTarget::canvas_mut(self)
        }
    }

    #[cfg(target_os = "linux")]
    impl GpuRenderTarget for EglSkiaRenderTarget {
        fn begin_frame(&mut self, width: i32, height: i32) -> Result<*mut c_void> {
            EglSkiaRenderTarget::begin_frame(self, width, height)
        }
        fn end_frame(&mut self) -> Result<()> {
            EglSkiaRenderTarget::end_frame(self)
        }
        fn present_frame(&mut self) -> Result<()> {
            EglSkiaRenderTarget::present_frame(self)
        }
        fn canvas_mut(&mut self) -> Result<&mut skia_safe::Canvas> {
            EglSkiaRenderTarget::canvas_mut(self)
        }
    }

    pub fn run() -> Result<()> {
        let input_path = std::env::args()
            .nth(1)
            .ok_or_else(|| anyhow!("usage: opencat-see <input.xml|.jsonl>"))?;
        let input_path = Path::new(&input_path);
        let source_text =
            std::fs::read_to_string(input_path).context("failed to read composition source")?;
        let base_dir = input_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let cache_base =
            dirs::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let cache_dir = cache_base.join(".opencat").join("assets");
        let loader = EngineLoader::new(base_dir, cache_dir).context("failed to create loader")?;
        let ctx = RqJsContext::new().context("failed to create js context")?;
        let mut pipeline =
            open(&source_text, loader, ctx).context("failed to open pipeline")?;
        let info = pipeline.info().clone();
        let total_frames = duration_secs_to_frames(info.duration, info.fps).max(1);
        let fps = info.fps.max(1);
        let width = info.width as i32;
        let height = info.height as i32;

        let mut media_ctx = MediaContext::new();
        media_ctx.set_composition_fps(info.fps);
        let mut executor = EngineDrawExecutor::new();
        let mut generated_cache: HashMap<GeneratedImageId, Image> = HashMap::new();

        let audio_track = build_audio_track_from_pipeline(&pipeline)
            .context("failed to premix audio track")?;
        let loop_sample_frames = composition_sample_frames(total_frames, fps, NonZeroU32::new(AUDIO_SAMPLE_RATE).unwrap());
        let audio_samples = audio_track.map(|track| Arc::new(track.samples));

        #[cfg(target_os = "linux")]
        let event_loop = {
            use winit::event_loop::EventLoopBuilder;
            use winit::platform::x11::EventLoopBuilderExtX11;
            let mut builder = EventLoopBuilder::new();
            // EGL path here needs an X11 window handle (pure Wayland wl_surface
            // support would require wayland-egl). Force the winit X11 backend.
            builder.with_x11();
            builder.build()
        };
        #[cfg(not(target_os = "linux"))]
        let event_loop = EventLoop::new();

        #[cfg(target_os = "linux")]
        let window = {
            // EGL does its own config selection on X11 and tolerates the window's
            // default visual on Mesa/XWayland, so no `with_x11_visual` is needed.
            match event_loop.raw_display_handle() {
                RawDisplayHandle::Xlib(handle) if !handle.display.is_null() => {}
                RawDisplayHandle::Xlib(_) => {
                    return Err(anyhow!("Xlib display handle is null"));
                }
                _ => {
                    return Err(anyhow!(
                        "opencat-see on Linux currently requires X11 (set WAYLAND_DISPLAY= to force X11)"
                    ));
                }
            }
            WindowBuilder::new()
                .with_title("OpenCat See")
                .with_inner_size(PhysicalSize::new(info.width, info.height))
                .with_resizable(false)
                .build(&event_loop)
                .context("failed to create window")?
        };
        #[cfg(not(target_os = "linux"))]
        let window = WindowBuilder::new()
            .with_title("OpenCat See")
            .with_inner_size(PhysicalSize::new(info.width, info.height))
            .with_resizable(false)
            .build(&event_loop)
            .context("failed to create window")?;

        let mut gpu_target = build_platform_render_target(&window, width, height)?;

        if let Err(error) = render_pipeline_frame_to_gpu_target(
            &mut pipeline,
            &mut media_ctx,
            &mut executor,
            &mut generated_cache,
            gpu_target.as_mut(),
            width,
            height,
            0,
        ) {
            return Err(anyhow!("failed to render warmup frame: {error:#}"));
        }
        std::thread::sleep(Duration::from_millis(100));

        let audio_playback = AudioPlayback::new(audio_samples, loop_sample_frames)?;
        let frame_duration = Duration::from_secs_f64(1.0 / fps as f64);
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
                    if let Err(error) = render_pipeline_frame_to_gpu_target(
            &mut pipeline,
            &mut media_ctx,
            &mut executor,
            &mut generated_cache,
            gpu_target.as_mut(),
            width,
            height,
            frame_index,
        ) {
                        eprintln!("render error: {error:#}");
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

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn main() -> anyhow::Result<()> {
    app::run()
}
