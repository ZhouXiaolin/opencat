//! 裸 GL 原语（Windows/WGL、Linux/EGL）：FBO 与 PBO 管理。
//!
//! GL 函数面统一走 [`glow`]（WebGL 同名 API 形状）——本 crate 不再手写
//! `extern`：`glow::Context` 由调用方在 current 上下文下构建一次
//! （engine 的 [`crate::gl_session`]，loader 复用 win32/EGL 的入口解析），
//! 此后所有原语都携带它。
//!
//! Skia 的 `wrap_backend_render_target` 需要一个已存在的 FBO id，PBO 双缓冲
//! 读回（[`ReadbackPixels`]）是 Skia 没有暴露的异步读回——这两件事 Skia 与
//! GPU 命令流执行器都不管，落在这里。
//!
//! 调用线程必须有 current 的 GL 上下文——调用方由 engine 的 GL 会话保证
//! （预览链不使用本模块：预览画默认帧缓冲，无需 FBO/PBO）。

use std::num::NonZeroU32;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use glow::HasContext;

use crate::NativeTexture;

/// glow 的句柄包装是 NonZero 的——0 永远不进 glow(调用方已在 NativeTexture
/// 层面把 0 当「无纹理」拦截)。
fn nz(v: u32) -> NonZeroU32 {
    NonZeroU32::new(v).expect("glow handle 0")
}

const GL_MAP_READ_BIT: u32 = 0x0001;

/// Skia 的 `gpu::gl::TextureInfo` 要填的目标与内部格式（GL 枚举）。
///
/// 这两个值不是本 crate 用的，是给调用方填 Skia 结构体用的——放在这里，
/// 调用方就不必自己写 GL 魔数。
pub const TEXTURE_2D: u32 = 0x0DE1;
pub const RGBA8: u32 = 0x8058;

/// 建一个以 `tex` 为颜色附件的 FBO，返回 FBO id。
///
/// FBO 不完整时返回 `None`。成功时 FBO 已解绑（绑回 0），不干扰调用方
/// 后续的 GL 状态。
///
/// 返回的 id 对调用方**不透明**：它只被塞进 Skia 的 `FramebufferInfo`。
/// 调用方不得对它做 GL 调用，也不得做算术。
pub fn create_framebuffer(gl: &glow::Context, tex: NativeTexture) -> Option<u32> {
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
        let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        if status != glow::FRAMEBUFFER_COMPLETE {
            gl.delete_framebuffer(fbo);
            return None;
        }
        Some(fbo.0.get())
    }
}

