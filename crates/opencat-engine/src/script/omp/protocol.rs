//! isolate 团队后端的引擎无关协议面。
//!
//! 本模块是 js-openmp §7 worker 适配面的 Rust 侧定义：host 与「每线程
//! 一个独立引擎实例」之间的消息协议。**不 import 任何具体引擎类型**
//! （quickjs / Hermes 实现者只需为本文件的 trait 提供适配层）——引擎
//! 无关性由这条机械边界保证。数据面 = 结构化克隆同构（number/string/
//! bool/null/plain array/plain object），无跨实例共享对象（SAB 不进
//! 协议，见 docs/js-openmp.md §2）。

use std::sync::Arc;

use serde_json::Value as JsonValue;

/// 一次数组写回（Set/Acc 通道的行产出条目）。
///
/// `arg` 是 capture 名单中的槽位（绑定层把该槽的实参换成数组令牌，应用
/// 时按槽位取回真数组对象）。`acc=false` 语义 = 赋值（`c[i]=v`）；
/// `acc=true` = 对帧初值的累加（`c[i]+=v`，TR15 §8 array-section
/// reduction、initializer=0 的映射，见 docs/js-openmp.md §6.2）。v1 值域
/// 限 f64（worker 产出非数 = 行错误 → 毒化）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IsolateWrite {
    pub arg: usize,
    pub idx: u32,
    pub acc: bool,
    pub val: f64,
}

/// 一行的产出：数组写回条目 + ctx 垫片的 oval 段。按行序应用——完成序
/// 无关，字节确定。
#[derive(Debug, Default)]
pub struct RowOut {
    pub writes: Vec<IsolateWrite>,
    /// ctx 垫片产出：7 f64 / oval（cx, cy, r, cr, cg, cb, ca）。后四者
    /// 已是 0-255 量化终值（`Math.round` 在 worker JS 侧完成，与 JS 腿
    /// 垫片 `batchColor` 同码同值）；host 侧只做 rect 变换 + f32/u8 组包。
    pub ovals: Vec<f64>,
}

/// 一次 execute 的帧环境（每帧构造一次，全团队共享只读）。
pub struct FrameEnv {
    /// plain/ctx/array/prng 参数已令牌化的 JSON 数组文本（绑定层产出；
    /// array 槽 = `{"__ocOmpArr": k}`，prng 槽 = null）。
    pub args_json: Arc<str>,
    /// 预采池。行 i 的段 = `[i*stride, (i+1)*stride)`；stride=0 = 无采集。
    pub prng: Arc<[f64]>,
    pub stride: usize,
}

/// capture 槽位类（与 ir args 序对齐）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SlotKind {
    /// 原值（args_json 里的 JSON 值直进 worker）。
    Plain,
    /// 零参 PRNG——worker 把该槽绑到预采游标函数。
    Prng,
    /// Set/Acc 数组——worker 把该槽绑到令牌对象（`__ocOmpArr` = 槽位）。
    Array,
    /// canvas 2D 上下文——worker 把该槽绑到垫片 recorder（行内记
    /// fillStyle/满圈 arc，fill 时量化入 ovals 段；实参 JSON 被忽略）。
    Ctx,
}

/// worker bundle：行函数 + 页面函数闭包 + shim，一次 eval、多帧复用。
pub struct Bundle {
    /// ir 全文（bundle 缓存键）。
    pub key: Arc<str>,
    /// bundle 源码（IIFE，返回 {__ocHeader, __ocRow, __ocNext, __ocSetPrng}）。
    pub src: Arc<str>,
    /// 与 args 序对齐的槽位类。
    pub kinds: Arc<[SlotKind]>,
}

/// 引擎适配面（js-openmp §7）：每个 worker 线程持有独立引擎实例。
/// 所有方法只在 owning 线程上被调用（team 保证）——engine 实例普遍
/// 线程绑定（V8 OwnedIsolate !Send 是纪律而非缺陷），所以**没有 Send
/// 上界**：worker 永不跨线程移动。
pub trait JsWorker {
    /// eval bundle（按 bundle.key 缓存；重复 eval 同 key = 空操作）。
    /// 失败 = bundle 语法/形态问题，整条标注毒化。
    fn eval_bundle(&mut self, bundle: &Bundle) -> Result<(), String>;
    /// 外层 header 枚举（行值列表）。
    fn run_header(&mut self, bundle: &Bundle, env: &FrameEnv) -> Result<Vec<f64>, String>;
    /// 执行一行：行值 row_val，预采段 [row*stride, (row+1)*stride)。
    /// 返回该行 Set/Acc 产出（行内顺序保留）。
    fn run_row(
        &mut self,
        bundle: &Bundle,
        env: &FrameEnv,
        row: usize,
        row_val: f64,
    ) -> Result<RowOut, String>;
}

