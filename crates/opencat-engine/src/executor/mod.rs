pub mod paint;
pub mod path;
pub mod path_tiny;
mod replay;

use opencat_core::ir::draw_frame::DrawOpFrame;
use opencat_core::ir::draw_types::ImageRef;
use skia_safe::{Canvas, Image, Paint, PathBuilder, RuntimeEffect};
#[cfg(target_os = "linux")]
use skia_safe::skottie::Animation;
use std::collections::HashMap;
use std::path::Path;

/// Cached Lottie animation handle. A `()` placeholder keeps the cache field
/// (and the replay match arm that reads it) compiling when Skottie is off.
/// Keep the `target_os = "linux"` condition in sync with the `skottie`
/// feature on `skia-safe` in `Cargo.toml`.
#[cfg(target_os = "linux")]
pub(crate) type LottieAnimation = Animation;
#[cfg(not(target_os = "linux"))]
pub(crate) type LottieAnimation = ();

/// Statistics returned after a frame execution.
#[derive(Debug, Default)]
pub struct DrawStats {
    pub op_count: u32,
    pub cache_hits: u32,
}

/// Error type for draw execution failures.
#[derive(Debug)]
pub struct DrawError(pub String);

impl std::fmt::Display for DrawError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DrawError: {}", self.0)
    }
}

impl std::error::Error for DrawError {}

/// Engine-side draw executor. Owns the current canvas state
/// and replays DrawOpFrame onto a skia_safe::Canvas.
pub struct EngineDrawExecutor {
    pub(crate) current_path: Option<PathBuilder>,
    pub(crate) current_fill_paint: Paint,
    pub(crate) current_stroke_paint: Paint,
    pub(crate) current_alpha: f32,
    pub(crate) compiled_pictures: HashMap<u64, skia_safe::Picture>,
    /// Native Lottie animations, keyed by bundle id. Stays empty when the
    /// `skottie` feature is off (e.g. macOS and Windows) — see [`Self::ensure_lottie_animations`].
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub(crate) lottie_cache: HashMap<String, LottieAnimation>,
    /// CSS `backdrop-filter` 两遍法：每个 `SaveBackdropLayer` 之前的 op 重放
    /// 录成的 picture（按出现顺序）。这版 Skia 的 `SaveLayerRec::backdrop`
    /// 嵌套在祖先层内会静默失效，改为层内 `draw_picture` + filter 打底。
    /// 仅在 `replay_frame` 内非空。
    pub(crate) backdrop_pictures: Vec<skia_safe::Picture>,
    /// 下一个待消费的 `backdrop_pictures` 槽位。
    pub(crate) backdrop_next: usize,
    /// 预录前缀期间为真：此时 `SaveBackdropLayer` 走原始 Skia 路径且不消费槽位。
    pub(crate) backdrop_building: bool,
}

impl EngineDrawExecutor {
    pub fn new() -> Self {
        Self {
            current_path: None,
            current_fill_paint: Paint::default(),
            current_stroke_paint: Paint::default(),
            current_alpha: 1.0,
            compiled_pictures: HashMap::new(),
            lottie_cache: HashMap::new(),
            backdrop_pictures: Vec::new(),
            backdrop_next: 0,
            backdrop_building: false,
        }
    }

    pub fn begin_frame(&mut self) {
        self.current_path = None;
        self.current_fill_paint = Paint::default();
        let mut sp = Paint::default();
        sp.set_style(skia_safe::paint::Style::Stroke);
        self.current_stroke_paint = sp;
        self.current_alpha = 1.0;
    }
}

impl Default for EngineDrawExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Prepared frame media for executor consumption.
#[derive(Default)]
pub struct EnginePreparedFrameMedia {
    pub images: Vec<Image>,
    pub image_index: HashMap<ImageRef, usize>,
    pub runtime_effects: Vec<PreparedRuntimeEffect>,
    /// 与 `images` 平行的解码缩放比（解码宽 / 声明原图宽）。预缩小解码时
    /// 重放按此把 `DrawOp::ImageRect` 的声明坐标系 src 映射到解码像素，并
    /// 补偿 `DrawOp::Image` 的自然尺寸语义；缺省 1.0（native/lazy，无缩放）。
    pub image_scales: Vec<f32>,
}

/// 一个已编译的 runtime effect，附带 liangyi 后端声明的执行计划。
///
/// `plan` 是后端的判断（内核输出是否只依赖像素自身坐标），不是内核作者的
/// 注解：`Rows` 表示可按整数像素行分带并行光栅且结果逐位不变。分带时每个
/// worker 线程需各自编译（`skia_safe::RuntimeEffect` 非 `Send`），因此保留
/// `hash`（线程本地编译 memo 的键）与 `sksl`。
pub struct PreparedRuntimeEffect {
    pub effect: RuntimeEffect,
    pub plan: opencat_liangyi::ExecutionPlan,
    pub hash: u64,
    pub sksl: String,
}

impl From<RuntimeEffect> for PreparedRuntimeEffect {
    /// 整块执行（无分带计划）——测试与不使用 liangyi codegen 的路径用。
    fn from(effect: RuntimeEffect) -> Self {
        Self {
            effect,
            plan: opencat_liangyi::ExecutionPlan::Whole,
            hash: 0,
            sksl: String::new(),
        }
    }
}

impl EngineDrawExecutor {
    pub fn execute(
        &mut self,
        draw: &DrawOpFrame,
        media: &EnginePreparedFrameMedia,
        target: &mut Canvas,
    ) -> Result<DrawStats, DrawError> {
        self.begin_frame();
        replay::replay_frame(self, target, draw, media)
    }

    #[cfg(target_os = "linux")]
    pub fn ensure_lottie_animations<P: Fn(&str) -> Option<Vec<u8>>>(
        &mut self,
        draw: &DrawOpFrame,
        resolve_bytes: P,
    ) {
        use opencat_core::ir::draw_op::DrawOp;
        let bundle_ids: Vec<String> = draw
            .ops
            .iter()
            .chain(draw.subtrees.iter().flat_map(|s| s.iter()))
            .filter_map(|op| match op {
                DrawOp::LottieRect { bundle_id, .. } => Some(bundle_id.clone()),
                _ => None,
            })
            .filter(|id| !self.lottie_cache.contains_key(id))
            .collect();
        for bundle_id in bundle_ids {
            if let Some(bytes) = resolve_bytes(&bundle_id) {
                if let Ok(json) = std::str::from_utf8(&bytes) {
                    if let Some(anim) = Animation::from_str(json) {
                        self.lottie_cache.insert(bundle_id, anim);
                    }
                }
            }
        }
    }

    /// Without Skottie there is no native Lottie renderer: leave the cache
    /// empty so [`crate::executor::replay`] skips `LottieRect` ops instead of
    /// pulling `skottie` into the binary.
    #[cfg(not(target_os = "linux"))]
    pub fn ensure_lottie_animations<P: Fn(&str) -> Option<Vec<u8>>>(
        &mut self,
        _draw: &DrawOpFrame,
        _resolve_bytes: P,
    ) {
    }

    pub fn evict_range(&mut self, fingerprint: u64) {
        self.compiled_pictures.remove(&fingerprint);
    }
}
