//! `@omp` 通用循环派发 runtime：解析+编译缓存 → args 解码 → rayon 行并行
//! → op 流直推。
//!
//! 全链确定性三保证：rayon indexed `par_iter().map().collect()` 保序产出
//! （alpha 叠层敏感）；f64 运算无归约无 race（行间零共享可变状态）；同
//! 参数两跑逐位一致（帧=f(t) 不变）。任何失败（编译/解码/头部枚举/行执
//! 行/采集数不整除）都毒化缓存条目 → 后续帧 prep 直接 false → 骨架不再
//! 跑 → fallback 循环语义纯净（PRNG 不被无谓消耗）。

#[cfg(feature = "simd")]
pub mod simd_kernel;
pub mod compile;
pub mod parse;
pub mod protocol;
#[cfg(feature = "js-quickjs")]
pub mod rq_team;
pub mod team;
pub mod transform;
#[cfg(feature = "js-v8")]
pub mod v8_team;
pub mod vm;
pub mod vm_soa;

pub use transform::transform;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

use rayon::prelude::*;
use serde_json::Value as JsonValue;

use opencat_core::canvas::paint::BlendMode;
use opencat_core::ir::draw_op::{ColorU8, DrawOp};
use opencat_core::script::canvas_op::CanvasOp;
use opencat_core::script::recorder::{MutationRecorder, MutationStore};

use compile::{CaptureKind, Program};
use vm::Value;

// isolate 团队工厂：引擎二选一（js-v8 → V8Worker / js-quickjs → RqWorker）。
#[cfg(feature = "js-v8")]
use v8_team::global_team;
#[cfg(feature = "js-quickjs")]
use rq_team::global_team;

struct CacheEntry {
    program: Arc<Program>,
    poisoned: bool,
    /// SoA 向量执行面（prepare 失败 = None → 标量 VM 兜底）。
    soa: Option<Arc<vm_soa::SoaPlan>>,
    /// isolate 团队面（no-ctx 循环的通用后端：完整 JS 语义含递归，
    /// 数组写回走 Set/Acc 通道）。ctx 循环归 opcode/SoA 面。
    iso: Option<Arc<protocol::Bundle>>,
}

static CACHE: LazyLock<Mutex<HashMap<String, CacheEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// std::simd lane kernel 缓存（`parallel for simd` 专属；`simd` feature
/// 门控——off 时 simd 构造走 isolate 后端，对纯算术循环结果等价）。值
/// None = 行执行/对拍失败的 poison 条目：后续帧 prep 命中即回落 isolate
/// （不毒化主 CACHE——isolate 语义等价，只是慢些）。
#[cfg(feature = "simd")]
static SIMD_CACHE: LazyLock<Mutex<HashMap<String, Option<Arc<simd_kernel::Kernel>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// `OC_OMP_TRACE=1`：native 接手时打一行（只打首次）——冒烟/性能跑前确认
/// 循环真走了 rayon 腿而非静默降级。
static TRACE_HIT: AtomicBool = AtomicBool::new(false);

fn trace_take(n_rows: usize, n_ovals: usize) {
    if !TRACE_HIT.swap(true, Ordering::SeqCst) {
        eprintln!("[omp] took over: {n_rows} rows, {n_ovals} ovals");
    }
}

fn cache_lock() -> MutexGuard<'static, HashMap<String, CacheEntry>> {
    CACHE.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(feature = "simd")]
fn simd_cache_lock() -> MutexGuard<'static, HashMap<String, Option<Arc<simd_kernel::Kernel>>>> {
    SIMD_CACHE.lock().unwrap_or_else(|e| e.into_inner())
}

/// ir 对象（`{loop, fns, args, captures}`）→ 编译产物。提取期在 irobj 里
/// 传入的形态恒定，字段缺失即编译失败（降级）。
fn compile_ir(ir: &JsonValue) -> Result<Program, String> {
    let loop_src = ir.get("loop").and_then(|v| v.as_str()).ok_or("missing `loop`")?;
    let args: Vec<String> = ir
        .get("args")
        .and_then(|v| v.as_array())
        .ok_or("missing `args`")?
        .iter()
        .map(|v| v.as_str().map(|s| s.to_string()).ok_or("non-string arg name"))
        .collect::<Result<_, _>>()?;
    let fns_obj = ir
        .get("fns")
        .and_then(|v| v.as_object())
        .ok_or("missing `fns`")?;
    let mut fns: Vec<(String, String)> = Vec::with_capacity(fns_obj.len());
    for (k, v) in fns_obj {
        let src = v.as_str().ok_or_else(|| format!("fn `{k}` source not a string"))?;
        fns.push((k.clone(), src.to_string()));
    }
    let captures = ir.get("captures").and_then(|v| v.as_object());
    let arg_kinds: Vec<CaptureKind> = args
        .iter()
        .map(|n| match captures.and_then(|c| c.get(n)).and_then(|v| v.as_str()) {
            Some("prng") => CaptureKind::Prng,
            Some("ctx") => CaptureKind::Ctx,
            _ => CaptureKind::Plain,
        })
        .collect();
    let private: Vec<String> = ir
        .get("private")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .map(|v| v.as_str().map(|s| s.to_string()).ok_or("non-string private name"))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    let mut program = compile::compile(loop_src, &fns, &args, &arg_kinds, &private)?;
    // schedule 子句 → 行分块策略（编译器不感知；ir 缺省 = Auto）。
    // dynamic,1 =「共享队列逐行拉取」——rayon work-stealing 缺省即此形态
    // （min_len 缺省就是 1，with_min_len(1) 是空操作），映射 Auto。
    program.schedule = parse_schedule(ir);
    Ok(program)
}

