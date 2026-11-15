//! `@omp` 通用循环派发 runtime 数据面：bundle 缓存 → args 解码 → 团队行派发
//! → 写回/ovals 按行序落地。
//!
//! 数据面（缓存/路由/校验/写回组包）在此 crate，引擎只注入 `OmpTeam`
//! 实现（desktop = rayon + V8/QuickJS isolate，web = worker + SAB）。
//! Rust 侧只做判定、搬运、按序应用。
//!
//! 全链确定性三保证：行产出按行号槽位合并（调度不影响字节）；f64 运算
//! 无归约无 race（行间零共享可变状态）；同参数两跑逐位一致（帧=f(t)）。
//! 任何失败（bundle 构建/解码/头部枚举/行执行/采集数不整除）都毒化缓存
//! 条目 → 后续帧 prep 直接 false → 骨架不再跑 → fallback 循环语义纯净
//! （PRNG 不被无谓消耗）。

pub mod protocol;
pub mod transform;

pub use transform::transform;

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

use serde_json::Value as JsonValue;

use crate::canvas::paint::BlendMode;
use crate::ir::draw_op::{ColorU8, DrawOp};
use crate::script::canvas_op::CanvasOp;
use crate::script::recorder::{MutationRecorder, MutationStore};

/// 团队行派发面（引擎注入）：header 枚举 + 行派发 + 按行序收集。
/// 任何行失败 = 整次 Err（产出不落地，由数据面毒化）。
pub trait OmpTeam {
    /// 团队 worker 能否重建 ArrayRw 槽的 TypedArray 快照（`rebuild_shared`）。
    /// 不能的腿（quickjs isolate）遇带描述符的行必抛 TDZ——入口按此诚实
    /// 拒绝，原因可读而不是行半跑。
    fn supports_shared(&self) -> bool {
        false
    }

    fn execute(
        &self,
        bundle: &protocol::Bundle,
        args_json: &Arc<str>,
        shared: Arc<[protocol::SharedBuf]>,
        prng: Arc<[f64]>,
        sched: &TeamSched,
    ) -> Result<Vec<protocol::RowOut>, String>;
}

/// 一次 execute 的行派发参数（引擎无关形态）。
pub struct TeamSched {
    /// static = true：连续区间预分；false：允许窃取（chunk 粒度）。
    pub r#static: bool,
    pub chunk: usize,
    /// 最多占用多少 worker（num_threads 子句；None = 全员）。
    pub num_threads: Option<usize>,
}

struct CacheEntry {
    poisoned: bool,
    /// worker 团队面（唯一后端：完整 JS 语义含递归与 ctx，数组写回走
    /// Set/Acc 通道）。`None` 只出现在毒化条目里。
    iso: Option<Arc<protocol::Bundle>>,
}

/// `schedule(kind[, chunk])` 文本 → 行派发形态。transform 存子句原文
/// （如 `"dynamic,1"`），这里解析一次。
#[derive(Clone, Copy)]
enum Schedule {
    /// 连续区间预分，倾向不窃取。
    Static,
    /// chunk 行粒度拉取。
    Dynamic(u32),
    /// guided / auto / runtime / 缺省：自适应。
    Auto,
}

fn parse_schedule(ir: &JsonValue) -> Schedule {
    let Some(text) = ir.get("schedule").and_then(|v| v.as_str()) else {
        return Schedule::Auto;
    };
    let mut parts = text.split(',');
    let kind = parts.next().unwrap_or("").trim();
    let chunk = parts.next().and_then(|v| v.trim().parse::<u32>().ok());
    match (kind, chunk) {
        ("static", _) => Schedule::Static,
        ("dynamic", c) => Schedule::Dynamic(c.unwrap_or(1).max(1)),
        _ => Schedule::Auto,
    }
}

static CACHE: LazyLock<Mutex<HashMap<String, CacheEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn cache_lock() -> MutexGuard<'static, HashMap<String, CacheEntry>> {
    CACHE.lock().unwrap_or_else(|e| e.into_inner())
}

thread_local! {
    /// 本线程最近一次路由决定（`explain` = 没有可接管的构造）。
    static ROUTE: RefCell<Option<&'static str>> = const { RefCell::new(None) };
}

