//! Vulkan 会话：独立 VkDevice 或 ANGLE 共享 VkDevice 上的 Skia backend。
//!
//! `VK_ICD_FILENAMES` 是进程级变量；Skia / ANGLE / wgpu 必须看到同一份
//! ICD 清单，由宿主统一设置。
//!
//! **线程纪律**：Vulkan device 与 queue 非线程安全，会话建与用同线程。

use anyhow::{Result, anyhow};
use ash::vk::Handle;
use skia_safe::gpu::vk;

/// 进程级 Vulkan 引导：让 Skia / ANGLE / wgpu 三家看到同一份 ICD。
///
/// 若调用方没设 `VK_ICD_FILENAMES`，就从 `OC_ANGLE_DIR` 推出同目录的
/// `vk_swiftshader_icd.json` 并设上。已设的值不动。
///
/// 幂等，可重复调。
pub fn bootstrap_icd() {
    if std::env::var_os("VK_ICD_FILENAMES").is_some() {
        return;
    }
    let Some(dir) = std::env::var_os("OC_ANGLE_DIR") else {
        return;
    };
    let icd = std::path::Path::new(&dir).join("vk_swiftshader_icd.json");
    if !icd.exists() {
        tracing::warn!(
            path = %icd.display(),
            "OC_ANGLE_DIR 下无 vk_swiftshader_icd.json：Skia/wgpu 的 Vulkan loader \
             将回退系统默认 ICD"
        );
        return;
    }
    // ICD json 的 library_path 必须是绝对路径：相对路径只在 CWD 恰好匹配时
    // 生效（Vulkan loader 按 CWD 解析相对路径）。
    unsafe {
        std::env::set_var("VK_ICD_FILENAMES", &icd);
    }
    tracing::debug!(icd = %icd.display(), "已设 VK_ICD_FILENAMES");
}

/// 建好的 Vulkan 会话：持 device/queue 句柄与 Skia 的 backend 上下文。
///
/// `BackendContext` 借用 `get_proc` 闭包，二者必须同生命周期。闭包要
/// 捕获 `ash::Instance`（device 的 proc 解析走它），而 `BackendContext`
/// 又要借用闭包——这是自引用。解法：把两者放进同一个 `Box`，用
/// `Box::leak` 把该分配延长到 `'static`（会话销毁时随进程结束回收，
/// 代价是一小块永不释放的内存）。
pub struct VkSession {
    inner: Box<SessionInner>,
}

/// 会话内部结构：`get_proc` 闭包与借用它的 `BackendContext` 同处一个
/// 分配里。
struct SessionInner {
    /// 自建 instance/device 的所有权（[`VkSession::new`] 路径）。ANGLE
    /// 句柄路径（[`VkSession::from_angle`]）为 `None`——两边资源都归
    /// ANGLE 所有，**绝不销毁**。
    owned: Option<OwnedSession>,
    /// 物理设备句柄（Skia 的 backend context 引用它）。
    physical_device: vk::PhysicalDevice,
    /// 队列句柄。
    queue: vk::Queue,
    /// 队列族索引。
    queue_index: usize,
    /// Skia 的 Vulkan backend 上下文（引用上面的句柄）。
    backend: vk::BackendContext<'static>,
}

/// [`VkSession::new`] 自建资源的所有权。
struct OwnedSession {
    _instance: std::sync::Arc<ash::Instance>,
    _device: ash::Device,
}