fn parse_schedule(ir: &JsonValue) -> compile::Schedule {
    match ir.get("schedule").and_then(|v| v.as_str()) {
        Some("static") => compile::Schedule::Static,
        Some(s) if s.starts_with("dynamic") => {
            let chunk = s.split(',').nth(1).and_then(|c| c.trim().parse().ok()).unwrap_or(1);
            if chunk > 1 {
                compile::Schedule::Dynamic(chunk)
            } else {
                compile::Schedule::Auto
            }
        }
        _ => compile::Schedule::Auto,
    }
}

fn poison(key: &str, reason: &str) {
    let mut cache = cache_lock();
    if let Some(e) = cache.get_mut(key) {
        e.poisoned = true;
    }
    eprintln!("[omp] take failed, poisoned: {reason}");
}

/// OpenMP `schedule` → rayon 行分块派发（indexed 保序产出，调度只影响
/// 负载分布不影响字节）。static = 等分禁窃取（均匀行负载最优）；
/// dynamic(chunk) = chunk 行粒度拉取；guided/auto/runtime = rayon 自适应。
macro_rules! rows_collect {
    ($n:expr, $sched:expr, $f:expr) => {
        match $sched {
            compile::Schedule::Static => {
                let chunk = ($n).div_ceil(rayon::current_num_threads().max(1));
                (0..$n)
                    .into_par_iter()
                    .with_max_len(chunk)
                    .map($f)
                    .collect::<Result<Vec<_>, String>>()
            }
            compile::Schedule::Dynamic(chunk) => {
                (0..$n)
                    .into_par_iter()
                    .with_min_len(chunk.max(1) as usize)
                    .map($f)
                    .collect::<Result<Vec<_>, String>>()
            }
            _ => (0..$n)
                .into_par_iter()
                .map($f)
                .collect::<Result<Vec<_>, String>>(),
        }
    };
}

/// `__ocOmpPrep(ir)` — 编译并缓存。true = 骨架可跑；false = 本帧与后续帧
/// 全部降级（毒化条目直接短路，不再重编译）。`OC_OMP_DISABLE=1` 强制
/// 全降级（seg 对拍的 JS 腿用）。
pub fn omp_prep(ir: &JsonValue) -> bool {
    if std::env::var("OC_OMP_DISABLE").ok().as_deref() == Some("1") {
        return false;
    }
    let key = ir.to_string();
    // 分流：simd 构造 + feature on → lane kernel 栈（直线算术循环的正统
    // 实现面）。白名单外形态 prepare 拒绝 → poison 本表（None 条目）回落
    // isolate，不毒化主 CACHE——isolate 对纯算术循环语义等价。
    #[cfg(feature = "simd")]
    if ir.get("simd").and_then(|v| v.as_bool()) == Some(true) {
        let mut sc = simd_cache_lock();
        if let Some(slot) = sc.get(&key) {
            if slot.is_some() {
                return true;
            }
            // poison 条目：fall through isolate（条目保留，拒绝日志只打一次）。
        } else {
            match simd_kernel::prepare(ir) {
                Ok(k) => {
                    sc.insert(key, Some(k));
                    return true;
                }
                Err(e) => {
                    sc.insert(key.clone(), None);
                    eprintln!("[omp] simd kernel skipped, isolate backend: {e}");
                }
            }
        }
    }
    let mut cache = cache_lock();
    if let Some(e) = cache.get(&key) {
        return !e.poisoned;
    }
    // 分流：无 ctx capture → isolate 团队后端（通用 JS 语义，数组写回走
    // Set/Acc；不走 opcode 编译器——递归等超白名单形态在 isolate 下合法，
    // smallpt 即此案）。有 ctx → opcode/SoA 面（op 流落点）。
    let has_ctx = ir
        .get("captures")
        .and_then(|c| c.as_object())
        .map_or(false, |m| m.values().any(|v| v.as_str() == Some("ctx")));
    // 分流：isolate 团队优先（S5 起含 ctx 垫片面——ctx 循环也上 isolate，
    // V8 JIT 行函数 + 8 物理核行并行快于 SoA op 解释）；bundle 构建（结构
    // 面）失败才回 opcode/SoA 编译器（ctx 循环）或毒化（无 ctx = SoA 面没
    // 有 op 流落点，原语义）。
    match protocol::build_bundle(ir) {
        Ok(b) => {
            cache.insert(key, CacheEntry {
                program: Arc::new(Program::empty()),
                poisoned: false,
                soa: None,
                iso: Some(Arc::new(b)),
            });
            return true;
        }
        Err(e) if !has_ctx => {
            cache.insert(key, CacheEntry {
                program: Arc::new(Program::empty()),
                poisoned: true,
                soa: None,
                iso: None,
            });
            eprintln!("[omp] isolate bundle build failed, degraded: {e}");
            return false;
        }
        Err(_) => {} // ctx 循环：回 SoA 编译器兜底（不毒化）
    }
    match compile_ir(ir) {
        Ok(program) => {
            // 无 ctx capture 的标注走不了 native（op 流没有落点）——毒化
            // 拒绝，否则骨架会消耗 PRNG 而 take 恒 false → fallback 发散。
            if !program.arg_kinds.contains(&CaptureKind::Ctx) {
                cache.insert(key, CacheEntry {
                    program: Arc::new(Program::empty()),
                    poisoned: true,
                    soa: None,
                    iso: None,
                });
                eprintln!("[omp] no ctx capture, degraded");
                return false;
            }
            let soa = match vm_soa::prepare(&program) {
                Ok(plan) => Some(Arc::new(plan)),
                Err(e) => {
                    eprintln!("[omp] soa prepare skipped, scalar vm backend: {e}");
                    None
                }
            };
            cache.insert(key, CacheEntry {
                program: Arc::new(program),
                poisoned: false,
                soa,
                iso: None,
            });
            true
        }
        Err(e) => {
            cache.insert(key, CacheEntry {
                program: Arc::new(Program::empty()),
                poisoned: true,
                soa: None,
                iso: None,
            });
            eprintln!("[omp] compile failed, degraded: {e}");
            false
        }
    }
}

