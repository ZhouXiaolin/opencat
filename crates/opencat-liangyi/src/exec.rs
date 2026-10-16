//! 后端无关的内核执行入口。
//!
//! 编译产物（[`crate::CompiledEffect`]）决定执行形态：
//!
//! - **CPU 后端**（[`crate::Backend::Cpu`]）：本模块把 f64 解释器接到宿主
//!   的任意 [`RenderTarget`] 上 —— `run_pixel_into`（逐像素重绘，rayon
//!   按行并行）/ `run_scan_into`（就地顺序扫描，单线程）。宿主只提供
//!   可写 straight RGBA8 缓冲（Skia surface / CpuBuffer / wgpu mapped）。
//! - **SKSL 后端**：执行发生在绘制期，由宿主着色器管线拥有；liangyi 交付
//!   codegen 源码（`CompiledEffect::sksl`）与 uniform 声明序。
//!
//! JS 侧对应 `ly.pixel` / `ly.scan`，以及 `ly.surface(w,h).apply / .sweep`。

use super::interp::{self, ScanDirection};
use super::pixel::RenderTarget;
use super::program::Val;
use super::{Backend, Program};

/// 逐像素内核写入目标（CPU 后端）。
pub fn run_pixel_into(program: &Program, target: &mut dyn RenderTarget, ctx: &interp::InterpCtx) {
    debug_assert_eq!(program.backend(), Backend::Cpu);
    interp::render_into(
        program,
        target.width(),
        target.height(),
        ctx,
        target.straight_rgba_mut(),
    );
}

/// 顺序扫描内核就地写入目标（CPU 后端）。
pub fn run_scan_into(
    program: &Program,
    target: &mut dyn RenderTarget,
    uniforms: &[Val],
    direction: ScanDirection,
) {
    debug_assert_eq!(program.backend(), Backend::Cpu);
    interp::render_scan(
        program,
        target.width(),
        target.height(),
        uniforms,
        direction,
        target.straight_rgba_mut(),
    );
}
