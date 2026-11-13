//! @omp 编译器：IR → 扁平 opcode 程序（栈机可执行）。
//!
//! 结构：标注外层循环被「剥壳」——外层循环头留给 runtime 求值出
//! ys（行并行维度），外层体编译成主程序；页面函数编译成独立代码
//! 对象，CALL/RET 帧调用。
//!
//! 槽位编码：低位 = 帧局部槽；0x8000 位 = 全局帧槽（标注 args 原序
//! 0..n，n = 外层循环变量）。全局帧每行由 runtime 装一次，函数帧只
//! 含局部——页面函数闭包引用全局参数天然正确，参数遮蔽全局也是合法
//! JS 语义（作用域就近），无需降级。所有白名单违规一律 `Err`——上层
//! 把 Err 翻译成降级（原循环照跑），绝不产错码。

use std::collections::HashMap;

use super::parse::{self, BinOp, Expr, Stmt, TplPart, UnOp};

const GLOBAL_FLAG: u16 = 0x8000;

// ── opcode ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MathFn {
    Sin,
    Cos,
    Exp,
    Hypot,
    Floor,
    Max,
    Min,
    Abs,
    Sqrt,
    Round,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CaptureKind {
    /// 普通值（数字/数组/对象，JSON 解码）。
    Plain,
    /// 零参 PRNG 采集点——骨架预采的 Float64Array 按序供给。
    Prng,
    /// canvas facet（ctx 对象：fillStyle/beginPath/arc/fill）。
    Ctx,
}

#[derive(Clone, Debug)]
pub enum TplOp {
    Lit(u16), // consts_s 下标
    Slot(u16),
}

#[derive(Clone, Debug)]
pub enum Op {
    ConstF { dst: u16, idx: u16 },
    ConstS { dst: u16, idx: u16 },
    ConstB { dst: u16, val: bool },
    ConstNull { dst: u16 },
    ConstUndef { dst: u16 },
    Copy { dst: u16, src: u16 },
    Bin { dst: u16, op: BinOp, a: u16, b: u16 },
    Un { dst: u16, op: UnOp, a: u16 },
    Jump { to: u32 },
    JumpIfFalse { cond: u16, to: u32 },
    JumpIfTrue { cond: u16, to: u32 },
    Math { dst: u16, f: MathFn, args: Vec<u16> },
    CallFn { dst: u16, fid: u16, args: Vec<u16> },
    /// 从当前（函数）帧返回，返回值写回 call 点的 dst 槽。
    Ret { val: u16 },
    /// PRNG 采集：cap = 采集序号（Env 游标按序取值）。
    CallCapture { dst: u16, cap: u16 },
    ToFixed { dst: u16, src: u16, digits: u16 },
    Member { dst: u16, obj: u16, name: u16 }, // name = consts_s 下标
    Index { dst: u16, obj: u16, idx: u16 },
    SetIndex { obj: u16, idx: u16, val: u16 },
    SetMember { obj: u16, name: u16, val: u16 },
    ArrLit { dst: u16, items: Vec<u16> },
    ObjLit { dst: u16, fields: Vec<(u16, u16)> }, // (name 下标, 槽)
    Template { dst: u16, parts: Vec<TplOp> },
    // canvas facet 动作（写入行缓冲，最后由 runtime 拼成 FillOvalBatch）。
    FacetFillStyle { val: u16 },
    FacetBeginPath,
    FacetArc { args: Vec<u16> },
    FacetFill,
    /// 段终止（主程序 / 头部快照各自的出口）。
    Halt,
}

#[derive(Clone, Debug)]
pub struct FnMeta {
    /// 收尾扁平化后：在 prog.code 里的段起点。
    pub start: u32,
    /// 局部槽数（不含俘获全局——全局走 0x8000 编码直读 Env.globals）。
    pub n_slots: u16,
    pub n_params: u16,
    /// 闭包引用的全局：(名, 全局帧下标)。runtime 建帧时填尾部。
    pub captured: Vec<(String, u16)>,
}

#[derive(Clone, Debug)]
pub struct Program {
    pub code: Vec<Op>,
    pub consts_f: Vec<f64>,
    pub consts_s: Vec<String>,
    pub fns: Vec<FnMeta>,
    /// 标注 args 原序；全局帧 0..n_args = args，n_args = 外层循环变量。
    pub arg_names: Vec<String>,
    pub arg_kinds: Vec<CaptureKind>,
    pub loop_var: String,
    pub n_globals: u16,
    pub n_slots: u16,
    /// 头部快照（剥壳外层头的可执行体）在 code 里的起点与 ys 数组槽。
    /// runtime 用它枚举外层变量序列（行并行维度）。
    pub header_start: u32,
    pub header_ys_slot: u16,
}

// ── 静态检查 ────────────────────────────────────────────────────────

/// 计算语句序列的最大 for 嵌套深度（剥壳后的相对深度；直接语句 = 0）。
fn max_loop_depth(stmts: &[Stmt], acc: usize) -> usize {
    let mut out = acc;
    for s in stmts {
        match s {
            Stmt::For { body, .. } => out = out.max(max_loop_depth(body, acc + 1)),
            Stmt::If(_, then, els) => {
                out = out.max(max_loop_depth(then, acc)).max(max_loop_depth(els, acc));
            }
            Stmt::Block(body) => out = out.max(max_loop_depth(body, acc)),
            _ => {}
        }
    }
    out
}

