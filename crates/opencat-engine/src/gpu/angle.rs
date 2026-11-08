//! ANGLE 离屏上下文：engine GL 命令流的执行后端（Windows）。
//!
//! 拍板（plan.md 2026-11-06）：GL 流执行不再直调桌面 GL——PMREM 等真实
//! 浏览器路径在 WGL/桌面驱动上段错误（`gl.create_vertex_array()` 驱动内
//! 部崩），ANGLE 严格按 WebGL/GLES 语义校验参数（无效参数返回错误而不崩
//! 驱动）。
//!
//! 两种 ANGLE 来源，EGL 调用面一致（全部经 `eglGetProcAddress` 解析）：
//! - 静态（mozangle，仅 D3D11 后端），无运行时 DLL 依赖；
//! - 动态（`OC_ANGLE_DIR` 指向含 libEGL/libGLESv2 与 vk_swiftshader 的目录），
//!   Vulkan 后端 + SwiftShader 设备 = Chrome `--use-angle=swiftshader` 同一
//!   渲染器（swgl 参考的出处）。
//!
//! pbuffer 用 1×1：流执行的落点是纹理/FBO，surface 只承担上下文的可当前
//! 性，尺寸无意义。

use std::ffi::{CString, c_char, c_void};
use std::sync::OnceLock;

use anyhow::{Result, anyhow};

type Display = *const c_void;
type Config = *const c_void;
type Context = *const c_void;
type Surface = *const c_void;

// eglext_angle.h / egl.h 枚举（字面值 + 出处注释；bindings 没收全）。
const EGL_NONE: i32 = 0x3038;
const EGL_PLATFORM_ANGLE_ANGLE: u32 = 0x3202;
const EGL_PLATFORM_ANGLE_TYPE_ANGLE: i32 = 0x3203;
const EGL_PLATFORM_ANGLE_MAX_VERSION_MAJOR_ANGLE: i32 = 0x3204;
const EGL_PLATFORM_ANGLE_MAX_VERSION_MINOR_ANGLE: i32 = 0x3205;
const EGL_PLATFORM_ANGLE_TYPE_D3D11_ANGLE: i32 = 0x3208;
const EGL_PLATFORM_ANGLE_DEVICE_TYPE_ANGLE: i32 = 0x3209;
const EGL_PLATFORM_ANGLE_TYPE_VULKAN_ANGLE: i32 = 0x3450;
const EGL_PLATFORM_ANGLE_DEVICE_TYPE_SWIFTSHADER_ANGLE: i32 = 0x3487;
const EGL_SURFACE_TYPE: i32 = 0x3033;
const EGL_PBUFFER_BIT: i32 = 0x0001;
const EGL_RENDERABLE_TYPE: i32 = 0x3040;
const EGL_OPENGL_ES3_BIT: i32 = 0x0040;
const EGL_RED_SIZE: i32 = 0x3024;
const EGL_GREEN_SIZE: i32 = 0x3023;
const EGL_BLUE_SIZE: i32 = 0x3022;
const EGL_CONTEXT_MAJOR_VERSION: i32 = 0x3098;
const EGL_CONTEXT_MINOR_VERSION: i32 = 0x30FB;
const EGL_WIDTH: i32 = 0x3057;
const EGL_HEIGHT: i32 = 0x3056;
const EGL_DRAW: i32 = 0x3059;
const EGL_READ: i32 = 0x305A;

/// EGL 函数面（静态/动态两来源同形）。
struct Egl {
    get_proc: unsafe extern "system" fn(*const c_char) -> *const c_void,
    get_platform_display: unsafe extern "system" fn(u32, *mut c_void, *const i32) -> Display,
    initialize: unsafe extern "system" fn(Display, *mut i32, *mut i32) -> u32,
    choose_config: unsafe extern "system" fn(Display, *const i32, *mut Config, i32, *mut i32) -> u32,
    create_context: unsafe extern "system" fn(Display, Config, Context, *const i32) -> Context,
    create_pbuffer_surface: unsafe extern "system" fn(Display, Config, *const i32) -> Surface,
    make_current: unsafe extern "system" fn(Display, Surface, Surface, Context) -> u32,
    get_error: unsafe extern "system" fn() -> i32,
    destroy_surface: unsafe extern "system" fn(Display, Surface) -> u32,
    destroy_context: unsafe extern "system" fn(Display, Context) -> u32,
    terminate: unsafe extern "system" fn(Display) -> u32,
    release_thread: unsafe extern "system" fn() -> u32,
    get_current_display: unsafe extern "system" fn() -> Display,
    get_current_surface: unsafe extern "system" fn(i32) -> Surface,
    get_current_context: unsafe extern "system" fn() -> Context,
    /// 平台属性：动态来源走 Vulkan+SwiftShader，静态来源走 D3D11。
    swiftshader: bool,
}