impl Drop for SessionInner {
    fn drop(&mut self) {
        if self.owned.is_none() {
            // ANGLE 借来的 device：Skia 析构前 device 必须还活着——
            // 「渲染会话先于 GL 会话析构」的字段序保证这一点。若 display
            // 被提前 eglTerminate（见 FrameRenderer::new 的会话遮蔽修复），
            // VMA destroy 会撞已销毁的 device（lvp 实测 ICD 内解引用）。
        }
        // owned 的 instance/device 由字段顺序自动 drop；ANGLE 路径无事可做。
        let _ = &self.owned;
    }
}
impl VkSession {
    /// 建 Vulkan 会话并交给 Skia 建立 backend 上下文。
    ///
    /// `VK_ICD_FILENAMES` 决定加载哪个 ICD；调用方负责设好。
    pub fn new() -> Result<Self> {
        bootstrap_icd();
        let entry = unsafe { ash::Entry::load() }
            .map_err(|e| anyhow!("Vulkan loader 加载失败（检查 VK_ICD_FILENAMES）: {e}"))?;

        // Vulkan 1.1 是 Skia 的最低要求（Skia 侧 BackendContextBuilder 明确写明）。
        let app_name = ash::vk::ApplicationInfo::default()
            .application_name(c"opencat")
            .application_version(1)
            .engine_name(c"opencat")
            .engine_version(1)
            .api_version(ash::vk::API_VERSION_1_1);
        let create_info = ash::vk::InstanceCreateInfo::default().application_info(&app_name);
        let instance = unsafe { entry.create_instance(&create_info, None) }
            .map_err(|e| anyhow!("Vulkan create_instance 失败: {e}"))?;

        // 物理设备：优先 SwiftShader。
        let phys = unsafe { instance.enumerate_physical_devices() }
            .map_err(|e| anyhow!("Vulkan enumerate_physical_devices 失败: {e}"))?;
        let mut chosen = None;
        for p in &phys {
            let props = unsafe { instance.get_physical_device_properties(*p) };
            let name = unsafe {
                std::ffi::CStr::from_ptr(props.device_name.as_ptr())
                    .to_string_lossy()
                    .into_owned()
            };
            tracing::debug!(device = %name, "vulkan 物理设备");
            if name.contains("SwiftShader") {
                chosen = Some(*p);
            }
        }
        let physical = chosen
            .or_else(|| phys.first().copied())
            .ok_or_else(|| anyhow!("Vulkan 无可用物理设备"))?;

        // 队列族：Skia 要 graphics 队列。
        let families = unsafe { instance.get_physical_device_queue_family_properties(physical) };
        let queue_index = families
            .iter()
            .position(|f| f.queue_flags.contains(ash::vk::QueueFlags::GRAPHICS))
            .ok_or_else(|| anyhow!("Vulkan 无 graphics 队列族"))? as u32;

        // device + queue。
        let priority = [1.0f32];
        let queue_info = ash::vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_index)
            .queue_priorities(&priority);
        let device_info =
            ash::vk::DeviceCreateInfo::default().queue_create_infos(std::slice::from_ref(&queue_info));
        let device = unsafe { instance.create_device(physical, &device_info, None) }
            .map_err(|e| anyhow!("Vulkan create_device 失败: {e}"))?;
        let queue = unsafe { device.get_device_queue(queue_index, 0) };

        // 自引用：先把闭包与句柄放进 Box，再把 Box 泄漏成 'static，
        // 这样 BackendContext 能安全借用闭包。
        // instance 被闭包捕获（device 的 proc 解析走它），所以用 Arc
        // 与结构体字段共享同一个 instance。
        let instance = std::sync::Arc::new(instance);
        let instance_handle = instance.handle();
        let device_handle = device.handle();
        let get_proc_instance = instance.clone();
        let get_proc = move |of: vk::GetProcOf| -> vk::GetProcResult {
            let name = unsafe { of.name() };
            let name = name.to_str().unwrap_or("");
            match of {
                vk::GetProcOf::Instance(_, _) => unsafe {
                    entry
                        .get_instance_proc_addr(instance_handle, name.as_ptr() as *const i8)
                        .map(|f| f as *const std::ffi::c_void)
                        .unwrap_or(std::ptr::null())
                },
                vk::GetProcOf::Device(_, _) => unsafe {
                    get_proc_instance
                        .get_device_proc_addr(device_handle, name.as_ptr() as *const i8)
                        .map(|f| f as *const std::ffi::c_void)
                        .unwrap_or(std::ptr::null())
                },
            }
        };

        // Box::leak 先给闭包一个稳定的 'static 地址，再建 backend。
        // `new_builder` 收 `&impl GetProc`（Sized），所以这里传具体类型
        // 的引用，不擦成 trait object。
        let get_proc: &'static _ = Box::leak(Box::new(get_proc));

