//! 分带并行：liangyi 决定一个内核的逐像素工作怎么切成可并行的行带。
//!
//! 语言层（[`crate::lower_sksl`]）只在"每个输出像素只依赖自己的坐标"时
//! 才在 codegen 产物里声明可按行分带（[`crate::ExecutionPlan::Rows`]）。
//! 具体切几条带、每条多高、要不要并行，由本模块拥有；宿主只提供
//! "把一条带光栅到自己的缓冲"的闭包，不参与调度决策。
//!
//! 分带的像素语义：带是整数像素行，带边界两侧都被同一个 rect 完全覆盖，
//! 所以每条带独立光栅的结果与整块光栅逐像素相同（各带按 SrcOver 拼回）。

use rayon::prelude::*;

/// 目标矩形的一条水平带。`y0` 是相对 rect 顶部的像素行偏移。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Band {
    pub y0: u32,
    pub height: u32,
}

/// 每带最少行数。再小则每条带的固定开销（surface 分配 + 拼回）超过收益。
const MIN_ROWS_PER_BAND: u32 = 8;
/// 低于此像素总量直接整块执行：小 rect 的并行收益为负。
const MIN_PARALLEL_PIXELS: u64 = 1 << 16;

/// 分带方案。`None` 表示不值得并行，调用方走整块路径。
pub fn plan(width: u32, height: u32) -> Option<Vec<Band>> {
    let threads = rayon::current_num_threads();
    if threads < 2 || height < 2 * MIN_ROWS_PER_BAND {
        return None;
    }
    if u64::from(width) * u64::from(height) < MIN_PARALLEL_PIXELS {
        return None;
    }
    let bands = (height / MIN_ROWS_PER_BAND) as usize;
    let bands = bands.min(threads);
    if bands < 2 {
        return None;
    }
    let band_h = height.div_ceil(bands as u32);
    let mut out = Vec::with_capacity(bands);
    let mut y0 = 0;
    while y0 < height {
        let height = band_h.min(height - y0);
        out.push(Band { y0, height });
        y0 += height;
    }
    Some(out)
}

/// 按 [`plan`] 分带并行执行 `f`，返回按 y 升序的 `(带, 结果)`。
///
/// `None` 表示 [`plan`] 判定不该并行 —— 此时 `f` 一次都不会被调用，
/// 调用方走整块路径。
pub fn try_parallel<F, R>(width: u32, height: u32, f: F) -> Option<Vec<(Band, R)>>
where
    F: Fn(Band) -> R + Sync,
    R: Send,
{
    let bands = plan(width, height)?;
    Some(
        bands
            .into_par_iter()
            .map(|band| {
                let out = f(band);
                (band, out)
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_cover_the_rect_exactly_without_overlap() {
        let bands = plan(1920, 1080).expect("1920x1080 should be worth splitting");
        assert!(bands.len() >= 2, "{bands:?}");
        let mut next = 0;
        for band in &bands {
            assert_eq!(band.y0, next, "bands must be contiguous");
            assert!(band.height >= 1);
            next += band.height;
        }
        assert_eq!(next, 1080, "bands must cover the whole rect");
    }

    #[test]
    fn small_rects_stay_whole() {
        assert!(plan(40, 40).is_none(), "tiny cells must not pay parallel cost");
        assert!(plan(1920, 4).is_none(), "too few rows to split");
    }

    #[test]
    fn parallel_run_matches_serial_per_band() {
        let Some(parts) = try_parallel(1920, 1080, |band| band.y0 * 2 + band.height) else {
            return; // 单核机器：整块路径，无需断言
        };
        let mut next = 0;
        for (band, value) in parts {
            assert_eq!(band.y0, next);
            assert_eq!(value, band.y0 * 2 + band.height);
            next += band.height;
        }
        assert_eq!(next, 1080);
    }
}
