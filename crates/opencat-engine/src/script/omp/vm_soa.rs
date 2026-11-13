//! @omp SoA 向量执行器——「循环体 = per-point shader」的执行环境
//! （liangyi ExecutionPlan 的形态复刻）。
//!
//! 内层 for 的循环体被视作 per-point 纯函数：每个变量一条 f64 数组
//! （长度 = 内层迭代数），每条 IR 指令一次批量循环。连续 f64 数组的简单
//! 循环由 rustc/LLVM 自动向量化（fadd/fmul 独立舍入，Rust 默认禁 FMA
//! 收缩——逐位结果与标量路径一致）；行间仍由 rayon 并行，行内点序即
//! JS 逐点序，输出叠层序不变。
//!
//! 支持面（超出即 [`prepare`] 报错，缓存 None → 标量 VM 兜底）：
//! - 内层 for 直线体（无 if/无嵌套循环/无数组与对象写）
//! - `rgba(r, g, b, a)` 模板 + `fillStyle/beginPath/arc/fill` 五连
//!   （batchColor 数字段语义的数值复刻：串化往返 ≡ 数值直用）
//! - 页面函数：数学 + 对象/数组字面量 + 函数内 for 循环（循环变量恒
//!   行常量——向量条件即 JS 逐点分支，执行期拒绝）+ 步数预算兜底
//! - `colors[idx]` gather 与行常量对象成员读；`colors[向量下标]` 是惰性
//!   gather（每点选行常量数组的一行，元素级读在消费点展开成 Num 向量）

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use super::compile::{MathFn, Op, Program, TplOp};
use super::parse::BinOp;
use super::vm::{js_round, js_to_fixed, to_int32, Value, Vm};

// ── 准备产物 ────────────────────────────────────────────────────────

/// rgba 模板的一个分量来源。
#[derive(Clone, Debug)]
pub enum Seg {
    /// 数值槽（f64 域）。
    Num(u16),
    /// 模板里的常量数字文本。
    Lit(f64),
    /// `x.toFixed(d)` 产物槽（执行期该槽是 toFixed 串化-parse 回来的值）。
    Fixed(u16),
}

#[derive(Clone, Debug)]
pub struct ColorSpec {
    pub r: Seg,
    pub g: Seg,
    pub b: Seg,
    pub a: Seg,
}

/// 内层 for 的位置结构与 facet 五连。
#[derive(Debug)]
pub struct SoaPlan {
    /// bound 段起点（Jump 回边目标）。
    pub for_head: u32,
    /// 内层 for 的 JumpIfFalse 位置；body 从 jf_pos+1 开始。
    pub jf_pos: u32,
    /// 循环收尾后（Jump{for_head} 的下一位置）继续标量执行。
    pub l_end: u32,
    /// 内层循环变量槽。
    pub vslot: u16,
    /// bound 是 `<=`（true）还是 `<`。
    pub cmp_lte: bool,
    /// bound 比较的 b 槽（行常量上界值）。
    pub bound_b_slot: u16,
    /// step 合并 op（Bin Add vslot）的 b 槽。
    pub step_b_slot: u16,
    /// body 段内 CallCapture 个数（骨架每行采集数）。
    pub cap_count: usize,
    /// 模板 op 位置 → 颜色规格。
    pub color_at: HashMap<u32, ColorSpec>,

    /// body 调用过的页面函数 SoA-ok 面（per fn id）。
    pub fn_soa: Vec<bool>,
}

// ── 准备 pass：IR 形状识别 ──────────────────────────────────────────

