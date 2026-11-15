//! `@omp` 通用循环派发 runtime：bundle 缓存 → args 解码 → worker 行并行
//! → 写回/ovals 按行序落地。
//!
//! 单一后端：行体仍是 JS，跑在 worker 引擎（V8 或 QuickJS）的独立实例里，
//! 由 `team` 派发。Rust 侧只做判定、搬运、按序应用。
//!
//! 全链确定性三保证：行产出按行号槽位合并（调度不影响字节）；f64 运算
//! 无归约无 race（行间零共享可变状态）；同参数两跑逐位一致（帧=f(t)）。
//! 任何失败（bundle 构建/解码/头部枚举/行执行/采集数不整除）都毒化缓存
//! 条目 → 后续帧 prep 直接 false → 骨架不再跑 → fallback 循环语义纯净
//! （PRNG 不被无谓消耗）。

pub mod protocol;
#[cfg(feature = "js-quickjs")]
pub mod rq_team;
pub mod team;
pub mod transform;
#[cfg(feature = "js-v8")]
pub mod v8_team;

pub use transform::transform;

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

use serde_json::Value as JsonValue;

use opencat_core::canvas::paint::BlendMode;
use opencat_core::ir::draw_op::{ColorU8, DrawOp};
use opencat_core::script::canvas_op::CanvasOp;
use opencat_core::script::recorder::{MutationRecorder, MutationStore};


// isolate 团队工厂：引擎二选一（js-v8 → V8Worker / js-quickjs → RqWorker）。
#[cfg(feature = "js-v8")]
use v8_team::global_team;
#[cfg(feature = "js-quickjs")]
use rq_team::global_team;

struct CacheEntry {
    poisoned: bool,
    /// worker 团队面（唯一后端：完整 JS 语义含递归与 ctx，数组写回走
    /// Set/Acc 通道）。`None` 只出现在毒化条目里。
    iso: Option<Arc<protocol::Bundle>>,
}

/// `schedule(kind[, chunk])` 文本 → 行派发形态。transform 存子句原文
/// （如 `"dynamic,1"`），这里解析一次。
#[derive(Clone, Copy)]
pub(crate) enum Schedule {
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
    // 单一后端：isolate 团队（行体仍是 JS，V8 JIT 执行）。bundle 构建失败即
    // 毒化——降级到原循环语义，PRNG 不再被无谓消耗。
    match protocol::build_bundle(ir) {
        Ok(b) => {
            mark_route("isolate", "bundle built");
            cache.insert(key, CacheEntry { poisoned: false, iso: Some(Arc::new(b)) });
            true
        }
        Err(e) => {
            cache.insert(key, CacheEntry { poisoned: true, iso: None });
            mark_route("degraded", &format!("bundle build failed: {e}"));
            eprintln!("[omp] bundle build failed, degraded: {e}");
            false
        }
    }
}

/// `__ocOmpTake(ir, ...args, skeleton)` — native 循环体派发。true = 循环被
/// 旁路（op 流已直推）；false = 降级（原循环照跑）。
pub fn omp_take(
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
        return take_isolate(&key, ir, &bundle, args, shared, prng, store, writes, reds);
    }
    // prep 通过即 iso 为 Some（上面已早返回）；能到这里只剩降级路径。
    false
}

