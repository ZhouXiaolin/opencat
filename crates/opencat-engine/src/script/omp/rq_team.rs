//! rquickjs 适配层（js-openmp §7 trait 的 quickjs 实现）：每 worker 线程
//! 一个独立 Runtime+Context，线程独占（建/用/弃同线程；team 保证 worker
//! 永不跨线程）。与主 realm 的 `script/quickjs.rs`（RqJsContext）平行——
//! 本文件是 omp 面带 rquickjs 类型的唯一模块。数据面照 `v8_team.rs`：行
//! 调用（行值 + 令牌化实参 → flat slots 数组），无跨实例共享（数据全走
//! JSON/数值段拷贝）。与 V8 腿唯一差异 = 预采段用 plain packed Array 而
//! 非 Float64Array（bundle 模板只用 `length` 与 `buf[cur++]`，语义同形）。

use std::collections::HashMap;
use std::sync::Arc;

use rquickjs::{Array, Context, Ctx, Function, Object, Persistent, Runtime, TypedArray, Value, function::Args};

use super::protocol::{Bundle, FrameEnv, IsolateWrite, JsWorker, RowOut};
use crate::script::quickjs::{exception, from_json};

pub struct RqWorker {
    runtime: Runtime,
    context: Context,
    bundles: HashMap<String, BundleInst>,
}

struct BundleInst {
    header: Persistent<Function<'static>>,
    row: Persistent<Function<'static>>,
    set_caps: Persistent<Function<'static>>,
    set_prng: Persistent<Function<'static>>,
    /// 已装载的 capture 帧（args_json 的 Arc 指针指纹，v8_team 同款）：
    /// 同帧所有行共享同一 Arc，指针相等 = capture 已装载，跳过 set_caps
    /// 与 JSON 重解析；换帧（指针不同）才解析一次并按槽序赋值。Prng 槽
    /// 由 JS 侧模板强制赋 `__ocNext`（v_i 占位被忽略）。
    caps_key: Option<Arc<str>>,
}

impl RqWorker {
    pub fn new() -> Self {
        let runtime = Runtime::new().expect("rquickjs runtime");
        let context = Context::full(&runtime).expect("rquickjs context");
        Self { runtime, context, bundles: HashMap::new() }
    }
}

impl Default for RqWorker {
    fn default() -> Self {
        Self::new()
    }
}

/// 变参调用（rquickjs 的 `call` 只收定长 tuple，capture 实参数量不定 →
/// 手动装 `Args` 再 `call_arg`）。
fn call_variadic<'js, R: rquickjs::FromJs<'js>>(
    ctx: &Ctx<'js>,
    f: &Function<'js>,
    argv: Vec<Value<'js>>,
) -> Result<R, String> {
    let mut args = Args::new(ctx.clone(), argv.len());
    for a in argv {
        args.push_arg(a).map_err(|_| "arg push failed".to_string())?;
    }
    args.apply(f).map_err(|_| "call failed".to_string())
}

/// capture 装载：bundle 顶层 let 的帧值。指针指纹跳帧重装（v8_team 同款）。
fn ensure_caps<'a>(
    ctx: &Ctx<'_>,
    bundles: &'a mut HashMap<String, BundleInst>,
    bundle: &Bundle,
    args_json: &Arc<str>,
) -> Result<&'a mut BundleInst, String> {
    if let Some(inst) = bundles.get_mut(bundle.key.as_ref()) {
        if inst.caps_key.as_ref().is_some_and(|k| Arc::ptr_eq(k, args_json)) {
            return Ok(inst);
        }
    }
    let Some(inst) = bundles.get_mut(bundle.key.as_ref()) else {
        return Err("bundle not evaluated".into());
    };
    let set_caps = inst.set_caps.clone();
    let parsed = serde_json::from_str::<serde_json::Value>(args_json.as_ref())
        .map_err(|e| format!("args json parse failed: {e}"))?;
    let serde_json::Value::Array(items) = parsed else {
        return Err("args json not array".into());
    };
    let mut argv = Vec::with_capacity(items.len());
    for item in items {
        argv.push(from_json(ctx, item).map_err(|e| format!("arg convert failed: {e}"))?);
    }
    let f: Function = set_caps
        .restore(ctx)
        .map_err(|_| "setCaps restore failed".to_string())?;
    let _: Value = call_variadic(ctx, &f, argv).map_err(|_| exception(ctx, "setCaps").to_string())?;
    let inst = bundles.get_mut(bundle.key.as_ref()).expect("inst re-borrow");
    inst.caps_key = Some(args_json.clone());
    Ok(inst)
}