/// 表达式里是否出现任何调用（循环头必须无调用：runtime 直接求值头部，
/// 采集点也不允许落在头部打乱游标对齐）。
fn expr_has_call(e: &Expr) -> bool {
    match e {
        Expr::Call { .. } => true,
        Expr::Member(a, _) => expr_has_call(a),
        Expr::Index(a, b) => expr_has_call(a) || expr_has_call(b),
        Expr::Unary(_, a) => expr_has_call(a),
        Expr::Binary(_, a, b) => expr_has_call(a) || expr_has_call(b),
        Expr::Logical(_, a, b) => expr_has_call(a) || expr_has_call(b),
        Expr::Ternary(a, b, c) => expr_has_call(a) || expr_has_call(b) || expr_has_call(c),
        Expr::Assign(a, b) => expr_has_call(a) || expr_has_call(b),
        Expr::ArrayLit(items) => items.iter().any(expr_has_call),
        Expr::Object(fields) => fields.iter().any(|(_, v)| expr_has_call(v)),
        Expr::Template(parts) => parts.iter().any(|p| match p {
            TplPart::Lit(_) => false,
            TplPart::Expr(x) => expr_has_call(x),
        }),
        _ => false,
    }
}

/// 表达式是否引用某标识符（内层头引用外层变量检测用）。
fn expr_uses_ident(e: &Expr, name: &str) -> bool {
    match e {
        Expr::Ident(n) => n == name,
        Expr::Member(a, _) => expr_uses_ident(a, name),
        Expr::Index(a, b) => expr_uses_ident(a, name) || expr_uses_ident(b, name),
        Expr::Unary(_, a) => expr_uses_ident(a, name),
        Expr::Binary(_, a, b) => expr_uses_ident(a, name) || expr_uses_ident(b, name),
        Expr::Logical(_, a, b) => expr_uses_ident(a, name) || expr_uses_ident(b, name),
        Expr::Ternary(a, b, c) => {
            expr_uses_ident(a, name) || expr_uses_ident(b, name) || expr_uses_ident(c, name)
        }
        Expr::Assign(a, b) => expr_uses_ident(a, name) || expr_uses_ident(b, name),
        Expr::ArrayLit(items) => items.iter().any(|x| expr_uses_ident(x, name)),
        Expr::Object(fields) => fields.iter().any(|(_, v)| expr_uses_ident(v, name)),
        Expr::Template(parts) => parts.iter().any(|p| match p {
            TplPart::Lit(_) => false,
            TplPart::Expr(x) => expr_uses_ident(x, name),
        }),
        Expr::Call { obj, name: n, args } => {
            obj.as_ref().map(|o| expr_uses_ident(o, name)).unwrap_or(false)
                || n == name
                || args.iter().any(|x| expr_uses_ident(x, name))
        }
        _ => false,
    }
}

/// 收集语句序列里所有裸调用名（递归检测用）。
fn collect_calls_stmts(stmts: &[Stmt], out: &mut Vec<String>) {
    for s in stmts {
        match s {
            Stmt::Let(_, e) => collect_calls_expr(e, out),
            Stmt::Expr(e) => collect_calls_expr(e, out),
            Stmt::If(c, then, els) => {
                collect_calls_expr(c, out);
                collect_calls_stmts(then, out);
                collect_calls_stmts(els, out);
            }
            Stmt::Block(body) => collect_calls_stmts(body, out),
            Stmt::For { init, bound, step, body, .. } => {
                collect_calls_expr(init, out);
                collect_calls_expr(bound, out);
                collect_calls_expr(step, out);
                collect_calls_stmts(body, out);
            }
            Stmt::Return(Some(e)) => collect_calls_expr(e, out),
            Stmt::Return(None) => {}
        }
    }
}

fn collect_calls_expr(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Call { obj, name, args } => {
            if obj.is_none() {
                out.push(name.clone());
            }
            if let Some(o) = obj {
                collect_calls_expr(o, out);
            }
            for a in args {
                collect_calls_expr(a, out);
            }
        }
        Expr::Member(a, _) => collect_calls_expr(a, out),
        Expr::Index(a, b) => {
            collect_calls_expr(a, out);
            collect_calls_expr(b, out);
        }
        Expr::Unary(_, a) => collect_calls_expr(a, out),
        Expr::Binary(_, a, b) | Expr::Logical(_, a, b) | Expr::Assign(a, b) => {
            collect_calls_expr(a, out);
            collect_calls_expr(b, out);
        }
        Expr::Ternary(a, b, c) => {
            collect_calls_expr(a, out);
            collect_calls_expr(b, out);
            collect_calls_expr(c, out);
        }
        Expr::ArrayLit(items) => items.iter().for_each(|x| collect_calls_expr(x, out)),
        Expr::Object(fields) => fields.iter().for_each(|(_, v)| collect_calls_expr(v, out)),
        Expr::Template(parts) => parts.iter().for_each(|p| match p {
            TplPart::Lit(_) => {}
            TplPart::Expr(x) => collect_calls_expr(x, out),
        }),
        _ => {}
    }
}

/// 收集帧内所有本地绑定名（参数 + let + for 头变量，扁平）。
fn collect_locals_stmts(stmts: &[Stmt], out: &mut Vec<String>) {
    for s in stmts {
        match s {
            Stmt::Let(n, _) => out.push(n.clone()),
            Stmt::For { var, body, .. } => {
                out.push(var.clone());
                collect_locals_stmts(body, out);
            }
            Stmt::If(_, then, els) => {
                collect_locals_stmts(then, out);
                collect_locals_stmts(els, out);
            }
            Stmt::Block(body) => collect_locals_stmts(body, out),
            _ => {}
        }
    }
}