/// 删除 [`create_framebuffer`] 建的 FBO。
pub fn delete_framebuffer(gl: &glow::Context, fbo: u32) {
    if fbo == 0 {
        return;
    }
    unsafe {
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

/// 一块离屏合成目标：原生纹理 + FBO + 双缓冲读回器。
///
/// 建 [`Self::new`] 时要求 GL context current——FBO 与 PBO 都是 context
/// 资源。drop 时删 FBO；**纹理不归它管**（[`create_texture_rgba8`] 的
/// 调用方用 [`delete_texture`] 删）。
pub struct Target {
    gl: Arc<glow::Context>,
    tex: NativeTexture,
    fbo: u32,
    width: u32,
    height: u32,
    /// PBO 双缓冲读回。`None` 表示 GL 入口缺失，调用方退回同步读回。
    readback: Option<ReadbackPixels>,
}

impl Target {
    /// 用 `tex` 作颜色附件建目标。
    pub fn new(gl: Arc<glow::Context>, tex: NativeTexture, width: u32, height: u32) -> Result<Self> {
        let fbo = create_framebuffer(&gl, tex)
            .ok_or_else(|| anyhow!("创建 FBO 失败（GL 入口缺失或 FBO 不完整）"))?;
        let readback = ReadbackPixels::new(&gl, fbo, width, height);
        Ok(Self {
            gl,
            tex,
            fbo,
            width,
            height,
            readback,
        })
    }

    /// 目标纹理句柄。
    pub fn texture(&self) -> NativeTexture {
        self.tex
    }

    /// FBO id，交给 Skia 的 `FramebufferInfo`。对调用方**不透明**。
    pub fn fbo(&self) -> u32 {
        self.fbo
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// 是否有 PBO 双缓冲读回。`false` 时调用方走同步读回。
    pub fn has_readback(&self) -> bool {
        self.readback.is_some()
    }

    /// 等上一帧的读回完成。**每帧合成开始前调用**。
    ///
    /// GL 的 context 命令流是 FIFO：上一帧的 `glReadPixels` 天然排在渲染
    /// 之后，CPU 不需要显式等待。此方法只为与
    /// [`crate::metal::Target::wait_inflight`] 接口对齐——Metal 跨队列无
    /// 隐式顺序，那边必须真等。
    pub fn wait_inflight(&mut self) {}

    /// 取回**上一帧**的像素，并为当前帧发起一次读回。首帧返回 `None`。
    pub fn readback(&mut self) -> Option<Vec<u8>> {
        self.readback
            .as_mut()
            .and_then(|rb| rb.read_frame())
    }
}

impl Drop for Target {
    fn drop(&mut self) {
        if self.fbo != 0 {
            delete_framebuffer(&self.gl, self.fbo);
        }
    }
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
    gl: Arc<glow::Context>,
    fbo: u32,
    width: u32,
    height: u32,
    /// 两个 PBO 槽，构造时建好。
    slots: [Option<glow::NativeBuffer>; 2],
    /// 已发起、待取回的槽。`None` 表示还没有在途的读回。
    pending: Option<usize>,
}

impl ReadbackPixels {
    /// 建两个 PBO 槽。入口缺失时返回 `None`，调用方退回同步读回。
    ///
    /// 必须带 current 的 GL context 调用（PBO 是 context 资源）。
    pub fn new(gl: &Arc<glow::Context>, fbo: u32, width: u32, height: u32) -> Option<Self> {
        unsafe {
            let a = gl.create_buffer().ok()?;
            let b = gl.create_buffer().ok()?;
            let bytes = (width as usize * height as usize * 4) as i32;
            for slot in [&a, &b] {
                gl.bind_buffer(glow::PIXEL_PACK_BUFFER, Some(*slot));
                gl.buffer_data_size(glow::PIXEL_PACK_BUFFER, bytes, glow::STREAM_READ);
            }
            gl.bind_buffer(glow::PIXEL_PACK_BUFFER, None);
            Some(Self {
                gl: gl.clone(),
                fbo,
                width,
                height,
                slots: [Some(a), Some(b)],
                pending: None,
            })
        }
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
        unsafe {
            let gl = &self.gl;
            gl.bind_buffer(glow::PIXEL_PACK_BUFFER, self.slots[slot]);
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(glow::NativeFramebuffer(nz(self.fbo))));
            // 读缓冲是 FBO 的状态，Skia 未必留在 `COLOR_ATTACHMENT0` 上。
            gl.read_buffer(glow::COLOR_ATTACHMENT0);
            // 目标指针为 null 即「写进当前绑定的 PBO」，立即返回。
            gl.read_pixels(
                0,
                0,
                self.width as i32,
                self.height as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::BufferOffset(0),
            );
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
            gl.bind_buffer(glow::PIXEL_PACK_BUFFER, None);
        }
    }

    /// map 出 `slot` 的像素拷进 `Vec`。
    ///
    /// `GL_MAP_READ_BIT` 保证 map 期间那次传输已完成，不必另加栅栏。
    fn collect(&self, slot: usize, bytes: usize) -> Option<Vec<u8>> {
        unsafe {
            let gl = &self.gl;
            gl.bind_buffer(glow::PIXEL_PACK_BUFFER, self.slots[slot]);
            let ptr = gl.map_buffer_range(
                glow::PIXEL_PACK_BUFFER,
                0,
                bytes as i32,
                GL_MAP_READ_BIT,
            );
            let out = if ptr.is_null() {
                None
            } else {
                let slice = std::slice::from_raw_parts(ptr as *const u8, bytes);
                Some(slice.to_vec())
            };
            if !ptr.is_null() {
                gl.unmap_buffer(glow::PIXEL_PACK_BUFFER);
            }
            gl.bind_buffer(glow::PIXEL_PACK_BUFFER, None);
            out
        }
    }
}

impl Drop for ReadbackPixels {
    fn drop(&mut self) {
        unsafe {
            for slot in self.slots.iter_mut() {
                if let Some(buf) = slot.take() {
                    self.gl.delete_buffer(buf);
                }
            }
        }
    }
}
