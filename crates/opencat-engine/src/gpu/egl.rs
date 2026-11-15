//! EGL 上下文引导与 GL 入口解析（Linux）。
//!
//! 与 [`super::win32::WglContext`] 同位：宿主（预览器）拿到窗口后在这里建
//! GL 上下文；wgpu 以外部上下文模式加入，Skia 用 GL 后端，三者共享同一
//! EGL context，纹理才谈得上零拷贝。
//!
//! 离线渲染没有窗口，走 [`EglContext::offscreen`]（默认 display + 1×1
//! pbuffer，同为 4.6 core）——wgpu 自建的 GL 3.3 上下文实测画不出图元，
//! 离线与预览必须走同一条外部上下文代码。GL 入口解析（[`load_proc`]）
//! 三侧共用：wgpu 上下文上建 FBO/PBO、组装 Skia 接口，解析的都是同一批入口。
//!
//! 用 EGL 而非 GLX：Skia 的 binary-cache 构建内部链接 EGL，它的 GL 函数
//! 解析走 `eglGetProcAddress`，所以 EGL 是必需的。

use std::ffi::{CString, c_char, c_void};
use std::sync::OnceLock;

use anyhow::{Result, anyhow};
use khronos_egl as egl;

/// 解析一个 GL 入口点（线程安全，进程级缓存）。
///
/// 先问 libGL 的导出表：核心 GL 1.x/2.x 入口（glGetString、glGetIntegerv、
/// glReadPixels……）Mesa 的 `eglGetProcAddress` 历史上返回非空但**不可调用**
/// 的指针；Skia 拿去解引用就会段错误在 `GrGLExtensions::init`。GLVND 的
/// libGL 导出全部核心入口，`dlsym` 即得。只有扩展 / GL 版本专属入口
/// （glGetStringi、FBO/PBO 的部分变体……）才真正来自 `eglGetProcAddress`。
///
/// EGL 哨兵返回值（0/1/2/3/`usize::MAX`）按规范允许过滤掉。
pub fn load_proc(name: &str) -> *const c_void {
    let Ok(cname) = CString::new(name) else {
        return std::ptr::null();
    };

    let libgl = libgl();
    if libgl != 0 {
        let sym = unsafe { libc::dlsym(libgl as *mut c_void, cname.as_ptr()) };
        if !sym.is_null() {
            return sym as *const c_void;
        }
    }

    if let Some(proc) = egl_proc_address(&cname) {
        let raw = proc as *const () as usize;
        if !matches!(raw, 0 | 1 | 2 | 3 | usize::MAX) {
            return proc;
        }
    }

    std::ptr::null()
}

/// dlopen 的 libGL 句柄（进程生命周期，有意不 dlclose——GL 入口必须比所有
/// GL 使用方活得久）。找不到 libGL 时缓存 0，避免每次解析都重试。
static LIBGL: OnceLock<usize> = OnceLock::new();

fn libgl() -> usize {
    *LIBGL.get_or_init(|| {
        for soname in [b"libGL.so.1\0".as_ptr(), b"libGL.so\0".as_ptr()] {
            let handle =
                unsafe { libc::dlopen(soname as *const _, libc::RTLD_LAZY | libc::RTLD_GLOBAL) };
            if !handle.is_null() {
                return handle as usize;
            }
        }
        0
    })
}

/// dlopen 的 libxcb 句柄（同 [`libgl`] 策略：进程生命周期，有意不
/// dlclose——X 连接必须活得比 EGL display 久）。找不到时缓存 0。
static LIBXCB: OnceLock<usize> = OnceLock::new();

fn libxcb() -> usize {
    *LIBXCB.get_or_init(|| {
        let soname = b"libxcb.so.1\0".as_ptr();
        let handle =
            unsafe { libc::dlopen(soname as *const _, libc::RTLD_LAZY | libc::RTLD_GLOBAL) };
        if !handle.is_null() {
            return handle as usize;
        }
        0
    })
}

