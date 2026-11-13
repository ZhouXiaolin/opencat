//! `@omp` 通用循环派发 runtime：解析+编译缓存 → args 解码 → rayon 行并行
//! → op 流直推。
//!
//! 全链确定性三保证：rayon indexed `par_iter().map().collect()` 保序产出
//! （alpha 叠层敏感）；f64 运算无归约无 race（行间零共享可变状态）；同
//! 参数两跑逐位一致（帧=f(t) 不变）。任何失败（编译/解码/头部枚举/行执
//! 行/采集数不整除）都毒化缓存条目 → 后续帧 prep 直接 false → 骨架不再
//! 跑 → fallback 循环语义纯净（PRNG 不被无谓消耗）。

pub mod compile;
pub mod parse;
pub mod transform;
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

struct CacheEntry {
    program: Arc<Program>,
    poisoned: bool,
    /// SoA 向量执行面（prepare 失败 = None → 标量 VM 兜底）。
    soa: Option<Arc<vm_soa::SoaPlan>>,
}

static CACHE: LazyLock<Mutex<HashMap<String, CacheEntry>>> =
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
    compile::compile(loop_src, &fns, &args, &arg_kinds)
}

fn poison(key: &str, reason: &str) {
    let mut cache = cache_lock();
    if let Some(e) = cache.get_mut(key) {
        e.poisoned = true;
    }
    eprintln!("[omp] take failed, poisoned: {reason}");
}

/// `__ocOmpPrep(ir)` — 编译并缓存。true = 骨架可跑；false = 本帧与后续帧
/// 全部降级（毒化条目直接短路，不再重编译）。`OC_OMP_DISABLE=1` 强制
/// 全降级（seg 对拍的 JS 腿用）。
pub fn omp_prep(ir: &JsonValue) -> bool {
    if std::env::var("OC_OMP_DISABLE").ok().as_deref() == Some("1") {
        return false;
    }
    let key = ir.to_string();
    let mut cache = cache_lock();
    if let Some(e) = cache.get(&key) {
        return !e.poisoned;
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
            });
            true
        }
        Err(e) => {
            cache.insert(key, CacheEntry {
                program: Arc::new(Program::empty()),
                poisoned: true,
                soa: None,
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
) -> bool {
    let key = ir.to_string();
    let (prog, soa) = {
        let cache = cache_lock();
        let Some(entry) = cache.get(&key) else {
            return false;
        };
        if entry.poisoned {
            return false;
        }
        (entry.program.clone(), entry.soa.clone())
    };
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

    // rayon indexed 行并行：保序产出（alpha 叠层敏感），行间零共享。
    let take_t0 = std::time::Instant::now();
    let rows: Vec<vm::FacetState> = match backend {
        Backend::Soa { plan, n_inner } => match (0..n_outer)
            .into_par_iter()
            .map(|ri| {
                let mut g = globals.clone();
                g[loop_var_slot] = Value::Num(ys[ri]);
                let row_prng = &prng[ri * stride..(ri + 1) * stride];
                let mut facet = vm::FacetState::default();
                vm_soa::run_row(&prog, plan, g, row_prng, n_inner, &mut facet)?;
                Ok(facet)
            })
            .collect::<Result<Vec<_>, String>>()
        {
            Ok(r) => r,
            Err(e) => {
                poison(&key, &format!("soa row exec: {e}"));
                return false;
            }
        },
        Backend::Vm => match (0..n_outer)
            .into_par_iter()
            .map(|ri| {
                let mut g = globals.clone();
                g[loop_var_slot] = Value::Num(ys[ri]);
                let row_prng: &[f64] = if stride == 0 {
                    &[]
                } else {
                    &prng[ri * stride..(ri + 1) * stride]
                };
                vm::run_row(&prog, g, row_prng)
            })
            .collect::<Result<Vec<_>, String>>()
        {
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
        // 带 ctx 成员调用（无 ctx 的标注在 transform 期就跳过——native
        // 没有落点，骨架还会消耗 PRNG）。
        let src = "// @omp parallel for args(a, ctx)\nfor (let i = 0; i < 3; i += 1) {\n  ctx.fillRect(a, i, 1, 1);\n}\n";
        let out = transform::transform(src);
        let s = &*out;
        assert!(s.contains("__ocOmpPrep("));
        assert!(s.contains("__ocOmpTake("));
        assert!(s.contains("for (let i = 0; i < 3; i += 1) {"));
    }
}
