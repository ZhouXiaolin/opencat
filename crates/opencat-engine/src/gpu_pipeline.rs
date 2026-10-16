//! GPU 合成管线的开关与共享 GL context 支撑。
//!
//! 默认开启。设 `OC_GPU_PIPELINE=0`（或 `false` / `off`）退回 CPU 光栅路径。
//! 开启后：
//!
//! - 3D（`ctx.three`）渲染到显存，渲染目标的 GL texture id 经
//!   [`opencat_core::ir::generated_image::GpuImageHandle`] 交给 Skia 零拷贝
//!   导入（省掉每帧 `wgpu → CPU → Skia` 的整帧往返）；
//! - Skia 合成到自己的 GL render target，每帧只读回一次交给编码器。
//!
//! ## 线程与锁约束
//!
//! wgpu 的 GL 后端（Windows = WGL）把 context 绑在自己的线程上，并用一把
//! 互斥锁串行化访问。Skia 共享同一 `HGLRC`，因此二者**必须严格串行**：
//! Skia 操作期间持锁，期间不得调用 wgpu。锁有 1 秒超时（超时 panic），
//! 所以绝不允许交错或嵌套。
//!
//! 离线渲染天然逐帧串行（`3D → Skia 合成 → 编码`），符合该约束。

/// GPU 管线是否启用（用户开关）。
///
/// 默认开启。只有显式设 `OC_GPU_PIPELINE=0` / `false` / `off` 才走 CPU——
/// `1` / `true`（旧写法）与任何其他值都保持开启。
pub fn requested() -> bool {
    !matches!(
        std::env::var("OC_GPU_PIPELINE").as_deref(),
        Ok("0") | Ok("false") | Ok("off")
    )
}

/// 实际生效的 GPU 管线开关。
///
/// 纹理共享的合成器目前有 Windows（WGL）与 Linux（EGL）两个 GL 后端实现
/// （见 [`crate::gpu_compositor`]）。其他平台即使开着也走 CPU——
/// 否则 3D 会产出 GPU 句柄，而 CPU 合成器无法消费。
pub fn enabled() -> bool {
    cfg!(any(target_os = "windows", target_os = "linux")) && requested()
}