/// xcb_connect(NULL)：默认 X 连接（XWayland 会话同样有效），附屏幕号。
/// 走纯 xcb 而非 XOpenDisplay：本机实测 mesa EGL 的 X11/XCB 平台（同值
/// 0x31DC 双态）从 Xlib `Display*` 经 `XGetXCBConnection` 桥取连接拿到坏
/// 状态（"failed to get xcb screen" → EGL_NOT_INITIALIZED），直传
/// xcb_connection_t 完全正常。
fn xcb_open() -> Option<(*mut c_void, i32)> {
    let h = libxcb();
    if h == 0 {
        return None;
    }
    let sym = unsafe { libc::dlsym(h as *mut c_void, b"xcb_connect\0".as_ptr() as *const _) };
    if sym.is_null() {
        return None;
    }
    let connect: unsafe extern "C" fn(*const c_char, *mut i32) -> *mut c_void =
        unsafe { std::mem::transmute(sym) };
    let mut screen: i32 = -1;
    let conn = unsafe { connect(std::ptr::null(), &mut screen) };
    if conn.is_null() {
        return None;
    }
    let sym = unsafe {
        libc::dlsym(h as *mut c_void, b"xcb_connection_has_error\0".as_ptr() as *const _)
    };
    if !sym.is_null() {
        let has_err: unsafe extern "C" fn(*mut c_void) -> i32 = unsafe { std::mem::transmute(sym) };
        if unsafe { has_err(conn) } != 0 {
            xcb_close(conn);
            return None;
        }
    }
    Some((conn, screen))
}

/// xcb_disconnect（offscreen 路径自开的连接在 Drop 收口）。
fn xcb_close(conn: *mut c_void) {
    let h = libxcb();
    if h == 0 {
        return;
    }
    let sym = unsafe { libc::dlsym(h as *mut c_void, b"xcb_disconnect\0".as_ptr() as *const _) };
    if !sym.is_null() {
        let disconnect: unsafe extern "C" fn(*mut c_void) = unsafe { std::mem::transmute(sym) };
        unsafe { disconnect(conn) };
    }
}

/// `eglGetProcAddress`，经 libEGL.so.1 动态解析。加载失败时返回 `None`
/// （libEGL 缺失的系统上只剩 dlsym 一条路）。
static EGL: OnceLock<Option<egl::DynamicInstance<egl::EGL1_5>>> = OnceLock::new();

fn egl_proc_address(cname: &CString) -> Option<*const c_void> {
    let egl = EGL
        .get_or_init(|| unsafe { egl::DynamicInstance::<egl::EGL1_5>::load_required().ok() })
        .as_ref()?;
    egl.get_proc_address(cname.to_str().ok()?)
        .map(|p| p as *const () as *const c_void)
}

/// 初始化 EGL display 并挑一个 RGBA8 pbuffer + OPENGL_BIT config；配不出
/// 返回 `None`（调用方决定退路）。
fn initialize_and_choose(
    egl: &egl::DynamicInstance<egl::EGL1_5>,
    display: egl::Display,
    attrs: &[egl::Int; 13],
) -> Result<Option<egl::Config>> {
    egl.initialize(display)
        .map_err(|e| anyhow!("eglInitialize failed: {e}"))?;
    egl.choose_first_config(display, attrs)
        .map_err(|e| anyhow!("eglChooseConfig failed: {e}"))
}