/// 从编译产物识别 SoA 执行面。Err = 该循环不支持 SoA（标量 VM 兜底）。
pub fn prepare(prog: &Program) -> Result<SoaPlan, String> {
    let code = &prog.code[..prog.header_start as usize];

    // for 尾结构：Bin{dst:v, Add, a:v} 紧跟 Jump{to:h}（h < i 回边）。
    let mut anchor = None;
    for i in 0..code.len().saturating_sub(1) {
        let Op::Bin { dst, op: BinOp::Add, a, .. } = code[i] else {
            continue;
        };
        if a != dst {
            continue;
        }
        let Op::Jump { to: h, .. } = code[i + 1] else {
            continue;
        };
        if (h as usize) < i {
            anchor = Some((i, h as usize));
            break;
        }
    }
    let Some((step_pos, for_head)) = anchor else {
        return Err("no inner for found".into());
    };
    let Op::Bin { dst: vslot, b: step_b_slot, .. } = code[step_pos] else {
        unreachable!("anchor checked");
    };

    // bound 段 [for_head..cmp_pos)：Bin{dst:ct, Lt|LtE, a:vslot, b:bb}。
    let mut cmp = None;
    for p in for_head..step_pos {
        if let Op::Bin { op: op_cmp @ (BinOp::Lt | BinOp::LtE), a, b, .. } = code[p] {
            if a == vslot {
                cmp = Some((p, op_cmp == BinOp::LtE, b));
                break;
            }
        }
    }
    let Some((cmp_pos, cmp_lte, bound_b_slot)) = cmp else {
        return Err("bound shape mismatch".into());
    };
    let Op::JumpIfFalse { to: l_end, .. } = code[cmp_pos + 1] else {
        return Err("missing JumpIfFalse after bound".into());
    };
    if (l_end as usize) != step_pos + 2 {
        return Err("JumpIfFalse target != l_end".into());
    }
    let jf_pos = cmp_pos as u32 + 1;
    let body_lo = jf_pos as usize + 1;

    // init/前导段 [0..for_head) 与尾段 [l_end..header_start)：纯标量。
    if code[..for_head].iter().any(is_control) {
        return Err("control flow before inner for".into());
    }
    if code[l_end as usize..].iter().any(|op| !matches!(op, Op::Halt)) {
        return Err("non-empty tail after inner for".into());
    }

    // body 段 [body_lo..step_pos)：直线白名单（含 step 段 ops，执行时跳过）。
    let body = &code[body_lo..step_pos];
    if body.is_empty() {
        return Err("empty body".into());
    }
    let mut cap_count = 0usize;
    let mut called_fns = Vec::new();
    let mut ffs_seen = false;
    for op in body.iter() {
        match op {
            Op::ConstF { .. } | Op::ConstUndef { .. } | Op::Copy { .. } | Op::Un { .. }
            | Op::Math { .. } => {}
            // 比较是纯值运算（f64 布尔 1.0/0.0）；控制流仍被白名单外拒绝。
            Op::Bin { .. } => {}
            Op::CallCapture { .. } => cap_count += 1,
            Op::CallFn { fid, .. } => called_fns.push(*fid),
            Op::Member { .. } | Op::Index { .. } | Op::ObjLit { .. } => {}
            Op::Template { .. } | Op::FacetFillStyle { .. } => {
                if ffs_seen {
                    return Err("multiple fillStyle in body".into());
                }
            }
            Op::FacetBeginPath | Op::FacetArc { .. } | Op::FacetFill => {}
            Op::ToFixed { .. } => {}
            _ => return Err(format!("unsupported body op: {op:?}")),
        }
    }

    // facet 动作面：恰一组 [Template → FacetFillStyle(相邻) → BeginPath →
    // FacetArc → FacetFill]，相对顺序固定；其间允许直线数学 op（arc 参数
    // 计算、零参调用占位等），执行期逐 op 正常处理。
    let mut color_at = HashMap::new();
    let mut fpos: Vec<usize> = Vec::new();
    let mut tpl_fixed_dsts: Vec<u16> = Vec::new();
    for (k, op) in body.iter().enumerate() {
        match op {
            Op::Template { .. } | Op::FacetFillStyle { .. } | Op::FacetBeginPath
            | Op::FacetArc { .. } | Op::FacetFill => fpos.push(k),
            _ => {}
        }
    }
    if fpos.len() == 5 {
        let (tk, fk, bk, ak, lk) = (fpos[0], fpos[1], fpos[2], fpos[3], fpos[4]);
        let ok_shape = fk == tk + 1
            && matches!(body[bk], Op::FacetBeginPath)
            && matches!(body[ak], Op::FacetArc { .. })
            && matches!(body[lk], Op::FacetFill);
        if !ok_shape {
            return Err("facet sequence shape mismatch".into());
        }
        let Op::Template { dst, parts, .. } = &body[tk] else {
            unreachable!();
        };
        let Op::FacetFillStyle { val } = body[fk] else {
            unreachable!();
        };
        if *dst != val {
            return Err("fillStyle/template slot mismatch".into());
        }
        let (spec, consumed_fixed) = parse_rgba_template(parts, &prog.consts_s, body)?;
        color_at.insert((body_lo + tk) as u32, spec);
        tpl_fixed_dsts.extend(consumed_fixed);
    } else if !fpos.is_empty() {
        return Err("facet ops present but not exactly one group".into());
    }
    // 独立 ToFixed（非模板 alpha 段）不支持。
    for op in body {
        if let Op::ToFixed { dst, .. } = op {
            if !tpl_fixed_dsts.contains(dst) {
                return Err("standalone toFixed unsupported".into());
            }
        }
    }

    // 页面函数 SoA 面（递归闭包）。
    let mut fn_soa = vec![false; prog.fns.len()];
    for fid in &called_fns {
        check_fn_soa(prog, *fid as usize, &mut fn_soa)?;
    }

    Ok(SoaPlan {
        for_head: for_head as u32,
        jf_pos,
        l_end,
        vslot,
        cmp_lte,
        bound_b_slot,
        step_b_slot,
        cap_count,
        color_at,
        fn_soa,
    })
}

fn is_control(op: &Op) -> bool {
    matches!(
        op,
        Op::Jump { .. } | Op::JumpIfFalse { .. } | Op::JumpIfTrue { .. } | Op::Halt | Op::Ret { .. }
    )
}

/// 页面函数 SoA 面：数学 + 对象/数组字面量 + 段内控制流（循环变量须行
/// 常量——向量条件在执行期拒绝）；禁字符串面/capture/facet/成员写。
fn check_fn_soa(prog: &Program, fid: usize, ok: &mut Vec<bool>) -> Result<(), String> {
    if ok[fid] {
        return Ok(());
    }
    let start = prog.fns[fid].start as usize;
    // fn 段在 finalize 后位于 header 段之后（[main | header | fns…]），
    // 段终点 = 下一个 fn 起点或 code 尾。
    let mut end = prog.code.len();
    for (j, f) in prog.fns.iter().enumerate() {
        if j != fid && (f.start as usize) > start {
            end = end.min(f.start as usize);
        }
    }
    for op in &prog.code[start..end] {
        match op {
            Op::ConstF { .. } | Op::ConstUndef { .. } | Op::Copy { .. } | Op::Un { .. }
            | Op::Math { .. } | Op::Bin { .. } => {}
            Op::Jump { .. } | Op::JumpIfFalse { .. } | Op::JumpIfTrue { .. } => {}
            Op::CallFn { fid: inner, .. } => check_fn_soa(prog, *inner as usize, ok)?,
            Op::Member { .. } | Op::Index { .. } | Op::ArrLit { .. } | Op::ObjLit { .. }
            | Op::Ret { .. } => {}
            _ => return Err(format!("fn #{fid} has unsupported op: {op:?}")),
        }
    }
    ok[fid] = true;
    Ok(())
}

