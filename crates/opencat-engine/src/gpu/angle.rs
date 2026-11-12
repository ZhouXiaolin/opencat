//! ANGLE 离屏上下文：engine GL 命令流的执行后端。
//!
//! 拍板（plan.md 2026-11-06）：GL 流执行不再直调桌面 GL——PMREM 等真实
//! 浏览器路径在 WGL/桌面驱动上段错误（`gl.create_vertex_array()` 驱动内
//! 部崩），ANGLE 严格按 WebGL/GLES 语义校验参数（无效参数返回错误而不崩
//! 驱动）。
//!
//! 两种 ANGLE 来源，EGL 调用面一致（全部经 `eglGetProcAddress` 解析）：
//! - 静态（mozangle，仅 Windows，D3D11 后端），无运行时 DLL 依赖；
//! - 动态（`OC_ANGLE_DIR` 指向官方 ANGLE Linux 构建），Vulkan 后端。
//!
//! Linux 上没有 mozangle 路径：mozangle 的 `egl` feature 只支持 Windows
//! （其 build.rs 硬断言非 Windows 平台不支持 EGL 构建），且它打包的是
//! D3D11 后端。Linux 走官方 ANGLE 动态库，语义面与 Windows 的动态路径
//! 相同——两端共用同一个 ANGLE 实现，GL 流语义不随平台漂移。
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
// `EGL_ANGLE_device_vulkan` 扩展的 display 属性（ANGLE 交出自己的 Vulkan
// 句柄）。见 ANGLE 的 include/EGL/eglext_angle.h。
const EGL_VULKAN_INSTANCE_ANGLE: i32 = 0x34A9;
const EGL_VULKAN_PHYSICAL_DEVICE_ANGLE: i32 = 0x34AB;
const EGL_VULKAN_DEVICE_ANGLE: i32 = 0x34AC;
const EGL_VULKAN_QUEUE_ANGLE: i32 = 0x34AF;
const EGL_VULKAN_QUEUE_FAMILY_INDEX_ANGLE: i32 = 0x34D0;
/// ANGLE 自己的 `vkGetInstanceProcAddr`（跨 loader 的正确入口——ANGLE 的
/// instance 由它自带的 loader 创建，系统 loader 的同名函数不认识它）。
const EGL_VULKAN_GET_INSTANCE_PROC_ADDR: i32 = 0x34D1;
const EGL_PLATFORM_ANGLE_ANGLE: u32 = 0x3202;
const EGL_PLATFORM_ANGLE_TYPE_ANGLE: i32 = 0x3203;
const EGL_PLATFORM_ANGLE_MAX_VERSION_MAJOR_ANGLE: i32 = 0x3204;
const EGL_PLATFORM_ANGLE_MAX_VERSION_MINOR_ANGLE: i32 = 0x3205;
const EGL_PLATFORM_ANGLE_TYPE_D3D11_ANGLE: i32 = 0x3208;
const EGL_PLATFORM_ANGLE_DEVICE_TYPE_ANGLE: i32 = 0x3209;
const EGL_PLATFORM_ANGLE_DEVICE_TYPE_HARDWARE_ANGLE: i32 = 0x320A;
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
// `EGL_EXT_device_query`：display 的 EGLDevice 句柄。
const EGL_DEVICE_EXT: i32 = 0x322C;
// `EGL_ANGLE_device_vulkan` 的 Vulkan 句柄取自 EGLDevice（display 属性
// 查询会返回 EGL_BAD_ATTRIBUTE——句柄在 device 上，不在 display 上）。

/// EGL 函数面（静态/动态两来源同形）。
struct Egl {
    get_proc: unsafe extern "system" fn(*const c_char) -> *const c_void,
    /// 动态 ANGLE 的 dlopen/LoadLibrary 句柄（进程级保活，从不 dlclose）。
    /// Linux 上 dlsym 直取设备查询入口要用。
    module: usize,
    get_platform_display: unsafe extern "system" fn(u32, *mut c_void, *const i32) -> Display,
    initialize: unsafe extern "system" fn(Display, *mut i32, *mut i32) -> u32,
    /// `eglQueryDisplayAttribEXT`：display → EGLDevice（`EGL_DEVICE_EXT`）。
    query_display_attrib: Option<unsafe extern "system" fn(Display, i32, *mut c_void) -> u32>,
    /// `eglQueryDeviceAttribEXT`：EGLDevice 属性（Vulkan 句柄从这里取）。
    query_device_attrib: Option<unsafe extern "system" fn(*mut c_void, i32, *mut c_void) -> u32>,
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
    /// `eglDestroyImageKHR`——导出后销毁临时 EGLImage。
    destroy_image: unsafe extern "system" fn(Display, *mut c_void) -> u32,
    /// 设备类型：决定 `eglGetPlatformDisplayEXT` 的平台属性。
    device: AngleDevice,
}