/// 由 ir 组装 bundle（omp_prep 期，纯结构变换——不走 opcode 编译器，
/// 递归/完整 JS 语义不受 parse.rs 白名单限制）。失败 = ir 结构面超
/// isolate 子集。
pub fn build_bundle(ir: &JsonValue) -> Result<Bundle, String> {
    let args = ir
        .get("args")
        .and_then(|v| v.as_array())
        .ok_or("args missing")?;
    let captures = ir.get("captures").and_then(|v| v.as_object()).unwrap_or(&serde_json::Map::new()).clone();
    let mut kinds = Vec::with_capacity(args.len());
    let mut params = Vec::with_capacity(args.len());
    for (k, name) in args.iter().enumerate() {
        let name = name.as_str().ok_or("args item not a string")?;
        let kind = match captures.get(name).and_then(|v| v.as_str()) {
            Some("array") => SlotKind::Array,
            Some("prng") => SlotKind::Prng,
            Some("ctx") => SlotKind::Ctx,
            _ => SlotKind::Plain,
        };
        kinds.push(kind);
        params.push(name.to_string());
    }
    let header = ir.get("header").and_then(|v| v.as_str()).ok_or("header missing")?;
    let var = ir.get("var").and_then(|v| v.as_str()).ok_or("var missing")?;
    let body = ir.get("body").and_then(|v| v.as_str()).ok_or("body missing")?;
    if var.is_empty() {
        return Err("loop var not parsed".into());
    }
    let fns = ir
        .get("fns")
        .and_then(|v| v.as_object())
        .ok_or("fns missing")?;
    let mut fn_src = String::new();
    for (name, src) in fns {
        let src = src.as_str().ok_or("fn src not a string")?;
        fn_src.push_str(src);
        fn_src.push('\n');
    }
    let private = ir
        .get("private")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .map(|s| format!("let {s};\n"))
                .collect::<String>()
        })
        .unwrap_or_default();
    // reduction 面（TR15 §8.10 映射，docs/js-openmp.md §3.4）：行私有
    // 副本按 initializer 起值（+→0、*→1），行末产出终值槽（arg=1000+k，
    // k = ir.reduction 展开序）；host 按行序左折叠副本们（首副本起）。
    let mut red_vars: Vec<String> = Vec::new();
    let mut red_init_src = String::new();
    let mut red_emit_src = String::new();
    if let Some(rs) = ir.get("reduction").and_then(|v| v.as_array()) {
        for r in rs {
            let Some(op) = r.get(0).and_then(|v| v.as_str()) else {
                return Err("reduction op missing".into());
            };
            let init = match op {
                "+" => 0.0,
                "*" => 1.0,
                _ => return Err(format!("reduction op `{op}` not supported")),
            };
            let Some(names) = r.get(1).and_then(|v| v.as_array()) else {
                return Err("reduction list missing".into());
            };
            for n in names {
                let Some(n) = n.as_str() else {
                    return Err("reduction name not a string".into());
                };
                let k = red_vars.len();
                red_vars.push(n.to_string());
                red_init_src.push_str(&format!("  {n} = {init};\n"));
                red_emit_src.push_str(&format!(
                    "    __ocSlots[__ocN] = {}; __ocSlots[__ocN + 1] = 0; __ocSlots[__ocN + 2] = 0; __ocSlots[__ocN + 3] = {n}; __ocN += 4;\n",
                    1000 + k,
                ));
            }
        }
    }
    let red_decls = red_vars
        .iter()
        .map(|n| format!("let {n};"))
        .collect::<Vec<_>>()
        .join(" ");
    if !red_vars.is_empty() {
        let need = red_vars.len() * 4;
        red_emit_src.insert_str(
            0,
            &format!(
                "    if (__ocN + {need} > __ocSlots.length) __ocSlotsGrow(__ocN + {need});
"
            ),
        );
    }
    // ctx 垫片（docs/js-openmp.md §7.1）：受限 recorder 面。fillStyle 赋值
    // 存串（batchColor 同码手写扫描在 fill 时做，无正则零分配）；满圈 arc
    // 挂 pending；fill 量化入 ovals 段（7 f64/oval，色已 0-255 终值，
    // Math.round 在 JS 侧 = 与 JS 腿垫片同码同值）。快路之外的形态（非满
    // 圈弧、无 beginPath 连续 arc、行末悬挂、解析失败）行错误 → 毒化降
    // 级，绝不猜几何。行并行下行自包含：fillStyle 跨行保持是每 worker
    // 各自的（「行 i 写、行 j 读」依赖不可移植，docs 写明限制）。
    let ctx_names: Vec<&String> = params
        .iter()
        .enumerate()
        .filter(|(i, _)| kinds[*i] == SlotKind::Ctx)
        .map(|(_, n)| n)
        .collect();
    let ctx_shim_src = if ctx_names.is_empty() {
        String::from("  const __ocOvals = [];")
    } else {
        r#"  const __ocOvals = [];
  function __ocBatchColor(v, out) {
    if (v === undefined || v === null) return false;
    const s = String(v).trim();
    const c0 = s.charCodeAt(0);
    if (c0 === 35) { // '#'
      const hex = s.length === 4 ? s[1] + s[1] + s[2] + s[2] + s[3] + s[3]
        : (s.length === 7 || s.length === 9 ? s.slice(1, 7) : null);
      if (!hex) return false;
      const h = parseInt(hex, 16);
      if (!(h >= 0)) return false;
      const a = s.length === 9 ? parseInt(s.slice(7, 9), 16) : 255;
      if (!(a >= 0)) return false;
      out[0] = (h >> 16) & 255; out[1] = (h >> 8) & 255; out[2] = h & 255; out[3] = a;
      return true;
    }
    if (c0 === 114 && s.length > 10 && s.charCodeAt(1) === 103
      && s.charCodeAt(2) === 98 && s.charCodeAt(3) === 97
      && s.charCodeAt(4) === 40) { // rgba(
      let p = 5;
      const n = s.length;
      for (let k = 0; k < 4; k++) {
        while (p < n && (s.charCodeAt(p) === 32 || s.charCodeAt(p) === 9)) p++;
        const start = p;
        let seenDot = false, fv = 0, frac = 0;
        const scaled = k === 3; // alpha 分量是 0..1
        while (p < n) {
          const ch = s.charCodeAt(p);
          if (ch === (k < 3 ? 44 : 41)) break; // ',' or ')'
          if (ch >= 48 && ch <= 57) { fv = fv * 10 + (ch - 48); if (seenDot) frac += 1; p++; continue; }
          if (ch === 46 && !seenDot) { seenDot = true; p++; continue; }
          return false;
        }
        if (p === start || p >= n) return false;
        const v2 = frac > 0 ? fv / Math.pow(10, frac) : fv;
        if (!(v2 >= 0)) return false;
        out[k] = scaled ? (v2 > 1 ? 255 : Math.round(v2 * 255)) : (v2 > 255 ? 255 : Math.round(v2));
        p++;
      }
      if (p !== n) return false;
      return true;
    }
    return false;
  }
  function __ocCtx() {
    return {
      state: { globalAlpha: 1 },
      fillStyle: "rgba(0, 0, 0, 0)",
      __ocPend: null,
      beginPath() { this.__ocPend = null; },
      arc(x, y, r, a0, a1, ccw) {
        let sweep = a1 - a0;
        const tau = Math.PI * 2;
        if (ccw) { while (sweep > 0) sweep -= tau; while (sweep < -tau) sweep += tau; }
        else { while (sweep < 0) sweep += tau; while (sweep > tau) sweep -= tau; }
        if (!(Math.abs(sweep) >= tau - 1e-6)) throw new Error("omp ctx: non-full-circle arc");
        if (this.__ocPend) throw new Error("omp ctx: second arc without beginPath");
        this.__ocPend = [x, y, r];
      },
      fill() {
        const o = this.__ocPend;
        if (!o) throw new Error("omp ctx: fill without pending oval");
        this.__ocPend = null;
        const c = [0, 0, 0, 0];
        if (!__ocBatchColor(this.fillStyle, c)) {
          throw new Error("omp ctx: unsupported fillStyle " + this.fillStyle);
        }
        __ocOvals.push(o[0], o[1], o[2], c[0], c[1], c[2], c[3]);
      },
    };
  }"#
        .to_string()
    };
    // 行末悬挂（beginPath→arc 后未 fill 迭代就结束）：行并行下跨行挂起
    // 不可依赖——行错误毒化，语义收紧写进 docs。
    let dangling_check = match ctx_names.first() {
        Some(n) => format!(
            "    if ({n} && {n}.__ocPend) throw new Error(\"omp ctx: dangling oval at row end\");\n"
        ),
        None => String::new(),
    };
    // capture 槽 = IIFE 顶层声明（原名）：页面函数与行体自由引用解析到
    // 这里，__ocSetCaps 按帧赋值一次（ctx 槽除外——垫片常驻，实参忽略）。
    // fn 一次性声明在 IIFE 顶层——唯一 closure，V8 feedback vector 跨帧
    // 累积（行频重建 closure 会让每行重走 warmup，实测占掉一半墙钟）。
    let has_ctx = kinds.contains(&SlotKind::Ctx);
    let mut decl_parts: Vec<String> = params
        .iter()
        .enumerate()
        .map(|(i, n)| match kinds[i] {
            SlotKind::Ctx => format!("const {n} = __ocCtx();"),
            _ => format!("let {n};"),
        })
        .collect();
    if !red_vars.is_empty() {
        decl_parts.push(red_decls);
    }
    let cap_decls = decl_parts.join(" ");
    let mut set_body = Vec::with_capacity(params.len());
    for (i, name) in params.iter().enumerate() {
        let v = format!("v{i}");
        set_body.push(match kinds[i] {
            SlotKind::Prng => format!("  {name} = __ocNext;"),
            SlotKind::Ctx => continue,
            _ => format!("  {name} = {v};"),
        });
    }
    let set_body = set_body.join("\n");
    let set_params = (0..params.len())
        .map(|i| format!("v{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let src = format!(
        r#"(function () {{
  let __ocPrngBuf = null, __ocPrngCur = 0;
  function __ocNext() {{
    if (__ocPrngCur >= __ocPrngBuf.length) throw new Error("prng overrun");
    return __ocPrngBuf[__ocPrngCur++];
  }}
  let __ocSlots = new Float64Array(1 << 16), __ocN = 0;
  function __ocSlotsGrow(need) {{
    let m = __ocSlots.length;
    while (m < need) m *= 2;
    const t = new Float64Array(m);
    t.set(__ocSlots);
    __ocSlots = t;
  }}
  // 写回通道：Set/Acc 落行私有缓冲（copy-in 零字节的另一半——写也不出
  // worker，行末只 flush 每槽终值）。协议锁定帧初值 0（Acc = 0+Σ），
  // 缓冲复现任意 Set/Acc 组合的槽终值，flush 全按 Set 产出（acc=0）。
  // mark 数组去重 + list 保插入序：同槽 4 次合 1，写量缩 4 倍。
  const __ocBufs = [], __ocMarks = [], __ocLists = [];
  function __ocBuf(a, len) {{
    let b = __ocBufs[a];
    if (!b) {{
      b = __ocBufs[a] = new Float64Array(len);
      __ocMarks[a] = new Uint8Array(len);
      __ocLists[a] = [];
    }}
    return b;
  }}
  function __ocOmpSet(tok, i, v) {{
    const a = tok.__ocOmpArr;
    __ocBuf(a, tok.len)[i] = v;
    if (!__ocMarks[a][i]) {{ __ocMarks[a][i] = 1; __ocLists[a].push(i); }}
  }}
  function __ocOmpAcc(tok, i, v) {{
    const a = tok.__ocOmpArr;
    __ocBuf(a, tok.len)[i] += v;
    if (!__ocMarks[a][i]) {{ __ocMarks[a][i] = 1; __ocLists[a].push(i); }}
  }}
{ctx_shim_src}
{fn_src}
  {cap_decls}
  function __ocSetCaps({set_params}) {{
{set_body}
  }}
  function __ocHeader() {{
    const ys = [];
    for ({header}) {{ ys.push({var}); }}
    return ys;
  }}
  function __ocRow({var}) {{
    __ocN = 0;
    __ocOvals.length = 0;
{private}{red_init_src}{body}
    for (let a = 0; a < __ocLists.length; a++) {{
      const list = __ocLists[a];
      if (!list) continue;
      const buf = __ocBufs[a];
      for (let k = 0; k < list.length; k++) {{
        if (__ocN + 4 > __ocSlots.length) __ocSlotsGrow(__ocN + 4);
        __ocSlots[__ocN] = a;
        __ocSlots[__ocN + 1] = list[k];
        __ocSlots[__ocN + 2] = 0;
        __ocSlots[__ocN + 3] = buf[list[k]];
        __ocN += 4;
      }}
      __ocBufs[a] = null;
      __ocMarks[a] = null;
      __ocLists[a] = null;
    }}
{red_emit_src}{dangling_check}    // 行产出 = 单段 Float64Array：头 [n_slots, n_ovals] + slots 四元组
    // + ovals 七元组。一次 copy_contents 跨 API，host 按头分段。
    const nv = __ocOvals.length;
    const out = new Float64Array(2 + __ocN + nv);
    out[0] = __ocN / 4;
    out[1] = nv / 7;
    if (__ocN > 0) out.set(__ocSlots.subarray(0, __ocN), 2);
    if (nv > 0) out.set(__ocOvals, 2 + __ocN);
    return out;
  }}
  return {{
    __ocHeader,
    __ocRow,
    __ocSetCaps,
    __ocNext,
    __ocSetPrng(buf) {{ __ocPrngBuf = buf; __ocPrngCur = 0; }},
  }};
}})()"#
    );
    Ok(Bundle {
        key: Arc::from(ir.to_string().as_str()),
        src: Arc::from(src.as_str()),
        kinds: kinds.into(),
    })
}