/// 收集自由 Ident 引用（不在 locals 里的裸标识符；Math 除外）。
fn collect_free_idents_stmts(stmts: &[Stmt], locals: &[String], out: &mut Vec<String>) {
    for s in stmts {
        match s {
            Stmt::Let(_, e) => collect_free_idents_expr(e, locals, out),
            Stmt::Expr(e) => collect_free_idents_expr(e, locals, out),
            Stmt::If(c, then, els) => {
                collect_free_idents_expr(c, locals, out);
                collect_free_idents_stmts(then, locals, out);
                collect_free_idents_stmts(els, locals, out);
            }
            Stmt::Block(body) => collect_free_idents_stmts(body, locals, out),
            Stmt::For { init, bound, step, body, .. } => {
                collect_free_idents_expr(init, locals, out);
                collect_free_idents_expr(bound, locals, out);
                collect_free_idents_expr(step, locals, out);
                collect_free_idents_stmts(body, locals, out);
            }
            Stmt::Return(Some(e)) => collect_free_idents_expr(e, locals, out),
            Stmt::Return(None) => {}
        }
    }
}

fn collect_free_idents_expr(e: &Expr, locals: &[String], out: &mut Vec<String>) {
    match e {
        Expr::Ident(n) => {
            if n == "Math" || locals.contains(n) || out.contains(n) {
                return;
            }
            out.push(n.clone());
        }
        Expr::Member(a, _) => collect_free_idents_expr(a, locals, out),
        Expr::Index(a, b) => {
            collect_free_idents_expr(a, locals, out);
            collect_free_idents_expr(b, locals, out);
        }
        Expr::Unary(_, a) => collect_free_idents_expr(a, locals, out),
        Expr::Binary(_, a, b) | Expr::Logical(_, a, b) | Expr::Assign(a, b) => {
            collect_free_idents_expr(a, locals, out);
            collect_free_idents_expr(b, locals, out);
        }
        Expr::Ternary(a, b, c) => {
            collect_free_idents_expr(a, locals, out);
            collect_free_idents_expr(b, locals, out);
            collect_free_idents_expr(c, locals, out);
        }
        Expr::ArrayLit(items) => {
            items.iter().for_each(|x| collect_free_idents_expr(x, locals, out))
        }
        Expr::Object(fields) => fields
            .iter()
            .for_each(|(_, v)| collect_free_idents_expr(v, locals, out)),
        Expr::Template(parts) => parts.iter().for_each(|p| match p {
            TplPart::Lit(_) => {}
            TplPart::Expr(x) => collect_free_idents_expr(x, locals, out),
        }),
        // 裸调用名不是自由变量（页面函数或采集；未知调用由 compile_call 拒绝）。
        Expr::Call { obj, args, .. } => {
            if let Some(o) = obj {
                collect_free_idents_expr(o, locals, out);
            }
            for a in args {
                collect_free_idents_expr(a, locals, out);
            }
        }
        _ => {}
    }
}

const MATH_WHITELIST: &[(&str, MathFn)] = &[
    ("sin", MathFn::Sin),
    ("cos", MathFn::Cos),
    ("exp", MathFn::Exp),
    ("hypot", MathFn::Hypot),
    ("floor", MathFn::Floor),
    ("max", MathFn::Max),
    ("min", MathFn::Min),
    ("abs", MathFn::Abs),
    ("sqrt", MathFn::Sqrt),
    ("round", MathFn::Round),
];

// ── 编译入口 ────────────────────────────────────────────────────────

/// 编译入口。`fns` 是 transform 传递闭包抽取的（超集）页面函数源码。
pub fn compile(
    loop_src: &str,
    fns: &[(String, String)],
    arg_names: &[String],
    arg_kinds: &[CaptureKind],
) -> Result<Program, String> {
    if arg_names.len() != arg_kinds.len() {
        return Err("args/kinds length mismatch".into());
    }
    let looped = parse::Parser::parse_loop(loop_src)?;
    let Stmt::For { var, init, bound, allow_eq, step, body } = looped else {
        unreachable!("parse_loop guarantees For");
    };

    // 外层头必须无调用（内层头在 check_inner_headers）。
    for h in [&init, &bound, &step] {
        if expr_has_call(h) {
            return Err("outer loop header contains a call".into());
        }
    }

    let n_args = arg_names.len() as u16;
    let prng_n = arg_kinds.iter().filter(|k| **k == CaptureKind::Prng).count();

    // 头部标识符面：只许引用 args 与本巢循环变量——骨架只重放头部，
    // 体局部量在骨架作用域不存在（内层头引用体局部量会 ReferenceError）。
    let mut nest_vars: Vec<String> = vec![var.clone()];
    collect_nest_vars(&body, &mut nest_vars);
    let allowed: Vec<&str> = arg_names.iter().map(|s| s.as_str())
        .chain(nest_vars.iter().map(|s| s.as_str()))
        .collect();
    check_header_idents(&[&init, &bound, &step], &allowed)?;
    check_headers_in_body(&body, &allowed)?;

    // 内层头引用外层变量 + 存在采集 → 每行采集数不均匀 → 降级。
    let mut inner_uses_outer = false;
    check_inner_headers(&body, &var, &mut inner_uses_outer)?;
    if inner_uses_outer && prng_n > 0 {
        return Err("inner header references outer variable with captures".into());
    }

    // 函数源码表；预解析（语法违规早失败）+ 递归检测。
    let mut fn_src: HashMap<String, String> = HashMap::new();
    for (n, s) in fns {
        parse::Parser::parse_fn(s).map_err(|e| format!("function `{n}`: {e}"))?;
        fn_src.insert(n.clone(), s.clone());
    }
    check_recursion(&fn_src)?;

    let max_depth = max_loop_depth(&body, 0);

    let mut c = Compiler {
        prog: Program {
            code: Vec::new(),
            consts_f: Vec::new(),
            consts_s: Vec::new(),
            fns: Vec::new(),
            arg_names: arg_names.to_vec(),
            arg_kinds: arg_kinds.to_vec(),
            loop_var: var.clone(),
            n_globals: n_args + 1,
            n_slots: 0,
            header_start: 0,
            header_ys_slot: 0,
        },
        scope: HashMap::new(),
        fn_src: &fn_src,
        fn_ids: HashMap::new(),
        fn_bodies: Vec::new(),
        max_depth,
        depth: 0,
        cond_depth: 0,
        in_fn: false,
    };
    // 主帧作用域：args 原序（全局槽）+ 外层循环变量（全局槽）。
    for (i, n) in arg_names.iter().enumerate() {
        c.scope.insert(n.clone(), GLOBAL_FLAG | i as u16);
    }
    c.scope.insert(var.clone(), GLOBAL_FLAG | n_args);
    c.compile_stmts(&body)?;
    c.emit(Op::Halt);

    // 头部快照：剥壳外层头的可执行体——init/bound/step 只引用全局
    // （编译期由 unknown identifier 拒绝其他名字），独立帧执行后从
    // ys 数组槽取外层变量序列。
    let main_slots = c.prog.n_slots;
    c.scope.clear();
    for (i, n) in arg_names.iter().enumerate() {
        c.scope.insert(n.clone(), GLOBAL_FLAG | i as u16);
    }
    c.prog.n_slots = 1; // y → 局部槽 0
    c.scope.insert(var.clone(), 0);
    let ys_slot = c.temp();
    c.prog.header_start = c.here();
    c.emit(Op::ArrLit { dst: ys_slot, items: Vec::new() });
    let iv = c.compile_expr(&init)?;
    c.emit(Op::Copy { dst: 0, src: iv });
    let head = c.here();
    let bt = c.compile_expr(&bound)?;
    let ct = c.temp();
    c.emit(Op::Bin {
        dst: ct,
        op: if allow_eq { BinOp::LtE } else { BinOp::Lt },
        a: 0,
        b: bt,
    });
    let l_end = c.emit(Op::JumpIfFalse { cond: ct, to: 0 });
    let lt = c.temp();
    let ni = c.const_s("length");
    c.emit(Op::Member { dst: lt, obj: ys_slot, name: ni });
    c.emit(Op::SetIndex { obj: ys_slot, idx: lt, val: 0 });
    let st = c.compile_expr(&step)?;
    c.emit(Op::Bin { dst: 0, op: BinOp::Add, a: 0, b: st });
    c.emit(Op::Jump { to: head });
    c.patch_jump(l_end);
    c.emit(Op::Halt);
    c.prog.header_ys_slot = ys_slot;
    c.prog.n_slots = main_slots.max(c.prog.n_slots);
    finalize(&mut c.prog, c.fn_bodies);
    Ok(c.prog)
}