/// rgba 模板 parse：`rgba(` + 4 段（数值槽/常量数字/toFixed 槽），
/// 段间逗号（可带空格），末段后右括号。
fn parse_rgba_template(
    parts: &[TplOp],
    consts_s: &[String],
    body: &[Op],
) -> Result<(ColorSpec, Vec<u16>), String> {
    enum Item {
        Lit(String),
        Slot(u16),
    }
    let mut items: Vec<Item> = Vec::with_capacity(parts.len());
    for p in parts {
        match p {
            TplOp::Lit(i) => items.push(Item::Lit(consts_s[*i as usize].clone())),
            TplOp::Slot(s) => items.push(Item::Slot(*s)),
        }
    }

    // 文本流：cur 是当前 Lit 的剩余字节；耗尽时从 items 拉下一个 Lit。
    let mut cur: Vec<u8> = Vec::new();
    let mut item_i = 0usize;
    fn pull_lit(cur: &mut Vec<u8>, items: &[Item], item_i: &mut usize) -> bool {
        while cur.is_empty() && *item_i < items.len() {
            if let Item::Lit(s) = &items[*item_i] {
                cur.extend_from_slice(s.as_bytes());
                *item_i += 1;
            } else {
                break;
            }
        }
        !cur.is_empty()
    }

    // 1) 前缀 rgba(（可带前导空格）。
    while cur.is_empty() && !pull_lit(&mut cur, &items, &mut item_i) {
        return Err("template must start with rgba(".into());
    }
    let t = cur
        .iter()
        .position(|&c| c != b' ' && c != b'\t')
        .ok_or("template must start with rgba(")?;
    let rest = &cur[t..];
    if !rest.starts_with(b"rgba(") {
        return Err("template is not rgba(".into());
    }
    cur.drain(..t + 5);

    // 2) 四个段。
    let mut segs: Vec<Seg> = Vec::new();
    let mut fixed_dsts: Vec<u16> = Vec::new();
    for seg_i in 0..4 {
        if seg_i > 0 {
            loop {
                while cur.first().is_some_and(|&c| c == b' ' || c == b'\t') {
                    cur.remove(0);
                }
                if cur.is_empty() && !pull_lit(&mut cur, &items, &mut item_i) {
                    return Err("rgba separator missing".into());
                }
                if cur[0] == b',' {
                    cur.remove(0);
                    break;
                }
                return Err("rgba separator missing".into());
            }
        }
        while cur.first().is_some_and(|&c| c == b' ' || c == b'\t') {
            cur.remove(0);
        }
        if cur.is_empty() {
            match items.get(item_i) {
                Some(Item::Slot(slot)) => {
                    item_i += 1;
                    let (seg, consumed) = seg_from_slot(*slot, body)?;
                    if let Some(d) = consumed {
                        fixed_dsts.push(d);
                    }
                    segs.push(seg);
                    continue;
                }
                _ => return Err("rgba segment missing".into()),
            }
        }
        // 常量数字文本（数字/.//-，可跨 Lit 边界续接）。
        let mut num: Vec<u8> = Vec::new();
        loop {
            while !cur.is_empty()
                && (cur[0].is_ascii_digit() || cur[0] == b'.' || cur[0] == b'-' || cur[0] == b'+')
            {
                num.push(cur.remove(0));
            }
            if cur.is_empty()
                && item_i < items.len()
                && matches!(items[item_i], Item::Lit(_))
                && !num.is_empty()
            {
                pull_lit(&mut cur, &items, &mut item_i);
                continue;
            }
            break;
        }
        if num.is_empty() {
            return Err("rgba segment not a number/slot".into());
        }
        let v: f64 = String::from_utf8(num)
            .ok()
            .and_then(|s| s.parse().ok())
            .ok_or("rgba constant segment parse failed")?;
        segs.push(Seg::Lit(v));
    }

    // 3) 右括号。
    loop {
        while cur.first().is_some_and(|&c| c == b' ' || c == b'\t') {
            cur.remove(0);
        }
        if cur.is_empty() && !pull_lit(&mut cur, &items, &mut item_i) {
            return Err("rgba close paren missing".into());
        }
        if cur[0] == b')' {
            break;
        }
        return Err("rgba close paren missing".into());
    }
    if segs.len() != 4 {
        return Err("rgba template shape mismatch".into());
    }
    let mut it = segs.into_iter();
    Ok((
        ColorSpec {
            r: it.next().unwrap(),
            g: it.next().unwrap(),
            b: it.next().unwrap(),
            a: it.next().unwrap(),
        },
        fixed_dsts,
    ))
}

/// 槽 → Seg：Str 域槽溯源 toFixed——Fixed 存 **dst**（toFixed 串化-parse
/// 往返值所在的槽；src 是原值槽，读它 = 吃到未串化原值）；返回消费的
/// dst 供 standalone 检查。其余数值槽直用。
fn seg_from_slot(slot: u16, body: &[Op]) -> Result<(Seg, Option<u16>), String> {
    for op in body {
        if let Op::ToFixed { dst, .. } = op {
            if *dst == slot {
                return Ok((Seg::Fixed(*dst), Some(*dst)));
            }
        }
    }
    Ok((Seg::Num(slot), None))
}

// ── 每帧一次：迭代数探测 ────────────────────────────────────────────

