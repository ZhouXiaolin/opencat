//! isolate 团队调度（引擎无关）：常驻线程池，线程↔引擎实例一一绑定。
//!
//! schedule → 派发映射（TR15 §19.6.3 语义；产出按行序槽位合并，调度只
//! 影响负载分布不影响字节）：static = 连续区间预分（禁窃取，均匀行负载
//! 最优）；dynamic(chunk) = chunk 行粒度原子拉取；guided/auto/runtime =
//! 逐行自适应。行失败 = 整次 execute 失败（调用方毒化降级，产出不落
//! 地——无半应用状态）。

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use flume::Sender;

use super::protocol::{Bundle, FrameEnv, JsWorker, RowOut, SharedBuf};

type Job = Box<dyn FnOnce(&mut dyn JsWorker) + Send + 'static>;

/// 常驻团队（进程级单例；worker 由调用方注入的工厂构造——team 不认识
/// 具体引擎）。
pub struct Team {
    tx: Mutex<Sender<Job>>,
}

impl Team {
    pub fn new(n: usize, factory: impl Fn() -> Box<dyn JsWorker> + Send + Sync + 'static) -> Self {
        let (tx, rx) = flume::unbounded::<Job>();
        let rx = Arc::new(rx);
        let factory = Arc::new(factory);
        for _ in 0..n.max(1) {
            let rx = rx.clone();
            let factory = factory.clone();
            std::thread::Builder::new()
                .name("oc-omp-worker".into())
                .spawn(move || {
                    let mut worker = factory();
                    while let Ok(job) = rx.recv() {
                        // job 内部自捕获应答通道；panic 视为该行失败。
                        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                            || job(worker.as_mut()),
                        ));
                    }
                })
                .expect("spawn omp worker");
        }
        Self { tx: Mutex::new(tx) }
    }

    fn run(&self, job: Job) {
        let _ = self.tx.lock().expect("team tx").send(job);
    }
}

/// 一次 execute 的行派发参数。
pub struct Schedule {
    /// static = true：连续区间预分；false：atomic 拉取（chunk 粒度）。
    pub r#static: bool,
    pub chunk: usize,
    /// 最多占用多少 worker（num_threads 子句；None = 全员）。
    pub num_threads: Option<usize>,
}