static EGL: OnceLock<std::result::Result<Egl, String>> = OnceLock::new();

fn egl() -> Result<&'static Egl> {
    EGL.get_or_init(load_egl).as_ref().map_err(|e| anyhow!("{e}"))
}

fn load_egl() -> std::result::Result<Egl, String> {
    let get_proc: unsafe extern "system" fn(*const c_char) -> *const c_void;
    let swiftshader;
    match std::env::var_os("OC_ANGLE_DIR") {
        Some(dir) => {
            use std::os::windows::ffi::OsStrExt;
            use windows_sys::Win32::System::LibraryLoader::{
                GetProcAddress, LOAD_WITH_ALTERED_SEARCH_PATH, LoadLibraryExW,
            };
            let path = std::path::Path::new(&dir).join("libEGL.dll");
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            // ALTERED_SEARCH_PATH：libEGL 的依赖（libGLESv2）按它自己的目录解析。
            let module = unsafe {
                LoadLibraryExW(wide.as_ptr(), std::ptr::null_mut(), LOAD_WITH_ALTERED_SEARCH_PATH)
            };
            if module.is_null() {
                return Err(format!("angle: failed to load {}", path.display()));
            }
            let sym = unsafe { GetProcAddress(module, c"eglGetProcAddress".as_ptr().cast()) }
                .ok_or_else(|| format!("angle: eglGetProcAddress missing in {}", path.display()))?;
            get_proc = unsafe {
                std::mem::transmute::<
                    unsafe extern "system" fn() -> isize,
                    unsafe extern "system" fn(*const c_char) -> *const c_void,
                >(sym)
            };
            swiftshader = true;
        }
        None => {
            unsafe extern "system" fn static_get_proc(name: *const c_char) -> *const c_void {
                let name = unsafe { std::ffi::CStr::from_ptr(name) };
                mozangle::egl::get_proc_address(name.to_str().unwrap_or(""))
            }
            get_proc = static_get_proc;
            swiftshader = false;
        }
    }
    macro_rules! resolve {
        ($name:literal) => {{
            let p = unsafe { get_proc(concat!($name, "\0").as_ptr().cast()) };
            if p.is_null() {
                return Err(format!("angle: {} unresolved", $name));
            }
            unsafe { std::mem::transmute::<*const c_void, _>(p) }
        }};
    }
    Ok(Egl {
        get_proc,
        get_platform_display: resolve!("eglGetPlatformDisplayEXT"),
        initialize: resolve!("eglInitialize"),
        choose_config: resolve!("eglChooseConfig"),
        create_context: resolve!("eglCreateContext"),
        create_pbuffer_surface: resolve!("eglCreatePbufferSurface"),
        make_current: resolve!("eglMakeCurrent"),
        get_error: resolve!("eglGetError"),
        destroy_surface: resolve!("eglDestroySurface"),
        destroy_context: resolve!("eglDestroyContext"),
        terminate: resolve!("eglTerminate"),
        release_thread: resolve!("eglReleaseThread"),
        get_current_display: resolve!("eglGetCurrentDisplay"),
        get_current_surface: resolve!("eglGetCurrentSurface"),
        get_current_context: resolve!("eglGetCurrentContext"),
        swiftshader,
    })
}

pub struct AngleContext {
    display: Display,
    context: Context,
    surface: Surface,
}

