//! 裸 GL 原语（Windows/WGL）：wgpu 与 Skia 都不提供的 FBO 与 PBO 管理。
//!
//! 全片 GPU 化时 Skia 合成到一个自建 FBO 上（附着一张 wgpu 建的 GL
//! 纹理）：wgpu 只提供纹理、不提供 FBO，Skia 的 `wrap_backend_render_target`
//! 需要一个已存在的 FBO id，所以这几个入口只能直接调 GL。PBO 双缓冲读回
//! （[`ReadbackPixels`]）同理——Skia 没有暴露异步读回。
//!
//! 入口经 `wglGetProcAddress` 解析（FBO 属 GL 3.0+ / `ARB_framebuffer_object`，
//! 不在 opengl32.dll 的 1.1 导出表里）；`glReadPixels` 是 GL 1.0，直接由
//! opengl32.dll 导出，**不走** `wglGetProcAddress`。调用线程必须有 current
//! 的 GL context——调用方经 [`LazyRenderer3d::with_gl_context`] 保证。
//!
//! [`LazyRenderer3d::with_gl_context`]: crate::LazyRenderer3d::with_gl_context

use std::ffi::CString;
use std::os::raw::c_void;

#[link(name = "opengl32")]
unsafe extern "system" {
    fn wglGetProcAddress(name: *const u8) -> *const c_void;
    /// GL 1.0，由 opengl32.dll 直接导出，**不走** `wglGetProcAddress`。
    fn glReadPixels(
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        format: u32,
        ty: u32,
        pixels: *mut c_void,
    );
    /// GL 1.0，同上。FBO 的读缓冲是 FBO 自身状态，Skia 读过之后可能被留在
    /// 非 `COLOR_ATTACHMENT0` 的值上，必须每次显式设回来。
    fn glReadBuffer(mode: u32);
}

const GL_FRAMEBUFFER: u32 = 0x8D40;
const GL_COLOR_ATTACHMENT0: u32 = 0x8CE0;
const GL_TEXTURE_2D: u32 = 0x0DE1;
const GL_FRAMEBUFFER_COMPLETE: u32 = 0x8CD5;
const GL_PIXEL_PACK_BUFFER: u32 = 0x88EB;
const GL_STREAM_READ: u32 = 0x88E1;
const GL_MAP_READ_BIT: u32 = 0x0001;
const GL_RGBA: u32 = 0x1908;
const GL_UNSIGNED_BYTE: u32 = 0x1401;

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

/// 把 FBO 的颜色附件读回 CPU 的双缓冲读回器。
///
/// `read_pixels` 同步读回会把 GPU→CPU 的传输放在关键路径上：实测 1920×1080
/// RGBA 约 11.9ms/帧，占离线渲染每帧的 18%。这里改成 PBO 双缓冲——本帧把
/// 像素**发起**到 PBO 就返回（实测 0.1ms），传输交给 DMA 引擎，与下一帧的
/// 渲染重叠；下一帧再来取回上一帧的结果（实测等待 0.0ms）。
///
/// 为什么是双缓冲而不是单缓冲：`glReadPixels` 写进 PBO 是异步的，但**同一个
/// PBO 不能一边被 DMA 写、一边被 map 读**。两个槽轮转，读写才落在不同 PBO 上。
///
/// 代价：多一帧延迟，且容量为 2 帧 RGBA（1080p 约 16.6MB）。离线渲染逐帧串行，
/// 帧序不变，所以输出逐字节不变。
pub struct ReadbackPixels {
    fbo: u32,
    width: u32,
    height: u32,
    /// 两个 PBO 槽，构造时建好。
    slots: [u32; 2],
    /// 已发起、待取回的槽。`None` 表示还没有在途的读回。
    pending: Option<usize>,
}

impl ReadbackPixels {
    /// 建两个 PBO 槽。GL 入口缺失时返回 `None`，调用方退回同步读回。
    ///
    /// 必须带 current 的 GL context 调用（PBO 是 context 资源）。
    pub fn new(fbo: u32, width: u32, height: u32) -> Option<Self> {
        type GenBuf = unsafe extern "system" fn(i32, *mut u32);
        type BindBuf = unsafe extern "system" fn(u32, u32);
        type BufferData = unsafe extern "system" fn(u32, isize, *const c_void, u32);
        let (gb, bb, bd) = (
            gl_proc("glGenBuffers"),
            gl_proc("glBindBuffer"),
            gl_proc("glBufferData"),
        );
        // 读回路径要用的入口一次探齐，避免中途才发现缺——那会静默丢帧。
        let (bf, mb) = (gl_proc("glBindFramebuffer"), gl_proc("glMapBufferRange"));
        if gb.is_null() || bb.is_null() || bd.is_null() || bf.is_null() || mb.is_null() {
            return None;
        }
        let gen_buf: GenBuf = unsafe { std::mem::transmute(gb) };
        let bind: BindBuf = unsafe { std::mem::transmute(bb) };
        let data: BufferData = unsafe { std::mem::transmute(bd) };
        let mut slots = [0u32; 2];
        let bytes = (width as usize * height as usize * 4) as isize;
        unsafe {
            gen_buf(2, slots.as_mut_ptr());
            for id in slots {
                bind(GL_PIXEL_PACK_BUFFER, id);
                data(GL_PIXEL_PACK_BUFFER, bytes, std::ptr::null(), GL_STREAM_READ);
            }
            bind(GL_PIXEL_PACK_BUFFER, 0);
        }
        Some(Self {
            fbo,
            width,
            height,
            slots,
            pending: None,
        })
    }