/// 记下路由决定并打一行（同名不重打；`explain` 不覆盖已定的真路由）。
/// `OC_OMP_ROUTE=0` 静音。
fn mark_route(name: &'static str, why: &str) {
    ROUTE.with(|r| {
        let mut slot = r.borrow_mut();
        if *slot == Some(name) || (name == "explain" && slot.is_some()) {
            return;
        }
        *slot = Some(name);
        drop(slot);
        if std::env::var("OC_OMP_ROUTE").ok().as_deref() == Some("0") {
            return;
        }
        eprintln!("[omp] route: {name} ({why})");
    });
}

/// 当前路由：`isolate` / `explain` / `degraded`。
pub fn route() -> &'static str {
    ROUTE.with(|r| r.borrow().unwrap_or("explain"))
}

fn poison(key: &str, reason: &str) {
    let mut cache = cache_lock();
    if let Some(e) = cache.get_mut(key) {
        e.poisoned = true;
    }
    mark_route("degraded", reason);
    eprintln!("[omp] take failed, poisoned: {reason}");
}

/// `__ocOmpPrep(ir)` — 编译并缓存。true = 骨架可跑；false = 本帧与后续帧
/// 全部降级（毒化条目直接短路，不再重编译）。`OC_OMP_DISABLE=1` 强制
/// 全降级（seg 对拍的 JS 腿用）。
pub fn omp_prep(ir: &JsonValue) -> bool {
    if std::env::var("OC_OMP_DISABLE").ok().as_deref() == Some("1") {
        mark_route("degraded", "OC_OMP_DISABLE=1");
        return false;
    }
    let key = ir.to_string();
    let mut cache = cache_lock();
    if let Some(e) = cache.get(&key) {
        return !e.poisoned;
    }
    // 单一后端：isolate 团队（行体仍是 JS，worker JIT 执行）。bundle 构建失败即
    // 毒化——降级到原循环语义，PRNG 不再被无谓消耗。
    match protocol::build_bundle(ir) {
        Ok(b) => {
            mark_route("isolate", "bundle built");
            cache.insert(
                key,
                CacheEntry {
                    poisoned: false,
                    iso: Some(Arc::new(b)),
                },
            );
            true
        }
        Err(e) => {
            cache.insert(
                key,
                CacheEntry {
                    poisoned: true,
                    iso: None,
                },
            );
            mark_route("degraded", &format!("bundle build failed: {e}"));
            eprintln!("[omp] bundle build failed, degraded: {e}");
            false
        }
    }
}

/// `__ocOmpTake(ir, ...args, skeleton)` — native 循环体派发（引擎注入团队）。
/// true = 循环被旁路（op 流已直推）；false = 降级（原循环照跑）。
pub fn omp_take(
    team: &dyn OmpTeam,
    store: &mut MutationStore,
    ir: &JsonValue,
    args: &[JsonValue],
    shared: Arc<[protocol::SharedBuf]>,
    prng: &[f64],
    writes: &mut Vec<protocol::IsolateWrite>,
    reds: &mut Vec<f64>,
) -> bool {
    let key = ir.to_string();
    let iso = {
        let cache = cache_lock();
        let Some(entry) = cache.get(&key) else {
            return false;
        };
        if entry.poisoned {
            return false;
        }
        entry.iso.clone()
    };
    // isolate 团队分支（含 ctx 垫片面）：产出数组写回四元组（按行序填
    // 入 writes，调用方应用到真数组）+ ctx ovals 直推 FillOvalBatch。
    if let Some(bundle) = iso {
        if let Ok(f) = std::env::var("OC_OMP_DUMP_BUNDLE") {
            std::fs::write(&f, bundle.src.as_bytes()).ok();
        }
        return take_isolate(team, &key, ir, &bundle, args, shared, prng, store, writes, reds);
    }
    // prep 通过即 iso 为 Some（上面已早返回）；能到这里只剩降级路径。
    false
}