/// 收尾：函数码拼进主码缓冲（跳转目标按段起点重排）。
fn finalize(prog: &mut Program, mut bodies: Vec<Vec<Op>>) {
    let mut all = std::mem::take(&mut prog.code);
    for (f, code) in prog.fns.iter_mut().zip(bodies.drain(..)) {
        let start = all.len() as u32;
        for op in code {
            let op = match op {
                Op::Jump { to } => Op::Jump { to: to + start },
                Op::JumpIfFalse { cond, to } => Op::JumpIfFalse { cond, to: to + start },
                Op::JumpIfTrue { cond, to } => Op::JumpIfTrue { cond, to: to + start },
                other => other,
            };
            all.push(op);
        }
        f.start = start;
    }
    prog.code = all;
}

impl Program {
    /// 毒化条目的占位产物（永不执行——毒化在查表时短路）。
    pub fn empty() -> Self {
        Self {
            code: Vec::new(),
            consts_f: Vec::new(),
            consts_s: Vec::new(),
            fns: Vec::new(),
            arg_names: Vec::new(),
            arg_kinds: Vec::new(),
            loop_var: String::new(),
            n_globals: 0,
            n_slots: 0,
            header_start: 0,
            header_ys_slot: 0,
        }
    }
}

/// 收集嵌套 for 的循环变量名。
fn collect_nest_vars(stmts: &[Stmt], out: &mut Vec<String>) {
    for s in stmts {
        match s {
            Stmt::For { var, body, .. } => {
                if !out.contains(var) {
                    out.push(var.clone());
                }
                collect_nest_vars(body, out);
            }
            Stmt::If(_, then, els) => {
                collect_nest_vars(then, out);
                collect_nest_vars(els, out);
            }
            Stmt::Block(body) => collect_nest_vars(body, out),
            _ => {}
        }
    }
}

/// 头部表达式只许引用 allowed 名（args + 循环变量）。
fn check_header_idents(exprs: &[&Expr], allowed: &[&str]) -> Result<(), String> {
    fn walk(e: &Expr, allowed: &[&str]) -> Result<(), String> {
        match e {
            Expr::Ident(n) => {
                if allowed.contains(&n.as_str()) {
                    Ok(())
                } else {
                    Err(format!("loop header references `{n}` (not an arg or loop variable)"))
                }
            }
            Expr::Member(a, _) => walk(a, allowed),
            Expr::Index(a, b) => {
                walk(a, allowed)?;
                walk(b, allowed)
            }
            Expr::Unary(_, a) => walk(a, allowed),
            Expr::Binary(_, a, b) | Expr::Logical(_, a, b) | Expr::Assign(a, b) => {
                walk(a, allowed)?;
                walk(b, allowed)
            }
            Expr::Ternary(a, b, c) => {
                walk(a, allowed)?;
                walk(b, allowed)?;
                walk(c, allowed)
            }
            Expr::ArrayLit(items) => items.iter().try_for_each(|x| walk(x, allowed)),
            Expr::Object(fields) => fields.iter().try_for_each(|(_, v)| walk(v, allowed)),
            Expr::Template(parts) => parts.iter().try_for_each(|p| match p {
                TplPart::Lit(_) => Ok(()),
                TplPart::Expr(x) => walk(x, allowed),
            }),
            Expr::Call { .. } => Err("loop header contains a call".into()),
            _ => Ok(()),
        }
    }
    for e in exprs {
        walk(e, allowed)?;
    }
    Ok(())
}

