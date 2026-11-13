//! @omp 栈机：执行编译产物 + JS 数字语义 + canvas facet。
//!
//! facet 语义逐条复刻垫片快路（context2d.js）：满圈判定
//! `|sweep| >= 2π - 1e-6`、oval = (cx-rx, cy-ry, rx*2, ry*2)、fill 消费
//! pending、色解析 = batchColor 手写扫描（rgba 整数位拼 + 小数位计数、
//! rgb>255 钳 255、alpha 0..1 ×255 四舍五入、hex #rgb/#rrggbbaa）。
//! 任何 runtime 违规 → Err——上层把整帧降级回 JS 路径（PRNG 每帧
//! 重播种，单帧发散不跨帧）。

use std::sync::Arc;

use super::compile::{MathFn, Op, Program, TplOp};
use super::parse::{BinOp, UnOp};

const GLOBAL_FLAG: u16 = 0x8000;

// ── 值模型 ──────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum Value {
    Num(f64),
    Str(Arc<str>),
    Bool(bool),
    Null,
    Undef,
    Arr(Arc<Vec<Value>>),
    Obj(Arc<Vec<(String, Value)>>),
    /// PRNG 采集点标记（runtime 光标供值）。
    Prng,
    /// canvas facet 标记。
    Ctx { node_id: String, global_alpha: f64 },
}

struct Frame {
    slots: Vec<Value>,
    ret_dst: u16,
    ret_pc: usize,
}

#[derive(Debug, Default)]
pub struct FacetState {
    pub fill: Option<[u8; 4]>,
    /// 满圈 oval (x, y, w, h)。
    pub pending: Option<[f64; 4]>,
    /// 展平的矩形组 (x, y, w, h) × n。
    pub rects: Vec<f32>,
    /// 展平的颜色组 (r, g, b, a) × n。
    pub colors: Vec<u8>,
    pub count: usize,
}

impl FacetState {
    /// `ctx.fillStyle = <串>`：batch_color 解析落色。false = 解析失败。
    pub fn set_fill_str(&mut self, s: &str) -> bool {
        match batch_color(s) {
            Some(c) => {
                self.fill = Some(c);
                true
            }
            None => false,
        }
    }

    /// `ctx.arc(cx, cy, r, a0, a1)`：只吃满圈（与垫片 batch 通道同判据）。
    /// false = 非满圈（facet 通道不支持 → 行降级）。
    pub fn arc_pending(&mut self, cx: f64, cy: f64, r: f64, a0: f64, a1: f64) -> bool {
        let sweep = a1 - a0;
        if sweep.abs() < std::f64::consts::PI * 2.0 - 1e-6 {
            return false;
        }
        self.pending = Some([cx - r, cy - r, r * 2.0, r * 2.0]);
        true
    }

    /// `ctx.fill()`：消费 pending 满圈进行缓冲。false = 无 pending/无色。
    pub fn consume_fill(&mut self) -> bool {
        let Some(pending) = self.pending else {
            return false;
        };
        let Some(color) = self.fill else {
            return false;
        };
        self.rects.extend_from_slice(&[
            pending[0] as f32,
            pending[1] as f32,
            pending[2] as f32,
            pending[3] as f32,
        ]);
        self.colors.extend_from_slice(&color);
        self.count += 1;
        // 垫片同语义：fill 消费 pending（路径仍留，beginPath 才清）。
        self.pending = None;
        true
    }
}

// ── JS 语义助手 ─────────────────────────────────────────────────────

pub fn to_int32(x: f64) -> i32 {
    if !x.is_finite() {
        return 0;
    }
    ((x.trunc() as i64) as u32) as i32
}

pub fn to_uint32(x: f64) -> u32 {
    if !x.is_finite() {
        return 0;
    }
    (x.trunc() as i64) as u32
}