/// ANGLE 的设备类型（`EGL_PLATFORM_ANGLE_DEVICE_TYPE_ANGLE` 的取值）。
#[derive(Clone, Copy, PartialEq, Eq)]
enum AngleDevice {
    /// Vulkan 后端 + 硬件设备（GPU 加速）：日常渲染用。
    VulkanHardware,
    /// Vulkan 后端 + SwiftShader 设备（CPU 软件光栅）：与 swgl 参考
    /// （Chrome `--use-angle=swiftshader`）同一渲染器，像素级对齐用。
    VulkanSwiftShader,
    /// D3D11 后端：Windows 静态 mozangle 的唯一后端。
    D3D11,
}

static EGL: OnceLock<std::result::Result<Egl, String>> = OnceLock::new();

fn egl() -> Result<&'static Egl> {
    EGL.get_or_init(load_egl).as_ref().map_err(|e| anyhow!("{e}"))
}

/// 动态 ANGLE：加载目录里的 libEGL，取出 `eglGetProcAddress` 作为后续
/// 全部 EGL/GLES 入口的解析根。
///
/// Windows 走 `LoadLibraryExW` + `LOAD_WITH_ALTERED_SEARCH_PATH`（libEGL 的
/// 依赖 libGLESv2 按它自己的目录解析）；Linux 走 `dlopen` + `RTLD_LOCAL`
/// （同目录的 libGLESv2/libvk_swiftshader 由 ANGLE 自身按相对路径加载）。
/// 两端的库文件名不同，其余语义一致。
fn load_dynamic_angle(
    dir: &std::path::Path,
) -> std::result::Result<
    (
        unsafe extern "system" fn(*const c_char) -> *const c_void,
        usize,
    ),
    String,
> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::System::LibraryLoader::{
            GetProcAddress, LOAD_WITH_ALTERED_SEARCH_PATH, LoadLibraryExW,
        };
        let path = dir.join("libEGL.dll");
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
        let get_proc = unsafe {
            std::mem::transmute::<
                unsafe extern "system" fn() -> isize,
                unsafe extern "system" fn(*const c_char) -> *const c_void,
            >(sym)
        };
        Ok((get_proc, module as usize))
    }
    #[cfg(target_os = "linux")]
    {
        let path = dir.join("libEGL.so");
        let cpath = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())
            .map_err(|_| format!("angle: bad path {}", path.display()))?;
        // RTLD_LOCAL：ANGLE 自带的 libGLESv2 不污染进程全局符号表，
        // 系统的 Mesa libEGL/libGL 与它互不干扰。
        let module = unsafe { libc::dlopen(cpath.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
        if module.is_null() {
            let err = unsafe {
                std::ffi::CStr::from_ptr(libc::dlerror())
                    .to_string_lossy()
                    .into_owned()
            };
            return Err(format!("angle: failed to load {}: {err}", path.display()));
        }
        let sym = unsafe { libc::dlsym(module, c"eglGetProcAddress".as_ptr()) };
        if sym.is_null() {
            return Err(format!(
                "angle: eglGetProcAddress missing in {}",
                path.display()
            ));
        }
        let get_proc = unsafe {
            std::mem::transmute::<
                *mut c_void,
                unsafe extern "system" fn(*const c_char) -> *const c_void,
            >(sym)
        };
        Ok((get_proc, module as usize))
    }
}

