//! Linux 呈现器：EGL（X11）+ Skia GL 后端。
//!
//! 用 EGL 而非 GLX：Skia 的 binary-cache 构建内部链接 EGL，它的 GL 函数解析
//! 走 `eglGetProcAddress`，所以 EGL 是必需的。

use std::ffi::{CString, c_void};

use anyhow::{Result, anyhow};
use khronos_egl as egl;
use opencat_three::Renderer3d;
use skia_safe::{
    ColorType,
    gpu::{self, SurfaceOrigin, backend_render_targets, gl},
};

/// 宿主窗口句柄。两者都来自 `raw-window-handle` 的 Xlib 句柄。
pub struct WindowHandle {
    pub x11_display: *mut c_void,
    pub x11_window: u64,
}

pub struct Presenter {
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
unsafe impl Send for Presenter {}

impl Presenter {
    pub fn new(window: WindowHandle, _width: i32, _height: i32) -> Result<Self> {
        let display_handle = window.x11_display;
        let xwindow = window.x11_window;
        if display_handle.is_null() {
            return Err(anyhow!("X11 display handle is null"));
        }
        if xwindow == 0 {
            return Err(anyhow!("X11 window id is zero"));
        }

        // `load_required()` fails if libEGL.so.1 is missing or predates EGL 1.5.
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
            egl.create_window_surface(display, config, xwindow as egl::NativeWindowType, None)
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

        let interface =
            gl::Interface::new_load_with(|name| load_egl_proc_address(&egl, libgl, name))
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

    pub fn begin_frame(&mut self, width: i32, height: i32) -> Result<()> {
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
        Ok(())
    }

    pub fn end_frame(&mut self) -> Result<()> {
        let mut surface = self
            .current_surface
            .take()
            .ok_or_else(|| anyhow!("end_frame called before begin_frame"))?;
        self.skia.flush_and_submit_surface(&mut surface, None);
        drop(surface);
        Ok(())
    }

    pub fn present_frame(&mut self) -> Result<()> {
        self.make_current()?;
        self.egl
            .swap_buffers(self.display, self.surface)
            .map_err(|e| anyhow!("eglSwapBuffers failed: {e}"))?;
        Ok(())
    }

    /// EGL 后端不与 wgpu 共享上下文，所以为空操作。
    pub fn reset_backend_state(&mut self) {}

    /// EGL 后端不支持加入 wgpu；3D 走 CPU 读回。
    pub fn build_scene3_renderer(&mut self) -> Result<Option<Renderer3d>> {
        Ok(None)
    }

    pub fn with_canvas_and_resolver(
        &mut self,
        _textures: &opencat_gpu_native::TextureRegistry,
        f: &mut dyn FnMut(
            &mut skia_safe::Canvas,
            Option<&mut dyn crate::consumer::GpuImageResolver>,
        ) -> Result<()>,
    ) -> Result<()> {
        let surface = self.current_surface.as_mut().ok_or_else(|| {
            anyhow!("skia canvas requested before Linux framebuffer surface was ready")
        })?;
        #[allow(invalid_reference_casting)]
        let canvas: &mut skia_safe::Canvas = unsafe {
            &mut *(surface.canvas() as *const skia_safe::Canvas as *mut skia_safe::Canvas)
        };
        f(canvas, None)
    }
}

impl Drop for Presenter {
    fn drop(&mut self) {
        self.current_surface.take();
        self.skia.release_resources_and_abandon();
        // Unbind the context before tearing it down.
        let _ = self.egl.make_current(self.display, None, None, None);
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