/// JS Math.round：向 +∞ 取半（-0.5 → -0 在像素面不可见，归 +0）。
pub fn js_round(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let f = x.floor();
    let frac = x - f;
    if frac >= 0.5 {
        f + 1.0
    } else if frac <= -0.5 {
        f - 1.0
    } else {
        f
    }
}

fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if a == 0.0 && b == 0.0 {
        return if a.is_sign_positive() || b.is_sign_positive() { 0.0 } else { -0.0 };
    }
    if a > b {
        a
    } else {
        b
    }
}

fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if a == 0.0 && b == 0.0 {
        return if a.is_sign_negative() || b.is_sign_negative() { -0.0 } else { 0.0 };
    }
    if a < b {
        a
    } else {
        b
    }
}

/// JS toFixed（位数面：-0 归 0；非有限给名字）。V8 的十进制定点与
/// Rust `{:.N}` 同源（go/no-go 对拍 0 差）。
pub fn js_to_fixed(x: f64, digits: u16) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    let x = if x == 0.0 { 0.0 } else { x };
    format!("{:.*}", digits as usize, x)
}

/// JS String(number)：整数面直出；非整数走 Rust 最短往返（Grisu）。
/// 指数形态只在 |x|≥1e21 / 极小值与 JS 有别——模板路径只渲染
/// `|0` 整数与 toFixed 串，不经过这里。
pub fn js_num_to_string(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if x == 0.0 {
        return "0".into();
    }
    if x == x.trunc() && x.abs() < 1e15 {
        return (x as i64).to_string();
    }
    format!("{}", x)
}

/// batchColor 的 Rust 复刻（context2d.js:296-349 逐条对齐）。
pub fn batch_color(s: &str) -> Option<[u8; 4]> {
    let b = s.trim().as_bytes();
    if b.is_empty() {
        return None;
    }
    if b[0] == b'#' {
        let hexd = |c: u8| -> Option<u8> {
            match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                b'A'..=b'F' => Some(c - b'A' + 10),
                _ => None,
            }
        };
        return match b.len() {
            4 => {
                let r = hexd(b[1])?;
                let g = hexd(b[2])?;
                let bl = hexd(b[3])?;
                Some([r * 17, g * 17, bl * 17, 255])
            }
            7 | 9 => {
                let mut h: u32 = 0;
                for i in 1..7 {
                    h = h * 16 + hexd(b[i])? as u32;
                }
                let a = if b.len() == 9 {
                    (hexd(b[7])? as u32) * 16 + hexd(b[8])? as u32
                } else {
                    255
                };
                Some([((h >> 16) & 255) as u8, ((h >> 8) & 255) as u8, (h & 255) as u8, a as u8])
            }
            _ => None,
        };
    }
    if b.len() > 10 && b[0] == b'r' && b[1] == b'g' && b[2] == b'b' && b[3] == b'a' && b[4] == b'(' {
        let mut p = 5usize;
        let n = b.len();
        let mut out = [0u8; 4];
        for k in 0..4 {
            while p < n && (b[p] == b' ' || b[p] == b'\t') {
                p += 1;
            }
            let start = p;
            let mut seen_dot = false;
            let scaled = k == 3; // alpha 分量是 0..1
            let mut fv: f64 = 0.0;
            let mut frac = 0i32;
            while p < n {
                let ch = b[p];
                let term = if k < 3 { b',' } else { b')' };
                if ch == term {
                    break;
                }
                if ch.is_ascii_digit() {
                    fv = fv * 10.0 + (ch - b'0') as f64;
                    if seen_dot {
                        frac += 1;
                    }
                    p += 1;
                    continue;
                }
                if ch == b'.' && !seen_dot {
                    seen_dot = true;
                    p += 1;
                    continue;
                }
                return None;
            }
            if p == start || p >= n {
                return None;
            }
            let v2 = if frac > 0 { fv / 10f64.powi(frac) } else { fv };
            if !(v2 >= 0.0) {
                return None;
            }
            out[k] = if scaled {
                if v2 > 1.0 {
                    255
                } else {
                    js_round(v2 * 255.0) as u8
                }
            } else if v2 > 255.0 {
                255
            } else {
                js_round(v2) as u8
            };
            p += 1;
        }
        if p != n {
            return None;
        }
        return Some(out);
    }
    None
}