/// 嵌套体内的所有 for 头（含更深嵌套）做同样的标识符面检查。
fn check_headers_in_body(stmts: &[Stmt], allowed: &[&str]) -> Result<(), String> {
    for s in stmts {
        match s {
            Stmt::For { init, bound, step, body, .. } => {
                check_header_idents(&[init, bound, step], allowed)?;
                check_headers_in_body(body, allowed)?;
            }
            Stmt::If(_, then, els) => {
                check_headers_in_body(then, allowed)?;
                check_headers_in_body(els, allowed)?;
            }
            Stmt::Block(body) => check_headers_in_body(body, allowed)?,
            _ => {}
        }
    }
    Ok(())
}

/// 递归遍历嵌套 for 的头：查调用与外层变量引用。
fn check_inner_headers(stmts: &[Stmt], outer_var: &str, uses_outer: &mut bool) -> Result<(), String> {
    for s in stmts {
        match s {
            Stmt::For { init, bound, step, body, .. } => {
                for h in [init, bound, step] {
                    if expr_has_call(h) {
                        return Err("nested loop header contains a call".into());
                    }
                    if expr_uses_ident(h, outer_var) {
                        *uses_outer = true;
                    }
                }
                check_inner_headers(body, outer_var, uses_outer)?;
            }
            Stmt::If(_, then, els) => {
                check_inner_headers(then, outer_var, uses_outer)?;
                check_inner_headers(els, outer_var, uses_outer)?;
            }
            Stmt::Block(body) => check_inner_headers(body, outer_var, uses_outer)?,
            _ => {}
        }
    }
    Ok(())
}

/// 调用图环检测（函数相互调用；自归归入）。
fn check_recursion(fn_src: &HashMap<String, String>) -> Result<(), String> {
    let mut edges: HashMap<&str, Vec<String>> = HashMap::new();
    for (n, src) in fn_src {
        let fd = parse::Parser::parse_fn(src).map_err(|e| format!("function `{n}`: {e}"))?;
        let mut calls = Vec::new();
        collect_calls_stmts(&fd.body, &mut calls);
        let mut callees: Vec<String> = Vec::new();
        for c in calls {
            if fn_src.contains_key(&c) && !callees.contains(&c) {
                callees.push(c);
            }
        }
        edges.insert(n.as_str(), callees);
    }
    fn dfs<'e>(
        node: &'e str,
        edges: &'e HashMap<&'e str, Vec<String>>,
        stack: &mut Vec<&'e str>,
        done: &mut HashMap<&'e str, bool>,
    ) -> Result<(), String> {
        if let Some(&d) = done.get(node) {
            if d {
                return Ok(());
            }
            return Err(format!("recursive call involving `{node}`"));
        }
        stack.push(node);
        done.insert(node, false);
        if let Some(calls) = edges.get(node) {
            for i in 0..calls.len() {
                let c: &str = calls[i].as_str();
                if stack.contains(&c) {
                    return Err(format!("recursive call `{node}` -> `{c}`"));
                }
                dfs(c, edges, stack, done)?;
            }
        }
        stack.pop();
        done.insert(node, true);
        Ok(())
    }
    let mut done = HashMap::new();
    for n in fn_src.keys() {
        dfs(n.as_str(), &edges, &mut Vec::new(), &mut done)?;
    }
    Ok(())
}

// ── 逐帧编译 ────────────────────────────────────────────────────────

struct Compiler<'a> {
    prog: Program,
    /// 当前帧作用域：名 → 槽（全局槽带 0x8000 位）。
    scope: HashMap<String, u16>,
    fn_src: &'a HashMap<String, String>,
    fn_ids: HashMap<String, u16>,
    /// 函数码旁路缓冲（与 prog.fns 平行；finalize 时拼进 prog.code）。
    fn_bodies: Vec<Vec<Op>>,
    max_depth: usize,
    /// 当前嵌套 for 深度（剥壳外层体 = 0）。
    depth: usize,
    /// 条件分支嵌套（if 分支 / 三元臂 / 逻辑 rhs）——采集禁入。
    cond_depth: usize,
    in_fn: bool,
}

impl<'a> Compiler<'a> {
    fn temp(&mut self) -> u16 {
        let s = self.prog.n_slots;
        self.prog.n_slots += 1;
        s
    }

    fn const_f(&mut self, v: f64) -> u16 {
        if let Some(i) = self
            .prog
            .consts_f
            .iter()
            .position(|&x| x.to_bits() == v.to_bits())
        {
            return i as u16;
        }
        self.prog.consts_f.push(v);
        (self.prog.consts_f.len() - 1) as u16
    }

    fn const_s(&mut self, s: &str) -> u16 {
        if let Some(i) = self.prog.consts_s.iter().position(|x| x == s) {
            return i as u16;
        }
        self.prog.consts_s.push(s.to_string());
        (self.prog.consts_s.len() - 1) as u16
    }

    fn emit(&mut self, op: Op) -> u32 {
        self.prog.code.push(op);
        (self.prog.code.len() - 1) as u32
    }

    fn here(&self) -> u32 {
        self.prog.code.len() as u32
    }

    fn patch_jump(&mut self, at: u32) {
        let to = self.here();
        match &mut self.prog.code[at as usize] {
            Op::Jump { to: t }
            | Op::JumpIfFalse { to: t, .. }
            | Op::JumpIfTrue { to: t, .. } => *t = to,
            _ => unreachable!("patch on non-jump"),
        }
    }