/// Wayland 会话上把 GLVND 收敛到 mesa vendor（首次 EGL API 调用前调用）。
///
/// GLVND 对 legacy `eglGetDisplay` 的 vendor 选择是逐家试探（json 按
/// 文件名序，nvidia 先于 mesa）；本机实测 NVIDIA EGL 的 wayland 初始化
/// 在 Wayland 会话上段错误（崩点 wl_proxy_create_wrapper，且与系统
/// libwayland-client ABI 失配的 mesa wayland 链同点崩）。渲染两条路
///（from_window 的 legacy X Display*、offscreen 的显式 X 连接）都走 X11
/// platform，mesa 的 radeonsi 完全够用——把 vendor 表收敛到 mesa 即
/// 避开 NVIDIA 的探测路径。纯 NVIDIA 机器（无 mesa json）与用户已显式
/// 设置的环境保持原样。
fn prefer_mesa_vendor() {
    static DONE: OnceLock<()> = OnceLock::new();
    DONE.get_or_init(|| {
        if std::env::var_os("WAYLAND_DISPLAY").is_none()
            && std::env::var_os("XDG_SESSION_TYPE").map(|v| v != *"wayland").unwrap_or(true)
        {
            return;
        }
        if std::env::var_os("__EGL_VENDOR_LIBRARY_FILENAMES").is_some() {
            return;
        }
        let mesa = "/usr/share/glvnd/egl_vendor.d/50_mesa.json";
        if !std::path::Path::new(mesa).is_file() {
            return;
        }
        let key = CString::new("__EGL_VENDOR_LIBRARY_FILENAMES").expect("ascii env key");
        let val = CString::new(mesa).expect("ascii env value");
        unsafe { libc::setenv(key.as_ptr(), val.as_ptr(), 1) };
    });
}

/// 一个 X11 窗口上的 EGL 上下文。
///
/// [`Self::from_window`] 建好即 make current。drop 时解绑、删上下文/表面、
/// terminate display。GL 入口解析走进程级 [`load_proc`]。
///
/// RGBA8 窗口表面 + OpenGL 4.6 core。选 4.x core 而非 3.3：wgpu 建设备时
/// 默认开间接调用校验，要编译内部 compute 管线（GL 4.3+ 才有 compute）；
/// 3.3 上下文上它编译失败并把整个设备判死（"Parent device is lost"）。
/// wgpu 自建上下文写死请求 3.3，那边只能摘校验旗——两侧不对称是 wgpu 的
/// 注入口差异，不是意愿。wgpu 的外部上下文模式直接吃这个 current context。
pub struct EglContext {
    display: egl::Display,
    // Kept for diagnostics; not read after construction.
    _config: egl::Config,
    context: egl::Context,
    surface: egl::Surface,
    // DynamicInstance 只是一个 dlopen 句柄 + 函数表，drop 安全（引用计数）。
    _egl: egl::DynamicInstance<egl::EGL1_5>,
    // offscreen 路径自开的 xcb 连接（display 的宿主）；from_window 走
    // 调用方持有的连接，此字段为 null。
    x11_conn: *mut c_void,
}

// SAFETY: EGL 句柄只在 winit 事件循环线程上触碰（调用方保证）。
unsafe impl Send for EglContext {}

impl EglContext {
    /// 在 `x11_window`（XOpenDisplay 返回的 `x11_display` 上）建 GL 上下文
    /// 并 make current。
    pub fn from_window(x11_display: *mut c_void, x11_window: u64) -> Result<Self> {
        if x11_display.is_null() {
            return Err(anyhow!("X11 display handle is null"));
        }
        if x11_window == 0 {
            return Err(anyhow!("X11 window id is zero"));
        }

        // `load_required()` fails if libEGL.so.1 is missing or predates EGL 1.5.
        let egl = unsafe { egl::DynamicInstance::<egl::EGL1_5>::load_required() }
            .map_err(|e| anyhow!("failed to load libEGL.so.1: {e}"))?;
        prefer_mesa_vendor();

        // EGL defaults to OpenGL ES; bind desktop GL so the context below is GL.
        egl.bind_api(egl::OPENGL_API)
            .map_err(|e| anyhow!("eglBindAPI(OPENGL) failed: {e}"))?;

        // 显式 X11 platform：legacy `eglGetDisplay(Display*)` 的自动检测在
        // Wayland 会话上会回落编译时默认 platform（wayland），与本机已废
        // 的 wayland EGL 链相撞。见 offscreen() 内同名注释。
        const PLATFORM_X11: egl::Enum = 0x31DC; // EGL_PLATFORM_X11_EXT
        let display = unsafe {
            egl.get_platform_display(PLATFORM_X11, x11_display as egl::NativeDisplayType, &[egl::NONE as egl::Attrib])
        }
        .map_err(|e| anyhow!("eglGetPlatformDisplay(X11) failed: {e}"))?;
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

        let context_attrs: [egl::Int; 7] = [
            egl::CONTEXT_MAJOR_VERSION,
            4,
            egl::CONTEXT_MINOR_VERSION,
            6,
            egl::CONTEXT_OPENGL_PROFILE_MASK,
            egl::CONTEXT_OPENGL_CORE_PROFILE_BIT,
            egl::NONE,
        ];
        let context = egl
            .create_context(display, config, None, &context_attrs)
            .map_err(|e| anyhow!("eglCreateContext failed: {e}"))?;

        let surface = unsafe {
            egl.create_window_surface(display, config, x11_window as egl::NativeWindowType, None)
                .map_err(|e| anyhow!("eglCreateWindowSurface failed: {e}"))?
        };

        egl.make_current(display, Some(surface), Some(surface), Some(context))
            .map_err(|e| anyhow!("eglMakeCurrent failed: {e}"))?;

        Ok(Self {
            display,
            _config: config,
            context,
            surface,
            _egl: egl,
            x11_conn: std::ptr::null_mut(),
        })
    }