// ── 栈机 ────────────────────────────────────────────────────────────

pub struct Vm<'a> {
    pub(crate) prog: &'a Program,
    pub(crate) globals: Vec<Value>,
    frames: Vec<Frame>,
    pc: usize,
    prng: &'a [f64],
    prng_cursor: usize,
    pub facet: FacetState,
    /// 干跑模式（SoA 执行器求 step 标量用）：CallCapture 不消耗采集光标、
    /// facet 动作全部空转。
    pub(crate) dryrun: bool,
}

impl<'a> Vm<'a> {
    pub(crate) fn new(prog: &'a Program, globals: Vec<Value>, prng: &'a [f64]) -> Self {
        Self {
            frames: vec![Frame {
                slots: vec![Value::Undef; prog.n_slots as usize],
                ret_dst: 0,
                ret_pc: 0,
            }],
            prog,
            globals,
            pc: 0,
            prng,
            prng_cursor: 0,
            facet: FacetState::default(),
            dryrun: false,
        }
    }

    /// 标量槽读数（SoA 执行器求 init/bound/step 标量用）。
    pub fn slot_num(&self, slot: u16) -> Result<f64, String> {
        self.num(&self.get(slot))
    }

    pub fn set_pc(&mut self, pc: usize) {
        self.pc = pc;
    }

    pub fn set_dryrun(&mut self) {
        self.dryrun = true;
    }

    /// 标量域窥视（SoA 执行器读行常量）。
    pub fn peek_num(&self, slot: u16) -> Option<f64> {
        match self.get(slot) {
            Value::Num(x) => Some(x),
            _ => None,
        }
    }

    pub fn peek_arr(&self, slot: u16) -> Option<Vec<Value>> {
        match self.get(slot) {
            Value::Arr(a) => Some(a.as_ref().clone()),
            _ => None,
        }
    }

    pub fn peek_obj(&self, slot: u16) -> Option<Vec<(String, Value)>> {
        match self.get(slot) {
            Value::Obj(o) => Some(o.as_ref().clone()),
            _ => None,
        }
    }

    /// 跑到 `stop_before`（op 下标）为止。Halt 提前终止。
    pub fn exec_range(&mut self, stop_before: usize) -> Result<(), String> {
        let code_len = stop_before.min(self.prog.code.len());
        while self.pc < code_len {
            let op = &self.prog.code[self.pc];
            self.pc += 1;
            match op {
                Op::Halt => return Ok(()),
                Op::CallCapture { dst, .. } if self.dryrun => {
                    self.set(*dst, Value::Num(0.0));
                }
                Op::FacetFillStyle { .. }
                | Op::FacetBeginPath
                | Op::FacetArc { .. }
                | Op::FacetFill
                    if self.dryrun => {}
                _ => self.step(op)?,
            }
        }
        Ok(())
    }
}

/// 执行一行（外层一次迭代）。`prng` 是本行的采集段（骨架按原序预采）。
pub fn run_row(
    prog: &Program,
    globals: Vec<Value>,
    prng: &[f64],
) -> Result<FacetState, String> {
    let mut vm = Vm::new(prog, globals, prng);
    vm.exec_range(usize::MAX)?;
    if vm.frames.len() != 1 {
        return Err("call frames not unwound".into());
    }
    Ok(vm.facet)
}

