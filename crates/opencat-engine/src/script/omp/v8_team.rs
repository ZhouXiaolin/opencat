//! V8 适配层（js-openmp §7 trait 的 v8 实现）：每 worker 线程一个独立
//! isolate，线程独占（建/用/弃同线程，无共享无锁——OwnedIsolate 的
//! !Send 恰好把这条纪律钉进类型系统）。与主 realm 的 `script/v8.rs`
//! （V8JsContext）平行——本文件是 omp 面唯一带 v8 类型的模块，切换引擎 =
//! 重新实现 `protocol::JsWorker` 并让 `global_team()` 指向新工厂
//! （quickjs 版见 `rq_team.rs`）。

use std::collections::HashMap;

use v8::PinScope;

use super::protocol::{Bundle, FrameEnv, IsolateWrite, JsWorker, RowOut, SharedBuf, SharedKind};
use crate::script::v8::ensure_v8_initialized;

/// 从活的 TryCatch 提取 `message\nstack`（v8.rs 同款；宏化因为 TryCatch
/// 的 isolate 绑定类型不可具名——展开在调用点保持 scope 借用完整）。
macro_rules! caught {
    ($tc:expr, $context:expr) => {{
        let tc = $tc;
        let exception = tc
            .exception()
            .map(|e| e.to_rust_string_lossy(tc))
            .unwrap_or_else(|| "uncaught JavaScript exception".to_string());
        let stack = tc
            .stack_trace()
            .map(|s| s.to_rust_string_lossy(tc))
            .unwrap_or_default();
        if stack.is_empty() {
            format!("{}: {}", $context, exception)
        } else {
            format!("{}: {}", $context, stack)
        }
    }};
}

pub struct V8Worker {
    isolate: v8::OwnedIsolate,
    context: v8::Global<v8::Context>,
    bundles: HashMap<String, BundleInst>,
}

struct BundleInst {
    header: v8::Global<v8::Function>,
    row: v8::Global<v8::Function>,
    set_caps: v8::Global<v8::Function>,
    next: v8::Global<v8::Function>,
    set_prng: v8::Global<v8::Function>,
    /// 已装载的 capture 帧（args_json 的 Arc 指针指纹）：同帧所有行共享
    /// 同一 Arc，指针相等 = capture 已装载，跳过 set_caps + JSON 重解析。
    caps_key: Option<std::sync::Arc<str>>,
}

impl V8Worker {
    pub fn new() -> Self {
        ensure_v8_initialized();
        let mut isolate = v8::Isolate::new(Default::default());
        let context = {
            v8::scope!(let handle_scope, &mut isolate);
            let ctx = v8::Context::new(handle_scope, Default::default());
            v8::Global::new(handle_scope, ctx)
        };
        Self { isolate, context, bundles: HashMap::new() }
    }
}

impl Default for V8Worker {
    fn default() -> Self {
        Self::new()
    }
}

impl JsWorker for V8Worker {
    fn eval_bundle(&mut self, bundle: &Bundle) -> Result<(), String> {
        if self.bundles.contains_key(bundle.key.as_ref()) {
            return Ok(());
        }
        let context = self.context.clone();
        let Self { isolate, bundles, .. } = self;
        v8::scope!(let handle_scope, isolate);
        let context = v8::Local::new(handle_scope, &context);
        let scope = &mut v8::ContextScope::new(handle_scope, context);
        let Some(src) = v8::String::new(scope, bundle.src.as_ref()) else {
            return Err("bundle string alloc failed".into());
        };
        v8::tc_scope!(let tc, scope);
        let Some(script) = v8::Script::compile(tc, src, None) else {
            return Err(caught!(tc, "bundle compile"));
        };
        let Some(value) = script.run(tc) else {
            return Err(caught!(tc, "bundle eval"));
        };
        let Ok(obj) = v8::Local::<v8::Object>::try_from(value) else {
            return Err("bundle result not object".into());
        };
        let member = |name: &str| -> Result<v8::Global<v8::Function>, String> {
            let Some(key) = v8::String::new(tc, name) else {
                return Err("member key alloc failed".into());
            };
            let Some(v) = obj.get(tc, key.into()) else {
                return Err(format!("bundle member `{name}` get failed"));
            };
            let Ok(f) = v8::Local::<v8::Function>::try_from(v) else {
                return Err(format!("bundle member `{name}` not function"));
            };
            Ok(v8::Global::new(tc, f))
        };
        let header = member("__ocHeader")?;
        let row = member("__ocRow")?;
        let set_caps = member("__ocSetCaps")?;
        let next = member("__ocNext")?;
        let set_prng = member("__ocSetPrng")?;
        bundles.insert(
            bundle.key.to_string(),
            BundleInst { header, row, set_caps, next, set_prng, caps_key: None },
        );
        Ok(())
    }