fn load_egl() -> std::result::Result<Egl, String> {
    /// 取 `eglQueryDisplayAttrib`（EGL 1.5 核心函数）。
    ///
    /// ANGLE 的 `EGL_ANGLE_device_vulkan` 用它取 Vulkan 句柄；部分构建
    /// 只导出 `eglQueryDisplayAttribKHR` 别名。两个都取不到时返回
    /// `None`——此时拿不到 ANGLE 的 Vulkan 句柄（但 ANGLE 本身仍可用）。
    unsafe fn resolve_opt_query_display_attrib(
        get_proc: unsafe extern "system" fn(*const c_char) -> *const c_void,
    ) -> Option<unsafe extern "system" fn(Display, i32, *mut c_void) -> u32> {
        let core = get_proc(c"eglQueryDisplayAttrib".as_ptr().cast());
        let khr = get_proc(c"eglQueryDisplayAttribKHR".as_ptr().cast());
        let sym = if core.is_null() { khr } else { core };
        if sym.is_null() {
            return None;
        }
        Some(std::mem::transmute::<
            *const c_void,
            unsafe extern "system" fn(Display, i32, *mut c_void) -> u32,
        >(sym))
    }
    let get_proc: unsafe extern "system" fn(*const c_char) -> *const c_void;
    let module: usize;
    let device;
    match std::env::var_os("OC_ANGLE_DIR") {
        Some(dir) => {
            let (gp, m) = load_dynamic_angle(std::path::Path::new(&dir))?;
            get_proc = gp;
            module = m;
            // 动态 ANGLE 走 Vulkan 后端。设备由 OC_ANGLE_SWIFTSHADER 选：
            // 设了走 SwiftShader（CPU 软件光栅，与 swgl 参考同渲染器），
            // 不设走硬件 GPU（快，日常渲染用）。
            device = if std::env::var_os("OC_ANGLE_SWIFTSHADER").is_some() {
                AngleDevice::VulkanSwiftShader
            } else {
                AngleDevice::VulkanHardware
            };
        }
        None => {
            #[cfg(not(target_os = "windows"))]
            {
                return Err(
                    "angle: 本平台无静态 ANGLE（mozangle 仅 Windows），须设 OC_ANGLE_DIR \
                     指向含 libEGL/libGLESv2/vk_swiftshader 的目录"
                        .to_string(),
                );
            }
            #[cfg(target_os = "windows")]
            {
            unsafe extern "system" fn static_get_proc(name: *const c_char) -> *const c_void {
                let name = unsafe { std::ffi::CStr::from_ptr(name) };
                mozangle::egl::get_proc_address(name.to_str().unwrap_or(""))
            }
            get_proc = static_get_proc;
            module = 0;
            device = AngleDevice::D3D11;
            }
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
    macro_rules! resolve_opt {
        ($name:literal, $ty:ty) => {{
            let p = unsafe { get_proc(concat!($name, "\0").as_ptr().cast()) };
            if p.is_null() {
                None
            } else {
                Some(unsafe { std::mem::transmute::<*const c_void, $ty>(p) })
            }
        }};
    }
    // 设备查询入口：Electron 的 ANGLE 把 `eglQueryDisplayAttribEXT` 等
    // 直接导出在 libEGL.so 里，但 `eglGetProcAddress` 不返回它们——只能
    // dlsym。Windows 静态 mozangle 无设备路径（D3D11，不需要）。
    #[cfg(target_os = "linux")]
    unsafe fn dlsym_sym(module: usize, name: &std::ffi::CStr) -> *const c_void {
        if module == 0 {
            return std::ptr::null();
        }
        libc::dlsym(module as *mut c_void, name.as_ptr())
    }
    #[cfg(target_os = "linux")]
    macro_rules! dlsym_opt {
        ($name:literal, $ty:ty) => {{
            let name = concat!($name, "\0");
            let p = unsafe {
                dlsym_sym(
                    module,
                    std::ffi::CStr::from_bytes_with_nul_unchecked(name.as_bytes()),
                )
            };
            if p.is_null() {
                None
            } else {
                Some(unsafe { std::mem::transmute::<*const c_void, $ty>(p) })
            }
        }};
    }
    #[cfg(target_os = "linux")]
    let query_display_attrib = unsafe { resolve_opt_query_display_attrib(get_proc) }
        .or_else(|| dlsym_opt!("eglQueryDisplayAttribEXT", unsafe extern "system" fn(Display, i32, *mut c_void) -> u32));
    #[cfg(target_os = "linux")]
    let query_device_attrib = dlsym_opt!("eglQueryDeviceAttribEXT", unsafe extern "system" fn(*mut c_void, i32, *mut c_void) -> u32);
    #[cfg(not(target_os = "linux"))]
    let query_display_attrib = unsafe { resolve_opt_query_display_attrib(get_proc) };
    #[cfg(not(target_os = "linux"))]
    let query_device_attrib: Option<unsafe extern "system" fn(*mut c_void, i32, *mut c_void) -> u32> = None;
    Ok(Egl {
        get_proc,
        module,
        get_platform_display: resolve!("eglGetPlatformDisplayEXT"),
        initialize: resolve!("eglInitialize"),
        query_display_attrib,
        query_device_attrib,
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
        destroy_image: resolve!("eglDestroyImageKHR"),
        device,
    })
}

pub struct AngleContext {
    display: Display,
    context: Context,
    surface: Surface,
}

/// ANGLE 交出的 Vulkan 句柄（`EGL_ANGLE_device_vulkan` 扩展）。
///
/// 所有权仍属 ANGLE：这些句柄只在 ANGLE 的 display 活着期间有效，
/// **不可销毁**。Skia 的 Vulkan backend 借用它们建自己的上下文，于是
/// 两边共享同一个 device 与 queue——WebGL 纹理与 Skia 资源零拷贝。
pub struct AngleVulkanHandles {
    /// `VkInstance`。
    pub instance: *mut std::ffi::c_void,
    /// `VkPhysicalDevice`。
    pub physical_device: *mut std::ffi::c_void,
    /// `VkDevice`。
    pub device: *mut std::ffi::c_void,
    /// `VkQueue`。
    pub queue: *mut std::ffi::c_void,
    /// queue 所在的队列族索引。
    pub queue_family_index: u32,
    /// ANGLE 的 `vkGetInstanceProcAddr`（进程内函数指针）。
    pub get_instance_proc_addr: *mut std::ffi::c_void,
}

impl AngleContext {
    /// 建 ANGLE 离屏上下文并 make current（glow 的入口解析要求 current）。
    pub fn offscreen() -> Result<Self> {
        let egl = egl()?;
        let attribs: Vec<i32> = match egl.device {
            AngleDevice::VulkanHardware => vec![
                EGL_PLATFORM_ANGLE_TYPE_ANGLE,
                EGL_PLATFORM_ANGLE_TYPE_VULKAN_ANGLE,
                EGL_PLATFORM_ANGLE_DEVICE_TYPE_ANGLE,
                EGL_PLATFORM_ANGLE_DEVICE_TYPE_HARDWARE_ANGLE,
                EGL_NONE,
            ],
            AngleDevice::VulkanSwiftShader => vec![
                EGL_PLATFORM_ANGLE_TYPE_ANGLE,
                EGL_PLATFORM_ANGLE_TYPE_VULKAN_ANGLE,
                EGL_PLATFORM_ANGLE_DEVICE_TYPE_ANGLE,
                EGL_PLATFORM_ANGLE_DEVICE_TYPE_SWIFTSHADER_ANGLE,
                EGL_NONE,
            ],
            AngleDevice::D3D11 => vec![
                EGL_PLATFORM_ANGLE_TYPE_ANGLE,
                EGL_PLATFORM_ANGLE_TYPE_D3D11_ANGLE,
                EGL_PLATFORM_ANGLE_MAX_VERSION_MAJOR_ANGLE,
                3,
                EGL_PLATFORM_ANGLE_MAX_VERSION_MINOR_ANGLE,
                0,
                EGL_NONE,
            ],
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

    /// ANGLE 交出的 Vulkan 句柄（`EGL_ANGLE_device_vulkan` 扩展）。
    ///
    /// ANGLE 的 Vulkan 后端**自己**建 instance/device/queue；这个扩展把
    /// 它们交出来，于是 Skia 的 Vulkan backend 可以直接复用同一个 device
    /// ——同 API、同 device，WebGL 纹理与 Skia 资源真正零拷贝共享。
    ///
    /// **句柄在 EGLDevice 上，不在 display 上**：display 属性查询会返回
    /// `EGL_BAD_ATTRIBUTE`。路径：`eglQueryDisplayAttribEXT(dpy,
    /// EGL_DEVICE_EXT)` 拿 display 背后的 EGLDevice → `eglQueryDeviceAttribEXT`
    /// 逐项取 Vulkan 句柄。
    ///
    /// 句柄的**所有权仍属 ANGLE**：这里只是借用，不许销毁。
    ///
    /// 返回 `None` 表示当前 display 不支持该扩展（非 Vulkan 后端，或
    /// ANGLE 构建时未启用）。
    pub fn vulkan_handles(&self) -> Option<AngleVulkanHandles> {
        let egl = egl().ok()?;
        let query_device_attrib = egl.query_device_attrib?;
        unsafe {
            let mut dev_handle: u64 = 0;
            let query_display = egl.query_display_attrib?;
            if query_display(self.display, EGL_DEVICE_EXT, &mut dev_handle as *mut u64 as *mut c_void) == 0 {
                return None;
            }
            let device = dev_handle as *mut c_void;
            let mut instance: u64 = 0;
            let mut physical_device: u64 = 0;
            let mut vk_device: u64 = 0;
            let mut queue: u64 = 0;
            // ANGLE 按 EGLAttrib（64 位）写出参，统一用 u64 接。
            let mut queue_family: u64 = 0;
            let mut gipa: u64 = 0;
            let ok = |attr: i32, out: *mut c_void| query_device_attrib(device, attr, out) != 0;
            let i = &mut instance as *mut u64 as *mut c_void;
            let p = &mut physical_device as *mut u64 as *mut c_void;
            let d = &mut vk_device as *mut u64 as *mut c_void;
            let q = &mut queue as *mut u64 as *mut c_void;
            let f = &mut queue_family as *mut u64 as *mut c_void;
            let g = &mut gipa as *mut u64 as *mut c_void;
            if !ok(EGL_VULKAN_GET_INSTANCE_PROC_ADDR, g)
                || !ok(EGL_VULKAN_INSTANCE_ANGLE, i)
                || !ok(EGL_VULKAN_PHYSICAL_DEVICE_ANGLE, p)
                || !ok(EGL_VULKAN_DEVICE_ANGLE, d)
                || !ok(EGL_VULKAN_QUEUE_ANGLE, q)
                || !ok(EGL_VULKAN_QUEUE_FAMILY_INDEX_ANGLE, f)
            {
                return None;
            }
            Some(AngleVulkanHandles {
                instance: instance as *mut c_void,
                physical_device: physical_device as *mut c_void,
                device: vk_device as *mut c_void,
                queue: queue as *mut c_void,
                queue_family_index: queue_family as u32,
                get_instance_proc_addr: gipa as *mut c_void,
            })
        }
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

/// ANGLE 导出的 Vulkan image（`EGL_ANGLE_vulkan_image` 扩展）。
///
/// **为什么需要它**：Skia 走 Vulkan backend 后，它不再认 GL 纹理——要拿
/// 到的是 `VkImage`。ANGLE 的 Vulkan 后端内部就是 Vulkan，所以它能把自己的
/// 纹理以 `VkImage` 的形式交出来；Skia 再用 `backend_textures::make_vulkan`
/// 把它包成自己的资源。两边同一个 device（同一个 SwiftShader ICD），
/// 所以这是**真零拷贝**，没有 GPU→CPU 往返。
pub struct AngleVulkanImage {
    /// `VkImage` 句柄。
    pub image: *mut std::ffi::c_void,
    /// ANGLE 写回的完整 `VkImageCreateInfo`（标准 Vulkan 结构，按 sType
    /// 解析）。x64 上实际 ~96B：40B 的旧缓冲区会被 ANGLE 写穿，必须给足。
    pub create_info: [u64; 32],
}

/// 把 ANGLE 的一张纹理导出为 Vulkan image。
///
/// `texture` 是 ANGLE 的 GL 纹理名。调用方须已 make current。
///
/// 返回 `None` 表示扩展缺失或导出失败。
pub fn export_vulkan_image(texture: u32) -> Option<AngleVulkanImage> {
    let egl = egl().ok()?;
    unsafe {
        let create_image = {
            let p = (egl.get_proc)(c"eglCreateImageKHR".as_ptr().cast());
            if p.is_null() {
                return None;
            }
            std::mem::transmute::<*const c_void, unsafe extern "system" fn(Display, Context, u32, *const c_void, *const i32) -> *mut c_void>(p)
        };
        let export_vk = {
            let p = (egl.get_proc)(c"eglExportVkImageANGLE".as_ptr().cast());
            if p.is_null() {
                return None;
            }
            std::mem::transmute::<*const c_void, unsafe extern "system" fn(Display, *mut c_void, *mut c_void, *mut c_void) -> i32>(p)
        };
        let display = (egl.get_current_display)();
        let context = (egl.get_current_context)();
        if display.is_null() || context.is_null() {
            return None;
        }
        // source target 是 `EGL_GL_TEXTURE_2D_KHR`（不是裸纹理名）。
        const EGL_GL_TEXTURE_2D_KHR: u32 = 0x30B1;
        // `eglExportVkImageANGLE` 把 ANGLE 内部的 VkImageCreateInfo 写到这里。
        // ANGLE 按完整结构体写入（x64 ~96B），缓冲区必须装下整个结构。
        let mut create_info = [0u64; 32];
        let image = create_image(
            display,
            context,
            EGL_GL_TEXTURE_2D_KHR,
            texture as *const c_void,
            [EGL_NONE].as_ptr(),
        );
        if image.is_null() {
            return None;
        }
        let mut out_image: u64 = 0;
        let ok = export_vk(
            display,
            image,
            &mut out_image as *mut u64 as *mut c_void,
            create_info.as_mut_ptr() as *mut c_void,
        );
        (egl.destroy_image)(display, image);
        if ok == 0 {
            return None;
        }
        Some(AngleVulkanImage {
            image: out_image as *mut c_void,
            create_info,
        })
    }
}