/// 执行头部快照——枚举剥壳外层循环的变量序列（行并行维度）。
pub fn run_header(prog: &Program, globals: &[Value]) -> Result<Vec<f64>, String> {
    let mut vm = Vm::new(prog, globals.to_vec(), &[]);
    vm.set_pc(prog.header_start as usize);
    vm.exec_range(usize::MAX)?;
    let ys = vm.frames[0].slots[prog.header_ys_slot as usize].clone();
    let ys = vm.frames[0].slots[prog.header_ys_slot as usize].clone();
    match ys {
        Value::Arr(a) => {
            let mut out = Vec::with_capacity(a.len());
            for v in a.iter() {
                match v {
                    Value::Num(x) => out.push(*x),
                    _ => return Err("header ys contains non-number".into()),
                }
            }
            Ok(out)
        }
        _ => Err("header ys slot is not an array".into()),
    }
}

impl<'a> Vm<'a> {
    fn frame(&mut self) -> &mut Frame {
        self.frames.last_mut().unwrap()
    }

    fn get(&self, slot: u16) -> Value {
        if slot & GLOBAL_FLAG != 0 {
            self.globals[(slot & 0x7fff) as usize].clone()
        } else {
            self.frames.last().unwrap().slots[slot as usize].clone()
        }
    }

    fn set(&mut self, slot: u16, v: Value) {
        if slot & GLOBAL_FLAG != 0 {
            self.globals[(slot & 0x7fff) as usize] = v;
        } else {
            self.frame().slots[slot as usize] = v;
        }
    }

    /// 数值化（Bool/Null/Undef 折数；Arr/Obj/Str 拒绝——确定性面）。
    fn num(&self, v: &Value) -> Result<f64, String> {
        match v {
            Value::Num(x) => Ok(*x),
            Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            Value::Null => Ok(0.0),
            Value::Undef => Ok(f64::NAN),
            _ => Err(format!("non-numeric operand: {v:?}")),
        }
    }

    fn truthy(v: &Value) -> bool {
        match v {
            Value::Num(x) => *x != 0.0 && !x.is_nan(),
            Value::Str(s) => !s.is_empty(),
            Value::Bool(b) => *b,
            Value::Null | Value::Undef | Value::Prng => false,
            Value::Ctx { .. } => true,
            _ => true,
        }
    }

