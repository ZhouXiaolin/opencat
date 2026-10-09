//! 后端无关的像素数据 / 渲染目标抽象。
//!
//! Liangyi 的语言层（parse → IR → 派发）不认识任何具体后端；执行落到
//! 哪里由宿主提供 [`RenderTarget`] 实例：
//!
//! - CPU 后端（[`interp`]）：任何可写 straight RGBA8 缓冲都是目标 ——
//!   opencat-core 的 render target（Skia surface 的可写缓冲）经
//!   [`BufferViewMut`] 接入，自有的 [`CpuBuffer`] 供离线/测试路径；
//! - SKSL 后端：绘制期目标由宿主（CanvasKit `RuntimeEffect` / Skia
//!   surface / 将来的 wgpu）拥有，liangyi 只交付 codegen 产物（见
//!   [`crate::lower_sksl`] 与 [`crate::CompiledEffect`]）。
//!
//! 约定：一律 straight（non-premultiplied）alpha、行主序、像素 `i` 中心 = i
//! —— 与 [`interp`] 的采样/输出语义逐位一致，实例化时不得改变字节布局。

use std::sync::Arc;

/// 只读像素源（straight RGBA8）。
pub trait PixelBuffer {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn straight_rgba(&self) -> &[u8];
}

/// 可写渲染目标（straight RGBA8）。内核逐像素/顺序扫描的落点。
pub trait RenderTarget {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn straight_rgba_mut(&mut self) -> &mut [u8];
}

/// 宿主拥有的 CPU 像素缓冲实例（自含、可移动）。
#[derive(Debug, Clone)]
pub struct CpuBuffer {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl CpuBuffer {
    /// 新建零初始化缓冲。
    pub fn new(width: u32, height: u32) -> Self {
        let rgba = vec![0u8; width as usize * height as usize * 4];
        Self { width, height, rgba }
    }

    /// 从既有 straight RGBA8 字节构造（校验长度）。
    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, String> {
        let want = width as usize * height as usize * 4;
        if rgba.len() != want {
            return Err(format!(
                "CpuBuffer: rgba len {} != {}x{}x4",
                rgba.len(),
                width,
                height
            ));
        }
        Ok(Self { width, height, rgba })
    }

    pub fn into_rgba(self) -> Vec<u8> {
        self.rgba
    }
}

impl PixelBuffer for CpuBuffer {
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
    fn straight_rgba(&self) -> &[u8] {
        &self.rgba
    }
}

impl RenderTarget for CpuBuffer {
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
    fn straight_rgba_mut(&mut self) -> &mut [u8] {
        &mut self.rgba
    }
}

/// 借用宿主缓冲的可写目标视图 —— 「实例化到具体后端」的接缝：
/// Skia surface 的可写像素缓冲、wgpu 的 mapped buffer 等一切
/// `&mut [u8]` 布局即可接入，liangyi 无需知道缓冲归属。
pub struct BufferViewMut<'a> {
    width: u32,
    height: u32,
    rgba: &'a mut [u8],
}

impl<'a> BufferViewMut<'a> {
    pub fn new(width: u32, height: u32, rgba: &'a mut [u8]) -> Result<Self, String> {
        let want = width as usize * height as usize * 4;
        if rgba.len() != want {
            return Err(format!(
                "BufferViewMut: rgba len {} != {}x{}x4",
                rgba.len(),
                width,
                height
            ));
        }
        Ok(Self { width, height, rgba })
    }
}

impl PixelBuffer for BufferViewMut<'_> {
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
    fn straight_rgba(&self) -> &[u8] {
        self.rgba
    }
}

impl RenderTarget for BufferViewMut<'_> {
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
    fn straight_rgba_mut(&mut self) -> &mut [u8] {
        self.rgba
    }
}

/// child 像素源即 [`PixelBuffer`]：宿主传给解释器的采样输入与目标
/// 共用同一字节约定。
impl PixelBuffer for super::interp::ChildImage {
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
    fn straight_rgba(&self) -> &[u8] {
        &self.rgba
    }
}

/// 从任意 [`PixelBuffer`] 复制出 [`interp::ChildImage`]（Arc 共享，零拷贝克隆句柄）。
pub fn child_from_buffer<B: PixelBuffer>(buf: &B) -> super::interp::ChildImage {
    super::interp::ChildImage {
        width: buf.width(),
        height: buf.height(),
        rgba: Arc::from(buf.straight_rgba()),
    }
}