fn json_to_value(j: &JsonValue) -> Option<Value> {
    Some(match j {
        JsonValue::Number(n) => Value::Num(n.as_f64()?),
        JsonValue::String(s) => Value::Str(s.as_str().into()),
        JsonValue::Bool(b) => Value::Bool(*b),
        JsonValue::Null => Value::Null,
        JsonValue::Array(items) => {
            let mut vs = Vec::with_capacity(items.len());
            for it in items {
                vs.push(json_to_value(it)?);
            }
            Value::Arr(Arc::new(vs))
        }
        JsonValue::Object(map) => {
            let mut fs = Vec::with_capacity(map.len());
            for (k, v) in map {
                fs.push((k.clone(), json_to_value(v)?));
            }
            Value::Obj(Arc::new(fs))
        }
    })
}

/// `__ocOmpTake(ir, ...args, skeleton)` — native 循环体派发。true = 循环被
/// 旁路（op 流已直推）；false = 降级（原循环照跑）。
pub fn omp_take(
    store: &mut MutationStore,
    ir: &JsonValue,
    args: &[JsonValue],
    prng: &[f64],
    writes: &mut Vec<protocol::IsolateWrite>,
    reds: &mut Vec<f64>,
) -> bool {
    let key = ir.to_string();
    // simd lane kernel 分流（prep 缓存命中时）：产出 = IsolateWrite 同款
    // 四元组，应用面（v8.rs）零改动。
    #[cfg(feature = "simd")]
    {
        let sim = simd_cache_lock().get(&key).cloned().flatten();
        if let Some(kernel) = sim {
            return take_simd(&key, ir, kernel, args, prng, writes);
        }
    }
    let (prog, soa, iso) = {
        let cache = cache_lock();
        let Some(entry) = cache.get(&key) else {
            return false;
        };
        if entry.poisoned {
            return false;
        }
        (entry.program.clone(), entry.soa.clone(), entry.iso.clone())
    };
    // isolate 团队分支（含 ctx 垫片面）：产出数组写回四元组（按行序填
    // 入 writes，调用方应用到真数组）+ ctx ovals 直推 FillOvalBatch。
    if let Some(bundle) = iso {
        if let Ok(f) = std::env::var("OC_OMP_DUMP_BUNDLE") {
            std::fs::write(&f, bundle.src.as_bytes()).ok();
        }
        return take_isolate(&key, ir, &bundle, args, prng, store, writes, reds);
    }
    let arg_kinds = &prog.arg_kinds;
    if args.len() != prog.arg_names.len() {
        poison(&key, "args length mismatch");
        return false;
    }
    // @omp 派发面要求一个 ctx capture——没有它 op 流没有落点。
    let Some(ctx_pos) = arg_kinds.iter().position(|k| *k == CaptureKind::Ctx) else {
        return false;
    };
    let Some(node_id) = args[ctx_pos].get("__ocNodeId").and_then(|v| v.as_str()) else {
        poison(&key, "ctx missing __ocNodeId");
        return false;
    };
    let global_alpha = args[ctx_pos]
        .get("state")
        .and_then(|s| s.get("globalAlpha"))
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);

    // 全局帧：args 原序 + 外层循环变量槽（逐行覆写）。
    let mut globals: Vec<Value> = Vec::with_capacity(prog.n_globals as usize);
    for (i, k) in arg_kinds.iter().enumerate() {
        globals.push(match k {
            CaptureKind::Prng => Value::Prng,
            CaptureKind::Ctx => {
                Value::Ctx { node_id: node_id.to_string(), global_alpha }
            }
            CaptureKind::Plain => match json_to_value(&args[i]) {
                Some(v) => v,
                None => {
                    poison(&key, "arg value not representable");
                    return false;
                }
            },
        });
    }
    globals.push(Value::Undef);
    let loop_var_slot = (prog.n_globals - 1) as usize;

    // 头部枚举 ys（行维度）。
    let ys = match vm::run_header(&prog, &globals) {
        Ok(y) => y,
        Err(e) => {
            poison(&key, &format!("header eval: {e}"));
            return false;
        }
    };
    let n_outer = ys.len();
    if n_outer == 0 {
        // 空循环：native 与 JS 同为无操作，旁路且不落 op。
        return true;
    }
    // 采集数必须按行均分（编译期已禁「内层头引用外层变量 + 采集」，
    // 结构性保证每行采集数一致；不整除即语义错位，毒化）。
    if prng.len() % n_outer != 0 {
        poison(&key, &format!("capture count {} not divisible by {} rows", prng.len(), n_outer));
        return false;
    }
    let stride = prng.len() / n_outer;

    // 行执行：SoA 向量后端优先（liangyi ExecutionPlan 形态），标量 VM 兜底。
    enum Backend<'a> {
        Soa {
            plan: &'a vm_soa::SoaPlan,
            n_inner: usize,
        },
        Vm,
    }
    let backend = if let Some(plan) = soa.as_deref() {
        // 迭代数每帧一次（行常量语义）；骨架 stride 必须恰为
        // cap_count × n_inner，否则采集错位。
        match vm_soa::probe_n_inner(&prog, plan, &globals) {
            Ok(n_inner) if stride == plan.cap_count * n_inner => {
                Backend::Soa { plan, n_inner }
            }
            Ok(n_inner) => {
                poison(
                    &key,
                    &format!(
                        "soa stride mismatch: {} != cap {} × inner {}",
                        stride, plan.cap_count, n_inner
                    ),
                );
                return false;
            }
            Err(e) => {
                poison(&key, &format!("soa probe: {e}"));
                return false;
            }
        }
    } else {
        Backend::Vm
    };

    // rayon indexed 行并行：保序产出（alpha 叠层敏感），行间零共享；
    // schedule 子句决定分块策略。
    let take_t0 = std::time::Instant::now();
    let rows: Vec<vm::FacetState> = match backend {
        Backend::Soa { plan, n_inner } => match rows_collect!(n_outer, prog.schedule, |ri| {
            let mut g = globals.clone();
            g[loop_var_slot] = Value::Num(ys[ri]);
            let row_prng = &prng[ri * stride..(ri + 1) * stride];
            let mut facet = vm::FacetState::default();
            vm_soa::run_row(&prog, plan, g, row_prng, n_inner, &mut facet)?;
            Ok(facet)
        }) {
            Ok(r) => r,
            Err(e) => {
                poison(&key, &format!("soa row exec: {e}"));
                return false;
            }
        },
        Backend::Vm => match rows_collect!(n_outer, prog.schedule, |ri| {
            let mut g = globals.clone();
            g[loop_var_slot] = Value::Num(ys[ri]);
            let row_prng: &[f64] = if stride == 0 {
                &[]
            } else {
                &prng[ri * stride..(ri + 1) * stride]
            };
            vm::run_row(&prog, g, row_prng)
        }) {
            Ok(r) => r,
            Err(e) => {
                poison(&key, &format!("row exec: {e}"));
                return false;
            }
        },
    };

    let mut rects: Vec<f32> = Vec::new();
    let mut colors: Vec<u8> = Vec::new();
    let mut count = 0usize;
    for f in rows {
        rects.extend_from_slice(&f.rects);
        colors.extend_from_slice(&f.colors);
        count += f.count;
    }
    if count == 0 {
        return true;
    }
    if std::env::var("OC_OMP_TRACE").ok().as_deref() == Some("1") {
        trace_take(n_outer, count);
        eprintln!("[omp] take wall: {:?} for {n_outer} rows × {count} ovals", take_t0.elapsed());
    }
    // 与垫片 batchFlush 同序同值的状态面：aa / alpha / blend(source-over,
    // wire 3) / filter(none) / shadow(全零)。执行端是持久 sticky 状态，
    // 无条件下发同值幂等，op 流语义等价（像素逐位一致由 seg 对拍判定）。
    store.record_draw_op(&node_id, DrawOp::SetAntiAlias { enabled: true });
    store.record_draw_op(&node_id, DrawOp::SetGlobalAlpha {
        alpha: global_alpha.clamp(0.0, 1.0) as f32,
    });
    store.record_draw_op(&node_id, DrawOp::SetBlendMode { mode: BlendMode::SrcOver });
    store.record_canvas_op(&node_id, CanvasOp::SetCtxFilter { sigma: 0.0 });
    store.record_canvas_op(&node_id, CanvasOp::SetShadow {
        color: ColorU8 { r: 0, g: 0, b: 0, a: 0 },
        blur: 0.0,
        dx: 0.0,
        dy: 0.0,
    });
    store.record_fill_oval_batch(&node_id, rects, colors, count);
    true
}