    fn capture_ordinal(&self, name: &str) -> Option<u16> {
        let mut ord = 0u16;
        for (i, k) in self.prog.arg_kinds.iter().enumerate() {
            if *k == CaptureKind::Prng {
                if self.prog.arg_names[i] == name {
                    return Some(ord);
                }
                ord += 1;
            }
        }
        None
    }

    fn is_ctx_capture(&self, name: &str) -> bool {
        for (i, k) in self.prog.arg_kinds.iter().enumerate() {
            if *k == CaptureKind::Ctx && self.prog.arg_names[i] == name {
                return true;
            }
        }
        false
    }

    fn compile_stmts(&mut self, stmts: &[Stmt]) -> Result<(), String> {
        for s in stmts {
            self.compile_stmt(s)?;
        }
        Ok(())
    }

    fn bind(&mut self, name: &str) -> Result<u16, String> {
        if self.scope.contains_key(name) {
            return Err(format!("duplicate binding `{name}` (flat scope)"));
        }
        let s = self.prog.n_slots;
        self.prog.n_slots += 1;
        self.scope.insert(name.to_string(), s);
        Ok(s)
    }

    fn compile_stmt(&mut self, s: &Stmt) -> Result<(), String> {
        match s {
            Stmt::Let(name, init) => {
                let slot = self.bind(name)?;
                let v = self.compile_expr(init)?;
                self.emit(Op::Copy { dst: slot, src: v });
            }
            Stmt::Expr(e) => {
                let _ = self.compile_expr(e)?;
            }
            Stmt::Block(body) => self.compile_stmts(body)?,
            Stmt::If(cond, then, els) => {
                let t = self.compile_expr(cond)?;
                let j_else = self.emit(Op::JumpIfFalse { cond: t, to: 0 });
                self.cond_depth += 1;
                self.compile_stmts(then)?;
                self.cond_depth -= 1;
                if els.is_empty() {
                    self.patch_jump(j_else);
                } else {
                    let j_end = self.emit(Op::Jump { to: 0 });
                    self.patch_jump(j_else);
                    self.cond_depth += 1;
                    self.compile_stmts(els)?;
                    self.cond_depth -= 1;
                    self.patch_jump(j_end);
                }
            }
            Stmt::For { var, init, bound, allow_eq, step, body } => {
                let vslot = self.bind(var)?;
                let iv = self.compile_expr(init)?;
                self.emit(Op::Copy { dst: vslot, src: iv });
                let l_head = self.here();
                let bt = self.compile_expr(bound)?;
                let ct = self.temp();
                self.emit(Op::Bin {
                    dst: ct,
                    op: if *allow_eq { BinOp::LtE } else { BinOp::Lt },
                    a: vslot,
                    b: bt,
                });
                let l_end = self.emit(Op::JumpIfFalse { cond: ct, to: 0 });
                self.depth += 1;
                self.compile_stmts(body)?;
                self.depth -= 1;
                let st = self.compile_expr(step)?;
                self.emit(Op::Bin { dst: vslot, op: BinOp::Add, a: vslot, b: st });
                self.emit(Op::Jump { to: l_head });
                self.patch_jump(l_end);
            }
            Stmt::Return(e) => {
                if !self.in_fn {
                    return Err("`return` outside program function".into());
                }
                let v = match e {
                    Some(e) => self.compile_expr(e)?,
                    None => {
                        let t = self.temp();
                        self.emit(Op::ConstUndef { dst: t });
                        t
                    }
                };
                self.emit(Op::Ret { val: v });
            }
        }
        Ok(())
    }