        let backend = unsafe {
            vk::BackendContext::new_builder(
                instance_handle.as_raw() as *mut std::ffi::c_void as vk::Instance,
                physical.as_raw() as *mut std::ffi::c_void as vk::PhysicalDevice,
                device_handle.as_raw() as *mut std::ffi::c_void as vk::Device,
                (
                    queue.as_raw() as *mut std::ffi::c_void as vk::Queue,
                    queue_index as usize,
                ),
                get_proc,
                // ANGLE 把 device 钉在低 API 版本（lavapipe 上 1.2+ 函数
                // 全 NULL）。Skia 的 1.1 面够用（SwiftShader 同款验证过），
                // 显式声明，避免 Skia 按 1.2 要求解析不存在的入口。
                Some(vk::Version::new(1, 1, 0)),
            )
            .build()
        };

        Ok(Self {
            inner: Box::new(SessionInner {
                owned: Some(OwnedSession {
                    _instance: instance,
                    _device: device,
                }),
                physical_device: physical.as_raw() as *mut std::ffi::c_void as vk::PhysicalDevice,
                queue: queue.as_raw() as *mut std::ffi::c_void as vk::Queue,
                queue_index: queue_index as usize,
                backend,
            }),
        })
    }

    /// 物理设备句柄（导入纹理等需要）。
    pub fn physical_device(&self) -> vk::PhysicalDevice {
        self.inner.physical_device
    }

    /// 队列句柄。
    pub fn queue(&self) -> vk::Queue {
        self.inner.queue
    }

    /// 队列族索引。
    pub fn queue_index(&self) -> usize {
        self.inner.queue_index
    }

    /// Skia 的 Vulkan backend 上下文。
    pub fn backend(&self) -> &vk::BackendContext<'static> {
        &self.inner.backend
    }

    /// 从 ANGLE 交出的 Vulkan 句柄建 Skia 会话（零拷贝的前提）。
    ///
    /// **为什么不能用 [`VkSession::new`]**：ANGLE 的 Vulkan 后端自己建
    /// instance/device；Skia 若另建一套，两边就是不同的 `VkDevice`——
    /// image 内存无法跨 device 共享，「ANGLE 导出 VkImage → Skia 导入」
    /// 直接不成立。这个构造器把 Skia 的 backend 上下文直接建在 ANGLE 的
    /// device 与 queue 上，两边共享同一份资源。
    ///
    /// 句柄所有权仍在 ANGLE：这里只借用，不销毁。get_proc 经由进程内
    /// **ANGLE 自己的 `vkGetInstanceProcAddr`**（`EGL_VULKAN_GET_INSTANCE_
    /// PROC_ADDR`）解析——绝不能用 ash 加载的系统 loader：ANGLE 的
    /// instance 由 ANGLE 自带的 libvulkan 创建，跨 loader 调用直接段错误
    /// （实测 Skia 在 vkEnumerateDeviceExtensionProperties 内崩）。
    pub fn from_angle(handles: &super::angle::AngleVulkanHandles) -> Result<Self> {
        let instance = ash::vk::Instance::from_raw(handles.instance as u64);
        let physical = ash::vk::PhysicalDevice::from_raw(handles.physical_device as u64);
        let device = ash::vk::Device::from_raw(handles.device as u64);
        let queue = ash::vk::Queue::from_raw(handles.queue as u64);
        let queue_index = handles.queue_family_index;

        if handles.get_instance_proc_addr.is_null() {
            return Err(anyhow!("ANGLE 未提供 vkGetInstanceProcAddr"));
        }
        type PfnGetInstanceProcAddr = unsafe extern "system" fn(
            ash::vk::Instance,
            *const i8,
        ) -> *const std::ffi::c_void;
        let gipa: PfnGetInstanceProcAddr =
            unsafe { std::mem::transmute(handles.get_instance_proc_addr) };
        // 全局命令兜底：ANGLE 的 gipa 是 libGLESv2 内部的 dispatch（ANGLE
        // 的 Vulkan 后端编译在 libGLESv2 里），不认全局命令（返回 NULL，
        // Skia 会直接调用 → 段错误）。全局命令**不带任何 ANGLE 句柄参数**，
        // 用任一 Vulkan loader（优先进程内已加载的 libvulkan.so.1）解析都
        // 语义等价。**仅白名单命令走这条路**——instance/device 级函数若
        // gipa 拒绝，用别的 loader 解析会跨 loader 段错误（实测），宁可
        // 返回 NULL 让 Skia 走降级。
        /// Windows：解析与 ANGLE libEGL 同目录的 vulkan-1.dll。ANGLE 用
        /// 绝对路径加载自带的 loader，`LoadLibraryA("vulkan-1.dll")` 走标准
        /// 搜索序会命中 System32 的系统 loader——device 句柄不是它注册的，
        /// 其 gdpa 一调就段错误（实测）。必须用 ANGLE 同目录的那份。
        #[cfg(target_os = "windows")]
        fn angle_loader() -> windows_sys::Win32::Foundation::HMODULE {
            use windows_sys::Win32::System::LibraryLoader as W;
            unsafe {
                let egl = W::GetModuleHandleA(c"libEGL.dll".as_ptr() as *const u8);
                if egl.is_null() {
                    return std::ptr::null_mut();
                }
                let mut buf = [0u8; 512];
                let n = W::GetModuleFileNameA(egl, buf.as_mut_ptr(), buf.len() as u32) as usize;
                let path = String::from_utf8_lossy(&buf[..n.min(buf.len())]).into_owned();
                let dir = match path.rfind('\\') {
                    Some(i) => &path[..i],
                    None => return std::ptr::null_mut(),
                };
                let full = format!("{dir}\\vulkan-1.dll");
                let mut cname: Vec<u8> = full.bytes().chain(std::iter::once(0)).collect();
                W::LoadLibraryA(cname.as_ptr())
            }
        };
        const GLOBAL_COMMANDS: [&str; 5] = [
            "vkEnumerateInstanceExtensionProperties",
            "vkEnumerateInstanceLayerProperties",
            "vkEnumerateInstanceVersion",
            "vkCreateInstance",
            "vkGetInstanceProcAddr",
        ];
        let dlsym_global = move |name: &str| -> *const std::ffi::c_void {
            if !GLOBAL_COMMANDS.contains(&name) {
                return std::ptr::null();
            }
            #[cfg(target_os = "linux")]
            unsafe {
                let Ok(cname) = std::ffi::CString::new(name) else {
                    return std::ptr::null();
                };
                let handle = libc::dlopen(
                    c"libvulkan.so.1".as_ptr().cast(),
                    libc::RTLD_LAZY | libc::RTLD_LOCAL | libc::RTLD_NOLOAD,
                );
                let handle = if handle.is_null() {
                    libc::dlopen(c"libvulkan.so.1".as_ptr().cast(), libc::RTLD_LAZY | libc::RTLD_LOCAL)
                } else {
                    handle
                };
                if handle.is_null() {
                    return std::ptr::null();
                }
                let p = libc::dlsym(handle, cname.as_ptr());
                if p.is_null() { std::ptr::null() } else { p as *const std::ffi::c_void }
            }
            #[cfg(target_os = "windows")]
            unsafe {
                // 优先 ANGLE 同目录的 loader（与 device 句柄同源）；全局
                // 命令语义与 loader 无关，但用同一份最稳。
                let mut handle = (angle_loader)();
                if handle.is_null() {
                    handle = windows_sys::Win32::System::LibraryLoader::LoadLibraryA(
                        c"vulkan-1.dll".as_ptr() as *const u8,
                    );
                }
                if handle.is_null() {
                    return std::ptr::null();
                }
                let cname: Vec<u8> = name.bytes().chain(std::iter::once(0)).collect();
                windows_sys::Win32::System::LibraryLoader::GetProcAddress(handle, cname.as_ptr())
                    .map(|f| std::mem::transmute::<unsafe extern "system" fn() -> isize, *const std::ffi::c_void>(f))
                    .unwrap_or(std::ptr::null())
            }
            #[cfg(not(any(target_os = "linux", target_os = "windows")))]
            {
                let _ = name;
                std::ptr::null()
            }
        };
        // device 句柄由 ANGLE 自带 loader 注册。设备级函数走同一个 loader
        // 的 vkGetDeviceProcAddr，避免混合 dispatch。
        type PfnGetDeviceProcAddr =
            unsafe extern "system" fn(ash::vk::Device, *const i8) -> *const std::ffi::c_void;
        // loader 级 gdpa：**不能**经 ANGLE gipa 取——ANGLE 对部分 device
        // 命令有自己的 interpose（trace 里 vkQueueSubmit 与其他函数来自
        // 不同模块），Skia/VMA 拿到混合 dispatch 后析构崩在 ICD 内。
        // 直接 dlsym 真实 loader（ANGLE 自带 libvulkan.so.1 / vulkan-1.dll，
        // 进程内已加载）的 vkGetDeviceProcAddr，device 句柄本就是它注册的，
        // 函数表单一。
        let loader_gdpa: Option<PfnGetDeviceProcAddr> = unsafe {
            #[cfg(target_os = "linux")]
            {
                let handle = libc::dlopen(
                    c"libvulkan.so.1".as_ptr().cast(),
                    libc::RTLD_LAZY | libc::RTLD_LOCAL | libc::RTLD_NOLOAD,
                );
                let handle = if handle.is_null() {
                    libc::dlopen(
                        c"libvulkan.so.1".as_ptr().cast(),
                        libc::RTLD_LAZY | libc::RTLD_LOCAL,
                    )
                } else {
                    handle
                };
                if handle.is_null() {
                    None
                } else {
                    let p = libc::dlsym(handle, c"vkGetDeviceProcAddr".as_ptr());
                    if p.is_null() {
                        None
                    } else {
                        Some(std::mem::transmute::<*mut std::ffi::c_void, PfnGetDeviceProcAddr>(p))
                    }
                }
            }
            #[cfg(target_os = "windows")]
            {
                let mut handle = (angle_loader)();
                if handle.is_null() {
                    handle = windows_sys::Win32::System::LibraryLoader::LoadLibraryA(
                        c"vulkan-1.dll".as_ptr() as *const u8,
                    );
                }
                if handle.is_null() {
                    None
                } else {
                    windows_sys::Win32::System::LibraryLoader::GetProcAddress(
                        handle,
                        c"vkGetDeviceProcAddr".as_ptr() as *const u8,
                    )
                    .map(|f| {
                        std::mem::transmute::<unsafe extern "system" fn() -> isize, PfnGetDeviceProcAddr>(f)
                    })
                }
            }
            #[cfg(not(any(target_os = "linux", target_os = "windows")))]
            {
                let _ = &gipa;
                let _ = instance;
                None
            }
        };

        let get_proc = move |of: vk::GetProcOf| -> vk::GetProcResult {
            let name = unsafe { of.name() };
            let name = name.to_str().unwrap_or("");
            match of {
                vk::GetProcOf::Instance(_, _) => unsafe {
                    let p = gipa(instance, name.as_ptr() as *const i8);
                    let p = if p.is_null() {
                        dlsym_global(name)
                    } else {
                        p as *const std::ffi::c_void
                    };
                    if p.is_null() { std::ptr::null() } else { p as *const std::ffi::c_void }
                },
                vk::GetProcOf::Device(_, _) => unsafe {
                    let p = match loader_gdpa {
                        Some(f) => f(device, name.as_ptr() as *const i8),
                        None => {
                            // 无 loader 可用（理论上不发生）：退回 ANGLE dispatch。
                            let p = gipa(instance, c"vkGetDeviceProcAddr".as_ptr());
                            if p.is_null() {
                                std::ptr::null()
                            } else {
                                let f: PfnGetDeviceProcAddr = std::mem::transmute(p);
                                f(device, name.as_ptr() as *const i8)
                            }
                        }
                    };
                    if p.is_null() {
                        std::ptr::null()
                    } else {
                        p as *const std::ffi::c_void
                    }
                },
            }
        };
        let get_proc: &'static _ = Box::leak(Box::new(get_proc));

        let backend = unsafe {
            vk::BackendContext::new_builder(
                instance.as_raw() as *mut std::ffi::c_void as vk::Instance,
                physical.as_raw() as *mut std::ffi::c_void as vk::PhysicalDevice,
                device.as_raw() as *mut std::ffi::c_void as vk::Device,
                (
                    queue.as_raw() as *mut std::ffi::c_void as vk::Queue,
                    queue_index as usize,
                ),
                get_proc,
                // ANGLE 实例钉在 Vulkan 1.1；显式声明 1.1 面，Skia 便不
                // 再索要 loader 门控成 NULL 的 1.2/1.3 核心入口。
                Some(vk::Version::new(1, 1, 0)),
            )
            .build()
        };

        Ok(Self {
            inner: Box::new(SessionInner {
                owned: None,
                physical_device: physical.as_raw() as *mut std::ffi::c_void as vk::PhysicalDevice,
                queue: queue.as_raw() as *mut std::ffi::c_void as vk::Queue,
                queue_index: queue_index as usize,
                backend,
            }),
        })
    }
}

