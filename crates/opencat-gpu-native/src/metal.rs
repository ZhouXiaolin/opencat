//! macOS 的原生 Metal 落点（GL 平台的 [`crate::gl`] 对应物）。
//!
//! Metal 没有 GL 式的「context」：一块系统 MTLDevice 上，wgpu、Skia 与本
//! 模块各持一条 MTLCommandQueue，纹理天然跨队列共享。代价是**没有隐式同步**
//! ——GL 靠 context 命令流（FIFO）免费拿到的跨队列顺序，Metal 必须显式
//! 排队：wgpu 侧在 3D 渲染出口 `poll(Wait)`，本模块的读回 blit 见
//! [`Target::wait_inflight`]。
//!
//! - **上下文**（[`DeviceContext`]）：系统默认 MTLDevice + 一条自用命令
//!   队列。Skia 的 `mtl::BackendContext` 从这里取裸指针——MTLDevice 是
//!   进程级单例，与 wgpu 持有的是同一块设备，纹理指针因此可以直接互通。
//! - **渲染目标**（[`Target`]）：离屏合成目标的双缓冲读回（GL 平台
//!   `gl::Target` 的 PBO 对应物）。
//!
//! 像素约定：Metal 的 drawable 只能 BGRA8，但**离屏纹理允许 RGBA8Unorm**
//! ——合成目标是 wgpu 建的 `Rgba8Unorm`（MTLPixelFormat::RGBA8Unorm），
//! blit 读回即引擎全管线的 straight RGBA8 字节序，无需交换通道。

use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Result, anyhow};
// `Device/CommandQueue::as_ptr`（ForeignType）与 `TextureRef::from_ptr`
// （ForeignTypeRef）来自 metal 再导出的 foreign-types。
use metal::foreign_types::{ForeignType, ForeignTypeRef};
use metal::{MTLBlitOption, MTLOrigin, MTLResourceOptions, MTLSize};

use crate::NativeTexture;

/// blit 行距按 256 对齐（Apple 平台的保守对齐；读回按行解包）。
const ROW_ALIGN: usize = 256;

/// 系统默认 MTLDevice + 一条自用命令队列。
///
/// 本队列只做一件事：把合成目标纹理 blit 到 staging buffer 供 CPU 读回，
/// 不与 wgpu/Skia 的队列抢工作。裸指针取用后由调用方保证本结构活得比
/// Skia 的 `DirectContext` 久（作为字段排在它后面即可）。
pub struct DeviceContext {
    device: metal::Device,
    queue: metal::CommandQueue,
}

impl DeviceContext {
    /// 取系统默认设备并自建队列。
    pub fn system_default() -> Option<Self> {
        let device = metal::Device::system_default()?;
        let queue = device.new_command_queue();
        Some(Self { device, queue })
    }

    /// MTLDevice 裸指针（Skia `mtl::BackendContext` 的 device 参）。
    pub fn device_ptr(&self) -> *mut c_void {
        self.device.as_ptr() as *mut c_void
    }

    /// MTLCommandQueue 裸指针（Skia `mtl::BackendContext` 的 queue 参）。
    pub fn queue_ptr(&self) -> *mut c_void {
        self.queue.as_ptr() as *mut c_void
    }
}

/// 离屏合成目标的双缓冲读回（GL 平台 `gl::Target` 的 PBO 对应物）。
///
/// 纹理由 wgpu 建成、所有权在共享纹理表（keepalive）；这里只凭裸指针引用
/// 它，建两块 shared staging buffer 做双缓冲 blit：
///
/// - 帧头 [`Self::wait_inflight`]：CPU 等上一帧 blit 完成。Metal 跨队列
///   无隐式顺序，不先等它，Skia 这一帧对同一纹理的重绘就会与在途 blit 的
///   读并发（GL 靠 context FIFO 天然免掉这一步）。
/// - 帧尾 [`Self::readback`]：取上一帧数据 + 发起本帧 blit。调用方保证
///   Skia 已 flush（本帧像素已写入纹理），blit 与下一帧的 CPU/GPU 工作
///   重叠——与 GL PBO 双缓冲同构：首帧返回 `None`，之后交出上一帧。
pub struct Target {
    texture: NativeTexture,
    width: u32,
    height: u32,
    bytes_per_row: usize,
    buffers: [metal::Buffer; 2],
    /// 在途 blit 的完成标志。completion handler 写 `true`，帧头轮询读。
    done: Arc<AtomicBool>,
    queue: metal::CommandQueue,
    /// 已发起 blit 的帧数（0 = 尚无在途数据可取）。
    frames_blitted: usize,
}