/// 执行一轮：header 枚举 + 行派发 + 按行序槽位收集。
///
/// 行预采 stride 在此推导：prng 总数必须按 header 行数均分（TR15 每行
/// 迭代次数一致 → 采集数一致；不整除即语义错位 = Err）。
///
/// 返回槽位数组（下标 = 行号）。worker 侧 bundle eval 按 key 缓存——每
/// worker 首帧 eval 一次。
pub fn execute(
    team: &Team,
    bundle: &Bundle,
    args_json: &Arc<str>,
    shared: Arc<[SharedBuf]>,
    prng: Arc<[f64]>,
    sched: &Schedule,
) -> Result<Vec<Option<RowOut>>, String> {
    let ys: Arc<Vec<f64>> = {
        // header 枚举：单个任务跑 __ocHeader（哪个 worker 先闲用哪个）。
        let (htx, hrx) = flume::bounded::<Result<Vec<f64>, String>>(1);
        let bundle = Bundle {
            key: bundle.key.clone(),
            src: bundle.src.clone(),
            kinds: bundle.kinds.clone(),
        };
        let env_args = args_json.clone();
        let env_shared = shared.clone();
        team.run(Box::new(move |w| {
            let r = (|| {
                if w.eval_bundle(&bundle).is_err() {
                    return Err("bundle eval failed".into());
                }
                // header 同样装载 capture 帧（TA 描述符在这里也要重建——
                // header 体可读 capture 值）；prng/stride 与 header 无关。
                let env = FrameEnv {
                    args_json: env_args,
                    shared: env_shared,
                    prng: Arc::from(Vec::new()),
                    stride: 0,
                };
                w.run_header(&bundle, &env)
            })();
            let _ = htx.send(r);
        }));
        let ys = hrx.recv().map_err(|_| "header worker gone".to_string())??;
        Arc::new(ys)
    };
    let n = ys.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    if prng.len() % n != 0 {
        return Err(format!("prng count {} not divisible by {} rows", prng.len(), n));
    }
    let stride = prng.len() / n;

    // 行派发：产出 (row, writes) 流，主线程按行号槽位填。
    let (rtx, rrx) = flume::unbounded::<Result<(usize, RowOut), String>>();
    let next_row = Arc::new(AtomicUsize::new(0));
    let workers = sched.num_threads.unwrap_or(usize::MAX).max(1);
    if sched.r#static {
        // 连续区间预分：近似等分，禁窃取。
        let nw = workers.min(n);
        let base = n / nw;
        let rem = n % nw;
        let mut start = 0usize;
        for wi in 0..nw {
            let len = base + usize::from(wi < rem);
            let (lo, hi) = (start, start + len);
            start = hi;
            let (bundle, env_args, shared, prng, stride, rtx, ys) = (
                Bundle {
                    key: bundle.key.clone(),
                    src: bundle.src.clone(),
                    kinds: bundle.kinds.clone(),
                },
                args_json.clone(),
                shared.clone(),
                prng.clone(),
                stride,
                rtx.clone(),
                ys.clone(),
            );
            team.run(Box::new(move |w| {
                for ri in lo..hi {
                    let r = run_one(w, &bundle, &env_args, &shared, &prng, stride, ri, ys[ri]);
                    if rtx.send(r.map(|o| (ri, o))).is_err() {
                        return;
                    }
                }
            }));
        }
    } else {
        // dynamic/auto：atomic 拉取（chunk 粒度，缺省 1）。
        let chunk = sched.chunk.max(1);
        for _ in 0..workers.min(n) {
            let (bundle, env_args, shared, prng, stride, rtx, next_row, ys) = (
                Bundle {
                    key: bundle.key.clone(),
                    src: bundle.src.clone(),
                    kinds: bundle.kinds.clone(),
                },
                args_json.clone(),
                shared.clone(),
                prng.clone(),
                stride,
                rtx.clone(),
                next_row.clone(),
                ys.clone(),
            );
            team.run(Box::new(move |w| {
                loop {
                    let ri = next_row.fetch_add(chunk, Ordering::Relaxed);
                    if ri >= ys.len() {
                        return;
                    }
                    let hi = (ri + chunk).min(ys.len());
                    for r0 in ri..hi {
                        let r =
                            run_one(w, &bundle, &env_args, &shared, &prng, stride, r0, ys[r0]);
                        if rtx.send(r.map(|o| (r0, o))).is_err() {
                            return;
                        }
                    }
                }
            }));
        }
    }

    // 收集：任何行失败 → 整体失败（Err——产出不落地）。
    let mut slots: Vec<Option<RowOut>> = (0..n).map(|_| None).collect();
    let mut got = 0usize;
    while got < n {
        let (ri, out) = rrx.recv().map_err(|_| "row worker gone".to_string())??;
        slots[ri] = Some(out);
        got += 1;
    }
    Ok(slots)
}

/// 单行执行（worker 侧）：bundle 就绪 + 预采段 + run_row。
fn run_one(
    w: &mut dyn JsWorker,
    bundle: &Bundle,
    args_json: &Arc<str>,
    shared: &Arc<[SharedBuf]>,
    prng: &Arc<[f64]>,
    stride: usize,
    row: usize,
    row_val: f64,
) -> Result<RowOut, String> {
    if w.eval_bundle(bundle).is_err() {
        return Err("bundle eval failed".into());
    }
    let env = FrameEnv {
        args_json: args_json.clone(),
        shared: shared.clone(),
        prng: prng.clone(),
        stride,
    };
    w.run_row(bundle, &env, row, row_val)
}
