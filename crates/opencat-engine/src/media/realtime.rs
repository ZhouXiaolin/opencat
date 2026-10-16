//! 实时视频解码线程，native 侧对齐 web 的 `video-decode-worker.ts`。
//!
//! web 的预览器把解码放在 worker 线程，并给每个素材保留一段以播放头为中心的
//! 已解码帧窗口。主线程取帧先查窗口；查不到就**等**解码线程送来（web 的
//! `waitForRealtimeFrame`），而不是自己也去解一遍。本模块是同一套语义的 native
//! 实现：一个后台线程持有解码器，持续把窗口补满；主线程通过
//! [`VideoRealtimeDecoder::prefetch`] 告诉它播放头在哪。
//!
//! 窗口按 `(素材, 请求时刻)` 索引——**键里不含解码尺寸**。尺寸只影响解码开销，
//! 不影响「这一刻的帧是哪一个」；卡片缩放动画让尺寸每帧都变，把它放进键里会让
//! 预取永远打空。窗口帧按自己解码时的尺寸交给调用方，绘制路径用解码缩放比映射
//! src，所以尺寸不一致不会画错。
//!
//! 解码按素材**分片**到多个线程。一个合成帧里可能同时可见多个视频（GlassIntro
//! 的切场最多 3 个），单线程串行时各素材的解码耗时相加：实测 3 个素材约 38ms，
//! 越过 33ms 的帧预算，窗口于是逐帧漏空，主线程跟着一起等。分片后耗时取最长的
//! 一个而不是相加。

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use opencat_core::media::VideoColorMatrix;

use super::video::{MediaContext, VideoPreviewQuality};
use crate::runtime::cache::CacheCaps;

/// 播放头**之前**保留的帧数（web: `REALTIME_CACHE_BEHIND_FRAMES`）。
const BEHIND_FRAMES: usize = 3;

/// 播放头**之后**的帧数上限（web: `REALTIME_CACHE_HIGH_WATER_FRAMES`）。
const AHEAD_FRAMES: usize = 24;

/// 播放头跳变超过这个秒数就丢弃旧帧、从新位置重建（web: `REALTIME_SEEK_RESET_MIN_US`）。
const SEEK_RESET_SECS: f64 = 1.5;

/// 同上，单位是微秒。
const SEEK_RESET_US: i64 = (SEEK_RESET_SECS * 1_000_000.0) as i64;

/// 主线程等一帧的上限（web: `REALTIME_PRIME_TIMEOUT_MS`）。等不到就自己解，
/// 所以超时只影响速度，不影响正确性。
const PRIME_TIMEOUT: Duration = Duration::from_millis(1200);

/// 单素材窗口的字节上限。
///
/// web 的帧是 GPU 上的 `VideoFrame`，24 帧不占多少内存。native 的帧是裸 RGBA，
/// 1080p 一帧 8MB，所以必须再封一个字节上限。超限只**停止泵**，不驱逐——驱逐
/// 最旧的帧会把离播放头最近的帧丢掉，正好丢反。
const WINDOW_BUDGET_BYTES: usize = 48 << 20;

/// 第一阶段为单个素材连解的上限。跳变时可能要连解几十帧才追到播放头；这个上限
/// 只防病态情况，正常由「追上了」提前退出。
const MAX_CATCH_UP_FRAMES: usize = 64;

/// 解码分片数上限。
///
/// 分片让不同素材的解码并行，代价是每个解码器内部已经按帧并行
/// （`set_threading(Frame, nproc)`），分片之间会抢核。实测 3 个素材并行相对串行
/// 只有约 1.4×，而不是 3×——所以分片数不必多，够覆盖同时可见的素材即可。
const MAX_DECODE_SHARDS: usize = 1;

/// 窗口命中时交给调用方的帧。
pub struct CachedVideoFrame {
    pub data: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
}