    /// 把本上下文绑到当前线程。
    pub fn make_current(&self) -> Result<()> {
        self._egl
            .make_current(
                self.display,
                Some(self.surface),
                Some(self.surface),
                Some(self.context),
            )
            .map_err(|e| anyhow!("eglMakeCurrent failed for Linux render target: {e}"))
    }

    /// 当前线程绑定着的 (display, draw, read, context)；任一缺失（无线程
    /// 绑定）时返回 `None`。现场恢复用（`script::bridge` 的
    /// `CurrentContextGuard`）。
    pub fn get_current_context() -> Option<(egl::Display, egl::Surface, egl::Surface, egl::Context)>
    {
        let egl = EGL
            .get_or_init(|| unsafe { egl::DynamicInstance::<egl::EGL1_5>::load_required().ok() })
            .as_ref()?;
        let display = egl.get_current_display()?;
        let draw = egl.get_current_surface(egl::DRAW)?;
        let read = egl.get_current_surface(egl::READ)?;
        let context = egl.get_current_context()?;
        Some((display, draw, read, context))
    }

    /// 直接绑定一组已存在的 EGL 对象——现场恢复用，不经 [`EglContext`]。
    pub fn make_current_raw(
        display: egl::Display,
        draw: egl::Surface,
        read: egl::Surface,
        context: egl::Context,
    ) -> Result<()> {
        let egl = EGL
            .get_or_init(|| unsafe { egl::DynamicInstance::<egl::EGL1_5>::load_required().ok() })
            .as_ref()
            .ok_or_else(|| anyhow!("failed to load libEGL.so.1 (context restore)"))?;
        egl.make_current(display, Some(draw), Some(read), Some(context))
            .map_err(|e| anyhow!("eglMakeCurrent failed (context restore): {e}"))
    }

    /// 交换前后缓冲（双缓冲窗口）。
    pub fn swap_buffers(&self) -> Result<()> {
        self._egl
            .swap_buffers(self.display, self.surface)
            .map_err(|e| anyhow!("eglSwapBuffers failed: {e}"))
    }