impl Target {
    /// 引用一张 `RGBA8Unorm` 的合成目标纹理，建双 staging buffer。
    ///
    /// `tex` 是 [`NativeTexture`] 携带的 MTLTexture 裸指针——存活由共享
    /// 纹理表保证（凭据在读完前不归还）。
    pub fn new(tex: NativeTexture, width: u32, height: u32) -> Result<Self> {
        if tex.is_none() || width == 0 || height == 0 {
            return Err(anyhow!(
                "metal::Target：纹理句柄为空或尺寸非法 {width}x{height}"
            ));
        }
        let bytes_per_row = (width as usize * 4).div_ceil(ROW_ALIGN) * ROW_ALIGN;
        let len = bytes_per_row * height as usize;
        let device = metal::Device::system_default().ok_or_else(|| anyhow!("no Metal device"))?;
        let opts = MTLResourceOptions::StorageModeShared;
        let buffers = [
            device.new_buffer(len as u64, opts),
            device.new_buffer(len as u64, opts),
        ];
        let queue = device.new_command_queue();
        Ok(Self {
            texture: tex,
            width,
            height,
            bytes_per_row,
            buffers,
            done: Arc::new(AtomicBool::new(false)),
            queue,
            frames_blitted: 0,
        })
    }

    /// 目标纹理句柄（Skia 包渲染目标时取裸指针）。
    pub fn texture(&self) -> NativeTexture {
        self.texture
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// 有无读回入口（对齐 [`crate::gl::Target::has_readback`] 的调用形状；
    /// Metal 的 blit 入口恒在）。
    pub fn has_readback(&self) -> bool {
        true
    }

    /// 等在途 blit 完成。**每帧合成开始前必须调用**：保证 Skia 对合成目标
    /// 纹理的重绘不与上一帧的读回 blit 并发（Metal 无 GL 式 context FIFO）。
    pub fn wait_inflight(&mut self) {
        if self.frames_blitted == 0 {
            return;
        }
        while !self.done.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
    }

    /// 取回**上一帧**的像素并发起本帧 blit（与 GL PBO 路径同语义）。
    ///
    /// 调用时序：本帧 Skia 已 flush 到纹理 → [`Self::wait_inflight`] 已在
    /// 帧头等过上一帧 → 这里拷出的必然是完整像素。首帧没有在途数据，返回
    /// `None`。
    pub fn readback(&mut self) -> Option<Vec<u8>> {
        let prev = if self.frames_blitted == 0 {
            None
        } else {
            let slot = (self.frames_blitted - 1) % 2;
            Some(self.copy_out(&self.buffers[slot]))
        };

        let slot = self.frames_blitted % 2;
        self.done.store(false, Ordering::Release);
        self.submit_blit(slot);
        self.frames_blitted += 1;
        prev
    }

    /// 把纹理 blit 进第 `slot` 块 staging buffer（异步提交，DMA 与渲染重叠）。
    fn submit_blit(&self, slot: usize) {
        let texture = unsafe { metal::TextureRef::from_ptr(self.texture.raw() as *mut _) };
        let buffer = &self.buffers[slot];
        let bytes_per_row = self.bytes_per_row as u64;
        let bytes_per_image = bytes_per_row * self.height as u64;

        let cb = self.queue.new_command_buffer();
        let blit = cb.new_blit_command_encoder();
        blit.copy_from_texture_to_buffer(
            texture,
            0,
            0,
            MTLOrigin { x: 0, y: 0, z: 0 },
            MTLSize {
                width: self.width as u64,
                height: self.height as u64,
                depth: 1,
            },
            buffer,
            0,
            bytes_per_row,
            bytes_per_image,
            MTLBlitOption::empty(),
        );
        blit.end_encoding();

        let done = self.done.clone();
        let block = block::ConcreteBlock::new(move |_| {
            done.store(true, Ordering::Release);
        })
        .copy();
        cb.add_completed_handler(&block);
        cb.commit();
    }

    /// 从 shared staging buffer 解包成紧凑 RGBA8（去行对齐填充）。
    fn copy_out(&self, buffer: &metal::Buffer) -> Vec<u8> {
        let row_len = self.width as usize * 4;
        let mut out = Vec::with_capacity(row_len * self.height as usize);
        let base = buffer.contents() as *const u8;
        for row in 0..self.height as usize {
            let src = unsafe { std::slice::from_raw_parts(base.add(row * self.bytes_per_row), row_len) };
            out.extend_from_slice(src);
        }
        out
    }
}