struct WindowFrame {
    /// **请求时刻**，不是解码出来的帧的真实 pts。主线程按同一个时刻查询，
    /// 所以命中是精确的。
    pts_us: i64,
    width: u32,
    height: u32,
    data: Arc<Vec<u8>>,
}

#[derive(Default)]
struct AssetWindow {
    /// 按 `pts_us` 升序。
    frames: VecDeque<WindowFrame>,
    bytes: usize,
    /// 播放头所在的素材时刻。
    target_us: i64,
    /// 泵的下一个解码时刻。
    next_us: i64,
}

impl AssetWindow {
    /// 窗口里有没有落在 `time_us` 半个帧间隔内的帧。
    fn covers(&self, time_us: i64, tolerance_us: i64) -> bool {
        self.frames
            .iter()
            .any(|f| (f.pts_us - time_us).abs() <= tolerance_us)
    }
}

struct WindowSet {
    assets: HashMap<PathBuf, AssetWindow>,
    /// 泵向前推进的步长 = 合成帧时长。
    frame_dur_us: i64,
    /// 命中容差，取半个帧间隔：足以吸收浮点漂移，又绝不会跨到相邻帧。
    tolerance_us: i64,
}

struct Shared {
    window: Mutex<WindowSet>,
    /// 窗口新增一帧时通知正在等帧的主线程。
    ready: Condvar,
}

enum Command {
    Prefetch {
        path: PathBuf,
        time_secs: f64,
        target_size: Option<(u32, u32)>,
        color_matrix: Option<VideoColorMatrix>,
    },
    /// 预热：打开素材、探测元数据、解一帧丢掉。
    Warmup { path: PathBuf },
}

/// 一个素材的最新提示。线程据此知道播放头在哪、该按多大解。
struct Hint {
    time_secs: f64,
    target_size: Option<(u32, u32)>,
}

/// 一个解码分片：一个线程 + 它自己那份每素材帧窗口。
///
/// `MediaContext` 持有 FFmpeg 上下文，不是 `Send`，所以每个分片只能在线程内构造
/// 自己的一份，不能共享。
struct Shard {
    shared: Arc<Shared>,
    tx: Sender<Command>,
}

/// 后台解码线程组 + 每素材的帧窗口。由 [`MediaContext::enable_realtime_decode`]
/// 挂到 [`MediaContext`] 上。
///
/// 素材按「先到先分」轮流落到各分片：同一帧里一起出现的素材通常是一起被声明的，
/// 轮转能保证它们落到不同分片。分片归属一旦定下就不再变，`wait_for` 才能按同一
/// 张表找回同一个分片。
pub struct VideoRealtimeDecoder {
    shards: Vec<Shard>,
    assignments: Mutex<HashMap<PathBuf, usize>>,
}

impl VideoRealtimeDecoder {
    /// 起解码线程组。`fps` 是合成帧率，决定泵向前推进的步长。
    pub fn spawn(fps: u32, quality: VideoPreviewQuality) -> Self {
        let count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .min(MAX_DECODE_SHARDS);
        Self {
            shards: (0..count)
                .map(|index| Shard::spawn(fps, quality, index))
                .collect(),
            assignments: Mutex::new(HashMap::new()),
        }
    }

    /// `path` 落在哪个分片。首次见到该素材时按轮转分配。
    fn shard_for(&self, path: &Path) -> Option<&Shard> {
        let mut assignments = self.assignments.lock().ok()?;
        let index = match assignments.get(path) {
            Some(&index) => index,
            None => {
                let index = assignments.len() % self.shards.len();
                assignments.insert(path.to_path_buf(), index);
                index
            }
        };
        self.shards.get(index)
    }

    /// 预热一个素材：打开、探测、解一帧丢掉。开播前调用，让第一次真正取帧时
    /// 解码器已经就位。
    pub fn warmup(&self, path: &Path) {
        if let Some(shard) = self.shard_for(path) {
            shard.warmup(path);
        }
    }