    /// 编译表达式，返回持有其值的槽位（叶子标识符直接返回原槽）。
    fn compile_expr(&mut self, e: &Expr) -> Result<u16, String> {
        match e {
            Expr::Num(v) => {
                let t = self.temp();
                let idx = self.const_f(*v);
                self.emit(Op::ConstF { dst: t, idx });
                Ok(t)
            }
            Expr::Str(s) => {
                let t = self.temp();
                let idx = self.const_s(s);
                self.emit(Op::ConstS { dst: t, idx });
                Ok(t)
            }
            Expr::Bool(b) => {
                let t = self.temp();
                self.emit(Op::ConstB { dst: t, val: *b });
                Ok(t)
            }
            Expr::Null => {
                let t = self.temp();
                self.emit(Op::ConstNull { dst: t });
                Ok(t)
            }
            Expr::Undef => {
                let t = self.temp();
                self.emit(Op::ConstUndef { dst: t });
                Ok(t)
            }
            Expr::Ident(n) => self.scope.get(n).copied().ok_or_else(|| {
                format!("unknown identifier `{n}` (missing from annotation args?)")
            }),
            Expr::Member(base, name) => {
                if let Expr::Ident(b) = &**base {
                    if b == "Math" {
                        if name == "PI" {
                            let t = self.temp();
                            let idx = self.const_f(std::f64::consts::PI);
                            self.emit(Op::ConstF { dst: t, idx });
                            return Ok(t);
                        }
                        return Err(format!("unsupported Math member `{name}`"));
                    }
                }
                let o = self.compile_expr(base)?;
                let t = self.temp();
                let ni = self.const_s(name);
                self.emit(Op::Member { dst: t, obj: o, name: ni });
                Ok(t)
            }
            Expr::Index(base, idx) => {
                let o = self.compile_expr(base)?;
                let i = self.compile_expr(idx)?;
                let t = self.temp();
                self.emit(Op::Index { dst: t, obj: o, idx: i });
                Ok(t)
            }
            Expr::Unary(op, a) => {
                let av = self.compile_expr(a)?;
                let t = self.temp();
                self.emit(Op::Un { dst: t, op: *op, a: av });
                Ok(t)
            }
            Expr::Binary(op, a, b) => {
                let av = self.compile_expr(a)?;
                let bv = self.compile_expr(b)?;
                let t = self.temp();
                self.emit(Op::Bin { dst: t, op: *op, a: av, b: bv });
                Ok(t)
            }
            Expr::Logical(is_and, a, b) => {
                let av = self.compile_expr(a)?;
                let t = self.temp();
                self.emit(Op::Copy { dst: t, src: av });
                // 短路：|| 真值处出、&& 假值处出（返回操作数原值）。
                let j = self.emit(if *is_and {
                    Op::JumpIfFalse { cond: t, to: 0 }
                } else {
                    Op::JumpIfTrue { cond: t, to: 0 }
                });
                // rhs 可能不被求值——采集禁入。
                self.cond_depth += 1;
                let bv = self.compile_expr(b)?;
                self.cond_depth -= 1;
                self.emit(Op::Copy { dst: t, src: bv });
                self.patch_jump(j);
                Ok(t)
            }
            Expr::Ternary(cond, then, els) => {
                let cnd = self.compile_expr(cond)?;
                let t = self.temp();
                let j_else = self.emit(Op::JumpIfFalse { cond: cnd, to: 0 });
                self.cond_depth += 1;
                let tv = self.compile_expr(then)?;
                self.cond_depth -= 1;
                self.emit(Op::Copy { dst: t, src: tv });
                let j_end = self.emit(Op::Jump { to: 0 });
                self.patch_jump(j_else);
                self.cond_depth += 1;
                let ev = self.compile_expr(els)?;
                self.cond_depth -= 1;
                self.emit(Op::Copy { dst: t, src: ev });
                self.patch_jump(j_end);
                Ok(t)
            }
            Expr::Assign(target, value) => self.compile_assign(target, value),
            Expr::ArrayLit(items) => {
                let mut slots = Vec::with_capacity(items.len());
                for it in items {
                    slots.push(self.compile_expr(it)?);
                }
                let t = self.temp();
                self.emit(Op::ArrLit { dst: t, items: slots });
                Ok(t)
            }
            Expr::Object(fields) => {
                let mut fs = Vec::with_capacity(fields.len());
                for (k, v) in fields {
                    let ki = self.const_s(k);
                    let vs = self.compile_expr(v)?;
                    fs.push((ki, vs));
                }
                let t = self.temp();
                self.emit(Op::ObjLit { dst: t, fields: fs });
                Ok(t)
            }
            Expr::Template(parts) => {
                let mut ops = Vec::with_capacity(parts.len());
                for p in parts {
                    match p {
                        TplPart::Lit(s) => ops.push(TplOp::Lit(self.const_s(s))),
                        TplPart::Expr(x) => ops.push(TplOp::Slot(self.compile_expr(x)?)),
                    }
                }
                let t = self.temp();
                self.emit(Op::Template { dst: t, parts: ops });
                Ok(t)
            }
            Expr::Call { obj, name, args } => self.compile_call(obj.as_deref(), name, args),
        }
    }

    fn compile_assign(&mut self, target: &Expr, value: &Expr) -> Result<u16, String> {
        // ctx.fillStyle = <str> → facet 直写。
        if let Expr::Member(base, name) = target {
            if let Expr::Ident(b) = &**base {
                if self.is_ctx_capture(b) {
                    if name != "fillStyle" {
                        return Err(format!("unsupported ctx assignment `.{name}`"));
                    }
                    let v = self.compile_expr(value)?;
                    self.emit(Op::FacetFillStyle { val: v });
                    return Ok(v);
                }
            }
        }
        let v = self.compile_expr(value)?;
        match target {
            Expr::Ident(n) => {
                let slot = self
                    .scope
                    .get(n)
                    .copied()
                    .ok_or_else(|| format!("assignment to unknown identifier `{n}`"))?;
                self.emit(Op::Copy { dst: slot, src: v });
            }
            Expr::Member(base, name) => {
                let o = self.compile_expr(base)?;
                let ni = self.const_s(name);
                self.emit(Op::SetMember { obj: o, name: ni, val: v });
            }
            Expr::Index(base, idx) => {
                let o = self.compile_expr(base)?;
                let i = self.compile_expr(idx)?;
                self.emit(Op::SetIndex { obj: o, idx: i, val: v });
            }
            _ => return Err("assignment to non-assignable target".into()),
        }
        Ok(v)
    }