    fn run_header(&mut self, bundle: &Bundle, env: &FrameEnv) -> Result<Vec<f64>, String> {
        let context = self.context.clone();
        let Self { isolate, bundles, .. } = self;
        v8::scope!(let handle_scope, isolate);
        let context = v8::Local::new(handle_scope, &context);
        let scope = &mut v8::ContextScope::new(handle_scope, context);
        let inst = ensure_caps(bundles, bundle, scope, env)?;

        let header = v8::Local::new(scope, &inst.header);
        v8::tc_scope!(let tc, scope);
        let Some(result) = header.call(tc, v8::undefined(tc).into(), &[]) else {
            return Err(caught!(tc, "header"));
        };
        let Ok(out) = v8::Local::<v8::Array>::try_from(result) else {
            return Err("header result not array".into());
        };
        let mut ys = Vec::with_capacity(out.length() as usize);
        for i in 0..out.length() {
            let Some(v) = out.get_index(tc, i) else {
                return Err("header elem missing".into());
            };
            let Some(n) = v.number_value(tc) else {
                return Err("header elem not number".into());
            };
            ys.push(n);
        }
        Ok(ys)
    }

    fn run_row(
        &mut self,
        bundle: &Bundle,
        env: &FrameEnv,
        row: usize,
        row_val: f64,
    ) -> Result<RowOut, String> {
        let context = self.context.clone();
        let Self { isolate, bundles, .. } = self;
        v8::scope!(let handle_scope, isolate);
        let context = v8::Local::new(handle_scope, &context);
        let scope = &mut v8::ContextScope::new(handle_scope, context);
        let inst = ensure_caps(bundles, bundle, scope, env)?;

        // 行预采段 → Float64Array（拷贝行段；stride=0 跳过）。
        let mut prng_row = None;
        if env.stride > 0 {
            let lo = row.checked_mul(env.stride).ok_or("prng offset overflow")?;
            let hi = lo + env.stride;
            let slice = env.prng.get(lo..hi).ok_or("prng slice out of range")?;
            let mut bytes = Vec::with_capacity(slice.len() * 8);
            for v in slice {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            let bs = v8::ArrayBuffer::new_backing_store_from_vec(bytes);
            let ab = v8::ArrayBuffer::with_backing_store(scope, &bs.make_shared());
            let Some(ta) = v8::Float64Array::new(scope, ab, 0, env.stride) else {
                return Err("prng f64array alloc failed".into());
            };
            prng_row = Some(ta);
        }
        if let Some(ta) = prng_row {
            let set_prng = v8::Local::new(scope, &inst.set_prng);
            if set_prng
                .call(scope, v8::undefined(scope).into(), &[ta.into()])
                .is_none()
            {
                return Err("setPrng threw".into());
            }
        }

        // 行调用 = 单行值实参（capture 已在 IIFE 顶层 let，无需逐行传）。
        let row_fn = v8::Local::new(scope, &inst.row);
        let args = [v8::Number::new(scope, row_val).into()];
        v8::tc_scope!(let tc, scope);
        let Some(result) = row_fn.call(tc, v8::undefined(tc).into(), &args) else {
            return Err(caught!(tc, "row"));
        };

        // 行产出 = 单段 Float64Array：头 [n_slots, n_ovals] + slots 四元组
        // （arg/idx/acc/val）+ ovals 七元组。copy_contents 一次 memcpy 直读
        // backing store，无逐元素跨 API。
        let Ok(out) = v8::Local::<v8::Float64Array>::try_from(result) else {
            return Err("row result not f64array".into());
        };
        let n = out.length() as usize;
        if n < 2 {
            return Err("row result missing header".into());
        }
        let mut bytes = vec![0u8; n * 8];
        let copied = v8::ArrayBufferView::copy_contents(&out, &mut bytes);
        let vals: Vec<f64> = bytes[..copied]
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
    }
}

/// capture 装载：bundle 顶层 let 的帧值。同一帧的所有行共享同一
/// `Arc<str>`（args_json）——指针相等即已装载，跳过 set_caps 与 JSON
/// 重解析；换帧（指针不同）才解析一次并按槽序赋值。Prng 槽由 JS 侧
/// 模板强制赋 `__ocNext`（v_i 占位被忽略）。装载时解析树里的 TypedArray
/// 描述符（`{"__ocOmpShared": {slot, len}}`）由 [`rebuild_shared`] 从
/// `env.shared` 换成真 TypedArray。
fn ensure_caps<'a>(
    bundles: &'a mut HashMap<String, BundleInst>,
    bundle: &Bundle,
    scope: &mut PinScope<'_, '_>,
    env: &FrameEnv,
) -> Result<&'a mut BundleInst, String> {
    let Some(inst) = bundles.get_mut(bundle.key.as_ref()) else {
        return Err("bundle not evaluated".into());
    };
    if inst
        .caps_key
        .as_ref()
        .is_some_and(|k| std::sync::Arc::ptr_eq(k, &env.args_json))
    {
        return Ok(inst);
    }
    let Some(s) = v8::String::new(scope, env.args_json.as_ref()) else {
        return Err("args string alloc failed".into());
    };
    let Some(parsed) = v8::json::parse(scope, s) else {
        return Err("args json parse failed".into());
    };
    let Ok(arr) = v8::Local::<v8::Array>::try_from(parsed) else {
        return Err("args json not array".into());
    };
    let mut args: Vec<v8::Local<v8::Value>> = Vec::with_capacity(arr.length() as usize);
    for i in 0..arr.length() {
        let Some(v) = arr.get_index(scope, i) else {
            return Err(format!("arg {i} missing"));
        };
        let v = rebuild_shared(scope, v, &env.shared)?;
        args.push(v);
    }
    let set_caps = v8::Local::new(scope, &inst.set_caps);
    v8::tc_scope!(let tc, scope);
    if set_caps
        .call(tc, v8::undefined(tc).into(), &args)
        .is_none()
    {
        return Err(caught!(tc, "setCaps"));
    }
    let inst = bundles.get_mut(bundle.key.as_ref()).expect("inst re-borrow");
    inst.caps_key = Some(env.args_json.clone());
    Ok(inst)
}