    /// 告诉解码线程播放头现在在哪。线程随后把该素材的窗口补到播放头前方。
    pub fn prefetch(
        &self,
        path: &Path,
        time_secs: f64,
        target_size: Option<(u32, u32)>,
        color_matrix: Option<VideoColorMatrix>,
    ) {
        if let Some(shard) = self.shard_for(path) {
            shard.prefetch(path, time_secs, target_size, color_matrix);
        }
    }

    /// 等解码线程把 `(path, time_secs)` 这一帧放进窗口。
    ///
    /// 主线程**不应该**在窗口未命中时自己解码：两个解码器读同一个文件会互相拖慢
    /// （实测预热期合成耗时从 30ms 涨到 400ms 以上）。等不到就返回 `None`，调用方
    /// 再走同步解码兜底。
    pub fn wait_for(&self, path: &Path, time_secs: f64) -> Option<CachedVideoFrame> {
        self.shard_for(path)?.wait_for(path, time_secs)
    }
}

impl Shard {
    /// 起一个解码线程。`index` 只用于线程名，便于在 profiler 里区分分片。
    fn spawn(fps: u32, quality: VideoPreviewQuality, index: usize) -> Self {
        let frame_dur_us = (1_000_000.0 / fps.max(1) as f64).round() as i64;
        let shared = Arc::new(Shared {
            window: Mutex::new(WindowSet {
                assets: HashMap::new(),
                frame_dur_us,
                // 容差取半个帧间隔：最近帧总在半个帧间隔内，所以容差绝不会把请求
                // 甩给相邻的那一帧。
                tolerance_us: frame_dur_us / 2,
            }),
            ready: Condvar::new(),
        });
        let (tx, rx) = std::sync::mpsc::channel::<Command>();

        let worker_shared = Arc::clone(&shared);
        // 线程持有 `rx`，发送端全部丢弃时 `recv` 返回 `Err`，循环退出。
        let _thread = std::thread::Builder::new()
            .name(format!("opencat-video-decode-{index}"))
            .spawn(move || {
                // `MediaContext` 里有 FFmpeg 上下文，不是 `Send`，只能在线程内构造。
                // 关掉它自己的帧 LRU：窗口就是这里的缓存，再叠一层只会多占内存。
                let mut ctx = MediaContext::with_cache_caps(CacheCaps {
                    video_frames: 0,
                    ..CacheCaps::default()
                });
                ctx.set_composition_fps(fps);
                ctx.set_video_preview_quality(quality);

                let mut hints: HashMap<PathBuf, Hint> = HashMap::new();
                let mut hint_order: Vec<PathBuf> = Vec::new();
                while let Ok(command) = rx.recv() {
                    apply_command(&mut ctx, &mut hints, &mut hint_order, command);
                    // 排队的提示全部收下，后到的覆盖先到的：主线程已经走到下一帧了，
                    // 过期的播放头位置没有意义。
                    while let Ok(command) = rx.try_recv() {
                        apply_command(&mut ctx, &mut hints, &mut hint_order, command);
                    }

                    // 第一阶段：把播放头所在的那一帧解出来。
                    //
                    // 顺序按提示先后——主线程发完提示就按同一顺序取帧，第一个取不到
                    // 整帧就卡住。这一步不能混在下面的轮转里：轮转每个素材一轮只解一
                    // 帧，播放头跳变要连解几十帧才追得上，而其它素材每轮都可能在 seek，
                    // 轮到主线程要的那一帧时早已超过等待上限。
                    for path in &hint_order {
                        let Some(hint) = hints.get(path) else {
                            continue;
                        };
                        for _ in 0..MAX_CATCH_UP_FRAMES {
                            if is_covered(&worker_shared, path, hint.time_secs) {
                                break;
                            }
                            if !pump_one(
                                &mut ctx,
                                &worker_shared,
                                path,
                                hint.time_secs,
                                hint.target_size,
                            ) {
                                break;
                            }
                        }
                    }

                    // 第二阶段：轮转把每个素材的窗口往前填满。
                    //
                    // 这里不能「处理完这一批提示就回去等命令」——主线程在等某一帧时
                    // 不会发新提示，而播放头跳帧留下的缺口要连解好几帧才补得上。等
                    // 命令就会让主线程白等到超时。
                    loop {
                        let mut worked = false;
                        for (path, hint) in &hints {
                            worked |= pump_one(
                                &mut ctx,
                                &worker_shared,
                                path,
                                hint.time_secs,
                                hint.target_size,
                            );
                        }
                        // 每轮都收一次新提示：补满所有窗口可能要解上百帧，期间播放头
                        // 早往前走了。只在轮与轮之间收，迟到不超过一轮。
                        while let Ok(command) = rx.try_recv() {
                            apply_command(&mut ctx, &mut hints, &mut hint_order, command);
                        }
                        if !worked {
                            break;
                        }
                    }
                }
            })
            .expect("failed to spawn video decode thread");

        Self { shared, tx }
    }

