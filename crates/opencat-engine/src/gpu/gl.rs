//! WebGL2 命令流使用的裸 GL 原语。
//!
//! GL 函数面统一走 [`glow`]（WebGL 同名 API 形状）——本 crate 不再手写
//! `extern`：[`crate::gpu::session`] 在 ANGLE 上下文绑定到线程后构建一次
//! `glow::Context`。调用方把这份函数表传给这里的 GL 操作。
//!
//! 本模块创建命令流输出使用的纹理和 FBO。调用线程必须有 current 的 GL
//! 上下文，供 ANGLE 的 WebGL2 执行器使用。

use glow::HasContext;
use std::num::NonZeroU32;

use super::NativeTexture;

/// glow 的句柄包装是 NonZero 的——0 永远不进 glow(调用方已在 NativeTexture
/// 层面把 0 当「无纹理」拦截)。
fn nz(v: u32) -> NonZeroU32 {
    NonZeroU32::new(v).expect("glow handle 0")
}

/// Skia 的 `gpu::gl::TextureInfo` 要填的目标与内部格式（GL 枚举）。
///
/// 这两个值不是本 crate 用的，是给调用方填 Skia 结构体用的——放在这里，
/// 调用方就不必自己写 GL 魔数。
pub const TEXTURE_2D: u32 = 0x0DE1;
pub const RGBA8: u32 = 0x8058;

/// FBO id → 其深度渲染缓冲（[`create_framebuffer`] 登记，[`delete_framebuffer`]
/// 取走删除）。glow 0.13 没有附件查询入口，注册表兜底。
fn depth_renderbuffers() -> &'static std::sync::Mutex<std::collections::HashMap<u32, u32>> {
    static T: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<u32, u32>>> =
        std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

/// 建一个以 `tex` 为颜色附件、`width × height` DEPTH_COMPONENT24 渲染
/// 缓冲为深度附件的 FBO，返回 FBO id。
///
/// 深度附件是 WebGL 画布的默认语义（context 属性 depth=true）：3D 流的
/// 深度测试全指着它——没有它深度测试恒过，画家序后画者胜，近处物体被
/// 远处表面覆盖。
///
/// FBO 不完整时返回 `None`。成功时 FBO 已解绑（绑回 0），不干扰调用方
/// 后续的 GL 状态。
///
/// 执行器将返回的 id 绑定为流的输出。删除时，
/// [`delete_framebuffer`] 一并释放其深度附件。
pub fn create_framebuffer(
    gl: &glow::Context,
    tex: NativeTexture,
    width: u32,
    height: u32,
) -> Option<u32> {
    unsafe {
        let fbo = gl.create_framebuffer().ok()?;
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(glow::NativeTexture(nz(tex.raw() as u32))),
            0,
        );
        let depth = gl.create_renderbuffer().ok()?;
        gl.bind_renderbuffer(glow::RENDERBUFFER, Some(depth));
        gl.renderbuffer_storage(
            glow::RENDERBUFFER,
            glow::DEPTH24_STENCIL8,
            width as i32,
            height as i32,
        );
        gl.framebuffer_renderbuffer(
            glow::FRAMEBUFFER,
            glow::DEPTH_STENCIL_ATTACHMENT,
            glow::RENDERBUFFER,
            Some(depth),
        );
        gl.bind_renderbuffer(glow::RENDERBUFFER, None);
        let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        if status != glow::FRAMEBUFFER_COMPLETE {
            gl.delete_renderbuffer(depth);
            gl.delete_framebuffer(fbo);
            return None;
        }
        depth_renderbuffers()
            .lock()
            .unwrap()
            .insert(fbo.0.get(), depth.0.get());
        Some(fbo.0.get())
    }
}

/// 删除 [`create_framebuffer`] 建的 FBO（连同其深度渲染缓冲）。
pub fn delete_framebuffer(gl: &glow::Context, fbo: u32) {
    if fbo == 0 {
        return;
    }
    let depth = depth_renderbuffers().lock().unwrap().remove(&fbo);
    unsafe {
        if let Some(depth) = depth {
            gl.delete_renderbuffer(glow::NativeRenderbuffer(nz(depth)));
        }
        gl.delete_framebuffer(glow::NativeFramebuffer(nz(fbo)));
    }
}

/// 建一张 `width × height` 的 RGBA8 纹理（数据为空——作为渲染目标的颜色
/// 附件，内容由渲染写入）。返回纹理名。
///
/// 需要 GL context current。失败（含尺寸为 0）时返回 `None`。
pub fn create_texture_rgba8(gl: &glow::Context, width: u32, height: u32) -> Option<NativeTexture> {
    if width == 0 || height == 0 {
        return None;
    }
    unsafe {
        let tex = gl.create_texture().ok()?;
        gl.bind_texture(glow::TEXTURE_2D, Some(tex));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            width as i32,
            height as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            None,
        );
        // 只填 mip 0 的纹理在 GL 里默认 MIN_FILTER=NEAREST_MIPMAP_LINEAR 下
        // 是**不完整**的：FBO 回读仍拿得到内容，但采样一律返回 (0,0,0,1)。
        // 桌面 NVIDIA 对此宽容，ANGLE/Vulkan 严格——被 Skia 采样的纹理
        // 必须显式给非 mipmap 过滤。
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_S,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_T,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.bind_texture(glow::TEXTURE_2D, None);
        Some(NativeTexture::from_raw(tex.0.get() as u64))
    }
}

/// 删除 [`create_texture_rgba8`] 建的纹理。需要 GL context current。
pub fn delete_texture(gl: &glow::Context, tex: NativeTexture) {
    if tex.is_none() {
        return;
    }
    unsafe {
        gl.delete_texture(glow::NativeTexture(nz(tex.raw() as u32)));
    }
}
