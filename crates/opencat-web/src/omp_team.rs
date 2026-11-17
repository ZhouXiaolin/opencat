//! web 团队腿：core `OmpTeam` 数据面委托浏览器侧 `__ocOmpExecute`（TS
//! runner，先主线程单行派发；worker + SAB 版迁入时同一合同不变）。
//!
//! 行产出 = 合并 Float64Array（每行单段：头 `[n_slots, n_ovals]` + slots
//! 四元组（arg/idx/acc/val）+ ovals 七元组），此处按段头切分为
//! `Vec<RowOut>`——切分语义与 engine `v8_team::run_row` 逐字段同码。

use std::sync::Arc;

use wasm_bindgen::{JsCast, JsValue};

use opencat_core::script::omp::protocol::{self, SharedKind};
use opencat_core::script::omp::{OmpTeam, TeamSched};

pub struct WebOmpTeam;

/// SharedKind → TS runner 的 kind 码。码即判别值（protocol.rs `SharedKind::code`，
/// TS 表由 schema_gen 生成的 omp-schema.generated.ts 消费——不再手抄表序）。
fn kind_code(k: SharedKind) -> u32 {
    k.code()
}

fn js_err_string(v: &JsValue) -> String {
    v.as_string().unwrap_or_else(|| format!("{v:?}"))
}

impl OmpTeam for WebOmpTeam {
    // TS runner 侧 rebuild_shared 支持描述符重建（omp-runner.ts 递归替换
    // `{"__ocOmpShared": {slot, len}}` 为真 TypedArray）。
    fn supports_shared(&self) -> bool {
        true
    }

    fn execute(
        &self,
        bundle: &protocol::Bundle,
        args_json: &Arc<str>,
        shared: Arc<[protocol::SharedBuf]>,
        prng: Arc<[f64]>,
        sched: &TeamSched,
    ) -> Result<Vec<protocol::RowOut>, String> {
        let global = js_sys::global();
        let f = js_sys::Reflect::get(&global, &JsValue::from_str("__ocOmpExecute"))
            .map_err(|_| "omp runner missing (__ocOmpExecute)")?;
        let f = f
            .dyn_into::<js_sys::Function>()
            .map_err(|_| "__ocOmpExecute is not a function")?;

        let args = js_sys::Array::new();
        args.push(&JsValue::from_str(bundle.src.as_ref()));
        args.push(&JsValue::from_str(bundle.key.as_ref()));
        args.push(&JsValue::from_str(args_json.as_ref()));
        let list = js_sys::Array::new();
        let kinds = js_sys::Array::new();
        for buf in shared.iter() {
            list.push(&js_sys::Uint8Array::from(&buf.data[..]));
            kinds.push(&JsValue::from(kind_code(buf.kind)));
        }
        args.push(&list.into());
        args.push(&kinds.into());
        args.push(&js_sys::Float64Array::from(&prng[..]));
        args.push(&JsValue::from_bool(sched.r#static));
        args.push(&JsValue::from(sched.chunk as u32));
        args.push(&sched.num_threads.map_or(JsValue::NULL, |n| n.into()));

        // 失败 throw → Err（调用方毒化降级，原循环照跑）。（9 个实参超出
        // JsArgs 定参面，走 Reflect.apply。）
        let ret = js_sys::Reflect::apply(&f, &JsValue::UNDEFINED, &args)
            .map_err(|e| format!("__ocOmpExecute: {}", js_err_string(&e)))?;
        let merged = ret
            .dyn_into::<js_sys::Float64Array>()
            .map_err(|_| "__ocOmpExecute returned no Float64Array")?;
        let mut vals = vec![0f64; merged.length() as usize];
        merged.copy_to(&mut vals);

        // 按段头切分（引擎 run_row 同校验：`2 + n*4 + m*7` 恰好耗尽段）。
        let mut rows = Vec::new();
        let mut i = 0usize;
        while i < vals.len() {
            if vals.len() - i < 2 {
                return Err("omp rows: truncated segment header".into());
            }
            let n_slots = vals[i] as usize;
            let n_ovals = vals[i + 1] as usize;
            let seg = 2 + n_slots * 4 + n_ovals * 7;
            if vals.len() - i < seg {
                return Err("omp rows: truncated segment".into());
            }
            let mut writes = Vec::with_capacity(n_slots);
            for q in vals[i + 2..i + 2 + n_slots * 4].chunks_exact(4) {
                writes.push(protocol::IsolateWrite {
                    arg: q[0] as usize,
                    idx: q[1] as u32,
                    acc: q[2] != 0.0,
                    val: q[3],
                });
            }
            let mut ovals = Vec::with_capacity(n_ovals * 7);
            ovals.extend_from_slice(&vals[i + 2 + n_slots * 4..i + seg]);
            rows.push(protocol::RowOut { writes, ovals });
            i += seg;
        }
        Ok(rows)
    }
}