    /// 预热一个素材：打开、探测、解一帧丢掉。开播前调用，让第一次真正取帧时
    /// 解码器已经就位。
    fn warmup(&self, path: &Path) {
        let _ = self.tx.send(Command::Warmup {
            path: path.to_path_buf(),
        });
    }

    /// 告诉解码线程播放头现在在哪。线程随后把该素材的窗口补到播放头前方。
    fn prefetch(
        &self,
        path: &Path,
        time_secs: f64,
        target_size: Option<(u32, u32)>,
        color_matrix: Option<VideoColorMatrix>,
    ) {
        // 先建会话再投递：`wait_for` 用「有没有会话」区分「线程还没解到」和
        // 「线程根本不知道这个素材」。后者要立刻返回，让调用方自己解，不然第一次
        // 出现视频时主线程会白等一个超时。
        if let Ok(mut set) = self.shared.window.lock() {
            set.assets.entry(path.to_path_buf()).or_default();
        }
        // 线程已退出时忽略：主线程会走同步解码，不影响正确性。
        let _ = self.tx.send(Command::Prefetch {
            path: path.to_path_buf(),
            time_secs,
            target_size,
            color_matrix,
        });
    }

    /// 等解码线程把 `(path, time_secs)` 这一帧放进窗口。
    ///
    /// 主线程**不应该**在窗口未命中时自己解码：两个解码器读同一个文件会互相拖慢
    /// （实测预热期合成耗时从 30ms 涨到 400ms 以上）。等不到就返回 `None`，调用方
    /// 再走同步解码兜底。
    fn wait_for(&self, path: &Path, time_secs: f64) -> Option<CachedVideoFrame> {
        let time_us = (time_secs * 1_000_000.0).round() as i64;
        let deadline = Instant::now() + PRIME_TIMEOUT;
        let mut set = self.shared.window.lock().ok()?;
        // 没有会话：解码线程还不知道这个素材，等下去只会等到超时。
        set.assets.get(path)?;
        loop {
            if let Some(frame) = set.lookup(path, time_us) {
                return Some(frame);
            }
            let remaining = deadline.checked_duration_since(Instant::now())?;
            let (guard, _) = self.shared.ready.wait_timeout(set, remaining).ok()?;
            set = guard;
        }
    }

}

impl WindowSet {
    fn lookup(&self, path: &Path, time_us: i64) -> Option<CachedVideoFrame> {
        let asset = self.assets.get(path)?;
        let frame = asset
            .frames
            .iter()
            .find(|f| (f.pts_us - time_us).abs() <= self.tolerance_us)?;
        Some(CachedVideoFrame {
            data: Arc::clone(&frame.data),
            width: frame.width,
            height: frame.height,
        })
    }