/// ANGLE 导出 image 的元数据（从写回的 `VkImageCreateInfo` 解析）。
#[derive(Debug)]
pub struct AngleVkImageMeta {
    pub format: u32,
    pub mip_levels: u32,
    pub usage: u32,
    pub tiling: u32,
    pub initial_layout: u32,
}

impl super::angle::AngleVulkanImage {
    /// 解析 ANGLE 写回的 `VkImageCreateInfo`。
    ///
    /// ANGLE 按标准 Vulkan 布局写整个结构体；用 ash 的同型定义解引用。
    pub fn meta(&self) -> Result<AngleVkImageMeta> {
        let info = unsafe {
            (self.create_info.as_ptr() as *const ash::vk::ImageCreateInfo)
                .as_ref()
                .ok_or_else(|| anyhow!("VkImageCreateInfo 指针为空"))?
        };
        if info.s_type != ash::vk::StructureType::IMAGE_CREATE_INFO {
            return Err(anyhow!(
                "ANGLE 写回的 create info sType 异常（{:?}）——缓冲区布局不匹配",
                info.s_type
            ));
        }
        Ok(AngleVkImageMeta {
            format: info.format.as_raw() as u32,
            mip_levels: info.mip_levels,
            usage: info.usage.as_raw(),
            tiling: info.tiling.as_raw() as u32,
            initial_layout: info.initial_layout.as_raw() as u32,
        })
    }
}