    fn compile_call(
        &mut self,
        obj: Option<&Expr>,
        name: &str,
        args: &[Expr],
    ) -> Result<u16, String> {
        // 成员调用。
        if let Some(base) = obj {
            if let Expr::Ident(b) = base {
                // Math.* 白名单。
                if b == "Math" {
                    if name == "imul" {
                        if args.len() != 2 {
                            return Err("Math.imul expects 2 args".into());
                        }
                        let a = self.compile_expr(&args[0])?;
                        let bb = self.compile_expr(&args[1])?;
                        let t = self.temp();
                        self.emit(Op::Bin { dst: t, op: BinOp::Imul, a, b: bb });
                        return Ok(t);
                    }
                    let Some((_, f)) = MATH_WHITELIST.iter().find(|(n, _)| *n == name) else {
                        return Err(format!("Math.{name} not in whitelist"));
                    };
                    let f = *f;
                    let mut slots = Vec::with_capacity(args.len());
                    for a in args {
                        slots.push(self.compile_expr(a)?);
                    }
                    let t = self.temp();
                    self.emit(Op::Math { dst: t, f, args: slots });
                    return Ok(t);
                }
                // ctx facet 方法。
                if self.is_ctx_capture(b) {
                    return self.compile_facet_call(name, args);
                }
            }
            // 数字方法：toFixed（位数必须是字面量）。
            if name == "toFixed" {
                if args.len() != 1 {
                    return Err("toFixed expects 1 arg".into());
                }
                let Expr::Num(d) = &args[0] else {
                    return Err("toFixed digits must be a literal".into());
                };
                if !(0.0..=100.0).contains(d) {
                    return Err("toFixed digits out of range".into());
                }
                let o = self.compile_expr(base)?;
                let t = self.temp();
                self.emit(Op::ToFixed { dst: t, src: o, digits: *d as u16 });
                return Ok(t);
            }
            return Err(format!("unsupported method call `.{name}`"));
        }

        // 裸调用：PRNG 采集 → 页面函数。
        if let Some(cap) = self.capture_ordinal(name) {
            if !args.is_empty() {
                return Err(format!("capture `{name}` must be called with zero args"));
            }
            // 采集点必须落在最内层循环体的无条件位置。
            if self.in_fn || self.cond_depth > 0 || self.depth != self.max_depth {
                return Err(format!("capture `{name}` in conditional/non-innermost position"));
            }
            let t = self.temp();
            self.emit(Op::CallCapture { dst: t, cap });
            return Ok(t);
        }
        if self.fn_src.contains_key(name) {
            let fid = self.compile_fn(name)?;
            let mut slots = Vec::with_capacity(args.len());
            for a in args {
                slots.push(self.compile_expr(a)?);
            }
            let t = self.temp();
            self.emit(Op::CallFn { dst: t, fid, args: slots });
            return Ok(t);
        }
        Err(format!("unknown call `{name}` (not a page function or capture)"))
    }

    fn compile_facet_call(&mut self, name: &str, args: &[Expr]) -> Result<u16, String> {
        let t = self.temp();
        match name {
            "beginPath" if args.is_empty() => {
                self.emit(Op::FacetBeginPath);
            }
            "arc" if args.len() == 5 => {
                let mut slots = Vec::with_capacity(5);
                for a in args {
                    slots.push(self.compile_expr(a)?);
                }
                self.emit(Op::FacetArc { args: slots });
            }
            "fill" if args.is_empty() => {
                self.emit(Op::FacetFill);
            }
            _ => return Err(format!("unsupported ctx method `{name}`")),
        }
        self.emit(Op::ConstUndef { dst: t });
        Ok(t)
    }

    /// 编译页面函数为独立代码对象（CALL/RET 帧；帧局部槽位）。
    fn compile_fn(&mut self, name: &str) -> Result<u16, String> {
        if let Some(&fid) = self.fn_ids.get(name) {
            return Ok(fid);
        }
        let src = self
            .fn_src
            .get(name)
            .ok_or_else(|| format!("function `{name}` source not found"))?
            .clone();
        // 先占位（图级环检测已拦截真递归；防御性防重入死循环）。
        let fid = self.prog.fns.len() as u16;
        self.prog.fns.push(FnMeta {
            start: 0,
            n_slots: 0,
            n_params: 0,
            captured: Vec::new(),
        });
        self.fn_bodies.push(Vec::new());
        self.fn_ids.insert(name.to_string(), fid);

        let fd = parse::Parser::parse_fn(&src).map_err(|e| format!("function `{name}`: {e}"))?;
        let n_params = fd.params.len() as u16;

        // 自由标识符 = Ident 引用 − 本地绑定 → 必须命中全局帧（args/循环变量）。
        let mut locals: Vec<String> = fd.params.clone();
        collect_locals_stmts(&fd.body, &mut locals);
        let mut free: Vec<String> = Vec::new();
        collect_free_idents_stmts(&fd.body, &locals, &mut free);
        let mut captured: Vec<(String, u16)> = Vec::new();
        for fname in &free {
            let gs = if let Some(i) = self.prog.arg_names.iter().position(|n| n == fname) {
                i as u16
            } else if *fname == self.prog.loop_var {
                self.prog.n_globals - 1
            } else {
                return Err(format!(
                    "function `{name}` references `{fname}` not present in annotation args"
                ));
            };
            captured.push((fname.clone(), gs));
        }

        // 帧切换：独立 code / 作用域 / 上下文旗标。
        let saved_code = std::mem::replace(&mut self.prog.code, Vec::new());
        let saved_slots = self.prog.n_slots;
        let saved_scope = std::mem::take(&mut self.scope);
        let saved_depth = self.depth;
        let saved_cond = self.cond_depth;
        let saved_in_fn = self.in_fn;
        self.depth = 0;
        self.cond_depth = 0;
        self.in_fn = true;

        // 帧局部布局：参数 0..p，俘获全局不占局部槽（全局编码直读），
        // let 从 p 起。
        self.prog.n_slots = n_params;
        for (i, p) in fd.params.iter().enumerate() {
            self.scope.insert(p.clone(), i as u16);
        }
        for (fname, gs) in &captured {
            self.scope.insert(fname.clone(), GLOBAL_FLAG | gs);
        }

        let res = self.compile_stmts(&fd.body);
        if res.is_ok() {
            // 隐式返回：无 `return` 的路径漏出时交 undefined。
            if !matches!(self.prog.code.last(), Some(Op::Ret { .. })) {
                let t = self.temp();
                self.emit(Op::ConstUndef { dst: t });
                self.emit(Op::Ret { val: t });
            }
        }

        self.in_fn = saved_in_fn;
        self.cond_depth = saved_cond;
        self.depth = saved_depth;
        self.scope = saved_scope;
        let fn_code = std::mem::replace(&mut self.prog.code, saved_code);
        let fn_slots = self.prog.n_slots;
        self.prog.n_slots = saved_slots;

        res?;
        self.fn_bodies[fid as usize] = fn_code;
        self.prog.fns[fid as usize] = FnMeta {
            start: 0, // finalize 阶段回填
            n_slots: fn_slots,
            n_params,
            captured,
        };
        Ok(fid)
    }
}