impl JsWorker for RqWorker {
    fn eval_bundle(&mut self, bundle: &Bundle) -> Result<(), String> {
        if self.bundles.contains_key(bundle.key.as_ref()) {
            return Ok(());
        }
        let key = bundle.key.to_string();
        let src = bundle.src.clone();
        let inst = self.context.with(|ctx| -> Result<BundleInst, String> {
            let value: Value = ctx
                .eval(src.as_ref())
                .map_err(|_| exception(&ctx, "bundle eval").to_string())?;
            let obj = Object::from_value(value).map_err(|_| "bundle result not object".to_string())?;
            let member = |name: &str| -> Result<Persistent<Function<'static>>, String> {
                let f: Function = obj
                    .get(name)
                    .map_err(|_| format!("bundle member `{name}` get failed"))?;
                Ok(Persistent::save(&ctx, f))
            };
            Ok(BundleInst {
                header: member("__ocHeader")?,
                row: member("__ocRow")?,
                set_caps: member("__ocSetCaps")?,
                set_prng: member("__ocSetPrng")?,
                caps_key: None,
            })
        })?;
        self.bundles.insert(key, inst);
        Ok(())
    }

    fn run_header(&mut self, bundle: &Bundle, env: &FrameEnv) -> Result<Vec<f64>, String> {
        let Self { context, bundles, .. } = self;
        context.with(|ctx| -> Result<Vec<f64>, String> {
            let inst = ensure_caps(&ctx, bundles, bundle, &env.args_json)?;
            let header: Function = inst
                .header
                .clone()
                .restore(&ctx)
                .map_err(|_| "header restore failed".to_string())?;
            let result: Value = header.call(()).map_err(|_| exception(&ctx, "header").to_string())?;
            let arr =
                Array::from_value(result).map_err(|_| "header result not array".to_string())?;
            let mut ys = Vec::with_capacity(arr.len());
            for item in arr.iter::<Value>() {
                let item = item.map_err(|_| "header elem get failed")?;
                let Some(n) = item.as_number() else {
                    return Err("header elem not number".into());
                };
                ys.push(n);
            }
            Ok(ys)
        })
    }

    fn run_row(
        &mut self,
        bundle: &Bundle,
        env: &FrameEnv,
        row: usize,
        row_val: f64,
    ) -> Result<RowOut, String> {
        let Self { context, bundles, .. } = self;
        context.with(|ctx| -> Result<RowOut, String> {
            let inst = ensure_caps(&ctx, bundles, bundle, &env.args_json)?;

            // 行预采段 → plain packed Array（拷贝行段；stride=0 跳过）。
            if env.stride > 0 {
                let lo = row.checked_mul(env.stride).ok_or("prng offset overflow")?;
                let hi = lo + env.stride;
                let slice = env.prng.get(lo..hi).ok_or("prng slice out of range")?;
                let arr = Array::new(ctx.clone()).map_err(|_| "prng array alloc failed")?;
                for (i, v) in slice.iter().enumerate() {
                    arr.set(i, *v).map_err(|_| "prng elem set failed")?;
                }
                let set_prng: Function = inst
                    .set_prng
                    .clone()
                    .restore(&ctx)
                    .map_err(|_| "setPrng restore failed".to_string())?;
                set_prng
                    .call::<_, Value>((arr,))
                    .map_err(|_| exception(&ctx, "setPrng").to_string())?;
            }

            // 行调用 = 单行值实参（capture 已在 IIFE 顶层 let，无需逐行传）。
            let row_fn: Function = inst
                .row
                .clone()
                .restore(&ctx)
                .map_err(|_| "row restore failed".to_string())?;
            let result: Value = row_fn
                .call((row_val,))
                .map_err(|_| exception(&ctx, "row").to_string())?;

            // 行产出 = 单段 Float64Array：头 [n_slots, n_ovals] + slots 四元组
            // （arg/idx/acc/val）+ ovals 七元组。
            let typed = TypedArray::<f64>::from_value(result)
                .map_err(|_| "row result not f64array".to_string())?;
            let bytes = unsafe { typed.as_bytes().unwrap_or_default() };
            if bytes.len() < 16 {
                return Err("row result missing header".into());
            }
            let vals: Vec<f64> = bytes
                .chunks_exact(8)
                .map(|b| f64::from_le_bytes(b.try_into().unwrap()))
                .collect();
            let n_slots = vals[0] as usize;
            let n_ovals = vals[1] as usize;
            if 2 + n_slots * 4 + n_ovals * 7 != vals.len() {
                return Err("row segments malformed".into());
            }
            let mut writes = Vec::with_capacity(n_slots);
            for q in vals[2..2 + n_slots * 4].chunks_exact(4) {
                writes.push(IsolateWrite {
                    arg: q[0] as usize,
                    idx: q[1] as u32,
                    acc: q[2] != 0.0,
                    val: q[3],
                });
            }
            let mut ovals = Vec::with_capacity(n_ovals * 7);
            ovals.extend_from_slice(&vals[2 + n_slots * 4..]);
            Ok(RowOut { writes, ovals })
        })
    }
}

/// 进程级常驻团队（惰性：首次 isolate 派发时起线程池）。worker 数 =
/// CPU 物理核数；引擎切换点 = 这个工厂（js-v8 → v8_team.rs / js-quickjs →
/// 本文件）。
pub fn global_team() -> &'static super::team::Team {
    static TEAM: std::sync::LazyLock<super::team::Team> = std::sync::LazyLock::new(|| {
        // 物理核而非逻辑核：SMT 满载时每行被超线程打包+降频拖慢（v8_team
        // 实测 ~4.5x）——worker 数钉物理核。
        let n = num_cpus::get_physical();
        super::team::Team::new(n, || Box::new(RqWorker::new()))
    });
    &TEAM
}