/// 把 ANGLE 的 GL 纹理（流输出）零拷贝导入为 Skia image。
///
/// 路径：`export_vulkan_image` 拿 VkImage → 解析 create info → 包成 Skia 的
/// Vulkan backend texture。Skia 的 device 必须来自 [`VkSession::from_angle`]
/// （同一个 `VkDevice`），否则导入无意义。
///
/// layout 取 ANGLE 报告的 `initialLayout`；报告 UNDEFINED（ANGLE 未填或
/// 版本差异）时按 SHADER_READ_ONLY_OPTIMAL 兜底——导出的采样纹理最终都
/// 停在这个布局。
///
/// image 的内存归 ANGLE 的 GL 纹理所有（alloc 留空），调用方保证纹理在
/// 采样期内存活（合成完成前不删）。
pub fn import_stream_texture(
    skia: &mut skia_safe::gpu::DirectContext,
    native: crate::gpu::NativeTexture,
    width: u32,
    height: u32,
    queue_family: u32,
) -> Result<skia_safe::Image> {
    use skia_safe::gpu::{
        backend_textures, ganesh::vk::vk_types::ImageInfo as SkVkImageInfo, images, SurfaceOrigin,
    };

    let exported = super::angle::export_vulkan_image(native.raw() as u32)
        .ok_or_else(|| anyhow!("ANGLE 导出 VkImage 失败（扩展缺失或纹理无效）"))?;
    let meta = exported.meta()?;
    let layout_raw = if meta.initial_layout == ash::vk::ImageLayout::UNDEFINED.as_raw() as u32 {
        ash::vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL.as_raw() as u32
    } else {
        meta.initial_layout
    };
    let mut info = unsafe {
        SkVkImageInfo::new(
            exported.image as vk::Image,
            vk::Alloc::default(),
            std::mem::transmute::<u32, vk::ImageTiling>(meta.tiling),
            std::mem::transmute::<u32, vk::ImageLayout>(layout_raw),
            std::mem::transmute::<u32, vk::Format>(meta.format),
            meta.mip_levels,
            Some(queue_family),
            None,
            None,
            None,
        )
    };
    info.image_usage_flags = meta.usage;

    let backend_tex = unsafe {
        backend_textures::make_vk(
            (width as i32, height as i32),
            &info,
            "gpu-stream-output",
        )
    };
    // GL 纹理是 bottom-left 原点（FBO 行序自底向上），与 GL 后端导入一致。
    images::borrow_texture_from(
        skia,
        &backend_tex,
        SurfaceOrigin::BottomLeft,
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Premul,
        None,
    )
    .ok_or_else(|| anyhow!("borrow_texture_from 失败（Vulkan backend texture）"))
}