/// 标量干跑 body+step 求内层迭代数（行常量语义——bound/step 不引用
/// 行变量由编译期检查保证）。
pub fn probe_n_inner(prog: &Program, plan: &SoaPlan, globals: &[Value]) -> Result<usize, String> {
    let mut dry = Vm::new(prog, globals.to_vec(), &[]);
    dry.set_dryrun();
    dry.set_pc(0);
    dry.exec_range(plan.for_head as usize)?;
    let v0 = dry.slot_num(plan.vslot)?;
    dry.set_pc(plan.for_head as usize);
    dry.exec_range(plan.jf_pos as usize)?;
    let bt = dry.slot_num(plan.bound_b_slot)?;
    // body+step 干跑（facet/capture 空转），跑过 step 合并 op 后读 step 值。
    let body_hi = l_end_minus_jump(prog, plan);
    dry.set_pc(plan.jf_pos as usize + 1);
    dry.exec_range(body_hi + 1)?;
    let st = dry.slot_num(plan.step_b_slot)?;

    let mut n = 0usize;
    let mut x = v0;
    while if plan.cmp_lte { x <= bt } else { x < bt } {
        x += st;
        n += 1;
        if n > 1_000_000_000 {
            return Err("inner iteration count overflow".into());
        }
    }
    Ok(n)
}

// ── 执行器 ──────────────────────────────────────────────────────────

/// SoA 域的值：f64 向量（Rc 引用计数——read 是 +1 不是 memcpy，向量体
/// 恒行局部不跨线程）/ 标量（惰性广播）/ 对象 / 数组（记录或惰性 gather）。
#[derive(Clone, Debug)]
enum SVal {
    Num(Rc<Vec<f64>>),
    Scal(f64),
    Obj(Rc<ObjV>),
    Arr(ArrV),
}

type ObjV = Vec<(String, SVal)>;

/// 数组面：字面量记录（`mix` 的返回形状）或按点选行的惰性 gather
/// （`colors[colorIndex]`——每点选中行常量数组的一行，元素级读在消费点
/// 展开成 Num 向量，中间数组永不物化）。
#[derive(Clone, Debug)]
enum ArrV {
    Rec(Rc<Vec<SVal>>),
    Gather { base: Arc<Vec<Value>>, idx: Rc<Vec<f64>> },
}

impl ArrV {
    fn len(&self) -> usize {
        match self {
            ArrV::Rec(vs) => vs.len(),
            ArrV::Gather { base, .. } => base.len(),
        }
    }
}

/// Rc 向量取所有权（唯一引用直接拿，共享则拷贝一份）。
fn rc_take(v: Rc<Vec<f64>>) -> Vec<f64> {
    Rc::try_unwrap(v).unwrap_or_else(|r| (*r).clone())
}

/// 执行一行（SoA）。`prng` 是本行采集段（长度 = cap_count × n_inner）。
pub fn run_row(
    prog: &Program,
    plan: &SoaPlan,
    globals: Vec<Value>,
    prng: &[f64],
    n_inner: usize,
    facet: &mut super::vm::FacetState,
) -> Result<(), String> {
    let mut scalar = Vm::new(prog, globals, &[]);
    scalar.set_pc(0);
    scalar.exec_range(plan.for_head as usize)?;

    // 内层循环变量物化成向量：init 值（标量前缀已算好）按 JS 语义逐次
    // 累加 step（f64 连加——不能 i*step 替代，分数步长两种序不同值）。
    let v0 = scalar.slot_num(plan.vslot)?;
    let st = scalar.slot_num(plan.step_b_slot)?;
    let mut xs = vec![0.0; n_inner];
    let mut acc = v0;
    for item in xs.iter_mut() {
        *item = acc;
        acc += st;
    }
    let mut pools = vec![None; prog.n_slots as usize];
    if (plan.vslot as usize) < pools.len() {
        pools[plan.vslot as usize] = Some(SVal::Num(Rc::new(xs)));
    }

    let mut run = Run {
        prog,
        plan,
        pools,
        frames: Vec::new(),
        scalar: &mut scalar,
        prng,
        cap: 0,
        n: n_inner,
        pend_colors: None,
        pend_rects: None,
    };
    run.exec_body()?;
    // for 收尾跳过，续跑尾段（恒为 Halt——prepare 已检查）。
    run.scalar.set_pc(plan.l_end as usize);
    run.scalar.exec_range(prog.header_start as usize)?;
    *facet = std::mem::take(&mut run.scalar.facet);
    Ok(())
}

struct Run<'a, 'b> {
    prog: &'a Program,
    plan: &'a SoaPlan,
    /// 密集槽帧（下标即槽号；全局槽 ≥0x8000 不落这里，走标量域）。
    pools: Vec<Option<SVal>>,
    /// fn 帧自由表（call 深度有限——复用免每次调用的帧分配）。
    frames: Vec<Vec<Option<SVal>>>,
    scalar: &'b mut Vm<'a>,
    prng: &'a [f64],
    cap: usize,
    n: usize,
    /// 最近一次 FacetFillStyle 的批量颜色（fill 消费）。
    pend_colors: Option<Vec<u8>>,
    /// 最近一次 FacetArc 的批量 oval（fill 消费）。
    pend_rects: Option<Vec<f32>>,
}