    /// 丢掉播放头后方超出窗口的帧。
    fn trim(&mut self, path: &Path) {
        let Some(asset) = self.assets.get_mut(path) else {
            return;
        };
        let behind_limit = asset.target_us - BEHIND_FRAMES as i64 * self.frame_dur_us;
        while let Some(front) = asset.frames.front() {
            if front.pts_us >= behind_limit {
                break;
            }
            asset.bytes -= front.data.len();
            asset.frames.pop_front();
        }
    }
}

/// 收下一条提示。色彩矩阵是逐素材的，写进解码上下文；播放头位置进 `hints`。
fn apply_command(
    ctx: &mut MediaContext,
    hints: &mut HashMap<PathBuf, Hint>,
    hint_order: &mut Vec<PathBuf>,
    command: Command,
) {
    match command {
        Command::Warmup { path } => {
            // 第一次解一个 1080p 素材要 ~300ms，探测（`find_stream_info`）占大头。
            // 这一帧的结果丢弃——预热只要解码器就位。
            let _ = ctx.frame_rgba_at_time_sized(&path, 0.0, None);
        }
        Command::Prefetch {
            path,
            time_secs,
            target_size,
            color_matrix,
        } => {
            if let Some(matrix) = color_matrix {
                ctx.set_video_color_matrix(&path, matrix);
            }
            // 记下提示先后：同一帧里主线程也按这个顺序取帧。
            hint_order.retain(|p| p != &path);
            hint_order.push(path.clone());
            hints.insert(path, Hint { time_secs, target_size });
        }
    }
}

/// 窗口里有没有播放头所在的那一帧。
fn is_covered(shared: &Arc<Shared>, path: &Path, time_secs: f64) -> bool {
    let time_us = (time_secs * 1_000_000.0).round() as i64;
    let Ok(set) = shared.window.lock() else {
        return false;
    };
    set.assets
        .get(path)
        .is_some_and(|asset| asset.covers(time_us, set.tolerance_us))
}

