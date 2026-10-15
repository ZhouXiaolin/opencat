//! 裸 GL 原语（Windows/WGL）：wgpu 与 Skia 都不提供的 FBO 管理。
//!
//! 全片 GPU 化时 Skia 合成到一个自建 FBO 上（附着一张 wgpu 建的 GL
//! 纹理）：wgpu 只提供纹理、不提供 FBO，Skia 的 `wrap_backend_render_target`
//! 需要一个已存在的 FBO id，所以这几个入口只能直接调 GL。
//!
//! 入口经 `wglGetProcAddress` 解析（FBO 属 GL 3.0+ / `ARB_framebuffer_object`，
//! 不在 opengl32.dll 的 1.1 导出表里）。调用线程必须有 current 的 GL
//! context——调用方经 [`LazyRenderer3d::with_gl_context`] 保证。
//!
//! [`LazyRenderer3d::with_gl_context`]: crate::LazyRenderer3d::with_gl_context

use std::ffi::CString;
use std::os::raw::c_void;

#[link(name = "opengl32")]
unsafe extern "system" {
    fn wglGetProcAddress(name: *const u8) -> *const c_void;
}

const GL_FRAMEBUFFER: u32 = 0x8D40;
const GL_COLOR_ATTACHMENT0: u32 = 0x8CE0;
const GL_TEXTURE_2D: u32 = 0x0DE1;
const GL_FRAMEBUFFER_COMPLETE: u32 = 0x8CD5;

/// 解析一个 GL 入口点。
///
/// `wglGetProcAddress` 对已知函数可能返回 1/2/3/`usize::MAX` 这类哨兵值
/// （规范允许），必须过滤，否则会当成合法指针调用。
fn gl_proc(name: &str) -> *const c_void {
    let Ok(c) = CString::new(name) else {
        return std::ptr::null();
    };
    let p = unsafe { wglGetProcAddress(c.as_ptr() as *const u8) };
    if matches!(p as usize, 0 | 1 | 2 | 3 | usize::MAX) {
        std::ptr::null()
    } else {
        p
    }
}

/// 建一个以 `texture_id` 为颜色附件的 FBO，返回 FBO id。
///
/// 缺 GL 入口或 FBO 不完整时返回 `None`。成功时 FBO 已解绑（绑回 0），
/// 不干扰调用方后续的 GL 状态。
pub fn create_framebuffer(texture_id: u32) -> Option<u32> {
    type Gen = unsafe extern "system" fn(i32, *mut u32);
    type Bind = unsafe extern "system" fn(u32, u32);
    type Attach = unsafe extern "system" fn(u32, u32, u32, u32, i32);
    type Check = unsafe extern "system" fn(u32) -> u32;

    let (g, b, a, c) = (
        gl_proc("glGenFramebuffers"),
        gl_proc("glBindFramebuffer"),
        gl_proc("glFramebufferTexture2D"),
        gl_proc("glCheckFramebufferStatus"),
    );
    if g.is_null() || b.is_null() || a.is_null() || c.is_null() {
        return None;
    }
    let gen_fn: Gen = unsafe { std::mem::transmute(g) };
    let bind: Bind = unsafe { std::mem::transmute(b) };
    let attach: Attach = unsafe { std::mem::transmute(a) };
    let check: Check = unsafe { std::mem::transmute(c) };

    let mut fbo: u32 = 0;
    unsafe {
        gen_fn(1, &mut fbo);
        bind(GL_FRAMEBUFFER, fbo);
        attach(
            GL_FRAMEBUFFER,
            GL_COLOR_ATTACHMENT0,
            GL_TEXTURE_2D,
            texture_id,
            0,
        );
        let status = check(GL_FRAMEBUFFER);
        bind(GL_FRAMEBUFFER, 0);
        if status != GL_FRAMEBUFFER_COMPLETE {
            return None;
        }
    }
    Some(fbo)
}

/// 删除 [`create_framebuffer`] 建的 FBO。
pub fn delete_framebuffer(fbo: u32) {
    type Del = unsafe extern "system" fn(i32, *const u32);
    let p = gl_proc("glDeleteFramebuffers");
    if p.is_null() {
        return;
    }
    let del: Del = unsafe { std::mem::transmute(p) };
    unsafe { del(1, &fbo) };
}