    /// 无窗口离屏上下文（离线渲染用）。
    ///
    /// 与 [`Self::from_window`] 同为 OpenGL 4.6 core，但表面是 1×1 pbuffer，
    /// 只依赖默认 EGL display（X11/XWayland），不需要窗口。wgpu 外部上下文
    /// 模式从不往表面画东西（3D 渲到自己的 FBO/纹理），pbuffer 只为
    /// `eglMakeCurrent` 有个合法表面。
    ///
    /// wgpu 自建的离线 GL 上下文写死 3.3，实测（Mesa）画不出任何图元
    ///（clear 正常、draw 全丢，无任何 GL 报错）；4.6 core 的宿主上下文则
    /// 一切正常。离线路径一律走这里，与预览同一条外部上下文代码。
    pub fn offscreen() -> Result<Self> {
        let egl = unsafe { egl::DynamicInstance::<egl::EGL1_5>::load_required() }
            .map_err(|e| anyhow!("failed to load libEGL.so.1: {e}"))?;
        prefer_mesa_vendor();

        egl.bind_api(egl::OPENGL_API)
            .map_err(|e| anyhow!("eglBindAPI(OPENGL) failed: {e}"))?;

        let config_attrs: [egl::Int; 13] = [
            egl::SURFACE_TYPE,
            egl::PBUFFER_BIT,
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

        // Wayland 会话上 DEFAULT_DISPLAY 落 wayland platform：本机实测
        // mesa/nvidia 两条 vendor 链都在 eglInitialize 段错误，崩点同在
        // libwayland-client 的 wl_proxy_create_wrapper（系统库 ABI 失配）。
        // X11 同值 platform（0x31DC）从 Xlib Display* 走又会撞坏
        // XGetXCBConnection 桥。离屏渲染直接 xcb_connect + EGL_PLATFORM_
        // XCB_EXT（同值双态：native 传 xcb_connection_t 直连，实测
        // radeonsi EGL 1.5 正常）；xcb 不可开才退回 DEFAULT。
        const PLATFORM_XCB: egl::Enum = 0x31DC; // EGL_PLATFORM_XCB_EXT（与 X11_EXT 同值）
        const PLATFORM_XCB_SCREEN: egl::Attrib = 0x31DE; // EGL_PLATFORM_XCB_SCREEN_EXT
        let (display, x11_conn, config) = 'open: {
            if let Some((c, screen)) = xcb_open() {
                let d = unsafe {
                    egl.get_platform_display(
                        PLATFORM_XCB,
                        c as egl::NativeDisplayType,
                        &[PLATFORM_XCB_SCREEN, screen as egl::Attrib, egl::NONE as egl::Attrib],
                    )
                }
                .ok();
                if let Some(d) = d {
                    if let Some(cfg) = initialize_and_choose(&egl, d, &config_attrs)? {
                        break 'open (d, c, cfg);
                    }
                    let _ = egl.terminate(d);
                }
                xcb_close(c);
            }
            let d = unsafe { egl.get_display(egl::DEFAULT_DISPLAY) }
                .ok_or_else(|| anyhow!("eglGetDisplay(EGL_DEFAULT_DISPLAY) returned EGL_NO_DISPLAY"))?;
            let c = initialize_and_choose(&egl, d, &config_attrs)?
                .ok_or_else(|| anyhow!("no matching EGL config (need RGBA8 pbuffer + OPENGL_BIT)"))?;
            (d, std::ptr::null_mut(), c)
        };

        let context_attrs: [egl::Int; 7] = [
            egl::CONTEXT_MAJOR_VERSION,
            4,
            egl::CONTEXT_MINOR_VERSION,
            6,
            egl::CONTEXT_OPENGL_PROFILE_MASK,
            egl::CONTEXT_OPENGL_CORE_PROFILE_BIT,
            egl::NONE,
        ];
        let context = egl
            .create_context(display, config, None, &context_attrs)
            .map_err(|e| anyhow!("eglCreateContext failed: {e}"))?;

        let surface = egl
            .create_pbuffer_surface(display, config, &[egl::WIDTH, 1, egl::HEIGHT, 1, egl::NONE])
            .map_err(|e| anyhow!("eglCreatePbufferSurface failed: {e}"))?;

        egl.make_current(display, Some(surface), Some(surface), Some(context))
            .map_err(|e| anyhow!("eglMakeCurrent failed: {e}"))?;

        Ok(Self {
            display,
            _config: config,
            context,
            surface,
            _egl: egl,
            x11_conn,
        })
    }
}

impl Drop for EglContext {
    fn drop(&mut self) {
        // 与构造对称地拆；单项失败跳过即可，其余继续。
        let _ = self._egl.make_current(self.display, None, None, None);
        let _ = self._egl.destroy_context(self.display, self.context);
        let _ = self._egl.destroy_surface(self.display, self.surface);
        let _ = self._egl.terminate(self.display);
        if !self.x11_conn.is_null() {
            xcb_close(self.x11_conn);
        }
    }
}