/// 给 `path` 的窗口补一帧，返回是否真的解了。
///
/// 解码不持锁：一次解码可达数十毫秒，持锁会让主线程的查询一起排队。
fn pump_one(
    ctx: &mut MediaContext,
    shared: &Arc<Shared>,
    path: &Path,
    time_secs: f64,
    target_size: Option<(u32, u32)>,
) -> bool {
    let target_us = (time_secs * 1_000_000.0).round() as i64;

    let (at_us, frame_dur_us) = {
        let Ok(mut set) = shared.window.lock() else {
            return false;
        };
        let frame_dur_us = set.frame_dur_us;
        let tolerance_us = set.tolerance_us;

        let filled = {
            let asset = set.assets.entry(path.to_path_buf()).or_default();

            // 播放头跳变（快进/后退）超过阈值：旧帧全部作废，从新位置重建。
            if !asset.frames.is_empty() && (target_us - asset.target_us).abs() > SEEK_RESET_US {
                asset.frames.clear();
                asset.bytes = 0;
                asset.next_us = target_us;
            }
            asset.target_us = target_us;
            if asset.frames.is_empty() {
                asset.next_us = target_us;
            }

            let covered = asset.covers(target_us, tolerance_us);
            // 播放头在窗口之前，或在前方很远：直接跳过去。前方的小距离靠顺序解码
            // 追上——seek 比顺序解码贵得多，逐帧跳过时每次都 seek 会拖垮预览。
            if !covered && (asset.next_us > target_us || target_us - asset.next_us > SEEK_RESET_US) {
                asset.next_us = target_us;
            }
            // 已有的帧不重复解，只往前走。
            while asset.covers(asset.next_us, tolerance_us) {
                asset.next_us += frame_dur_us;
            }

            let ahead = asset.frames.iter().filter(|f| f.pts_us >= target_us).count();
            // 窗口满了只该停止「往前多解」，不该停止「追上播放头」：播放头那一帧
            // 还没解出来时停手，主线程就永远等不到它，只能等到超时。
            covered && (ahead >= AHEAD_FRAMES || asset.bytes >= WINDOW_BUDGET_BYTES)
        };
        set.trim(path);
        if filled {
            return false;
        }
        let at_us = set.assets.get(path).map(|a| a.next_us).unwrap_or(target_us);
        (at_us, frame_dur_us)
    };

    // 解码失败（损坏文件、解码器耗尽等）时保留窗口现状：主线程会走同步路径并把
    // 真正的错误报给调用方。
    let Ok((bitmap, _)) = ctx.frame_rgba_at_time_sized(path, at_us as f64 / 1_000_000.0, target_size)
    else {
        return false;
    };

    let Ok(mut set) = shared.window.lock() else {
        return false;
    };
    let Some(asset) = set.assets.get_mut(path) else {
        return false;
    };
    asset.bytes += bitmap.data.len();
    asset.frames.push_back(WindowFrame {
        pts_us: at_us,
        width: bitmap.width,
        height: bitmap.height,
        data: bitmap.data,
    });
    asset.next_us = at_us + frame_dur_us;
    set.trim(path);
    drop(set);
    shared.ready.notify_all();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(pts_us: i64, len: usize) -> WindowFrame {
        WindowFrame {
            pts_us,
            width: 16,
            height: 16,
            data: Arc::new(vec![0; len]),
        }
    }

    fn set_with(frames: Vec<WindowFrame>, target_us: i64) -> WindowSet {
        WindowSet {
            assets: HashMap::from([(
                PathBuf::from("a.mp4"),
                AssetWindow {
                    bytes: frames.iter().map(|f| f.data.len()).sum(),
                    frames: frames.into(),
                    target_us,
                    next_us: target_us,
                },
            )]),
            frame_dur_us: 33_333,
            tolerance_us: 16_666,
        }
    }

    #[test]
    fn lookup_hits_within_half_a_frame_and_misses_beyond() {
        let set = set_with(
            vec![frame(0, 8), frame(33_333, 8), frame(66_666, 8)],
            33_333,
        );

        // 精确命中。
        let hit = set.lookup(Path::new("a.mp4"), 33_333).expect("exact hit");
        assert_eq!(hit.width, 16);
        // 漂移在半个帧间隔内仍然命中。
        assert!(set.lookup(Path::new("a.mp4"), 40_000).is_some());
        // 半个帧间隔内总有一帧命中（最近帧），所以容差不会把请求甩给相邻帧；
        // 真正的 miss 只发生在窗口覆盖范围之外。
        assert!(set.lookup(Path::new("a.mp4"), 100_000).is_none());
        assert!(set.lookup(Path::new("b.mp4"), 33_333).is_none());
    }

    #[test]
    fn trim_drops_frames_behind_the_playhead_only() {
        let mut set = set_with(
            vec![
                frame(0, 8),
                frame(33_333, 8),
                frame(66_666, 8),
                frame(99_999, 8),
            ],
            133_332,
        );

        set.trim(Path::new("a.mp4"));

        let asset = set.assets.get(Path::new("a.mp4")).expect("asset kept");
        // 播放头前 3 帧：保留 33_333 起的三帧，丢掉 0。
        assert_eq!(asset.frames.len(), 3);
        assert_eq!(asset.frames.front().expect("front").pts_us, 33_333);
        assert_eq!(asset.bytes, 24);
    }

    #[test]
    fn covers_uses_the_same_half_frame_tolerance_as_lookup() {
        let set = set_with(vec![frame(33_333, 8)], 33_333);
        let asset = set.assets.get(Path::new("a.mp4")).expect("asset");
        assert!(asset.covers(33_333, set.tolerance_us));
        assert!(asset.covers(45_000, set.tolerance_us));
        assert!(!asset.covers(55_000, set.tolerance_us));
    }
}