impl Run<'_, '_> {
    fn exec_body(&mut self) -> Result<(), String> {
        let body_lo = self.plan.jf_pos as usize + 1;
        let body_hi = l_end_minus_jump(self.prog, self.plan);
        // 借读不克隆：prog 是 Copy 的 &'a Program，op 借用与 &mut self 无冲突。
        let prog = self.prog;
        let mut pc = body_lo;
        while pc < body_hi {
            let op = &prog.code[pc];
            pc += 1;
            self.exec_one(op)?;
        }
        Ok(())
    }

    /// 槽读：密集帧优先；否则标量域（globals/前导段）折成 SVal。
    fn read(&self, slot: u16) -> Result<SVal, String> {
        let i = slot as usize;
        if i < self.pools.len() {
            if let Some(v) = &self.pools[i] {
                return Ok(v.clone());
            }
        }
        sval_from_value(slot, &self.scalar)
    }

    fn write(&mut self, slot: u16, v: SVal) {
        let i = slot as usize;
        if i >= self.pools.len() {
            self.pools.resize(i + 1, None);
        }
        self.pools[i] = Some(v);
    }

    fn exec_one(&mut self, op: &Op) -> Result<(), String> {
        match op {
            Op::ConstF { dst, idx } => {
                let v = self.prog.consts_f[*idx as usize];
                self.write(*dst, SVal::Scal(v));
            }
            Op::ConstUndef { dst } => {
                self.write(*dst, SVal::Scal(f64::NAN));
            }
            Op::Copy { dst, src } => {
                let v = self.read(*src)?;
                self.write(*dst, v);
            }
            Op::Bin { dst, op: bin, a, b } => {
                let (av, bv) = (self.read(*a)?, self.read(*b)?);
                let r = self.bin(*bin, av, bv)?;
                self.write(*dst, r);
            }
            Op::Un { dst, op: un, a } => {
                let av = self.read(*a)?;
                self.write(*dst, self.un(*un, av)?);
            }
            Op::Math { dst, f, args } => {
                let vals: Vec<SVal> =
                    args.iter().map(|s| self.read(*s)).collect::<Result<_, _>>()?;
                self.write(*dst, self.math(*f, vals)?);
            }
            Op::CallCapture { dst, .. } => {
                let n = self.n;
                if self.cap + n > self.prng.len() {
                    return Err("capture underrun".into());
                }
                let seg = self.prng[self.cap..self.cap + n].to_vec();
                self.cap += n;
                self.write(*dst, SVal::Num(Rc::new(seg)));
            }
            Op::CallFn { dst, fid, args } => {
                let fargs: Vec<SVal> =
                    args.iter().map(|s| self.read(*s)).collect::<Result<_, _>>()?;
                let ret = self.call_fn(*fid as usize, fargs)?;
                self.write(*dst, ret);
            }
            Op::Member { dst, obj, name } => {
                let ov = self.read(*obj)?;
                let nm = self.prog.consts_s[*name as usize].clone();
                self.write(*dst, self.member(ov, &nm)?);
            }
            Op::Index { dst, obj, idx } => {
                let iv = self.read(*idx)?;
                self.write(*dst, self.index(*obj, iv)?);
            }
            Op::ObjLit { dst, fields } => {
                let mut fs: ObjV = Vec::with_capacity(fields.len());
                for (k, s) in fields {
                    let name = self.prog.consts_s[*k as usize].clone();
                    let v = self.read(*s)?;
                    fs.push((name, v));
                }
                self.write(*dst, SVal::Obj(Rc::new(fs)));
            }
            Op::ArrLit { dst, items } => {
                let vs: Vec<SVal> =
                    items.iter().map(|s| self.read(*s)).collect::<Result<_, _>>()?;
                self.write(*dst, SVal::Arr(ArrV::Rec(Rc::new(vs))));
            }
            Op::ToFixed { dst, src, .. } => {
                // 模板 alpha 段：toFixed 串化→parse 的往返值（帮助函数同序）。
                let sv = self.read(*src)?;
                let d = self.map_num(sv, fixed3_parse)?;
                self.write(*dst, d);
            }
            Op::Template { .. } => {
                // 串化已静态折叠进 ColorSpec（fillStyle 五连唯一模板），
                // 本体跳过；dst 只被 FacetFillStyle 消费，SoA 不读它。
            }
            Op::FacetFillStyle { .. } => {
                // 颜色批量面：唯一 ColorSpec（prepare 静态保证恰一组）。
                let spec = self
                    .plan
                    .color_at
                    .values()
                    .next()
                    .ok_or("fillStyle without colorspec")?
                    .clone();
                let rs = self.n_vec(self.read_seg(&spec.r)?)?;
                let gs = self.n_vec(self.read_seg(&spec.g)?)?;
                let bs = self.n_vec(self.read_seg(&spec.b)?)?;
                let as_ = self.n_vec(self.read_seg(&spec.a)?)?;
                let mut colors = Vec::with_capacity(self.n * 4);
                for i in 0..self.n {
                    let ch = |v: f64| -> Result<u8, String> {
                        if !(v >= 0.0) {
                            return Err("rgba component parse failed".into());
                        }
                        Ok(if v > 255.0 { 255 } else { js_round(v) as u8 })
                    };
                    let ca = |v: f64| -> Result<u8, String> {
                        if !(v >= 0.0) {
                            return Err("rgba alpha parse failed".into());
                        }
                        Ok(if v > 1.0 { 255 } else { js_round(v * 255.0) as u8 })
                    };
                    colors.push(ch(rs[i])?);
                    colors.push(ch(gs[i])?);
                    colors.push(ch(bs[i])?);
                    colors.push(ca(as_[i])?);
                }
                self.pend_colors = Some(colors);
            }
            Op::FacetBeginPath => {
                // 批量语义：每点 begin→arc→fill 顺序由 IR 线性保证，
                // pending 清理由 fill 消费等价覆盖。
            }
            Op::FacetArc { args } => {
                let mut cxv = Vec::new();
                let mut cyv = Vec::new();
                let mut rv = Vec::new();
                let mut a0v = Vec::new();
                let mut a1v = Vec::new();
                for (k, s) in args.iter().enumerate() {
                    let v = self.n_vec(self.read(*s)?)?;
                    match k {
                        0 => cxv = v,
                        1 => cyv = v,
                        2 => rv = v,
                        3 => a0v = v,
                        _ => a1v = v,
                    }
                }
                let mut rects = Vec::with_capacity(self.n * 4);
                for i in 0..self.n {
                    let (cx, cy, r, a0, a1) =
                        (cxv[i], cyv[i], rv[i], a0v[i], a1v[i]);
                    let sweep = a1 - a0;
                    if sweep.abs() < std::f64::consts::PI * 2.0 - 1e-6 {
                        return Err("non-full-circle arc unsupported in facet".into());
                    }
                    rects.extend_from_slice(&[
                        (cx - r) as f32,
                        (cy - r) as f32,
                        (r * 2.0) as f32,
                        (r * 2.0) as f32,
                    ]);
                }
                self.pend_rects = Some(rects);
            }
            Op::FacetFill => {
                let (Some(colors), Some(rects)) =
                    (self.pend_colors.take(), self.pend_rects.take())
                else {
                    return Err("fill without pending oval / fillStyle".into());
                };
                let st = &mut self.scalar.facet;
                st.rects.extend_from_slice(&rects);
                st.colors.extend_from_slice(&colors);
                st.count += rects.len() / 4;
            }
            _ => return Err(format!("soa exec: unsupported op {op:?}")),
        }
        Ok(())
    }

