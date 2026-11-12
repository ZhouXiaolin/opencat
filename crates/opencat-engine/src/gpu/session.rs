//! 离屏 GL 会话：GPU 合成与 GPU 命令流执行共用的上下文底座。
//!
//! opencat-three（wgpu）退役后，engine 的 GPU 基建直接建在裸 GL 上：
//! Windows = ANGLE（D3D11，[`crate::gpu::angle::AngleContext`]，WGL 桌面 GL
//! 已退役——PMREM 等真实浏览器路径在桌面驱动上段错误，plan.md 2026-11-06
//! 拍板）。Linux 同样优先 ANGLE（`OC_ANGLE_DIR` 指向官方 ANGLE Linux 构建，
//! Vulkan + SwiftShader = swgl 参考的渲染器）；未配置 ANGLE 目录时回退原生
//! EGL pbuffer（[`crate::gpu::egl::EglContext::offscreen`]）——桌面 GL 不认
//! GLSL ES 300，WebGL 腿会编译失败，故 ANGLE 是标准路径。
//! GL 函数面统一走 [`glow`]——会话建立时在 current 上下文下构建一次 glow
//! 上下文，此后 GPU 合成器与命令流执行器都携带同一份 [`glow::Context`]。
//!
//! **线程纪律**：GL 上下文非线程安全——会话在线程 A 建立就必须在线程 A
//! 使用（离线渲染的建与用同线程；预览走 `present` 平台模块自己的窗口
//! 上下文，不经这里）。

use std::sync::Arc;

use anyhow::Result;

/// 离屏 GL 会话：上下文（构造即 make current）+ 共享的 glow 函数面。
pub struct GlSession {
    gl: Arc<glow::Context>,
    #[cfg(target_os = "windows")]
    angle: crate::gpu::angle::AngleContext,
    #[cfg(target_os = "linux")]
    linux_backend: LinuxBackend,
}

/// Linux 侧的上下文底座：ANGLE（标准路径）或原生 EGL（回退）。
#[cfg(target_os = "linux")]
enum LinuxBackend {
    Angle(crate::gpu::angle::AngleContext),
    Egl(crate::gpu::egl::EglContext),
}

impl GlSession {
    /// 本会话的 ANGLE 上下文（ANGLE 路径才有）。
    ///
    /// Vulkan 合成路径凭它查 ANGLE 的 Vulkan 句柄（零拷贝的前提）。
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub fn angle_context(&self) -> Option<&crate::gpu::angle::AngleContext> {
        #[cfg(target_os = "windows")]
        return Some(&self.angle);
        #[cfg(target_os = "linux")]
        match &self.linux_backend {
            LinuxBackend::Angle(angle) => Some(angle),
            LinuxBackend::Egl(_) => None,
        }
    }

    /// 建离屏 GL 上下文并 make current，随后构建 glow 函数面。
    pub fn offscreen() -> Result<Self> {
        #[cfg(target_os = "windows")]
        {
            // AngleContext::offscreen 构造即 current——glow 的入口解析要求
            // current 上下文。入口一律走 eglGetProcAddress（ANGLE 静态进
            // 二进制，核心与扩展入口同一来源）。
            let angle = crate::gpu::angle::AngleContext::offscreen()?;
            let gl = unsafe {
                glow::Context::from_loader_function_cstr(|name| {
                    crate::gpu::angle::load_proc(name.to_str().unwrap_or(""))
                })
            };
            Ok(Self {
                gl: Arc::new(gl),
                angle,
            })
        }
        #[cfg(target_os = "linux")]
        {
            // 设了 OC_ANGLE_DIR 就走 ANGLE（与 Windows 同源的 WebGL 语义）；
            // 没设则回退原生 EGL，行为与改动前一致。
            if std::env::var_os("OC_ANGLE_DIR").is_some() {
                let angle = crate::gpu::angle::AngleContext::offscreen()?;
                let gl = unsafe {
                    glow::Context::from_loader_function_cstr(|name| {
                        crate::gpu::angle::load_proc(name.to_str().unwrap_or(""))
                    })
                };
                Ok(Self {
                    gl: Arc::new(gl),
                    linux_backend: LinuxBackend::Angle(angle),
                })
            } else {
                let egl = crate::gpu::egl::EglContext::offscreen()?;
                let gl = unsafe {
                    glow::Context::from_loader_function_cstr(|name| {
                        crate::gpu::egl::load_proc(name.to_str().unwrap_or(""))
                    })
                };
                Ok(Self {
                    gl: Arc::new(gl),
                    linux_backend: LinuxBackend::Egl(egl),
                })
            }
        }
    }

    /// 共享的 GL 函数面。GPU 合成器与命令流执行器用同一份。
    pub fn gl(&self) -> Arc<glow::Context> {
        self.gl.clone()
    }

    /// GL 入口解析器（Skia DirectContext 的 loader 与会话同源）。
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub fn skia_loader(
        &self,
    ) -> std::sync::Arc<dyn Fn(&str) -> *const std::ffi::c_void + Send + Sync> {
        #[cfg(target_os = "windows")]
        return std::sync::Arc::new(crate::gpu::angle::load_proc);
        #[cfg(target_os = "linux")]
        return match &self.linux_backend {
            LinuxBackend::Angle(_) => std::sync::Arc::new(crate::gpu::angle::load_proc),
            LinuxBackend::Egl(_) => std::sync::Arc::new(crate::gpu::egl::load_proc),
        };
    }

    /// 把本上下文绑到当前线程。幂等；每次 GL 使用前调一次，防外部 unmake。
    pub fn make_current(&self) -> Result<()> {
        #[cfg(target_os = "windows")]
        return self.angle.make_current();
        #[cfg(target_os = "linux")]
        return match &self.linux_backend {
            LinuxBackend::Angle(angle) => angle.make_current(),
            LinuxBackend::Egl(egl) => egl.make_current(),
        };
    }

    /// 当前线程的 ANGLE 绑定现场（Linux 上会话底座为原生 EGL 时返回 None）。
    ///
    /// 脚本桥在反射前存现场、反射后归还：反射器与主渲染路径共享同一线程
    /// 的 current 上下文，必须按底座选对接口。
    #[cfg(target_os = "linux")]
    pub fn current_angle_binding() -> Option<crate::gpu::angle::CurrentBinding> {
        crate::gpu::angle::CurrentBinding::save()
    }
}
