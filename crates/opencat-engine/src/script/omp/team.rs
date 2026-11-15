//! isolate 团队调度（引擎无关）：rayon 池，线程↔引擎实例一一绑定。
//!
//! schedule → 派发映射（TR15 §19.6.3 语义；产出按行序收集，调度只影响
//! 负载分布不影响字节）：static = 连续区间预分（`with_max_len`，等分行
//! 负载下禁中途窃取）；dynamic(chunk) = chunk 行粒度（`with_min_len`，可
//! 切到 chunk）；guided/auto/runtime = 自适应（默认）。行失败 = 整次
//! execute 失败（调用方毒化降级，产出不落地——无半应用状态）。

use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{Arc, Mutex},
};

use rayon::prelude::*;

use super::protocol::{Bundle, FrameEnv, JsWorker, RowOut, SharedBuf};

/// 池线程 → 引擎实例。实例与线程一一绑定（V8 OwnedIsolate 是 !Send，
/// QuickJS Runtime 同样线程亲和），所以只能放 thread-local。
///
/// worker 从槽里`take` 出来跑、跑完放回：行内 panic 时槽为空而不是
/// 「已借出」，下一个任务自建新实例（等价旧版 catch_unwind 丢弃该行）。
thread_local! {
    static WORKER: RefCell<Option<Box<dyn JsWorker>>> = const { RefCell::new(None) };
}

fn with_worker<R>(
    factory: &Arc<dyn Fn() -> Box<dyn JsWorker> + Send + Sync>,
    f: impl FnOnce(&mut dyn JsWorker) -> R,
) -> R {
    WORKER.with(|cell| {
        let mut w = match cell.borrow_mut().take() {
            Some(w) => w,
            None => factory(),
        };
        let r = f(w.as_mut());
        *cell.borrow_mut() = Some(w);
        r
    })
}

/// 常驻团队（进程级单例；worker 由调用方注入的工厂构造——team 不认识
/// 具体引擎）。池按 worker 数缓存：`num_threads` 子句要真限流时另建小池。
pub struct Team {
    factory: Arc<dyn Fn() -> Box<dyn JsWorker> + Send + Sync>,
    default_n: usize,
    pools: Mutex<HashMap<usize, Arc<rayon::ThreadPool>>>,
}

impl Team {
    pub fn new(n: usize, factory: impl Fn() -> Box<dyn JsWorker> + Send + Sync + 'static) -> Self {
        let factory: Arc<dyn Fn() -> Box<dyn JsWorker> + Send + Sync> = Arc::new(factory);
        let default_n = n.max(1);
        let mut pools = HashMap::new();
        pools.insert(default_n, Arc::new(build_pool(default_n)));
        Self { factory, default_n, pools: Mutex::new(pools) }
    }

    fn pool(&self, n: usize) -> Arc<rayon::ThreadPool> {
        let mut pools = self.pools.lock().expect("team pools");
        pools
            .entry(n)
            .or_insert_with(|| Arc::new(build_pool(n)))
            .clone()
    }

    /// 在指定规模的池里跑一段（header 枚举用；行派发走 par_iter）。
    fn run<R: Send>(&self, n: usize, f: impl FnOnce(&mut dyn JsWorker) -> R + Send) -> R {
        let factory = self.factory.clone();
        self.pool(n).install(move || with_worker(&factory, f))
    }
}

fn build_pool(n: usize) -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new()
        .num_threads(n)
        .thread_name(|i| format!("oc-omp-worker-{i}"))
        // QuickJS 腿的 JS 栈上限按 4 MiB 配（默认 256 KiB 会让递归语料抛
        // call stack exceeded）；JS 栈落在 C 栈上，所以线程栈要留足（V8
        // 腿用不到这么多，虚拟内存不计常驻）。
        .stack_size(16 * 1024 * 1024)
        // 不预热 worker 实例：V8 isolate 创建贵，交给首个任务懒建——短活跃
        // 窗的帧不该为一个不跑的任务付启动成本。
        .build()
        .expect("build omp pool")
}

/// 一次 execute 的行派发参数。
pub struct Schedule {
    /// static = true：连续区间预分；false：允许窃取（chunk 粒度）。
    pub r#static: bool,
    pub chunk: usize,
    /// 最多占用多少 worker（num_threads 子句；None = 全员）。
    pub num_threads: Option<usize>,
}

/// 执行一轮：header 枚举 + 行派发 + 按行序收集。
///
/// 行预采 stride 在此推导：prng 总数必须按 header 行数均分（TR15 每行
/// 迭代次数一致 → 采集数一致；不整除即语义错位 = Err）。
///
/// worker 侧 bundle eval 按 key 缓存——每 worker 首帧 eval 一次。
pub fn execute(
    team: &Team,
    bundle: &Bundle,
    args_json: &Arc<str>,
    shared: Arc<[SharedBuf]>,
    prng: Arc<[f64]>,
    sched: &Schedule,
) -> Result<Vec<RowOut>, String> {
    let nw = sched.num_threads.filter(|&k| k > 0).unwrap_or(team.default_n);
    let ys = team.run(nw, {
        let bundle = bundle.clone();
        let env_args = args_json.clone();
        let env_shared = shared.clone();
        move |w| -> Result<Vec<f64>, String> {
            if w.eval_bundle(&bundle).is_err() {
                return Err("bundle eval failed".into());
            }
            // header 同样装载 capture 帧（TA 描述符在这里也要重建——header
            // 体可读 capture 值）；prng/stride 与 header 无关。
            let env = FrameEnv {
                args_json: env_args,
                shared: env_shared,
                prng: Arc::from(Vec::new()),
                stride: 0,
            };
            w.run_header(&bundle, &env)
        }
    })?;
    let n = ys.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    if prng.len() % n != 0 {
        return Err(format!("prng count {} not divisible by {} rows", prng.len(), n));
    }
    let stride = prng.len() / n;
    let ys = Arc::new(ys);

    // 行派发：有序 collect（IndexedParallelIterator::collect 保序）——
    // 不需要行号槽位，也不需要应答通道。
    let pool_n = nw.min(n);
    let rows: Result<Vec<RowOut>, String> = team.pool(pool_n).install(|| {
        let factory = team.factory.clone();
        let job = |ri: usize| {
            with_worker(&factory, |w| {
                run_one(w, bundle, args_json, &shared, &prng, stride, ri, ys[ri])
            })
        };
        let it = (0..n).into_par_iter();
        if sched.r#static {
            // 连续区间预分：每段 ceil(n/线程数)，禁中途窃取。
            it.with_max_len(n.div_ceil(pool_n)).map(job).collect()
        } else if sched.chunk > 1 {
            it.with_min_len(sched.chunk).map(job).collect()
        } else {
            it.map(job).collect()
        }
    });

    // 任何行失败 → 整体失败（Err——产出不落地）。
    rows
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