    fn bin(&self, op: BinOp, a: SVal, b: SVal) -> Result<SVal, String> {
        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                // 加法族内联循环（LLVM 自动向量化主战场）。
                self.zip_num(a, b, |x, y| match op {
                    BinOp::Add => x + y,
                    BinOp::Sub => x - y,
                    BinOp::Mul => x * y,
                    _ => x / y,
                })
            }
            BinOp::Lt | BinOp::LtE | BinOp::Gt | BinOp::GtE | BinOp::Eq | BinOp::Neq => {
                // 布尔在 f64 域流动（1.0/0.0）；NaN 比较恒 false = JS 语义。
                self.zip_num(a, b, |x, y| {
                    let t = match op {
                        BinOp::Lt => x < y,
                        BinOp::LtE => x <= y,
                        BinOp::Gt => x > y,
                        BinOp::GtE => x >= y,
                        BinOp::Eq => x == y,
                        _ => x != y,
                    };
                    if t { 1.0 } else { 0.0 }
                })
            }
            _ => self.zip_num(a, b, |x, y| Vm::bin_num(op, x, y)),
        }
    }

    /// 数值二元的三态（Scal/Num × Scal/Num）：越晚物化向量越好；结果
    /// 直算进新缓冲（with_capacity，免 memset/拷贝）。
    fn zip_num(&self, a: SVal, b: SVal, f: impl Fn(f64, f64) -> f64) -> Result<SVal, String> {
        match (a, b) {
            (SVal::Num(x), SVal::Num(y)) => {
                if x.len() != y.len() {
                    return Err("vec length mismatch".into());
                }
                let mut d = Vec::with_capacity(x.len());
                for i in 0..x.len() {
                    d.push(f(x[i], y[i]));
                }
                Ok(SVal::Num(Rc::new(d)))
            }
            (SVal::Scal(x), SVal::Num(y)) => {
                let mut d = Vec::with_capacity(y.len());
                for i in 0..y.len() {
                    d.push(f(x, y[i]));
                }
                Ok(SVal::Num(Rc::new(d)))
            }
            (SVal::Num(x), SVal::Scal(y)) => {
                let mut d = Vec::with_capacity(x.len());
                for i in 0..x.len() {
                    d.push(f(x[i], y));
                }
                Ok(SVal::Num(Rc::new(d)))
            }
            (SVal::Scal(x), SVal::Scal(y)) => Ok(SVal::Scal(f(x, y))),
            _ => Err("bin on object".into()),
        }
    }

    fn un(&self, op: super::parse::UnOp, a: SVal) -> Result<SVal, String> {
        match op {
            super::parse::UnOp::Neg => self.map_num(a, |x| -x),
            super::parse::UnOp::BitNot => self.map_num(a, |x| (!to_int32(x)) as i32 as f64),
            super::parse::UnOp::Not => Err("not in soa body".into()),
        }
    }

    fn math(&self, f: MathFn, args: Vec<SVal>) -> Result<SVal, String> {
        let vec_len = args.iter().find_map(|v| match v {
            SVal::Num(x) => Some(x.len()),
            _ => None,
        });
        let Some(n) = vec_len else {
            let row: Vec<f64> = args
                .iter()
                .map(|v| match v {
                    SVal::Scal(x) => Ok(*x),
                    _ => Err("mixed math args".to_string()),
                })
                .collect::<Result<_, _>>()?;
            return Ok(SVal::Scal(Vm::math_num(f, &row)?));
        };
        let get = |v: &SVal, i: usize| match v {
            SVal::Num(x) => x[i],
            SVal::Scal(x) => *x,
            SVal::Obj(_) | SVal::Arr(_) => f64::NAN,
        };
        // 快路面：1/2 参栈数组直逐点（免每点 row 小 Vec 分配——多线程
        // 堆竞争下这个分配曾占 math 臂大头）。
        if args.len() <= 2 {
            let mut d = vec![0.0; n];
            match args.len() {
                1 => {
                    let a0 = &args[0];
                    for i in 0..n {
                        d[i] = Vm::math_num(f, &[get(a0, i)])?;
                    }
                }
                2 => {
                    let (a0, a1) = (&args[0], &args[1]);
                    for i in 0..n {
                        d[i] = Vm::math_num(f, &[get(a0, i), get(a1, i)])?;
                    }
                }
                _ => unreachable!(),
            }
            return Ok(SVal::Num(Rc::new(d)));
        }
        let mut row = vec![0.0; args.len()];
        let mut d = vec![0.0; n];
        for i in 0..n {
            for (r, v) in row.iter_mut().zip(args.iter()) {
                *r = get(v, i);
            }
            d[i] = Vm::math_num(f, &row)?;
        }
        Ok(SVal::Num(Rc::new(d)))
    }

    fn map_num(&self, v: SVal, f: impl Fn(f64) -> f64) -> Result<SVal, String> {
        Ok(match v {
            SVal::Scal(x) => SVal::Scal(f(x)),
            SVal::Num(x) => SVal::Num(Rc::new(x.iter().map(|&x| f(x)).collect())),
            SVal::Obj(_) | SVal::Arr(_) => return Err("map on non-numeric".into()),
        })
    }

    fn member(&self, obj: SVal, name: &str) -> Result<SVal, String> {
        match obj {
            SVal::Obj(fields) => fields
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
                .ok_or_else(|| format!("soa member miss .{name}")),
            SVal::Arr(a) if name == "length" => Ok(SVal::Scal(a.len() as f64)),
            _ => Err(format!("soa member miss .{name}")),
        }
    }

    fn index(&self, obj_slot: u16, idx: SVal) -> Result<SVal, String> {
        // SoA 池数组记录优先（页面函数返回的中间数组）。
        let i = obj_slot as usize;
        if i < self.pools.len() {
            if let Some(SVal::Arr(a)) = &self.pools[i] {
                return arr_elem(a, &idx);
            }
        }
        // 标量域行常量数组：向量下标 → 惰性 gather；标量下标 → 元素
        // （可能还是行内数组 → 记录）。
        let arr = self.scalar.peek_arr(obj_slot).ok_or("index on non-array")?;
        match idx {
            SVal::Num(ix) => Ok(SVal::Arr(ArrV::Gather { base: Arc::new(arr), idx: ix })),
            SVal::Scal(x) => Ok(value_to_sval(&arr_value_at(&arr, x))),
            _ => Err("array record indexed by vector".into()),
        }
    }

    fn call_fn(&mut self, fid: usize, args: Vec<SVal>) -> Result<SVal, String> {
        let f = &self.prog.fns[fid];
        if args.len() != f.n_params as usize {
            return Err(format!("soa call arity mismatch fn #{fid}"));
        }
        let start = f.start as usize;
        // fn 段在 finalize 后位于 header 段之后，段终点 = 下一个 fn 起点
        // 或 code 尾（与 check_fn_soa 同一布局口径）。
        let mut end = self.prog.code.len();
        for (j, g) in self.prog.fns.iter().enumerate() {
            if j != fid && (g.start as usize) > start {
                end = end.min(g.start as usize);
            }
        }
        // 帧池：形参槽 0..n_params = 实参；captured 全局走标量域。帧从
        // 自由表取（call 深度有限）——免每次调用的帧分配与释放。
        let n_slots = f.n_slots as usize;
        let saved = std::mem::replace(&mut self.pools, self.frames.pop().unwrap_or_default());
        if self.pools.len() < n_slots {
            self.pools.resize(n_slots, None);
        }
        self.pools[..n_slots].fill(None);
        for (i, v) in args.into_iter().enumerate() {
            self.pools[i] = Some(v);
        }
        let mut ret = SVal::Scal(f64::NAN);
        let mut failed: Option<String> = None;
        // 控制流感知的段内解释（fn 内 for：循环变量恒行常量 Scal，body
        // 照常向量批算）；向量条件 = JS 逐点分支 → 拒绝；步数预算兜死循环。
        // op 借读不克隆（prog 是 Copy 的 &'a Program，先拷出引用）。
        let prog = self.prog;
        let code = &prog.code;
        let mut pc = start;
        let mut steps = 0usize;
        while failed.is_none() {
            if pc >= end {
                break; // 落出段尾 = undefined 返回
            }
            steps += 1;
            if steps > 1_000_000 {
                failed = Some(format!("fn #{fid} step budget exceeded"));
                break;
            }
            let op = &code[pc];
            pc += 1;
            match op {
                Op::Ret { val } => match self.read(*val) {
                    Ok(v) => ret = v,
                    Err(e) => failed = Some(e),
                },
                Op::Jump { to } => pc = *to as usize,
                Op::JumpIfFalse { cond, to } => match self.cond_bool(*cond) {
                    Ok(false) => pc = *to as usize,
                    Ok(true) => {}
                    Err(e) => failed = Some(e),
                },
                Op::JumpIfTrue { cond, to } => match self.cond_bool(*cond) {
                    Ok(true) => pc = *to as usize,
                    Ok(false) => {}
                    Err(e) => failed = Some(e),
                },
                _ => {
                    if let Err(e) = self.exec_one(op) {
                        failed = Some(e);
                    }
                }
            }
        }
        // fn 局部放掉后帧还回自由表，主帧复位。
        for s in self.pools.iter_mut() {
            *s = None;
        }
        self.frames.push(std::mem::replace(&mut self.pools, saved));
        match failed {
            Some(e) => Err(e),
            None => Ok(ret),
        }
    }

    /// 跳转条件的标量布尔（向量条件 = JS 逐点分支 → SoA 拒绝）。
    fn cond_bool(&self, slot: u16) -> Result<bool, String> {
        match self.read(slot)? {
            SVal::Scal(x) => Ok(!(x == 0.0 || x.is_nan())),
            _ => Err("divergent control flow: vector condition".into()),
        }
    }

    fn read_seg(&self, seg: &Seg) -> Result<SVal, String> {
        match seg {
            Seg::Num(s) | Seg::Fixed(s) => self.read(*s),
            Seg::Lit(x) => Ok(SVal::Scal(*x)),
        }
    }

    fn n_vec(&self, v: SVal) -> Result<Vec<f64>, String> {
        match v {
            SVal::Num(x) => Ok(rc_take(x)),
            SVal::Scal(x) => Ok(vec![x; self.n]),
            SVal::Obj(_) | SVal::Arr(_) => Err("non-numeric in facet face".into()),
        }
    }
}