/// std::simd lane kernel 腿（`parallel for simd`，feature 门控）：header
/// 标量枚举 → 8-y 批打包（lane = 8 个外层迭代，尾批 NaN 补位）→ lane 域
/// 批间 rayon 并行 → 写产出按批序展开（= IsolateWrite 四元组）。simd 栈
/// 无 prng/ctx capture（prepare 白名单拒绝）——args 全是 plain 数字与数
/// 组令牌。任何失败 poison 本表（None 条目）→ 本帧 fallback 原循环，后
/// 续帧 prep 命中条目回落 isolate（语义等价）。
#[cfg(feature = "simd")]
fn take_simd(
    key: &str,
    ir: &JsonValue,
    kernel: Arc<simd_kernel::Kernel>,
    args: &[JsonValue],
    prng: &[f64],
    writes: &mut Vec<protocol::IsolateWrite>,
) -> bool {
    let _ = prng; // prng capture 在 prepare 被拒——恒空
    // plain 实参解码（数组槽 = 令牌对象，跳过不进值面——写产出按通道回
    // IsolateWrite.arg 落到真数组）。
    let caps = ir.get("captures").and_then(|c| c.as_object());
    let arg_names = ir.get("args").and_then(|a| a.as_array());
    let mut flat = Vec::with_capacity(args.len());
    for (i, a) in args.iter().enumerate() {
        let name = arg_names.and_then(|ns| ns.get(i)).and_then(|n| n.as_str());
        if name.and_then(|n| caps.and_then(|c| c.get(n))).and_then(|v| v.as_str()) == Some("array")
        {
            continue;
        }
        match a.as_f64() {
            Some(v) => flat.push(v),
            None => {
                simd_poison(key, "args not all numeric");
                return false;
            }
        }
    }
    let take_t0 = std::time::Instant::now();
    let ys = match simd_kernel::header_ys(&kernel, &flat) {
        Ok(ys) => ys,
        Err(e) => {
            simd_poison(key, &format!("header enum: {e}"));
            return false;
        }
    };
    let n_rows = ys.len();
    if n_rows == 0 {
        return true;
    }
    let verify = std::env::var("OC_OMP_VERIFY").ok().as_deref() == Some("1");
    // 8-y 批打包；批间 rayon indexed 保序（写产出按批序展开 = 行序语义）。
    let n_batches = n_rows.div_ceil(8);
    let batch_rows = rows_collect!(n_batches, parse_schedule(ir), |bi| {
        let slice = &ys[bi * 8..(bi * 8 + 8).min(n_rows)];
        let mut lane = simd_kernel::run_row(&kernel, slice, &flat)?;
        if verify {
            // 对拍面：产出按 (channel, idx, acc, val 位型) 全序排序后逐位
            // 比（同 idx 多写的应用序不进键——最终态由值字节锁定）；lane
            // 原序保留照常组包。
            let mut scalar = simd_kernel::run_row_verify(&kernel, slice, &flat)?;
            let mut lane_sorted = lane.clone();
            sort_sim_writes(&mut lane_sorted);
            sort_sim_writes(&mut scalar);
            if lane_sorted.len() != scalar.len()
                || lane_sorted.iter().zip(&scalar).any(|(a, b)| {
                    a.channel != b.channel
                        || a.idx != b.idx
                        || a.acc != b.acc
                        || a.val.to_bits() != b.val.to_bits()
                })
            {
                return Err("lane/scalar verify mismatch".into());
            }
        }
        Ok(lane)
    });
    let Ok(batch_rows) = batch_rows else {
        let e = batch_rows.unwrap_err();
        simd_poison(key, &format!("row exec: {e}"));
        return false;
    };
    let mut n_writes = 0usize;
    for row in batch_rows {
        for w in row {
            // 尾批 NaN 补位 lane 的哨兵写丢弃（对 TypedArray 本就无效）。
            if w.idx == u32::MAX {
                continue;
            }
            let Some(arg) = kernel.array_arg_pos(w.channel) else {
                simd_poison(key, "write channel out of range");
                return false;
            };
            writes.push(protocol::IsolateWrite { arg, idx: w.idx, acc: w.acc, val: w.val });
            n_writes += 1;
        }
    }
    if std::env::var("OC_OMP_TRACE").ok().as_deref() == Some("1") {
        let _ = trace_take(n_rows, n_writes);
        eprintln!(
            "[omp] simd take wall: {:?} for {n_rows} rows ({n_batches} batches × 8) × {n_writes} writes",
            take_t0.elapsed()
        );
    }
    true
}

