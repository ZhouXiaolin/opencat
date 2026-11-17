//! 渲染会话的系统内存门禁。
//!
//! 浏览器侧 drawFrame 三门禁（时间 / JS heap / core wasm 线性内存）测不到
//! 崩溃层：chrome 被内核 OOM 击杀时的压力来自 GPU/共享内存映射（journalctl
//! 实证 total-vm 1.5TB 而 anon-rss 只有 1.2GB），JS heap 与 wasm 线性内存
//! 都不含它。且 chrome 自带 oom_score_adj=300，内核 OOM 时永远先杀它——
//! 表现为渲染中途连接死亡，没有任何带诊断的错误路径。
//!
//! 本模块在驱动侧补一层：渲染请求期间并发轮询 /proc，系统可用内存跌破
//! 下限、或本会话 chrome 进程树 RSS 超上限时，SIGKILL 掉整棵树并让渲染
//! 请求 fail-loud 带出门禁原因。阈值可用环境变量覆盖：
//! `OC_GATE_MIN_AVAILABLE_MB`（默认 2048）、`OC_GATE_SESSION_RSS_CAP_MB`
//! （默认 12288；chrome 多进程把共享页重复计入，上限要放得宽）。

use std::collections::BTreeSet;
use std::fs;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Result, anyhow, bail};

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const MIN_AVAILABLE_MB_DEFAULT: u64 = 2048;
const SESSION_RSS_CAP_MB_DEFAULT: u64 = 12 * 1024;

fn gate_min_available_mb() -> u64 {
    std::env::var("OC_GATE_MIN_AVAILABLE_MB")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(MIN_AVAILABLE_MB_DEFAULT)
}

fn gate_session_rss_cap_mb() -> u64 {
    std::env::var("OC_GATE_SESSION_RSS_CAP_MB")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(SESSION_RSS_CAP_MB_DEFAULT)
}

/// 扫描 /proc 里当前所有 chrome 进程（浏览器树含 GPU/renderer 子进程，
/// comm 一律是 `chrome`；chromedriver 不算——它是我们的管理进程）。
fn chrome_pids_now() -> BTreeSet<i32> {
    let mut pids = BTreeSet::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return pids;
    };
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let Ok(pid) = name.parse::<i32>() else {
            continue;
        };
        let Ok(comm) = fs::read_to_string(entry.path().join("comm")) else {
            continue;
        };
        if comm.trim().starts_with("chrome") {
            pids.insert(pid);
        }
    }
    pids
}

/// session 创建前的系统 chrome 快照。差集才能在共享 chromedriver（外部
/// CHROMEDRIVER_URL 模式）下只管自己拉起的浏览器树。
pub fn snapshot_chrome_pids() -> BTreeSet<i32> {
    chrome_pids_now()
}

/// session 创建后的归属判定：差集（本会话新拉起的 chrome）；快照间无差异
/// 时兜底用全量（极端时序下 chromedriver 复用旧浏览器进程，仍应受管）。
pub fn session_chrome_pids(before: &BTreeSet<i32>) -> BTreeSet<i32> {
    let after = chrome_pids_now();
    let diff: BTreeSet<i32> = after.difference(before).copied().collect();
    if diff.is_empty() { after } else { diff }
}

fn mem_available_mb() -> Option<u64> {
    let text = fs::read_to_string("/proc/meminfo").ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("MemAvailable:") {
            let kb = rest.trim().trim_end_matches(" kB").trim().parse::<u64>().ok()?;
            return Some(kb / 1024);
        }
    }
    None
}

fn session_rss_mb(pids: &BTreeSet<i32>) -> u64 {
    // statm 第二字段是 resident 页数。共享映射在多进程间重复计入——上限
    // 因此放宽（默认 12GB），门禁语义是「失控增长」而非精确记账。
    let page_bytes = 4096u64;
    let mut total = 0u64;
    for pid in pids {
        if let Ok(statm) = fs::read_to_string(format!("/proc/{pid}/statm")) {
            if let Some(resident) = statm.split_whitespace().nth(1) {
                if let Ok(pages) = resident.parse::<u64>() {
                    total += pages * page_bytes;
                }
            }
        }
    }
    total / (1024 * 1024)
}

pub fn kill_tree(pids: &BTreeSet<i32>) {
    let alive: Vec<String> = pids
        .iter()
        .filter(|p| fs::metadata(format!("/proc/{p}")).is_ok())
        .map(|p| p.to_string())
        .collect();
    if alive.is_empty() {
        return;
    }
    let _ = Command::new("kill").arg("-9").args(&alive).status();
}

/// 门禁轮询循环：每 2s 检查一次，越限即 SIGKILL 本会话 chrome 树，然后
/// 把原因写进 `abort_reason`（字节 0 表示尚未触发）并返回——`select!`
/// 侧据此终止渲染请求。
///
/// 返回 `Some(reason)` 表示门禁触发（chrome 树已被杀死）。
async fn watch(
    pids: BTreeSet<i32>,
    abort_reason: Arc<AtomicU64>,
    // 门禁触发码：1 = 系统可用内存低，2 = 会话 RSS 超限
) -> &'static str {
    let min_avail = gate_min_available_mb();
    let rss_cap = gate_session_rss_cap_mb();
    loop {
        tokio::time::sleep(POLL_INTERVAL).await;
        if let Some(avail) = mem_available_mb() {
            if avail < min_avail {
                eprintln!(
                    "[mem-gate] MemAvailable {avail}MB < {min_avail}MB — killing session chrome tree {:?}",
                    pids
                );
                kill_tree(&pids);
                abort_reason.store(1, Ordering::SeqCst);
                return "system available memory below floor";
            }
        }
        let rss = session_rss_mb(&pids);
        if rss > rss_cap {
            eprintln!(
                "[mem-gate] session chrome RSS {rss}MB > {rss_cap}MB — killing {:?}",
                pids
            );
            kill_tree(&pids);
            abort_reason.store(2, Ordering::SeqCst);
            return "session chrome tree RSS over cap";
        }
    }
}

/// 把渲染请求包进门禁：并发跑 `fut` 与 /proc 轮询，任一边先结束即返回。
/// 门禁触发时 chrome 树已被 SIGKILL，请求会立刻失败——报出门禁原因而非
/// 模糊的「browser died」。请求自身失败且门禁未触发时，大概率是内核抢先
/// OOM（轮询间隔内发生），错误信息带上判据提示。
pub async fn with_memory_gate<T>(
    pids: &BTreeSet<i32>,
    fut: impl std::future::Future<Output = Result<T>>,
) -> Result<T> {
    let abort_reason = Arc::new(AtomicU64::new(0));
    let watcher = watch(pids.clone(), abort_reason.clone());
    let out = tokio::select! {
        biased;
        reason = watcher => {
            bail!("memory gate tripped: {reason} (chrome tree killed; tune OC_GATE_MIN_AVAILABLE_MB / OC_GATE_SESSION_RSS_CAP_MB)")
        }
        res = fut => res,
    };
    match out {
        Ok(v) => Ok(v),
        Err(err) => {
            if abort_reason.load(Ordering::SeqCst) != 0 {
                bail!("memory gate tripped during failed render: {err:#}");
            }
            Err(anyhow!(
                "{err:#}\nnote: if journalctl shows an oom-kill on chrome, the kernel\n\
                 preempted the 2s polling window — lower OC_GATE_MIN_AVAILABLE_MB\n\
                 or free system memory before rendering"
            ))
        }
    }
}
