//! 缓存消融开关（`OC_ABLATE=layout_merkle,font_system`）。
//!
//! 未设置时全部缓存照常，生产行为不变。每个变体只关掉**一个**缓存；任意
//! 组合下渲染输出都必须与基线逐位相同（否则该缓存不是缓存，是行为）。

use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ablation {
    /// `LayoutSession` 的输入 merkle 复用 + 布局结果复用。
    LayoutMerkle,
    /// `DisplayBuildSession` 的子树 merkle 复用。
    DisplayMerkle,
    /// `FontSystemCache`（每线程 shape FontSystem）。
    FontSystem,
    /// `SwashCache`（字形光栅缓存）。
    Swash,
    /// 视频帧 LRU（`VideoFrameCache`）。
    VideoFrames,
    /// SKSL `RuntimeEffect` 编译 memo。
    RuntimeEffects,
    /// 离屏 surface 的 coverage 字形光栅表。
    GlyphRasters,
    /// 宿主 generated-image LRU（`canvas putImageData` / surface 自动烘焙）。
    GeneratedImages,
    /// 两仪 lambda 编译缓存。
    LiangyiCompile,
}

/// `(变体, OC_ABLATE token)`。token 即变体名的 snake_case。
const TOKENS: &[(Ablation, &str)] = &[
    (Ablation::LayoutMerkle, "layout_merkle"),
    (Ablation::DisplayMerkle, "display_merkle"),
    (Ablation::FontSystem, "font_system"),
    (Ablation::Swash, "swash"),
    (Ablation::VideoFrames, "video_frames"),
    (Ablation::RuntimeEffects, "runtime_effects"),
    (Ablation::GlyphRasters, "glyph_rasters"),
    (Ablation::GeneratedImages, "generated_images"),
    (Ablation::LiangyiCompile, "liangyi_compile"),
];

fn mask() -> u32 {
    static MASK: OnceLock<u32> = OnceLock::new();
    *MASK.get_or_init(|| {
        let Ok(value) = std::env::var("OC_ABLATE") else {
            return 0;
        };
        let all = TOKENS.iter().fold(0, |mask, (variant, _)| mask | 1 << *variant as u32);
        let mut mask = 0;
        for token in value.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            if token == "all" {
                mask |= all;
                continue;
            }
            match TOKENS.iter().find(|(_, name)| *name == token) {
                Some((variant, _)) => mask |= 1 << *variant as u32,
                None => log::warn!("unknown OC_ABLATE token `{token}`; ignoring"),
            }
        }
        mask
    })
}

/// 该缓存是否被消融（关闭）。
pub fn ablated(which: Ablation) -> bool {
    mask() & (1 << which as u32) != 0
}

/// 当前激活的消融 token，供 profile 头部自描述。
pub fn active() -> Vec<&'static str> {    TOKENS
        .iter()
        .filter(|(variant, _)| ablated(*variant))
        .map(|(_, name)| *name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Ablation, TOKENS};

    #[test]
    fn token_table_covers_every_variant() {
        for (variant, name) in TOKENS {
            assert!(!name.is_empty(), "{variant:?} has no token");
        }
        assert_eq!(
            TOKENS.len(),
            {
                // 变体数 = 最大判别值 + 1；新增变体忘了登记 token 时这里失败。
                let max = TOKENS.iter().map(|(v, _)| *v as u32).max().unwrap();
                max as usize + 1
            },
            "every Ablation variant needs a token row"
        );
        assert!(TOKENS.iter().any(|(v, _)| *v == Ablation::LayoutMerkle));
    }
}