#[cfg(feature = "simd")]
fn sort_sim_writes(ws: &mut [simd_kernel::SimWrite]) {
    ws.sort_by_key(|w| (w.channel, w.idx, w.acc, w.val.to_bits()));
}

#[cfg(feature = "simd")]
fn simd_poison(key: &str, reason: &str) {
    let mut sc = simd_cache_lock();
    sc.insert(key.to_string(), None);
    eprintln!("[omp] simd take failed, isolate fallback next frames: {reason}");
}

/// isolate 团队腿（no-ctx）：实参校验（长度 + Array 槽令牌）→ FrameEnv
/// 组装 → 团队执行（header 枚举 + 行派发 + 行序槽位收集）→ writes。
/// 任何失败毒化（行失败 = 整次失败，产出不落地）。
fn take_isolate(
    key: &str,
    ir: &JsonValue,
    bundle: &Arc<protocol::Bundle>,
    args: &[JsonValue],
    prng: &[f64],
    store: &mut MutationStore,
    writes: &mut Vec<protocol::IsolateWrite>,
    reds: &mut Vec<f64>,
) -> bool {
    let n_args = ir.get("args").and_then(|a| a.as_array()).map_or(0, |a| a.len());
    if args.len() != n_args {
        poison(key, "args length mismatch");
        return false;
    }
    // Array 槽实参 = 令牌对象 `{"__ocOmpArr": k}`（k = 槽位号，v8.rs 绑定
    // 侧以令牌替代真数组——数组本体不过 isolate，copy-in 零字节）。ctx 槽
    // 实参 = `{"__ocNodeId", "state"}` JSON（worker 侧垫片常驻，实参只供
    // host 读 op 流落点）。
    let mut ctx_node_id: Option<String> = None;
    let mut ctx_alpha = 1.0f64;
    for (i, k) in bundle.kinds.iter().enumerate() {
        match *k {
            protocol::SlotKind::Array => {
                if args[i].get("__ocOmpArr").and_then(|v| v.as_u64()) != Some(i as u64) {
                    poison(key, &format!("array slot {i} token mismatch"));
                    return false;
                }
            }
            protocol::SlotKind::Ctx => {
                let Some(id) = args[i].get("__ocNodeId").and_then(|v| v.as_str()) else {
                    poison(key, &format!("ctx slot {i} missing __ocNodeId"));
                    return false;
                };
                ctx_node_id = Some(id.to_string());
                ctx_alpha = args[i]
                    .get("state")
                    .and_then(|s| s.get("globalAlpha"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1.0);
            }
            _ => {}
        }
    }
    let args_json: Arc<str> = match serde_json::to_string(args) {
        Ok(s) => Arc::from(s.as_str()),
        Err(e) => {
            poison(key, &format!("args serialize: {e}"));
            return false;
        }
    };

    let num_threads = ir.get("num_threads").and_then(|v| v.as_u64()).map(|v| v as usize);
    let sched = match parse_schedule(ir) {
        compile::Schedule::Static => team::Schedule { r#static: true, chunk: 1, num_threads },
        compile::Schedule::Dynamic(c) => team::Schedule { r#static: false, chunk: c as usize, num_threads },
        _ => team::Schedule { r#static: false, chunk: 1, num_threads },
    };
    let t0 = std::time::Instant::now();
    let slots = match team::execute(global_team(), bundle, &args_json, Arc::from(prng), &sched) {
        Ok(s) => s,
        Err(e) => {
            poison(key, &format!("isolate exec: {e}"));
            return false;
        }
    };
    let n_rows = slots.len();
    // reduction 面（TR15 §8.10 映射）：行产出的终值槽（arg=1000+k，k =
    // ir.reduction 展开序）不进 writes，按行序左折叠副本们（首副本起，
    // initializer 不重复参与——确定性收紧，docs/js-openmp.md §3.4）。
    // 零行 = 域空：OpenMP 语义变量保持原值 → 不接管（守卫 else 跑原
    // 循环，零次迭代幂等）；接了反而会拿空数组首元素写 undefined。
    let red_ops: Vec<&str> = ir
        .get("reduction")
        .and_then(|v| v.as_array())
        .map(|rs| {
            rs.iter()
                .flat_map(|r| {
                    let op = r.get(0).and_then(|v| v.as_str()).unwrap_or("+");
                    let n = r.get(1).and_then(|v| v.as_array()).map_or(0, |a| a.len());
                    std::iter::repeat(op).take(n)
                })
                .collect()
        })
        .unwrap_or_default();
    let has_red = !red_ops.is_empty();
    if has_red && n_rows == 0 {
        return false;
    }
    let mut folds: Vec<Option<f64>> = vec![None; red_ops.len()];
    // ctx 垫片 ovals：行序组包。rect 变换 = JS 腿垫片满圈快路同形
    // （cx-r, cy-r, 2r, 2r → f32）；色已是 worker JS 侧的 0-255 终值。
    let mut oval_rects: Vec<f32> = Vec::new();
    let mut oval_colors: Vec<u8> = Vec::new();
    let mut oval_count = 0usize;
    for row in slots.into_iter().flatten() {
        for w in row.writes {
            if has_red && w.arg >= 1000 {
                let k = w.arg - 1000;
                if k >= folds.len() {
                    poison(key, "reduction slot out of range");
                    return false;
                }
                folds[k] = Some(match folds[k] {
                    None => w.val,
                    Some(v) => match red_ops[k] {
                        "*" => v * w.val,
                        _ => v + w.val,
                    },
                });
            } else {
                writes.push(w);
            }
        }
        for o in row.ovals.chunks_exact(7) {
            let (cx, cy, r) = (o[0], o[1], o[2]);
            oval_rects.extend_from_slice(&[
                (cx - r) as f32,
                (cy - r) as f32,
                (r * 2.0) as f32,
                (r * 2.0) as f32,
            ]);
            oval_colors.extend_from_slice(&[o[3] as u8, o[4] as u8, o[5] as u8, o[6] as u8]);
            oval_count += 1;
        }
    }
    if has_red {
        if folds.iter().any(|f| f.is_none()) {
            poison(key, "reduction slot missing from row output");
            return false;
        }
        reds.extend(folds.into_iter().flatten());
    }
    if std::env::var("OC_OMP_TRACE").ok().as_deref() == Some("1") {
        eprintln!(
            "[omp] isolate take wall: {:?} for {n_rows} rows × {} writes × {oval_count} ovals",
            t0.elapsed(),
            writes.len()
        );
    }
    if oval_count == 0 {
        // 无 oval（纯数组写回 / red 循环，或该帧条件分支没画）。
        return true;
    }
    let Some(node_id) = ctx_node_id else {
        poison(key, "ovals without ctx slot");
        return false;
    };
    // 与垫片 batchFlush 同序同值的状态面：aa / alpha / blend(source-over,
    // wire 3) / filter(none) / shadow(全零)。执行端是持久 sticky 状态，
    // 无条件下发同值幂等，op 流语义等价（SoA 出口同款）。
    store.record_draw_op(&node_id, DrawOp::SetAntiAlias { enabled: true });
    store.record_draw_op(&node_id, DrawOp::SetGlobalAlpha {
        alpha: ctx_alpha.clamp(0.0, 1.0) as f32,
    });
    store.record_draw_op(&node_id, DrawOp::SetBlendMode { mode: BlendMode::SrcOver });
    store.record_canvas_op(&node_id, CanvasOp::SetCtxFilter { sigma: 0.0 });
    store.record_canvas_op(&node_id, CanvasOp::SetShadow {
        color: ColorU8 { r: 0, g: 0, b: 0, a: 0 },
        blur: 0.0,
        dx: 0.0,
        dy: 0.0,
    });
    store.record_fill_oval_batch(&node_id, oval_rects, oval_colors, oval_count);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::omp::transform;

    /// 语料形循环：嵌套两层 + 页面函数闭包 + PRNG 采集 + rgba 模板 +
    /// ctx facet。对拍 native 行并行 vs 逐点串行期望（手工期望值）。
    fn corpus_like_ir() -> JsonValue {
        serde_json::json!({
            "loop": "for (let y = 0; y < h; y += 1) {\n  for (let x = 0; x < w; x += 1) {\n    const noise = random();\n    const v = smooth(0, 1, x / w + noise * 0.001);\n    ctx.fillStyle = `rgba(${v * 200 | 0}, 10, 10, ${alpha.toFixed(3)})`;\n    ctx.beginPath();\n    ctx.arc(x + (noise - 0.5) * 0.12, y, 1, 0, Math.PI * 2);\n    ctx.fill();\n  }\n}",
            "fns": {
                "smooth": "function smooth(e0, e1, v) {\n  const t = clamp((v - e0) / (e1 - e0), 0, 1);\n  return t * t * (3 - 2 * t);\n}"
            },
            "args": ["w", "h", "alpha", "random", "ctx"],
            "captures": { "random": "prng", "ctx": "ctx" }
        })
    }

    #[test]
    fn compile_corpus_like_loop() {
        // clamp 是 smooth 的传递闭包——fns 缺它必须编译失败（降级面）。
        let ir_missing = corpus_like_ir();
        assert!(compile_ir(&ir_missing).is_err(), "missing callee must fail");
        let ir_full = serde_json::json!({
            "loop": ir_missing["loop"],
            "fns": {
                "smooth": ir_missing["fns"]["smooth"],
                "clamp": "function clamp(v, lo, hi) {\n  return Math.max(lo, Math.min(hi, v));\n}"
            },
            "args": ir_missing["args"], "captures": ir_missing["captures"]
        });
        let prog = compile_ir(&ir_full).expect("compile with clamp");
        assert_eq!(prog.arg_names.len(), 5);
        assert_eq!(prog.arg_kinds[3], CaptureKind::Prng);
        assert_eq!(prog.arg_kinds[4], CaptureKind::Ctx);
        assert_eq!(prog.loop_var, "y");
        assert_eq!(prog.n_globals, 6);
        assert!(!prog.fns.is_empty());
        assert!(prog.header_start > 0);
    }

    #[test]
    fn transform_emits_prep_guard() {
        // 带 ctx 成员调用的 bare 标注（OpenMP 缺省 default(shared)）：守卫
        // 照发；无 ctx capture 的循环由 omp_prep 毒化拒绝并打日志。
        let src = "// @omp parallel for shared(a, ctx)\nfor (let i = 0; i < 3; i += 1) {\n  ctx.fillRect(a, i, 1, 1);\n}\n";
        let out = transform::transform(src);
        let s = &*out;
        assert!(s.contains("__ocOmpPrep("));
        assert!(s.contains("__ocOmpTake("));
        assert!(s.contains("for (let i = 0; i < 3; i += 1) {"));
    }

    /// isolate 团队端到端（smallpt 形态 = 无 ctx + 数组 Set/Acc）：合成
    /// 已改写的 bundle ir，走 omp_take 全路径（令牌校验 → 团队执行 →
    /// 行序 writes），验证产出顺序、数值与两跑逐位一致。
    #[test]
    fn isolate_team_set_acc_end_to_end() {
        let ir = serde_json::json!({
            "header": "let y = 0; y < 6; y += 1",
            "var": "y",
            "body": "  for (let x = 0; x < 4; x += 1) {\n    __ocOmpSet(c, y * 4 + x, y * 10 + x);\n    __ocOmpAcc(acc, y * 4 + x, x + 1);\n  }\n",
            "fns": {},
            "args": ["c", "acc"],
            "captures": { "c": "array", "acc": "array" }
        });
        assert!(omp_prep(&ir), "no-ctx loop must take the isolate path");
        let args = vec![
            serde_json::json!({ "__ocOmpArr": 0, "len": 32 }),
            serde_json::json!({ "__ocOmpArr": 1, "len": 32 }),
        ];
        let run = || {
            let mut store = MutationStore::default();
            let mut writes = Vec::new();
            let mut reds = Vec::new();
            assert!(omp_take(&mut store, &ir, &args, &[], &mut writes, &mut reds));
            assert!(reds.is_empty(), "no reduction clause → no folded values");
            writes
        };
        let a = run();
        let b = run();
        assert_eq!(a.len(), 6 * 8, "6 rows × (4 Set + 4 Acc)");
        for ri in 0..6usize {
            for x in 0..4usize {
                // flush 按数组分组产出（组内保持 dirty 插入序=行内执行序）：
                // 前 4 条 = c 的 Set，后 4 条 = acc 的 Acc。
                let s = &a[ri * 8 + x];
                assert_eq!((s.arg, s.idx, s.acc), (0, (ri * 4 + x) as u32, false));
                assert_eq!(s.val, (ri * 10 + x) as f64);
                let t = &a[ri * 8 + 4 + x];
                // Acc 槽 flush 产出 = 槽终值、按 Set 应用（协议锁定帧初值
                // 0：set(0+Σ) ≡ get(0)+add(Σ)），所以 acc 恒 false。
                assert_eq!((t.arg, t.idx, t.acc), (1, (ri * 4 + x) as u32, false));
                assert_eq!(t.val, (x + 1) as f64);
            }
        }
        assert_eq!(a, b, "two runs must be bit-identical");
    }

    /// Array 槽令牌错误（槽位号错位）必须毒化——写回落错数组是静默错
    /// 像的源头，宁降级不冒险。
    #[test]
    fn isolate_team_bad_array_token_poisons() {
        let ir = serde_json::json!({
            "header": "let y = 0; y < 2; y += 1",
            "var": "y",
            "body": "  __ocOmpSet(c, y, y);\n",
            "fns": {},
            "args": ["c"],
            "captures": { "c": "array" }
        });
        assert!(omp_prep(&ir));
        let args = vec![serde_json::json!({ "__ocOmpArr": 7 })];
        let mut store = MutationStore::default();
        let mut writes = Vec::new();
        let mut reds = Vec::new();
        assert!(!omp_take(&mut store, &ir, &args, &[], &mut writes, &mut reds));
        // 毒化后同 ir 二次 take 直接 false（缓存短路）。
        assert!(!omp_take(&mut store, &ir, &args, &[], &mut writes, &mut reds));
    }

    /// reduction 端到端（TR15 §8.10 映射）：行私有副本起 initializer，行末
    /// 产终值槽（arg=1000+k），host 按行序左折叠副本们（首副本起，init 不
    /// 重复参与）。6 行 × acc+=y+1 → 副本 1..6 折叠 21；p 每行 ×2 两次 →
    /// 副本恒 4，折叠 4⁶=4096。产出序 = ir.reduction 展开序。
    #[test]
    fn isolate_team_reduction_folds_row_order() {
        let ir = serde_json::json!({
            "header": "let y = 0; y < 6; y += 1",
            "var": "y",
            "body": "  acc += y + 1;\n  p *= 2;\n  p *= 2;\n",
            "fns": {},
            "args": [],
            "captures": {},
            "reduction": [["+", ["acc"]], ["*", ["p"]]]
        });
        assert!(omp_prep(&ir), "no-ctx reduction loop must take isolate path");
        let mut store = MutationStore::default();
        let mut writes = Vec::new();
        let mut reds = Vec::new();
        assert!(omp_take(&mut store, &ir, &[], &[], &mut writes, &mut reds));
        assert!(writes.is_empty(), "no array slots → no writes");
        assert_eq!(reds, vec![21.0, 4096.0]);
    }

    /// 零行域不接管：OpenMP 语义下变量保持原值——守卫 else 跑原循环
    /// 零次迭代幂等；若接管会拿空折叠数组首元素写 undefined。
    #[test]
    fn isolate_team_reduction_zero_rows_falls_back() {
        let ir = serde_json::json!({
            "header": "let y = 0; y < 0; y += 1",
            "var": "y",
            "body": "  acc += 1;\n",
            "fns": {},
            "args": [],
            "captures": {},
            "reduction": [["+", ["acc"]]]
        });
        assert!(omp_prep(&ir));
        let mut store = MutationStore::default();
        let mut writes = Vec::new();
        let mut reds = Vec::new();
        assert!(!omp_take(&mut store, &ir, &[], &[], &mut writes, &mut reds));
        assert!(reds.is_empty());
    }

    /// ctx 垫片端到端（S5）：beginPath→arc→fill 满圈快路 → 行产出 ovals
    /// → FillOvalBatch 直推 store；red × ctx 共存（垫片产出与 red 终值槽
    /// 同行走 isolate）。rgba 串解析 = JS 腿垫片 batchColor 同码，alpha
    /// 0.5 → Math.round(0.5*255)=128（半上）。
    #[test]
    fn isolate_team_ctx_shim_with_reduction_end_to_end() {
        let ir = serde_json::json!({
            "header": "let y = 0; y < 3; y += 1",
            "var": "y",
            "body": "  ctx.fillStyle = `rgba(${(y * 40) | 0}, 10, 20, 0.5)`;\n  ctx.beginPath();\n  ctx.arc(y, y + 1, 2.5, 0, Math.PI * 2);\n  ctx.fill();\n  acc += y;\n",
            "fns": {},
            "args": ["ctx"],
            "captures": { "ctx": "ctx" },
            "reduction": [["+", ["acc"]]]
        });
        assert!(omp_prep(&ir), "ctx+reduction must take the isolate path");
        let args = vec![serde_json::json!({
            "__ocNodeId": "node-x",
            "state": { "globalAlpha": 1.0 }
        })];
        let mut store = MutationStore::default();
        let mut writes = Vec::new();
        let mut reds = Vec::new();
        assert!(omp_take(&mut store, &ir, &args, &[], &mut writes, &mut reds));
        assert!(writes.is_empty(), "no array slots → no writes");
        assert_eq!(reds, vec![3.0], "0+1+2 left-fold");
    }

    /// 垫片快路之外的形态必须行错误毒化（绝不下发错几何）：非满圈 arc。
    #[test]
    fn isolate_team_ctx_shim_non_full_circle_poisons() {
        let ir = serde_json::json!({
            "header": "let y = 0; y < 2; y += 1",
            "var": "y",
            "body": "  ctx.beginPath();\n  ctx.arc(y, y, 2, 0, 1);\n  ctx.fill();\n",
            "fns": {},
            "args": ["ctx"],
            "captures": { "ctx": "ctx" }
        });
        assert!(omp_prep(&ir));
        let args = vec![serde_json::json!({
            "__ocNodeId": "node-x",
            "state": { "globalAlpha": 1.0 }
        })];
        let mut store = MutationStore::default();
        let mut writes = Vec::new();
        let mut reds = Vec::new();
        assert!(!omp_take(&mut store, &ir, &args, &[], &mut writes, &mut reds));
        // 毒化后同 ir 二次 take 直接 false（缓存短路）。
        assert!(!omp_take(&mut store, &ir, &args, &[], &mut writes, &mut reds));
    }
}