/// isolate 团队腿（no-ctx）：实参校验（长度 + Array 槽令牌）→ FrameEnv
/// 组装 → 团队执行（header 枚举 + 行派发 + 行序槽位收集）→ writes。
/// 任何失败毒化（行失败 = 整次失败，产出不落地）。
fn take_isolate(
    team: &dyn OmpTeam,
    key: &str,
    ir: &JsonValue,
    bundle: &Arc<protocol::Bundle>,
    args: &[JsonValue],
    shared: Arc<[protocol::SharedBuf]>,
    prng: &[f64],
    store: &mut MutationStore,
    writes: &mut Vec<protocol::IsolateWrite>,
    reds: &mut Vec<f64>,
) -> bool {
    // 无零拷贝 TypedArray 通道的团队腿没有 `rebuild_shared`：带描述符的
    // 行会让 `let out;` 停在 TDZ，行一跑就抛「out is not initialized」。
    // 在入口诚实拒绝，原因直接可读（能力由团队声明，不按编译 feature）。
    if !team.supports_shared() && args.iter().any(|a| a.get("__ocOmpShared").is_some()) {
        poison(
            key,
            "team leg: TypedArray capture needs rebuild_shared (v8/worker leg)",
        );
        return false;
    }
    let n_args = ir
        .get("args")
        .and_then(|a| a.as_array())
        .map_or(0, |a| a.len());
    if args.len() != n_args {
        poison(key, "args length mismatch");
        return false;
    }
    // Array 槽实参 = 令牌对象 `{"__ocOmpArr": k}`（k = 槽位号，v8.rs 绑定
    // 侧以令牌替代真数组——数组本体不过 isolate，copy-in 零字节）。rw 槽
    // 实参 = 帧初快照：TypedArray 走 binary 旁路描述符（plain capture
    // 同路，v8 腿 rebuild_shared 重建），plain 数组走 JSON 文本；其余形态
    // （null / 对象串化物）= 快照不可信 → 毒化。ctx 槽实参 =
    // `{"__ocNodeId", "state"}` JSON（worker 侧垫片常驻，实参只供 host
    // 读 op 流落点）。
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
            protocol::SlotKind::ArrayRw => {
                let snapshot_ok = args[i].get("__ocOmpShared").is_some() || args[i].is_array();
                if !snapshot_ok {
                    poison(
                        key,
                        &format!(
                            "rw array slot {i} needs a snapshot (shared descriptor or plain array)"
                        ),
                    );
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

    // num_threads 子句优先；ICV 环境量 OC_OMP_NUM_THREADS 是其缺省
    // （TR15 §7.7 的 ICV 语义，docs/js-openmp.md §12 T1 项）。
    let num_threads = ir
        .get("num_threads")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .or_else(|| {
            std::env::var("OC_OMP_NUM_THREADS")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|n| *n > 0)
        });
    let sched = match parse_schedule(ir) {
        Schedule::Static => TeamSched {
            r#static: true,
            chunk: 1,
            num_threads,
        },
        Schedule::Dynamic(c) => TeamSched {
            r#static: false,
            chunk: c as usize,
            num_threads,
        },
        Schedule::Auto => TeamSched {
            r#static: false,
            chunk: 1,
            num_threads,
        },
    };
    // 诊断计时挂 trace 开关：std Instant 在 wasm32 平台 panic（无时间源），
    // 不能无条件取墙钟。
    let trace_omp = std::env::var("OC_OMP_TRACE").ok().as_deref() == Some("1");
    let t0 = trace_omp.then(std::time::Instant::now);
    let slots = match team.execute(bundle, &args_json, shared, Arc::from(prng), &sched) {
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
    for row in slots {
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
    if trace_omp {
        eprintln!(
            "[omp] isolate take wall: {:?} for {n_rows} rows × {} writes × {oval_count} ovals",
            t0.map(|t| t.elapsed()),
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
    store.record_draw_op(
        &node_id,
        DrawOp::SetGlobalAlpha {
            alpha: ctx_alpha.clamp(0.0, 1.0) as f32,
        },
    );
    store.record_draw_op(
        &node_id,
        DrawOp::SetBlendMode {
            mode: BlendMode::SrcOver,
        },
    );
    store.record_canvas_op(&node_id, CanvasOp::SetCtxFilter { sigma: 0.0 });
    store.record_canvas_op(
        &node_id,
        CanvasOp::SetShadow {
            color: ColorU8 {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            },
            blur: 0.0,
            dx: 0.0,
            dy: 0.0,
        },
    );
    store.record_fill_oval_batch(&node_id, oval_rects, oval_colors, oval_count);
    true
}