/// body 段终点 = step 合并 op（Bin Add vslot）的位置。
fn l_end_minus_jump(prog: &Program, plan: &SoaPlan) -> usize {
    let lo = plan.jf_pos as usize + 1;
    for i in lo..plan.l_end as usize {
        if let Op::Bin { dst, op: BinOp::Add, a, .. } = prog.code[i] {
            if dst == plan.vslot && a == plan.vslot {
                return i;
            }
        }
    }
    plan.l_end as usize
}

/// 标量域值 → SVal（行常量 Num/Obj/Arr）。
fn sval_from_value(slot: u16, scalar: &Vm) -> Result<SVal, String> {
    if let Some(x) = scalar.peek_num(slot) {
        return Ok(SVal::Scal(x));
    }
    if let Some(fields) = scalar.peek_obj(slot) {
        let mut fs: ObjV = Vec::with_capacity(fields.len());
        for (k, v) in fields {
            fs.push((k, value_to_sval(&v)));
        }
        return Ok(SVal::Obj(Rc::new(fs)));
    }
    if let Some(arr) = scalar.peek_arr(slot) {
        return Ok(SVal::Arr(ArrV::Rec(Rc::new(
            arr.iter().map(value_to_sval).collect(),
        ))));
    }
    Err(format!("soa scalar slot {slot} not numeric/object/array"))
}

