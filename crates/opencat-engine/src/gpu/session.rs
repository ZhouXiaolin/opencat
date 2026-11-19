//! 离屏 WebGL 兼容会话：ANGLE 的 Vulkan device 上执行 WebGL2 GL 命令流。
//!
//! Engine 的渲染 backend 只有 Vulkan。ANGLE 只提供 WebGL 调用面：它接受
//! GLES3 命令，并落到 Vulkan device。硬件 Vulkan 与 SwiftShader 是 device
//! 选择，不是两个 backend。
//!
//! GL 函数面统一走 [`glow`]。会话建立时在 ANGLE current 上下文下构建一次
//! glow 上下文，此后 WebGL2 命令流执行器携带同一份 [`glow::Context`]。
//!
//! **线程纪律**：GL 上下文非线程安全——会话在线程 A 建立就必须在线程 A
//! 使用。

use std::sync::Arc;

use anyhow::Result;

/// 离屏 GL 会话：上下文（构造即 make current）+ 共享的 glow 函数面。
pub struct GlSession {
    gl: Arc<glow::Context>,
    angle: crate::gpu::angle::AngleContext,
}

impl GlSession {
    /// 本会话的 ANGLE 上下文。
    ///
    /// Vulkan 合成路径凭它查 ANGLE 的 Vulkan 句柄（零拷贝的前提）。
    pub fn angle_context(&self) -> &crate::gpu::angle::AngleContext {
        &self.angle
    }

    /// 建 ANGLE Vulkan 离屏上下文并 make current，随后构建 glow 函数面。
    pub fn offscreen() -> Result<Self> {
        // AngleContext::offscreen 构造即 current——glow 的入口解析要求
        // current 上下文。所有入口都来自 ANGLE 的 eglGetProcAddress。
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

    /// 给 WebGL 流执行器和 GLSL 反射器使用的 GL 函数表。
    pub fn gl(&self) -> Arc<glow::Context> {
        self.gl.clone()
    }

    /// 把本上下文绑到当前线程。幂等；每次 GL 使用前调一次，防外部 unmake。
    pub fn make_current(&self) -> Result<()> {
        return self.angle.make_current();
    }

    /// 当前线程的 ANGLE 绑定现场。
    ///
    /// 脚本桥在反射前存现场、反射后归还：反射器与主渲染路径共享同一线程
    /// 的 current 上下文，必须按底座选对接口。
    #[cfg(target_os = "linux")]
    pub fn current_angle_binding() -> Option<crate::gpu::angle::CurrentBinding> {
        crate::gpu::angle::CurrentBinding::save()
    }
}