    fn js_eq(l: &Value, r: &Value) -> bool {
        match (l, r) {
            (Value::Num(a), Value::Num(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Null, Value::Null) => true,
            (Value::Undef, Value::Undef) => true,
            _ => false,
        }
    }

    fn js_to_string(v: &Value) -> Result<String, String> {
        match v {
            Value::Num(x) => Ok(js_num_to_string(*x)),
            Value::Str(s) => Ok(s.to_string()),
            Value::Bool(b) => Ok(if *b { "true".into() } else { "false".into() }),
            Value::Null => Ok("null".into()),
            Value::Undef => Ok("undefined".into()),
            _ => Err(format!("cannot stringify {v:?}")),
        }
    }

pub fn math_num(f: MathFn, args: &[f64]) -> Result<f64, String> {
        let arity = |n: usize| -> Result<(), String> {
            if args.len() != n {
                Err(format!("arity mismatch for {f:?}"))
            } else {
                Ok(())
            }
        };
        match f {
            MathFn::Sin => {
                arity(1)?;
                Ok(libm::sin(args[0]))
            }
            MathFn::Cos => {
                arity(1)?;
                Ok(libm::cos(args[0]))
            }
            MathFn::Exp => {
                arity(1)?;
                Ok(libm::exp(args[0]))
            }
            MathFn::Hypot => {
                arity(2)?;
                Ok(libm::hypot(args[0], args[1]))
            }
            MathFn::Floor => {
                arity(1)?;
                Ok(args[0].floor())
            }
            MathFn::Abs => {
                arity(1)?;
                Ok(args[0].abs())
            }
            MathFn::Sqrt => {
                arity(1)?;
                Ok(args[0].sqrt())
            }
            MathFn::Round => {
                arity(1)?;
                Ok(js_round(args[0]))
            }
            MathFn::Max => {
                arity(2)?;
                Ok(js_max(args[0], args[1]))
            }
            MathFn::Min => {
                arity(2)?;
                Ok(js_min(args[0], args[1]))
            }
        }
    }

pub fn bin_num(op: BinOp, a: f64, b: f64) -> f64 {
        match op {
            BinOp::Add => a + b,
            BinOp::Sub => a - b,
            BinOp::Mul => a * b,
            BinOp::Div => a / b,
            BinOp::Mod => a % b, // fmod = JS %
            BinOp::Imul => (((to_int32(a) as i64) * (to_int32(b) as i64)) as i32) as f64,
            BinOp::BitAnd => (to_int32(a) & to_int32(b)) as i32 as f64,
            BinOp::BitOr => (to_int32(a) | to_int32(b)) as i32 as f64,
            BinOp::BitXor => (to_int32(a) ^ to_int32(b)) as i32 as f64,
            BinOp::Shl => (((to_uint32(a) << (to_uint32(b) & 31)) as i32) as f64),
            BinOp::ShrS => ((to_int32(a) >> (to_uint32(b) & 31)) as f64),
            BinOp::ShrU => ((to_uint32(a) >> (to_uint32(b) & 31)) as f64),
            // 比较运算由调用侧处理（见 exec 的 Lt..GtE 分支）。
            BinOp::Lt | BinOp::LtE | BinOp::Gt | BinOp::GtE | BinOp::Eq | BinOp::Neq => {
                unreachable!("comparison ops handled separately")
            }
        }
    }

    fn compare(op: BinOp, a: f64, b: f64) -> bool {
        match op {
            BinOp::Lt => a < b,
            BinOp::LtE => a <= b,
            BinOp::Gt => a > b,
            BinOp::GtE => a >= b,
            _ => unreachable!("comparison only"),
        }
    }

    fn member(&self, obj: &Value, name: &str) -> Result<Value, String> {
        match obj {
            Value::Obj(fields) => Ok(fields
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
                .unwrap_or(Value::Undef)),
            Value::Arr(a) if name == "length" => Ok(Value::Num(a.len() as f64)),
            Value::Str(s) if name == "length" => Ok(Value::Num(s.len() as f64)),
            Value::Ctx { node_id, global_alpha } => match name {
                "__ocNodeId" => Ok(Value::Str(node_id.as_str().into())),
                "globalAlpha" => Ok(Value::Num(*global_alpha)),
                _ => Err(format!("unsupported ctx member read `.{name}`")),
            },
            _ => Err(format!("member read on {obj:?}")),
        }
    }

    fn step(&mut self, op: &Op) -> Result<(), String> {
        match op {
            Op::ConstF { dst, idx } => {
            let v = self.prog.consts_f[*idx as usize];
            self.set(*dst, Value::Num(v));
            }
            Op::ConstS { dst, idx } => {
            let s = self.prog.consts_s[*idx as usize].clone();
            self.set(*dst, Value::Str(s.into()));
            }
            Op::ConstB { dst, val } => self.set(*dst, Value::Bool(*val)),
            Op::ConstNull { dst } => self.set(*dst, Value::Null),
            Op::ConstUndef { dst } => self.set(*dst, Value::Undef),
            Op::Copy { dst, src } => {
            let v = self.get(*src);
            self.set(*dst, v);
            }
            Op::Bin { dst, op, a, b } => {
                let av = self.get(*a);
                let bv = self.get(*b);
                match op {
                    BinOp::Eq => {
                        let r = Self::js_eq(&av, &bv);
                        self.set(*dst, Value::Bool(r));
                    }
                    BinOp::Neq => {
                        let r = !Self::js_eq(&av, &bv);
                        self.set(*dst, Value::Bool(r));
                    }
                    BinOp::Lt | BinOp::LtE | BinOp::Gt | BinOp::GtE => {
                        let an = self.num(&av)?;
                        let bn = self.num(&bv)?;
                        let r = Self::compare(*op, an, bn);
                        self.set(*dst, Value::Bool(r));
                    }
                    BinOp::Add => {
                        // JS +：任一为串 → 串接；否则数值。
                        let r = if matches!(av, Value::Str(_)) || matches!(bv, Value::Str(_))
                        {
                            let mut s = Self::js_to_string(&av)?;
                            s.push_str(&Self::js_to_string(&bv)?);
                            Value::Str(s.into())
                        } else {
                            Value::Num(self.num(&av)? + self.num(&bv)?)
                        };
                        self.set(*dst, r);
                    }
                    _ => {
                        let an = self.num(&av)?;
                        let bn = self.num(&bv)?;
                        self.set(*dst, Value::Num(Vm::bin_num(*op, an, bn)));
                    }
                }
            }
            Op::Un { dst, op, a } => {
                let av = self.get(*a);
                let r = match op {
                    UnOp::Neg => Value::Num(-self.num(&av)?),
                    UnOp::Not => Value::Bool(!Self::truthy(&av)),
                    UnOp::BitNot => Value::Num((!to_int32(self.num(&av)?)) as i32 as f64),
                };
                self.set(*dst, r);
            }
            Op::Jump { to } => self.pc = *to as usize,
            Op::JumpIfFalse { cond, to } => {
                let c = self.get(*cond);
                if !Self::truthy(&c) {
                    self.pc = *to as usize;
                }
            }
            Op::JumpIfTrue { cond, to } => {
                let c = self.get(*cond);
                if Self::truthy(&c) {
                    self.pc = *to as usize;
                }
            }
            Op::Math { dst, f, args } => {
                let mut nums = Vec::with_capacity(args.len());
                for s in args {
                    nums.push(self.num(&self.get(*s))?);
                }
                let r = Vm::math_num(*f, &nums)?;
                self.set(*dst, Value::Num(r));
            }
            Op::CallFn { dst, fid, args } => {
                let f = &self.prog.fns[*fid as usize];
                let mut slots = vec![Value::Undef; f.n_slots as usize];
                if args.len() != f.n_params as usize {
                    return Err(format!("call arity mismatch for fn #{}", fid));
                }
                for (i, s) in args.iter().enumerate() {
                    slots[i] = self.get(*s);
                }
                for (i, (_, gs)) in f.captured.iter().enumerate() {
                    slots[(f.n_params as usize) + i] =
                        self.globals[*gs as usize].clone();
                }
                let ret_pc = self.pc;
                let ret_dst = *dst;
                self.frames.push(Frame { slots, ret_dst, ret_pc });
                self.pc = f.start as usize;
            }
            Op::Ret { val } => {
                let v = self.get(*val);
                let f = self.frames.pop().unwrap();
                if self.frames.is_empty() {
                    return Err("return from main frame".into());
                }
                self.frames.last_mut().unwrap().slots[f.ret_dst as usize] = v;
                self.pc = f.ret_pc;
            }
            Op::CallCapture { dst, cap } => {
                let _ = cap; // 光标即序：采集按编译序逐个供给
                if self.prng_cursor >= self.prng.len() {
                    return Err("capture underrun".into());
                }
                let v = self.prng[self.prng_cursor];
                self.prng_cursor += 1;
                self.set(*dst, Value::Num(v));
            }
            Op::ToFixed { dst, src, digits } => {
                let v = self.get(*src);
                let x = match &v {
                    Value::Num(x) => *x,
                    _ => return Err(format!("toFixed on {v:?}")),
                };
                let s = js_to_fixed(x, *digits);
                self.set(*dst, Value::Str(s.into()));
            }
            Op::Member { dst, obj, name } => {
                let o = self.get(*obj);
                let n = &self.prog.consts_s[*name as usize];
                let r = self.member(&o, n)?;
                self.set(*dst, r);
            }
            Op::Index { dst, obj, idx } => {
                let o = self.get(*obj);
                let iv = self.get(*idx);
                let r = match (&o, &iv) {
                    (Value::Arr(a), Value::Num(x)) => {
                        let i = *x;
                        if i < 0.0 || i != i.trunc() || i.is_nan() {
                            Value::Undef
                        } else {
                            let iu = i as usize;
                            if iu < a.len() {
                                a[iu].clone()
                            } else {
                                Value::Undef
                            }
                        }
                    }
                    _ => return Err(format!("index read on {o:?}[{iv:?}]")),
                };
                self.set(*dst, r);
            }
            Op::SetIndex { obj, idx, val } => {
                let mut o = self.get(*obj);
                let iv = self.get(*idx);
                let v = self.get(*val);
                match (&mut o, &iv) {
                    (Value::Arr(a), Value::Num(x)) => {
                        let i = *x;
                        if i < 0.0 || i != i.trunc() || i.is_nan() {
                            return Err(format!("bad index write {x}"));
                        }
                        let iu = i as usize;
                        // JS 语义：arr[arr.length] = v 是追加（头部快照
                        // 枚举 ys 全走这条）；间隙跳写仍拒绝。
                        if iu > a.len() {
                            return Err("index write out of range".into());
                        }
                        let a = Arc::make_mut(a);
                        if iu == a.len() {
                            a.push(v);
                        } else {
                            a[iu] = v;
                        }
                    }
                    _ => return Err(format!("index write on {o:?}[{iv:?}]")),
                }
                self.set(*obj, o);
            }
            Op::SetMember { obj, name, val } => {
                let mut o = self.get(*obj);
                let n = &self.prog.consts_s[*name as usize];
                let v = self.get(*val);
                match &mut o {
                    Value::Obj(fields) => {
                        let f = Arc::make_mut(fields);
                        if let Some(slot) = f.iter_mut().find(|(k, _)| k == n) {
                            slot.1 = v;
                        } else {
                            f.push((n.clone(), v));
                        }
                    }
                    _ => return Err(format!("member write on {o:?}.{n}")),
                }
                self.set(*obj, o);
            }
            Op::ArrLit { dst, items } => {
                let mut vs = Vec::with_capacity(items.len());
                for s in items {
                    vs.push(self.get(*s));
                }
                self.set(*dst, Value::Arr(Arc::new(vs)));
            }
            Op::ObjLit { dst, fields } => {
                let mut fs = Vec::with_capacity(fields.len());
                for (k, s) in fields {
                    let name = self.prog.consts_s[*k as usize].clone();
                    fs.push((name, self.get(*s)));
                }
                self.set(*dst, Value::Obj(Arc::new(fs)));
            }
            Op::Template { dst, parts } => {
                let mut out = String::new();
                for p in parts {
                    match p {
                        TplOp::Lit(i) => out.push_str(&self.prog.consts_s[*i as usize]),
                        TplOp::Slot(s) => {
                            let v = self.get(*s);
                            out.push_str(&Self::js_to_string(&v)?);
                        }
                    }
                }
                self.set(*dst, Value::Str(out.into()));
            }
            Op::FacetFillStyle { val } => {
                let v = self.get(*val);
                match &v {
                    Value::Str(s) => {
                        if !self.facet.set_fill_str(s) {
                            return Err(format!("fillStyle parse failed: {s}"));
                        }
                    }
                    _ => return Err(format!("unsupported fillStyle {v:?}")),
                }
            }
            Op::FacetBeginPath => self.facet.pending = None,
            Op::FacetArc { args } => {
                if args.len() != 5 {
                    return Err("facet arc arity".into());
                }
                let mut ns = [0f64; 5];
                for (i, s) in args.iter().enumerate() {
                    ns[i] = self.num(&self.get(*s))?;
                }
                let [cx, cy, r, a0, a1] = ns;
                if !self.facet.arc_pending(cx, cy, r, a0, a1) {
                    return Err("non-full-circle arc unsupported in facet".into());
                }
            }
            Op::FacetFill => {
                if !self.facet.consume_fill() {
                    return Err("fill without pending oval / fillStyle".into());
                }
            }
            Op::Halt => {}
        }
        Ok(())
    }
}