/// 标量域值 → SVal 记录（lenient：Str/Bool/Null/Undef 落数值面 NaN——
/// JS 数字强转同向；下游 rgba 组装遇 NaN 会拒绝 → 毒化兜底）。
fn value_to_sval(v: &Value) -> SVal {
    match v {
        Value::Num(x) => SVal::Scal(*x),
        Value::Obj(o) => SVal::Obj(Rc::new(
            o.iter().map(|(k, v)| (k.clone(), value_to_sval(v))).collect(),
        )),
        Value::Arr(a) => SVal::Arr(ArrV::Rec(Rc::new(a.iter().map(value_to_sval).collect()))),
        _ => SVal::Scal(f64::NAN),
    }
}

/// 标量域数组元素（可能还是数组）——负/非整/越界 → Undef（数值面 NaN）。
fn arr_value_at(arr: &[Value], idx: f64) -> Value {
    if idx < 0.0 || idx != idx.trunc() || idx.is_nan() {
        return Value::Undef;
    }
    arr.get(idx as usize).cloned().unwrap_or(Value::Undef)
}

/// 数组记录/gather 的元素读（消费点展开：Gather → 按点行内取数）。
fn arr_elem(a: &ArrV, idx: &SVal) -> Result<SVal, String> {
    match (a, idx) {
        (ArrV::Rec(vs), SVal::Scal(x)) => {
            if *x < 0.0 || *x != x.trunc() || x.is_nan() {
                return Ok(SVal::Scal(f64::NAN));
            }
            Ok(vs
                .get(*x as usize)
                .cloned()
                .unwrap_or(SVal::Scal(f64::NAN)))
        }
        (ArrV::Gather { base, idx: sel }, SVal::Scal(k)) => {
            if *k < 0.0 || *k != k.trunc() || k.is_nan() {
                return Ok(SVal::Num(Rc::new(vec![f64::NAN; sel.len()])));
            }
            let mut d = Vec::with_capacity(sel.len());
            for &row in sel.iter() {
                match arr_value_at(base, row) {
                    Value::Arr(sub) => d.push(arr_at(&sub, *k)),
                    _ => d.push(f64::NAN),
                }
            }
            Ok(SVal::Num(Rc::new(d)))
        }
        // 每点选不同记录/二段向量下标：SoA 面外（语料无此形状）。
        _ => Err("per-point element select on array record".into()),
    }
}

/// VM Index 语义：非整/负/越界 → NaN（JS undefined 参与数值面）。
fn arr_at(arr: &[Value], idx: f64) -> f64 {
    if idx < 0.0 || idx != idx.trunc() || idx.is_nan() {
        return f64::NAN;
    }
    let iu = idx as usize;
    match arr.get(iu) {
        Some(Value::Num(x)) => *x,
        Some(_) => f64::NAN,
        None => f64::NAN,
    }
}

/// toFixed(3) 的串化往返（format+parse 与串路径同序同源）。
fn fixed3_parse(x: f64) -> f64 {
    js_to_fixed(x, 3).parse::<f64>().unwrap_or(f64::NAN)
}