/// 解析树里的 TypedArray 描述符 → 从 `env.shared` 重建真 TypedArray：
/// per-worker 字节克隆 + 独立 backing store（v1 快照语义——行函数对 TA
/// 的写不出 worker，写回只有 Set/Acc 令牌通道；f64 8 字节对齐由分配器
/// 保证）。非描述符的容器递归替换成员；描述符字段异常 = 行错误 → 毒化。
fn rebuild_shared<'s>(
    scope: &mut PinScope<'s, '_>,
    v: v8::Local<'s, v8::Value>,
    shared: &[SharedBuf],
) -> Result<v8::Local<'s, v8::Value>, String> {
    if !v.is_object() {
        return Ok(v);
    }
    let Ok(obj) = v8::Local::<v8::Object>::try_from(v) else {
        return Err("shared: object cast".into());
    };
    let key = v8::String::new(scope, "__ocOmpShared").ok_or("shared: key alloc")?;
    let Some(dv) = obj.get(scope, key.into()) else {
        return Err("shared: desc get".into());
    };
    if !dv.is_undefined() {
        // 描述符节点：字段全由 walker 产出（JSON.parse 面无非 own 键），
        // 直接解，不猜。
        let Ok(d) = v8::Local::<v8::Object>::try_from(dv) else {
            return Err("shared: desc not object".into());
        };
        let num = |name: &str| -> Result<usize, String> {
            let k = v8::String::new(scope, name).ok_or("shared: key alloc")?;
            let val = d.get(scope, k.into()).ok_or("shared: field get")?;
            val.number_value(scope)
                .map(|n| n as usize)
                .ok_or_else(|| format!("shared: {name} not number"))
        };
        let slot = num("slot")?;
        let len = num("len")?;
        let buf = shared.get(slot).ok_or("shared: slot out of range")?;
        if buf.data.len() != len * buf.kind.elem_size() {
            return Err("shared: byte size mismatch".into());
        }
        let bytes: Box<[u8]> = buf.data.to_vec().into_boxed_slice();
        let bs = v8::ArrayBuffer::new_backing_store_from_boxed_slice(bytes);
        let ab = v8::ArrayBuffer::with_backing_store(scope, &bs.make_shared());
        let ta: v8::Local<v8::Value> = match buf.kind {
            SharedKind::F64 => v8::Float64Array::new(scope, ab, 0, len)
                .ok_or("shared: f64 view alloc")?
                .into(),
            SharedKind::F32 => v8::Float32Array::new(scope, ab, 0, len)
                .ok_or("shared: f32 view alloc")?
                .into(),
            SharedKind::U8Clamped => v8::Uint8ClampedArray::new(scope, ab, 0, len)
                .ok_or("shared: u8c view alloc")?
                .into(),
            SharedKind::U8 => v8::Uint8Array::new(scope, ab, 0, len)
                .ok_or("shared: u8 view alloc")?
                .into(),
            SharedKind::I8 => v8::Int8Array::new(scope, ab, 0, len)
                .ok_or("shared: i8 view alloc")?
                .into(),
            SharedKind::U16 => v8::Uint16Array::new(scope, ab, 0, len)
                .ok_or("shared: u16 view alloc")?
                .into(),
            SharedKind::I16 => v8::Int16Array::new(scope, ab, 0, len)
                .ok_or("shared: i16 view alloc")?
                .into(),
            SharedKind::U32 => v8::Uint32Array::new(scope, ab, 0, len)
                .ok_or("shared: u32 view alloc")?
                .into(),
            SharedKind::I32 => v8::Int32Array::new(scope, ab, 0, len)
                .ok_or("shared: i32 view alloc")?
                .into(),
        };
        return Ok(ta);
    }
    if v.is_array() {
        let Ok(arr) = v8::Local::<v8::Array>::try_from(v) else {
            return Err("shared: array cast".into());
        };
        for i in 0..arr.length() {
            let Some(e) = arr.get_index(scope, i) else {
                return Err("shared: elem get".into());
            };
            if e.is_object() {
                let r = rebuild_shared(scope, e, shared)?;
                if arr.set_index(scope, i, r).is_none() {
                    return Err("shared: elem set".into());
                }
            }
        }
        return Ok(v);
    }
    // plain object：JSON.parse 产物，own enumerable 键逐一换成员。
    let Some(names) = obj.get_own_property_names(
        scope,
        v8::GetPropertyNamesArgs {
            mode: v8::KeyCollectionMode::OwnOnly,
            ..Default::default()
        },
    ) else {
        return Err("shared: own names".into());
    };
    for i in 0..names.length() {
        let Some(nv) = names.get_index(scope, i) else {
            return Err("shared: name get".into());
        };
        let Some(val) = obj.get(scope, nv) else {
            return Err("shared: member get".into());
        };
        if val.is_object() {
            let r = rebuild_shared(scope, val, shared)?;
            if obj.set(scope, nv, r).is_none() {
                return Err("shared: member set".into());
            }
        }
    }
    Ok(v)
}

/// 进程级常驻团队（惰性：首次 isolate 派发时起线程池）。worker 数 =
/// CPU 物理核数；引擎切换点 = 这个工厂（js-v8 → 本文件 / js-quickjs →
/// rq_team.rs——见 Cargo.toml features 注释）。
pub fn global_team() -> &'static super::team::Team {
    static TEAM: std::sync::LazyLock<super::team::Team> = std::sync::LazyLock::new(|| {
        // 物理核而非逻辑核：8C/16T 机器上 16 isolate 满载时每行被 SMT
        // 打包+降频拖慢 ~4.5x（纯 Rust 对照同现象，非引擎锁）——worker
        // 数钉物理核实测最优（smallpt t8 3.61s vs t16 3.85s）。
        let n = num_cpus::get_physical();
        super::team::Team::new(n, || Box::new(V8Worker::new()))
    });
    &TEAM
}