/// isolate 团队腿（no-ctx）：实参校验（长度 + Array 槽令牌）→ FrameEnv
/// 组装 → 团队执行（header 枚举 + 行派发 + 行序槽位收集）→ writes。
/// 任何失败毒化（行失败 = 整次失败，产出不落地）。
fn take_isolate(
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
    // QuickJS 腿没有 `rebuild_shared`（rq_team 无 TypedArray 零拷贝通道）：
    // 带描述符的行会让 `let out;` 停在 TDZ，行一跑就抛「out is not
    // initialized」。在入口诚实拒绝，原因直接可读。
    #[cfg(not(feature = "js-v8"))]
    if args.iter().any(|a| a.get("__ocOmpShared").is_some()) {
        poison(key, "quickjs leg: TypedArray capture needs v8 zero-copy shared");
        return false;
    }
    let n_args = ir.get("args").and_then(|a| a.as_array()).map_or(0, |a| a.len());
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
                let snapshot_ok =
                    args[i].get("__ocOmpShared").is_some() || args[i].is_array();
                if !snapshot_ok {
                    poison(key, &format!("rw array slot {i} needs a snapshot (shared descriptor or plain array)"));
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
        Schedule::Static => team::Schedule { r#static: true, chunk: 1, num_threads },
        Schedule::Dynamic(c) => team::Schedule { r#static: false, chunk: c as usize, num_threads },
        Schedule::Auto => team::Schedule { r#static: false, chunk: 1, num_threads },
    };
    let t0 = std::time::Instant::now();
    let slots = match team::execute(global_team(), bundle, &args_json, shared, Arc::from(prng), &sched)
    {
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

    fn empty_shared() -> Arc<[protocol::SharedBuf]> {
        Arc::from(Vec::new())
    }
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
    fn schedule_text_parses_to_dispatch() {
        let s = |t: &str| parse_schedule(&serde_json::json!({ "schedule": t }));
        assert!(matches!(s("static,2"), Schedule::Static));
        assert!(matches!(s("dynamic,4"), Schedule::Dynamic(4)));
        assert!(matches!(s("dynamic"), Schedule::Dynamic(1)));
        assert!(matches!(s("guided"), Schedule::Auto));
        assert!(matches!(parse_schedule(&serde_json::json!({})), Schedule::Auto));
    }

    #[test]
    fn transform_emits_prep_guard() {
        // 带 ctx 成员调用的 bare 标注（OpenMP 缺省 default(shared)）：守卫
        // 照发；无 ctx capture 的循环由 omp_prep 毒化拒绝并打日志。
        let src = "// @omp parallel for shared(a, ctx)\nfor (let i = 0; i < 3; i += 1) {\n  ctx.beginPath();\n  ctx.arc(a, i, 1, 0, 6.283, false);\n  ctx.fill();\n}\n";
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
            assert!(omp_take(&mut store, &ir, &args, empty_shared(), &[], &mut writes, &mut reds));
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
        assert!(!omp_take(&mut store, &ir, &args, empty_shared(), &[], &mut writes, &mut reds));
        // 毒化后同 ir 二次 take 直接 false（缓存短路）。
        assert!(!omp_take(&mut store, &ir, &args, empty_shared(), &[], &mut writes, &mut reds));
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
        assert!(omp_take(&mut store, &ir, &[], empty_shared(), &[], &mut writes, &mut reds));
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
        assert!(!omp_take(&mut store, &ir, &[], empty_shared(), &[], &mut writes, &mut reds));
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
        assert!(omp_take(&mut store, &ir, &args, empty_shared(), &[], &mut writes, &mut reds));
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
        assert!(!omp_take(&mut store, &ir, &args, empty_shared(), &[], &mut writes, &mut reds));
        // 毒化后同 ir 二次 take 直接 false（缓存短路）。
        assert!(!omp_take(&mut store, &ir, &args, empty_shared(), &[], &mut writes, &mut reds));
    }

    /// rw 数组槽端到端（快照通道，2026-11-15 判据面删除后启用）：实参 =
    /// plain JSON 快照，每行从快照切片起步（行间零污染），Set 写穿 +
    /// flush 按行序产出终值。所有行都写下标 0 且读到的都是帧初快照
    /// ——若行首重置失效，值会依赖 worker 调度历史（测试即不稳定）。
    #[test]
    fn isolate_team_rw_array_snapshot_end_to_end() {
        let ir = serde_json::json!({
            "header": "let y = 0; y < 3; y += 1",
            "var": "y",
            "body": "  __ocOmpSet(out, 0, out[0] + y + 1);\n",
            "fns": {},
            "args": ["out"],
            "captures": { "out": "array_rw" }
        });
        assert!(omp_prep(&ir), "rw array loop must take the isolate path");
        let args = vec![serde_json::json!([10.0, 20.0, 30.0])];
        let run = || {
            let mut store = MutationStore::default();
            let mut writes = Vec::new();
            let mut reds = Vec::new();
            assert!(omp_take(&mut store, &ir, &args, empty_shared(), &[], &mut writes, &mut reds));
            writes
        };
        let a = run();
        let b = run();
        assert_eq!(a.len(), 3, "one write per row");
        for (ri, w) in a.iter().enumerate() {
            assert_eq!((w.arg, w.idx, w.acc), (0, 0, false));
            assert_eq!(w.val, 11.0 + ri as f64, "row {ri} must read the pristine snapshot");
        }
        assert_eq!(a, b, "two runs must be bit-identical");
    }

    /// rw 槽快照形态校验：null（读取失败的绑定产物）必须毒化。
    #[test]
    fn isolate_team_rw_array_bad_snapshot_poisons() {
        let ir = serde_json::json!({
            "header": "let y = 0; y < 2; y += 1",
            "var": "y",
            "body": "  __ocOmpSet(out, y, out[y]);\n",
            "fns": {},
            "args": ["out"],
            "captures": { "out": "array_rw" }
        });
        assert!(omp_prep(&ir));
        let args = vec![JsonValue::Null];
        let mut store = MutationStore::default();
        let mut writes = Vec::new();
        let mut reds = Vec::new();
        assert!(!omp_take(&mut store, &ir, &args, empty_shared(), &[], &mut writes, &mut reds));
        assert!(!omp_take(&mut store, &ir, &args, empty_shared(), &[], &mut writes, &mut reds));
    }

    /// 采集守卫（运行期机械面）：骨架按「每调用点每行一次」静态预采，
    /// 行体实际消费不符（条件分支少调用）= 采集流错位——行末计数不符
    /// 即毒化，绝不静默。超消费向由 __ocNext overrun 兜住（一并覆盖）。
    #[test]
    fn isolate_team_prng_consumption_guard_poisons() {
        // 少消费：段长 2（4 行 × 2 预采），行体只调一次。
        let under = serde_json::json!({
            "header": "let y = 0; y < 4; y += 1",
            "var": "y",
            "body": "  const t = random();\n  __ocOmpSet(out, y, t);\n",
            "fns": {},
            "args": ["random", "out"],
            "captures": { "random": "prng", "out": "array" }
        });
        assert!(omp_prep(&under));
        let args = vec![JsonValue::Null, serde_json::json!({ "__ocOmpArr": 1, "len": 4 })];
        let pool: Vec<f64> = (0..8).map(|i| i as f64).collect();
        let mut store = MutationStore::default();
        let mut writes = Vec::new();
        let mut reds = Vec::new();
        assert!(!omp_take(&mut store, &under, &args, empty_shared(), &pool, &mut writes, &mut reds));
        assert!(!omp_take(&mut store, &under, &args, empty_shared(), &pool, &mut writes, &mut reds));

        // 超消费：段长 1，行体调两次（__ocNext overrun throw）。
        let over = serde_json::json!({
            "header": "let y = 0; y < 4; y += 1",
            "var": "y",
            "body": "  const t = random() + random();\n  __ocOmpSet(out, y, t);\n",
            "fns": {},
            "args": ["random", "out"],
            "captures": { "random": "prng", "out": "array" }
        });
        assert!(omp_prep(&over));
        let args2 = vec![JsonValue::Null, serde_json::json!({ "__ocOmpArr": 1, "len": 4 })];
        let pool1: Vec<f64> = (0..4).map(|i| i as f64).collect();
        assert!(!omp_take(&mut store, &over, &args2, empty_shared(), &pool1, &mut writes, &mut reds));
    }

    /// ctx 词表外**方法**调用 = TypeError（plain 对象缺方法自然 throw，
    /// 零拦截开销）→ 行错误毒化；白名单静态判据已删（2026-11-15）。
    /// **属性**写不拦截（见 §6.3）——单独测一步钉死这个语义。
    #[test]
    fn isolate_team_ctx_unknown_member_poisons() {
        // 未知方法 ctx.stroke() → TypeError → 毒化。
        let ir = serde_json::json!({
            "header": "let y = 0; y < 2; y += 1",
            "var": "y",
            "body": "  ctx.beginPath();\n  ctx.arc(y, y, 2, 0, Math.PI * 2);\n  ctx.stroke();\n",
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
        assert!(!omp_take(&mut store, &ir, &args, empty_shared(), &[], &mut writes, &mut reds));
        assert!(!omp_take(&mut store, &ir, &args, empty_shared(), &[], &mut writes, &mut reds));

        // 属性写（ctx.lineWidth = 3）是静默 no-op，不毒化——拦截它需要
        // Proxy set 陷阱，实测 inspector 语料 +48% take wall，否决。
        let wr = serde_json::json!({
            "header": "let y = 0; y < 2; y += 1",
            "var": "y",
            "body": "  ctx.lineWidth = 3;\n  ctx.fillStyle = '#808080';\n  ctx.beginPath();\n  ctx.arc(y, y, 2, 0, Math.PI * 2);\n  ctx.fill();\n",
            "fns": {},
            "args": ["ctx"],
            "captures": { "ctx": "ctx" }
        });
        assert!(omp_prep(&wr));
        let mut writes2 = Vec::new();
        let mut reds2 = Vec::new();
        assert!(
            omp_take(&mut store, &wr, &args, empty_shared(), &[], &mut writes2, &mut reds2),
            "property writes must stay un-intercepted (zero per-op overhead)"
        );
    }

    /// T2 溶解的机械面（js-openmp.md §12）：break 指向已不存在的巢层
    /// （canonical 禁令）在 bundle 行函数作用域 = SyntaxError，bundle
    /// eval 失败毒化——无静态审计，机械成立。指向体内内层巢的 break
    /// 是合法 JS，语义照常（不经此路径）。
    #[test]
    fn isolate_team_break_out_of_nest_poisons_at_eval() {
        let ir = serde_json::json!({
            "header": "let y = 0; y < 2; y += 1",
            "var": "y",
            "body": "  if (y > 0) break;\n  __ocOmpSet(out, y, 1);\n",
            "fns": {},
            "args": ["out"],
            "captures": { "out": "array" }
        });
        assert!(omp_prep(&ir), "transform does not audit break; prep passes");
        let args = vec![serde_json::json!({ "__ocOmpArr": 0, "len": 2 })];
        let mut store = MutationStore::default();
        let mut writes = Vec::new();
        let mut reds = Vec::new();
        assert!(!omp_take(&mut store, &ir, &args, empty_shared(), &[], &mut writes, &mut reds));
        assert!(!omp_take(&mut store, &ir, &args, empty_shared(), &[], &mut writes, &mut reds));
    }
}