    /// 发起本帧的读回，并取回**上一帧**的结果。
    ///
    /// 返回 `None` 仅发生在首帧（还没有在途读回可取）。此后每帧都返回上一帧的
    /// 像素，所以调用方必须容忍一帧延迟（末帧由 `encode_rgba_frames` 多渲染
    /// 一帧排空）。
    pub fn read_frame(&mut self) -> Option<Vec<u8>> {
        let bytes = self.width as usize * self.height as usize * 4;

        // 下一个写入槽必须**先**定：取回会把 `pending` 清空，之后再看就只能
        // 得到 0，两帧会抢同一个 PBO，双缓冲失效。
        let next_slot = self.pending.map_or(0, |slot| 1 - slot);

        // 先取回上一帧发起的那次传输。此时它早已在后台完成，map 不阻塞。
        let out = match self.pending.take() {
            Some(slot) => self.collect(slot, bytes),
            None => None,
        };

        // 再为当前帧发起一次异步读回，落到另一个槽。
        self.issue(next_slot);
        self.pending = Some(next_slot);
        out
    }

    /// 发起一次异步读回到 `slot`。
    fn issue(&self, slot: usize) {
        type BindBuf = unsafe extern "system" fn(u32, u32);
        type BindFbo = unsafe extern "system" fn(u32, u32);
        let (bb, bf) = (gl_proc("glBindBuffer"), gl_proc("glBindFramebuffer"));
        // `new` 已探过这两个入口。
        let bind: BindBuf = unsafe { std::mem::transmute(bb) };
        let bind_fbo: BindFbo = unsafe { std::mem::transmute(bf) };
        unsafe {
            bind(GL_PIXEL_PACK_BUFFER, self.slots[slot]);
            bind_fbo(GL_FRAMEBUFFER, self.fbo);
            // 读缓冲是 FBO 的状态，Skia 未必留在 `COLOR_ATTACHMENT0` 上。
            glReadBuffer(GL_COLOR_ATTACHMENT0);
            // 目标指针为 null 即「写进当前绑定的 PBO」，立即返回。
            glReadPixels(
                0,
                0,
                self.width as i32,
                self.height as i32,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                std::ptr::null_mut(),
            );
            bind_fbo(GL_FRAMEBUFFER, 0);
            bind(GL_PIXEL_PACK_BUFFER, 0);
        }
    }

    /// map 出 `slot` 的像素拷进 `Vec`。
    ///
    /// `GL_MAP_READ_BIT` 保证 map 期间那次传输已完成，不必另加栅栏。
    fn collect(&self, slot: usize, bytes: usize) -> Option<Vec<u8>> {
        type BindBuf = unsafe extern "system" fn(u32, u32);
        type MapBufferRange = unsafe extern "system" fn(u32, isize, isize, u32) -> *mut c_void;
        type UnmapBuffer = unsafe extern "system" fn(u32) -> u8;
        let (bb, mb, ub) = (
            gl_proc("glBindBuffer"),
            gl_proc("glMapBufferRange"),
            gl_proc("glUnmapBuffer"),
        );
        // `new` 已探过 `glBindBuffer` 与 `glMapBufferRange`。
        let bind: BindBuf = unsafe { std::mem::transmute(bb) };
        let map: MapBufferRange = unsafe { std::mem::transmute(mb) };
        let unmap: UnmapBuffer = unsafe { std::mem::transmute(ub) };
        unsafe {
            bind(GL_PIXEL_PACK_BUFFER, self.slots[slot]);
            let ptr = map(GL_PIXEL_PACK_BUFFER, 0, bytes as isize, GL_MAP_READ_BIT);
            let out = if ptr.is_null() {
                None
            } else {
                let slice = std::slice::from_raw_parts(ptr as *const u8, bytes);
                Some(slice.to_vec())
            };
            if !ptr.is_null() && !ub.is_null() {
                unmap(GL_PIXEL_PACK_BUFFER);
            }
            bind(GL_PIXEL_PACK_BUFFER, 0);
            out
        }
    }
}

impl Drop for ReadbackPixels {
    fn drop(&mut self) {
        if self.slots[0] == 0 {
            return;
        }
        type DelBuf = unsafe extern "system" fn(i32, *const u32);
        let p = gl_proc("glDeleteBuffers");
        if p.is_null() {
            return;
        }
        let del: DelBuf = unsafe { std::mem::transmute(p) };
        unsafe { del(2, self.slots.as_ptr()) };
    }
}