impl AngleContext {
    /// 建 ANGLE 离屏上下文并 make current（glow 的入口解析要求 current）。
    pub fn offscreen() -> Result<Self> {
        let egl = egl()?;
        let attribs: Vec<i32> = if egl.swiftshader {
            vec![
                EGL_PLATFORM_ANGLE_TYPE_ANGLE,
                EGL_PLATFORM_ANGLE_TYPE_VULKAN_ANGLE,
                EGL_PLATFORM_ANGLE_DEVICE_TYPE_ANGLE,
                EGL_PLATFORM_ANGLE_DEVICE_TYPE_SWIFTSHADER_ANGLE,
                EGL_NONE,
            ]
        } else {
            vec![
                EGL_PLATFORM_ANGLE_TYPE_ANGLE,
                EGL_PLATFORM_ANGLE_TYPE_D3D11_ANGLE,
                EGL_PLATFORM_ANGLE_MAX_VERSION_MAJOR_ANGLE,
                3,
                EGL_PLATFORM_ANGLE_MAX_VERSION_MINOR_ANGLE,
                0,
                EGL_NONE,
            ]
        };
        unsafe {
            let display = (egl.get_platform_display)(
                EGL_PLATFORM_ANGLE_ANGLE,
                std::ptr::null_mut(),
                attribs.as_ptr(),
            );
            if display.is_null() {
                return Err(anyhow!(
                    "angle: eglGetPlatformDisplayEXT failed ({:#x})",
                    (egl.get_error)()
                ));
            }
            let (mut major, mut minor) = (0, 0);
            if (egl.initialize)(display, &mut major, &mut minor) == 0 {
                return Err(anyhow!("angle: eglInitialize failed ({:#x})", (egl.get_error)()));
            }

            // RGBA8 + pbuffer + GLES3 可渲染。
            let config_attrs = [
                EGL_SURFACE_TYPE,
                EGL_PBUFFER_BIT,
                EGL_RENDERABLE_TYPE,
                EGL_OPENGL_ES3_BIT,
                EGL_RED_SIZE,
                8,
                EGL_GREEN_SIZE,
                8,
                EGL_BLUE_SIZE,
                8,
                EGL_NONE,
            ];
            let mut config: Config = std::ptr::null();
            let mut num_config = 0;
            if (egl.choose_config)(display, config_attrs.as_ptr(), &mut config, 1, &mut num_config)
                == 0
                || num_config < 1
            {
                return Err(anyhow!(
                    "angle: no GLES3 RGBA8 pbuffer config ({:#x})",
                    (egl.get_error)()
                ));
            }

            // GLES 3.0 上下文（WebGL2 的后端语义面）。
            let context_attrs =
                [EGL_CONTEXT_MAJOR_VERSION, 3, EGL_CONTEXT_MINOR_VERSION, 0, EGL_NONE];
            let context =
                (egl.create_context)(display, config, std::ptr::null(), context_attrs.as_ptr());
            if context.is_null() {
                return Err(anyhow!("angle: eglCreateContext failed ({:#x})", (egl.get_error)()));
            }

            // 1×1 pbuffer（占位 surface，尺寸无语义）。
            let surface_attrs = [EGL_WIDTH, 1, EGL_HEIGHT, 1, EGL_NONE];
            let surface = (egl.create_pbuffer_surface)(display, config, surface_attrs.as_ptr());
            if surface.is_null() {
                return Err(anyhow!(
                    "angle: eglCreatePbufferSurface failed ({:#x})",
                    (egl.get_error)()
                ));
            }
            let ctx = Self {
                display,
                context,
                surface,
            };
            ctx.make_current()?;
            Ok(ctx)
        }
    }

    /// 把本上下文绑到当前线程。幂等；每次 GL 使用前调一次。
    pub fn make_current(&self) -> Result<()> {
        let egl = egl()?;
        unsafe {
            if (egl.make_current)(self.display, self.surface, self.surface, self.context) == 0 {
                return Err(anyhow!("angle: make_current failed ({:#x})", (egl.get_error)()));
            }
        }
        Ok(())
    }
}

impl Drop for AngleContext {
    fn drop(&mut self) {
        let Ok(egl) = egl() else { return };
        unsafe {
            (egl.make_current)(self.display, std::ptr::null(), std::ptr::null(), std::ptr::null());
            (egl.destroy_surface)(self.display, self.surface);
            (egl.destroy_context)(self.display, self.context);
            (egl.terminate)(self.display);
            // 线程态一并释放：SwiftShader 的工作线程须在 ExitProcess 杀线程
            // 之前收尾，否则进程退出卡死在 DLL 卸载里。
            (egl.release_thread)();
        }
    }
}

/// 当前线程的 EGL 绑定现场（GLSL 反射器借用后归还）。
pub struct CurrentBinding {
    display: Display,
    draw: Surface,
    read: Surface,
    context: Context,
}

impl CurrentBinding {
    /// 记下当前绑定；没有绑定（或 EGL 不可用）返回 None。
    pub fn save() -> Option<Self> {
        let egl = egl().ok()?;
        unsafe {
            let context = (egl.get_current_context)();
            if context.is_null() {
                return None;
            }
            Some(Self {
                display: (egl.get_current_display)(),
                draw: (egl.get_current_surface)(EGL_DRAW),
                read: (egl.get_current_surface)(EGL_READ),
                context,
            })
        }
    }

    /// 恢复记下的绑定。
    pub fn restore(self) -> Result<()> {
        let egl = egl()?;
        unsafe {
            if (egl.make_current)(self.display, self.draw, self.read, self.context) == 0 {
                return Err(anyhow!("angle: restore current failed ({:#x})", (egl.get_error)()));
            }
        }
        Ok(())
    }
}

/// ANGLE 的 GL 入口解析（glow 与 Skia loader 共用）。
pub fn load_proc(name: &str) -> *const c_void {
    let Ok(egl) = egl() else {
        return std::ptr::null();
    };
    let Ok(name) = CString::new(name) else {
        return std::ptr::null();
    };
    unsafe { (egl.get_proc)(name.as_ptr()) }
}
