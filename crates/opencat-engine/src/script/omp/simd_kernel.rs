//! std::simd lane kernel 栈（`parallel for simd` / `for simd` 构造的正统
//! 实现面，TR15 §18.5）。nightly-only：`portable_simd` 由 crate root 的
//! `cfg_attr(feature = "simd")` 门控（default off——CI stable 不挂）。
//!
//! 三栈里的第三栈：isolate 团队（通用 JS 语义）之外的「直线算术循环 lane
//! 化」专用后端。ir.body（行体文本）经 oxc 解析 + 白名单验证，转成 owned
//! 表达式树；执行 = 树解释器在 lane 域跑（f64x8 一批 8 点），行间 rayon
//! 并行（mod.rs 侧 `rows_collect!`）。与标量域（f64 单点）同一棵树、同一
//! 段解释器（`Domain` trait 双实现）——`OC_OMP_VERIFY=1` 时每行 lane 产出
//! 与标量重算逐位对拍（f64x8 每 lane 是 IEEE f64；sin/cos/exp/log/atan
//! 逐 lane 调 libm，两域应逐位一致）。
//!
//! lane 语义（TR15 §18.5 的实现近似）：全 lane 同步执行 + active 面抑制
//! ——分支不写 per-lane PC，条件外的赋值/写产出被抑制；break/continue
//! 是 per-lane flag。线性迭代 + masked exits，直线算术循环下与逐点语义
//! 等价。已知近似（docs/js-openmp.md §4.1）：`&&`/`||` 无短路（lane 域布尔
//! 化）；Math.min/max 的 ±0 分辨不复刻（NaN 传播复刻）；页面函数调用、
//! 数组读、prng capture、reduction 不进 lane kernel——prepare 拒绝，循
//! 环回落 isolate 后端（不毒化，isolate 下语义等价）。

use std::sync::Arc;

// portable_simd 的方法面：simd_eq/ne/lt/le/gt/ge 在 `SimdPartialEq/Ord`，
// select 在 `Select`（本 nightly 的 `Mask` 无 to_int——布尔化在
// to_f64x8 用 select 归一 {0.0, 1.0}，cast 的 true=-1.0 会污染布尔算术；
// splat/from_array 是 `Simd` 关联函数）。
use std::simd::prelude::{Select, SimdFloat, SimdInt, SimdPartialEq, SimdPartialOrd};

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    AssignmentOperator, BinaryOperator, CallExpression, Expression, ForStatementInit,
    LogicalOperator, Statement, UnaryOperator, UpdateOperator, VariableDeclarationKind,
};
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde_json::Value as JsonValue;

// ── owned 表达式树（oxc AST 的一次性扁平化，Kernel 自持无借用）────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    LAnd,
    LOr,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum MathFn {
    Abs,
    Min,
    Max,
    Floor,
    Ceil,
    Round,
    Sqrt,
    Hypot,
    Sin,
    Cos,
    Exp,
    Log,
    Atan,
    Atan2,
    Pow,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum AssignOp {
    Eq,
    AddEq,
    SubEq,
    MulEq,
    DivEq,
    RemEq,
}

#[derive(Debug, Clone)]
enum Expr {
    Num(f64),
    Var(usize),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Assign(AssignOp, usize, Box<Expr>),
    /// 三元（lane 域 select）。
    Cond(Box<Expr>, Box<Expr>, Box<Expr>),
    CallMath(MathFn, Vec<Expr>),
    /// `arr[idx] = v` / `arr[idx] += v`：写产出（arr = ir args 槽位号，
    /// host 应用面 = isolate Set/Acc 同款 IsolateWrite）。
    SetArr(usize, bool, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone)]
enum Stmt {
    /// `let/const x = e`（未初始化 = None）。同块重名 = prepare 拒绝。
    Decl(Vec<(usize, Option<Expr>)>),
    Expr(Expr),
    /// 裸块 = 顺序执行（无独立作用域；break/continue 穿透给最近的 For）。
    Block(Vec<Stmt>),
    /// `for (init; test; update) body`（update 已脱糖成赋值表达式）。
    For(
        Option<Vec<(usize, Option<Expr>)>>,
        Option<Expr>,
        Option<Expr>,
        Vec<Stmt>,
    ),
    If(Box<Expr>, Vec<Stmt>, Option<Vec<Stmt>>),
    Break,
    Continue,
}

pub struct Kernel {
    /// 外层循环变量的槽号（header init 的 `let` 声明分配；simd 栈不依赖
    /// 任何 JS 引擎，行值枚举 = 标量域跑一遍 header）。
    header_var: usize,
    header_init: Option<Expr>,
    header_test: Option<Expr>,
    header_update: Option<Expr>,
    body: Vec<Stmt>,
    /// plain 实参名（按 ir args 序；数组槽不在内）。
    arg_names: Vec<String>,
    /// plain 实参的槽号（与 arg_names 同序——执行入口按此赋 slots）。
    arg_slots: Vec<usize>,
    /// 数组写回目标名 → ir args 槽位（产出 IsolateWrite.arg）。
    array_names: Vec<(String, usize)>,
    /// 槽总数。
    n_slots: usize,
}

impl Kernel {
    /// 写产出通道号 → ir args 槽位（IsolateWrite.arg）。
    pub fn array_arg_pos(&self, channel: usize) -> Option<usize> {
        self.array_names.get(channel).map(|(_, pos)| *pos)
    }
}

// ── oxc 解析 + 白名单转 owned 树 ──────────────────────────────────────

struct Compiler {
    /// 名字 → 槽位。实参先入（外层循环变量 0 号），body 声明追加。
    names: std::collections::HashMap<String, usize>,
    n_slots: usize,
    arrays: Vec<(String, usize)>,
    /// 嵌套深度（防炸栈）。
    depth: u32,
}

type CResult<T> = Result<T, String>;

impl Compiler {
    fn slot(&mut self, name: &str) -> CResult<usize> {
        self.names
            .get(name)
            .copied()
            .ok_or_else(|| format!("undeclared identifier `{name}` (simd kernel is closure-free)"))
    }
    fn decl_slot(&mut self, name: &str) -> CResult<usize> {
        if self.names.contains_key(name) {
            return Err(format!("redeclaration of `{name}`"));
        }
        let s = self.n_slots;
        self.names.insert(name.to_string(), s);
        self.n_slots += 1;
        Ok(s)
    }
    fn array_slot(&mut self, name: &str) -> CResult<usize> {
        if let Some((_, k)) = self.arrays.iter().find(|(n, _)| n == name) {
            return Ok(*k);
        }
        self.arrays.push((name.to_string(), self.arrays.len()));
        Ok(self.arrays.len() - 1)
    }

    fn stmts(&mut self, stmts: &[Statement<'_>]) -> CResult<Vec<Stmt>> {
        self.depth += 1;
        if self.depth > 32 {
            return Err("nesting too deep".into());
        }
        let mut out = Vec::with_capacity(stmts.len());
        for s in stmts {
            out.push(self.stmt(s)?);
        }
        self.depth -= 1;
        Ok(out)
    }

    fn stmt(&mut self, s: &Statement<'_>) -> CResult<Stmt> {
        match s {
            Statement::EmptyStatement(_) => Ok(Stmt::Expr(Expr::Num(0.0))),
            Statement::BreakStatement(_) => Ok(Stmt::Break),
            Statement::ContinueStatement(_) => Ok(Stmt::Continue),
            Statement::BlockStatement(b) => Ok(Stmt::Block(self.stmts(&b.body)?)),
            Statement::VariableDeclaration(v) => Ok(Stmt::Decl(self.var_decl(v)?)),
            Statement::ExpressionStatement(e) => Ok(Stmt::Expr(self.expr(&e.expression)?)),
            Statement::ForStatement(f) => {
                let init = match &f.init {
                    Some(ForStatementInit::VariableDeclaration(v)) => Some(self.var_decl(v)?),
                    None => None,
                    _ => return Err("for init: only `let` declarations supported".into()),
                };
                let test = match &f.test {
                    Some(e) => Some(self.expr(e)?),
                    None => None,
                };
                // update 位置收 `x += c` / `x++` / 任意赋值表达式。
                let update = match &f.update {
                    Some(e) => Some(self.expr(e)?),
                    None => None,
                };
                let body = match &f.body {
                    Statement::BlockStatement(b) => self.stmts(&b.body)?,
                    other => self.stmts(std::slice::from_ref(other))?,
                };
                Ok(Stmt::For(init, test, update, body))
            }
            Statement::IfStatement(i) => {
                let cond = self.expr(&i.test)?;
                let then = match &i.consequent {
                    Statement::BlockStatement(b) => self.stmts(&b.body)?,
                    other => self.stmts(std::slice::from_ref(other))?,
                };
                let els = match &i.alternate {
                    Some(Statement::BlockStatement(b)) => Some(self.stmts(&b.body)?),
                    Some(other) => Some(self.stmts(std::slice::from_ref(other))?),
                    None => None,
                };
                Ok(Stmt::If(Box::new(cond), then, els))
            }
            _ => Err("unsupported statement in simd kernel".into()),
        }
    }

    fn var_decl(
        &mut self,
        v: &oxc_ast::ast::VariableDeclaration<'_>,
    ) -> CResult<Vec<(usize, Option<Expr>)>> {
        if v.kind != VariableDeclarationKind::Let && v.kind != VariableDeclarationKind::Const {
            return Err("`var` declarations not supported in simd kernel".into());
        }
        let mut out = Vec::with_capacity(v.declarations.len());
        for d in &v.declarations {
            let oxc_ast::ast::BindingPattern::BindingIdentifier(id) = &d.id else {
                return Err("destructuring not supported".into());
            };
            let slot = self.decl_slot(&id.name)?;
            let init = match &d.init {
                Some(e) => Some(self.expr(e)?),
                None => None,
            };
            out.push((slot, init));
        }
        Ok(out)
    }

    fn expr(&mut self, e: &Expression<'_>) -> CResult<Expr> {
        self.depth += 1;
        if self.depth > 64 {
            return Err("expression too deep".into());
        }
        let out = self.expr_inner(e);
        self.depth -= 1;
        out
    }

    fn expr_inner(&mut self, e: &Expression<'_>) -> CResult<Expr> {
        match e.get_inner_expression() {
            Expression::NumericLiteral(n) => Ok(Expr::Num(n.value)),
            Expression::Identifier(id) => Ok(Expr::Var(self.slot(&id.name)?)),
            Expression::BinaryExpression(b) => {
                let op = match b.operator {
                    BinaryOperator::Addition => BinOp::Add,
                    BinaryOperator::Subtraction => BinOp::Sub,
                    BinaryOperator::Multiplication => BinOp::Mul,
                    BinaryOperator::Division => BinOp::Div,
                    BinaryOperator::Remainder => BinOp::Rem,
                    BinaryOperator::LessThan => BinOp::Lt,
                    BinaryOperator::LessEqualThan => BinOp::Le,
                    BinaryOperator::GreaterThan => BinOp::Gt,
                    BinaryOperator::GreaterEqualThan => BinOp::Ge,
                    BinaryOperator::Equality => BinOp::Eq,
                    BinaryOperator::Inequality => BinOp::Ne,
                    _ => return Err("unsupported binary op".into()),
                };
                Ok(Expr::Bin(
                    op,
                    Box::new(self.expr(&b.left)?),
                    Box::new(self.expr(&b.right)?),
                ))
            }
            Expression::LogicalExpression(l) => {
                let op = match l.operator {
                    LogicalOperator::And => BinOp::LAnd,
                    LogicalOperator::Or => BinOp::LOr,
                    _ => return Err("?? not supported".into()),
                };
                Ok(Expr::Bin(
                    op,
                    Box::new(self.expr(&l.left)?),
                    Box::new(self.expr(&l.right)?),
                ))
            }
            Expression::UnaryExpression(u) => match u.operator {
                UnaryOperator::UnaryNegation => Ok(Expr::Neg(Box::new(self.expr(&u.argument)?))),
                UnaryOperator::LogicalNot => Ok(Expr::Not(Box::new(self.expr(&u.argument)?))),
                UnaryOperator::UnaryPlus => self.expr(&u.argument),
                _ => Err("typeof/void/delete not supported".into()),
            },
            Expression::ConditionalExpression(c) => Ok(Expr::Cond(
                Box::new(self.expr(&c.test)?),
                Box::new(self.expr(&c.consequent)?),
                Box::new(self.expr(&c.alternate)?),
            )),
            Expression::AssignmentExpression(a) => {
                let op = match a.operator {
                    AssignmentOperator::Assign => AssignOp::Eq,
                    AssignmentOperator::Addition => AssignOp::AddEq,
                    AssignmentOperator::Subtraction => AssignOp::SubEq,
                    AssignmentOperator::Multiplication => AssignOp::MulEq,
                    AssignmentOperator::Division => AssignOp::DivEq,
                    AssignmentOperator::Remainder => AssignOp::RemEq,
                    _ => return Err("unsupported assignment op".into()),
                };
                match &a.left {
                    // INHERIT 枚举（SimpleAssignmentTarget 的变体平铺进
                    // AssignmentTarget），直接按平铺变体匹配。
                    oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(id) => {
                        let slot = self.slot(&id.name)?;
                        Ok(Expr::Assign(op, slot, Box::new(self.expr(&a.right)?)))
                    }
                    oxc_ast::ast::AssignmentTarget::ComputedMemberExpression(m) => {
                        // `arr[expr] = v`：Set/Acc 写产出（simd 面独立识别，
                        // 产出通道与 isolate 同款 IsolateWrite）。
                        let Expression::Identifier(obj) = m.object.get_inner_expression() else {
                            return Err("array write target must be a plain name".into());
                        };
                        let arr = self.array_slot(&obj.name)?;
                        let idx = self.expr(&m.expression)?;
                        let val = self.expr(&a.right)?;
                        Ok(Expr::SetArr(arr, op != AssignOp::Eq, Box::new(idx), Box::new(val)))
                    }
                    _ => Err("unsupported assignment target".into()),
                }
            }
            Expression::UpdateExpression(u) => {
                // `x++` / `--x`（for 头惯用面）→ 复合赋值。
                let op = match u.operator {
                    UpdateOperator::Increment => AssignOp::AddEq,
                    UpdateOperator::Decrement => AssignOp::SubEq,
                };
                let oxc_ast::ast::SimpleAssignmentTarget::AssignmentTargetIdentifier(id) =
                    &u.argument
                else {
                    return Err("++/-- target must be a plain identifier".into());
                };
                let slot = self.slot(&id.name)?;
                Ok(Expr::Assign(op, slot, Box::new(Expr::Num(1.0))))
            }
            Expression::CallExpression(c) => self.call(c),
            _ => Err("unsupported expression in simd kernel".into()),
        }
    }

    fn call(&mut self, c: &CallExpression<'_>) -> CResult<Expr> {
        // Set/Acc 协议调用（transform 文本级改写产物：`arr[i] = v` →
        // `__ocOmpSet(arr, i, v)`、`arr[i] += v` → `__ocOmpAcc(arr, i, v)`）
        // → 数组写产出（与 ComputedMember 赋值同款 SetArr；arr 须已在
        // ir captures 注册的数组槽）。
        if let Expression::Identifier(id) = c.callee.get_inner_expression() {
            if id.name == "__ocOmpSet" || id.name == "__ocOmpAcc" {
                let [a, i, v] = c.arguments.as_slice() else {
                    return Err(format!("{}(arr, idx, val) expects 3 args", id.name));
                };
                let Some(ae) = a.as_expression() else {
                    return Err(format!("{} arr arg not an expression", id.name));
                };
                let Expression::Identifier(arr) = ae.get_inner_expression() else {
                    return Err(format!("{} arr arg must be a plain name", id.name));
                };
                let channel = self.array_slot(&arr.name)?;
                let Some(ie) = i.as_expression() else {
                    return Err(format!("{} idx arg not an expression", id.name));
                };
                let Some(ve) = v.as_expression() else {
                    return Err(format!("{} val arg not an expression", id.name));
                };
                return Ok(Expr::SetArr(
                    channel,
                    id.name == "__ocOmpAcc",
                    Box::new(self.expr(ie)?),
                    Box::new(self.expr(ve)?),
                ));
            }
            // 其余裸名调用 = 页面函数 / PRNG，拒绝（lane 面不做预采语义）。
            return Err(format!(
                "bare call `{}` (prng/page fn) not supported in simd kernel",
                id.name
            ));
        }
        // 只收 `Math.fn(...)` 成员调用（object 须为 Math 标识符）。
        let Expression::StaticMemberExpression(m) = c.callee.get_inner_expression() else {
            return Err("only Math.* calls supported in simd kernel".into());
        };
        let Expression::Identifier(obj) = m.object.get_inner_expression() else {
            return Err("only Math.* calls supported in simd kernel".into());
        };
        if obj.name != "Math" {
            return Err(format!("only Math.* calls supported (got `{}`)", obj.name));
        }
        let args: Vec<Expr> = c
            .arguments
            .iter()
            .map(|a| match a.as_expression() {
                Some(e) => self.expr(e),
                None => Err("spread args not supported".into()),
            })
            .collect::<CResult<_>>()?;
        if args.is_empty() {
            return Err(format!("Math.{}() with no args not supported", m.property.name));
        }
        let Some(fn_kind) = math_fn(&m.property.name) else {
            return Err(format!("unsupported function `Math.{}`", m.property.name));
        };
        Ok(Expr::CallMath(fn_kind, args))
    }
}

fn math_fn(name: &str) -> Option<MathFn> {
    Some(match name {
        "abs" => MathFn::Abs,
        "min" => MathFn::Min,
        "max" => MathFn::Max,
        "floor" => MathFn::Floor,
        "ceil" => MathFn::Ceil,
        "round" => MathFn::Round,
        "sqrt" => MathFn::Sqrt,
        "hypot" => MathFn::Hypot,
        "sin" => MathFn::Sin,
        "cos" => MathFn::Cos,
        "exp" => MathFn::Exp,
        "log" => MathFn::Log,
        "atan" => MathFn::Atan,
        "atan2" => MathFn::Atan2,
        "pow" => MathFn::Pow,
        _ => return None,
    })
}

// ── prepare：ir → Kernel（header + body 两段，共用槽表）───────────────

pub fn prepare(ir: &JsonValue) -> Result<Arc<Kernel>, String> {
    if ir
        .get("reduction")
        .and_then(|v| v.as_array())
        .is_some_and(|r| !r.is_empty())
    {
        return Err("reduction not supported in simd kernel".into());
    }
    let args = ir
        .get("args")
        .and_then(|v| v.as_array())
        .ok_or("args missing")?;
    let captures = ir
        .get("captures")
        .and_then(|v| v.as_object())
        .ok_or("captures missing")?;
    let var = ir
        .get("var")
        .and_then(|v| v.as_str())
        .ok_or("var missing")?
        .to_string();
    let header = ir
        .get("header")
        .and_then(|v| v.as_str())
        .ok_or("header missing")?
        .to_string();
    let body = ir
        .get("body")
        .and_then(|v| v.as_str())
        .ok_or("body missing")?
        .to_string();

    let mut c = Compiler {
        names: std::collections::HashMap::new(),
        n_slots: 0,
        arrays: Vec::new(),
        depth: 0,
    };
    // 槽布局：plain 实参先注册（槽 0..n_plain），外层循环变量由 header 的
    // init `let` 声明分配（var 不预注册——decl_slot 重名拒绝）。prng/array/
    // ctx 槽不在值域——数组走写产出通道，prng/ctx 直接拒绝。
    let mut arg_names = Vec::with_capacity(args.len());
    let mut arg_slots = Vec::with_capacity(args.len());
    for a in args {
        let name = a.as_str().ok_or("args item not a string")?;
        match captures.get(name).and_then(|v| v.as_str()) {
            // 数组槽 = 写产出通道（SetArr→IsolateWrite），不占值槽。
            Some("array") => {
                c.array_slot(name)?;
            }
            None | Some("shared") => {
                arg_names.push(name.to_string());
                arg_slots.push(c.decl_slot(name)?);
            }
            Some(k) => return Err(format!("capture kind `{k}` not supported in simd kernel")),
        }
    }

    // header 三段：拼等价 for 源走同一编译面，摘 init/test/update。
    let header_src = format!("for ({header}) {{ }}");
    let (header_init, header_test, header_update) = with_program(&header_src, |program| {
        let Some(Statement::ForStatement(f)) = program.body.first() else {
            return Err("header not a for".into());
        };
        let init = match &f.init {
            Some(ForStatementInit::VariableDeclaration(v)) => {
                let d = c.var_decl(v).map_err(|e| format!("header init: {e}"))?;
                d.into_iter().next().and_then(|(_, e)| e)
            }
            _ => None,
        };
        let test = match &f.test {
            Some(t) => Some(c.expr(t).map_err(|e| format!("header test: {e}"))?),
            None => None,
        };
        let update = match &f.update {
            Some(u) => Some(c.expr(u).map_err(|e| format!("header update: {e}"))?),
            None => None,
        };
        Ok((init, test, update))
    })?;
    let header_var = *c
        .names
        .get(&var)
        .ok_or_else(|| "loop var not declared by header init".to_string())?;

    // body：共用槽表（header 声明的名字可见——行体引用外层变量与实参）。
    let body_src = format!("{{ {body} }}");
    let stmts = with_program(&body_src, |program| c.stmts(&program.body))
        .map_err(|e| format!("body: {e}"))?;
    let arrays = c
        .arrays
        .into_iter()
        .map(|(name, ch)| {
            let _ = ch;
            let pos = args
                .iter()
                .position(|a| a.as_str() == Some(name.as_str()))
                .ok_or_else(|| format!("array `{name}` not in args"))?;
            Ok((name, pos))
        })
        .collect::<CResult<Vec<_>>>()?;

    Ok(Arc::new(Kernel {
        header_var,
        header_init,
        header_test,
        header_update,
        body: stmts,
        arg_names,
        arg_slots,
        array_names: arrays,
        n_slots: c.n_slots,
    }))
}

fn with_program<T>(
    src: &str,
    f: impl FnOnce(&oxc_ast::ast::Program<'_>) -> Result<T, String>,
) -> Result<T, String> {
    let allocator = Allocator::new();
    let parsed = Parser::new(&allocator, src, SourceType::mjs()).parse();
    if parsed.diagnostics.has_errors() {
        let first = parsed
            .diagnostics
            .first()
            .map(|d| d.message.to_string())
            .unwrap_or_default();
        return Err(format!("parse: {first}"));
    }
    f(&parsed.program)
}

// ── 执行域：标量 f64 与 lane f64x8 同一棵树 ──────────────────────────

trait Domain: Copy {
    type V: Copy + Default;
    const LANES: usize;
    fn splat(v: f64) -> Self::V;
    fn lane(v: Self::V, i: usize) -> f64;
    fn from_lanes(lanes: [f64; 8]) -> Self::V;
    fn add(a: Self::V, b: Self::V) -> Self::V;
    fn sub(a: Self::V, b: Self::V) -> Self::V;
    fn mul(a: Self::V, b: Self::V) -> Self::V;
    fn div(a: Self::V, b: Self::V) -> Self::V;
    fn rem(a: Self::V, b: Self::V) -> Self::V;
    /// 比较产出布尔域（true = 1.0）。
    fn lt(a: Self::V, b: Self::V) -> Self::V;
    fn le(a: Self::V, b: Self::V) -> Self::V;
    fn gt(a: Self::V, b: Self::V) -> Self::V;
    fn ge(a: Self::V, b: Self::V) -> Self::V;
    fn eq(a: Self::V, b: Self::V) -> Self::V;
    fn ne(a: Self::V, b: Self::V) -> Self::V;
    /// select(c, t, f)：布尔域非零取 t。
    fn cond(c: Self::V, t: Self::V, f: Self::V) -> Self::V;
    /// 布尔化（非零 → 1.0）。
    fn truthy(c: Self::V) -> Self::V;
    fn any_true(mask: Self::V) -> bool;
    fn none_true(mask: Self::V) -> bool;
}

#[derive(Clone, Copy)]
struct Scalar;

impl Domain for Scalar {
    type V = f64;
    const LANES: usize = 1;
    fn splat(v: f64) -> f64 {
        v
    }
    fn lane(v: f64, _: usize) -> f64 {
        v
    }
    fn from_lanes(lanes: [f64; 8]) -> f64 {
        lanes[0]
    }
    fn add(a: f64, b: f64) -> f64 {
        a + b
    }
    fn sub(a: f64, b: f64) -> f64 {
        a - b
    }
    fn mul(a: f64, b: f64) -> f64 {
        a * b
    }
    fn div(a: f64, b: f64) -> f64 {
        a / b
    }
    fn rem(a: f64, b: f64) -> f64 {
        // JS % = fmod（符号随被除数）。
        a % b
    }
    fn lt(a: f64, b: f64) -> f64 {
        (a < b) as u8 as f64
    }
    fn le(a: f64, b: f64) -> f64 {
        (a <= b) as u8 as f64
    }
    fn gt(a: f64, b: f64) -> f64 {
        (a > b) as u8 as f64
    }
    fn ge(a: f64, b: f64) -> f64 {
        (a >= b) as u8 as f64
    }
    fn eq(a: f64, b: f64) -> f64 {
        (a == b) as u8 as f64
    }
    fn ne(a: f64, b: f64) -> f64 {
        (a != b) as u8 as f64
    }
    fn cond(c: f64, t: f64, f: f64) -> f64 {
        if c != 0.0 {
            t
        } else {
            f
        }
    }
    fn truthy(c: f64) -> f64 {
        (c != 0.0) as u8 as f64
    }
    fn any_true(m: f64) -> bool {
        m != 0.0
    }
    fn none_true(m: f64) -> bool {
        m == 0.0
    }
}

#[derive(Clone, Copy)]
struct Lane8;

impl Domain for Lane8 {
    type V = std::simd::f64x8;
    const LANES: usize = 8;
    fn splat(v: f64) -> Self::V {
        std::simd::f64x8::splat(v)
    }
    fn lane(v: Self::V, i: usize) -> f64 {
        v.as_array()[i]
    }
    fn from_lanes(lanes: [f64; 8]) -> Self::V {
        std::simd::f64x8::from_array(lanes)
    }
    fn add(a: Self::V, b: Self::V) -> Self::V {
        a + b
    }
    fn sub(a: Self::V, b: Self::V) -> Self::V {
        a - b
    }
    fn mul(a: Self::V, b: Self::V) -> Self::V {
        a * b
    }
    fn div(a: Self::V, b: Self::V) -> Self::V {
        a / b
    }
    fn rem(a: Self::V, b: Self::V) -> Self::V {
        a % b
    }
    fn lt(a: Self::V, b: Self::V) -> Self::V {
        to_f64x8(a.simd_lt(b))
    }
    fn le(a: Self::V, b: Self::V) -> Self::V {
        to_f64x8(a.simd_le(b))
    }
    fn gt(a: Self::V, b: Self::V) -> Self::V {
        to_f64x8(a.simd_gt(b))
    }
    fn ge(a: Self::V, b: Self::V) -> Self::V {
        to_f64x8(a.simd_ge(b))
    }
    fn eq(a: Self::V, b: Self::V) -> Self::V {
        to_f64x8(a.simd_eq(b))
    }
    fn ne(a: Self::V, b: Self::V) -> Self::V {
        to_f64x8(a.simd_ne(b))
    }
    fn cond(c: Self::V, t: Self::V, f: Self::V) -> Self::V {
        c.simd_ne(Self::splat(0.0)).select(t, f)
    }
    fn truthy(c: Self::V) -> Self::V {
        to_f64x8(c.simd_ne(Self::splat(0.0)))
    }
    fn any_true(m: Self::V) -> bool {
        m.simd_ne(Self::splat(0.0)).any()
    }
    fn none_true(m: Self::V) -> bool {
        !m.simd_ne(Self::splat(0.0)).any()
    }
}

fn to_f64x8(m: std::simd::Mask<i64, 8>) -> std::simd::f64x8 {
    // mask lane 原位型 = -1/0（全 1 位型 cast 出 -1.0）——布尔面算术
    // （`1 - truthy(x)`、active 乘积链）依赖 {0.0, 1.0}，select 归一。
    m.select(std::simd::f64x8::splat(1.0), std::simd::f64x8::splat(0.0))
}

// ── 解释器（双域单实现）───────────────────────────────────────────────

/// 行产出条目：数组通道号、下标、是否累加、终值（行内序 = 执行序）。
#[derive(Debug, Clone)]
pub struct SimWrite {
    pub channel: usize,
    pub idx: u32,
    pub acc: bool,
    pub val: f64,
}

struct Flow<D: Domain> {
    /// 本语句面（active = 该 lane/点继续执行）。
    active: D::V,
    brk: D::V,
    cont: D::V,
}

struct Run<'k, D: Domain> {
    kernel: &'k Kernel,
    slots: Vec<D::V>,
    writes: Vec<SimWrite>,
}

impl<D: Domain> Run<'_, D> {
    fn expr(&mut self, e: &Expr, flow: &Flow<D>) -> D::V {
        match e {
            Expr::Num(v) => D::splat(*v),
            Expr::Var(s) => self.slots[*s],
            Expr::Bin(op, a, b) => {
                let (va, vb) = (self.expr(a, flow), self.expr(b, flow));
                match op {
                    BinOp::Add => D::add(va, vb),
                    BinOp::Sub => D::sub(va, vb),
                    BinOp::Mul => D::mul(va, vb),
                    BinOp::Div => D::div(va, vb),
                    BinOp::Rem => D::rem(va, vb),
                    BinOp::Lt => D::lt(va, vb),
                    BinOp::Le => D::le(va, vb),
                    BinOp::Gt => D::gt(va, vb),
                    BinOp::Ge => D::ge(va, vb),
                    BinOp::Eq => D::eq(va, vb),
                    BinOp::Ne => D::ne(va, vb),
                    // lane 域无短路（TR15 simd 语义合法近似）；布尔域 = 乘
                    // / 德摩根。
                    BinOp::LAnd => D::mul(va, vb),
                    BinOp::LOr => D::sub(
                        D::splat(1.0),
                        D::mul(D::sub(D::splat(1.0), D::truthy(va)), D::sub(D::splat(1.0), D::truthy(vb))),
                    ),
                }
            }
            Expr::Neg(a) => D::sub(D::splat(0.0), self.expr(a, flow)),
            Expr::Not(a) => D::sub(D::splat(1.0), D::truthy(self.expr(a, flow))),
            Expr::Assign(op, slot, rhs) => {
                let cur = self.slots[*slot];
                let v = match op {
                    AssignOp::Eq => self.expr(rhs, flow),
                    AssignOp::AddEq => D::add(cur, self.expr(rhs, flow)),
                    AssignOp::SubEq => D::sub(cur, self.expr(rhs, flow)),
                    AssignOp::MulEq => D::mul(cur, self.expr(rhs, flow)),
                    AssignOp::DivEq => D::div(cur, self.expr(rhs, flow)),
                    AssignOp::RemEq => D::rem(cur, self.expr(rhs, flow)),
                };
                // active 面外抑制（lane 语义：条件外的赋值不落）。
                self.slots[*slot] = D::cond(flow.active, v, cur);
                v
            }
            Expr::Cond(c, t, f) => {
                let (vc, vt, vf) = (self.expr(c, flow), self.expr(t, flow), self.expr(f, flow));
                D::cond(vc, vt, vf)
            }
            Expr::CallMath(f, args) => {
                let mut vs: [D::V; 3] = [D::splat(0.0); 3];
                for (i, a) in args.iter().enumerate().take(3) {
                    vs[i] = self.expr(a, flow);
                }
                self.call_math(*f, &vs)
            }
            Expr::SetArr(channel, acc, idx, val) => {
                let vi = self.expr(idx, flow);
                let vv = self.expr(val, flow);
                // += 直接标 acc=true（v8.rs 应用面按「对当前值累加」应用；
                // 协议锁定帧初值 0 → 与 isolate Set/Acc 同约。行内同下标
                // 两次 += = 两次按序累加，字节确定）。
                for l in 0..D::LANES {
                    if D::lane(flow.active, l) != 0.0 {
                        let i = D::lane(vi, l);
                        self.writes.push(SimWrite {
                            channel: *channel,
                            idx: if i >= 0.0 && i.is_finite() { i as u32 } else { u32::MAX },
                            acc: *acc,
                            val: D::lane(vv, l),
                        });
                    }
                }
                D::splat(0.0)
            }
        }
    }

    fn call_math(&mut self, f: MathFn, vs: &[D::V; 3]) -> D::V {
        match f {
            MathFn::Abs => {
                let a = vs[0];
                D::cond(D::lt(a, D::splat(0.0)), D::sub(D::splat(0.0), a), a)
            }
            MathFn::Min => {
                // NaN 传播复刻（±0 分辨不复刻，docs 已知近似）。
                let (a, b) = (vs[0], vs[1]);
                let nan = D::ne(a, a);
                let nan_b = D::cond(D::truthy(nan), D::splat(1.0), D::ne(b, b));
                D::cond(nan_b, D::splat(f64::NAN), D::cond(D::le(a, b), a, b))
            }
            MathFn::Max => {
                let (a, b) = (vs[0], vs[1]);
                let nan = D::ne(a, a);
                let nan_b = D::cond(D::truthy(nan), D::splat(1.0), D::ne(b, b));
                D::cond(nan_b, D::splat(f64::NAN), D::cond(D::ge(a, b), a, b))
            }
            MathFn::Floor => self.map1(vs[0], libm::floor),
            MathFn::Ceil => self.map1(vs[0], libm::ceil),
            MathFn::Round => self.map1(vs[0], |x| (x + 0.5).floor()), // JS 半上
            MathFn::Sqrt => self.map1(vs[0], libm::sqrt),
            MathFn::Sin => self.map1(vs[0], libm::sin),
            MathFn::Cos => self.map1(vs[0], libm::cos),
            MathFn::Exp => self.map1(vs[0], libm::exp),
            MathFn::Log => self.map1(vs[0], libm::log),
            MathFn::Atan => self.map1(vs[0], libm::atan),
            MathFn::Hypot => self.map2(vs[0], vs[1], libm::hypot),
            MathFn::Atan2 => self.map2(vs[0], vs[1], libm::atan2),
            MathFn::Pow => self.map2(vs[0], vs[1], libm::pow),
        }
    }

    fn map1(&mut self, a: D::V, f: impl Fn(f64) -> f64) -> D::V {
        let mut lanes = [0.0f64; 8];
        for (l, o) in lanes.iter_mut().enumerate().take(D::LANES) {
            *o = f(D::lane(a, l));
        }
        D::from_lanes(lanes)
    }

    fn map2(&mut self, a: D::V, b: D::V, f: impl Fn(f64, f64) -> f64) -> D::V {
        let mut lanes = [0.0f64; 8];
        for (l, o) in lanes.iter_mut().enumerate().take(D::LANES) {
            *o = f(D::lane(a, l), D::lane(b, l));
        }
        D::from_lanes(lanes)
    }

    fn stmts(&mut self, stmts: &[Stmt], flow: &mut Flow<D>) {
        for s in stmts {
            self.stmt(s, flow);
            // break/continue 后块内剩余抑制（外层循环的 update 由 For 消
            // 费 cont）。
            if D::none_true(flow.active) {
                break;
            }
            let stop = D::truthy(D::add(flow.brk, flow.cont));
            flow.active = D::mul(flow.active, D::sub(D::splat(1.0), stop));
        }
    }

    fn stmt(&mut self, s: &Stmt, flow: &mut Flow<D>) {
        match s {
            Stmt::Break => flow.brk = flow.active, // 只对 active 面 break
            Stmt::Continue => flow.cont = flow.active,
            Stmt::Block(list) => {
                // 裸块 = 顺序执行（无独立作用域；break/continue 穿透）。
                self.stmts(list, flow);
            }
            Stmt::Expr(e) => {
                let _ = self.expr(e, flow);
            }
            Stmt::Decl(decls) => {
                for (slot, init) in decls {
                    let v = match init {
                        Some(e) => {
                            let v = self.expr(e, flow);
                            D::cond(flow.active, v, D::splat(f64::NAN))
                        }
                        None => D::splat(f64::NAN),
                    };
                    self.slots[*slot] = v;
                }
            }
            Stmt::If(cond, then, els) => {
                let c = D::truthy(self.expr(cond, flow));
                // then 面 = active & c；else 面 = active & !c。break/continue
                // 在分支内冒泡。
                let save = (flow.active, flow.brk, flow.cont);
                flow.active = D::mul(flow.active, c);
                self.stmts(then, flow);
                let (ta, tb, tc) = (flow.active, flow.brk, flow.cont);
                flow.active = save.0;
                flow.brk = save.1;
                flow.cont = save.2;
                if let Some(els) = els {
                    flow.active = D::mul(save.0, D::sub(D::splat(1.0), c));
                    self.stmts(els, flow);
                    flow.active = D::add(flow.active, D::mul(ta, c));
                    flow.brk = D::add(flow.brk, tb);
                    flow.cont = D::add(flow.cont, tc);
                } else {
                    flow.active = D::add(
                        D::mul(save.0, D::sub(D::splat(1.0), c)),
                        D::mul(ta, c),
                    );
                    flow.brk = D::add(flow.brk, tb);
                    flow.cont = D::add(flow.cont, tc);
                }
                flow.active = D::truthy(flow.active);
            }
            Stmt::For(init, test, update, body) => {
                if let Some(decls) = init {
                    self.stmt(&Stmt::Decl(decls.clone()), flow);
                }
                // 循环面 = active & !brk（外层 break 的 lane 不进循环）。
                let mut work = D::mul(flow.active, D::truthy(D::sub(D::splat(1.0), D::truthy(flow.brk))));
                let mut guard = 0u64;
                loop {
                    guard += 1;
                    if guard > 1_000_000 {
                        // 防御边界：非收敛循环（语义面已被 prepare 白名单收
                        // 紧，正常不可达）。
                        panic!("simd for guard tripped");
                    }
                    if let Some(t) = test {
                        let c = D::truthy(self.expr(t, &Flow { active: work, brk: work, cont: work }));
                        work = D::mul(work, c); // cond false → 离开（循环后继续）
                    }
                    if D::none_true(work) {
                        break;
                    }
                    let mut body_flow = Flow {
                        active: work,
                        brk: D::splat(0.0),
                        cont: D::splat(0.0),
                    };
                    self.stmts(body, &mut body_flow);
                    let broke = D::truthy(body_flow.brk);
                    let contd = D::truthy(body_flow.cont);
                    // update 对「未 break」面执行（含 continue 的 lane）。
                    let run_on = D::mul(body_flow.active, D::truthy(D::sub(D::splat(1.0), broke)));
                    if let Some(u) = update {
                        let _ = self.expr(u, &Flow { active: run_on, brk: broke, cont: contd });
                    }
                    // 下一轮面：未 break 的 lane（cond 再判）。
                    work = D::mul(run_on, D::truthy(D::sub(D::splat(1.0), broke)));
                    if D::none_true(work) {
                        break;
                    }
                }
                // flow.active 不变：离开循环的 lane（cond false / break）循
                // 环后都继续；flow.brk 不动（break 只被最近的 For 消化）。
            }
        }
    }
}

// ── 执行入口（mod.rs 调用）────────────────────────────────────────────

/// 行值枚举（标量域跑 header 三段——simd 栈无 JS 引擎依赖）。
pub fn header_ys(kernel: &Kernel, args: &[f64]) -> Result<Vec<f64>, String> {
    let mut run = Run::<Scalar> {
        kernel,
        slots: {
            let mut slots = vec![f64::NAN; kernel.n_slots];
            for (slot, v) in kernel.arg_slots.iter().zip(args) {
                slots[*slot] = *v;
            }
            slots
        },
        writes: Vec::new(),
    };
    if let Some(init) = &kernel.header_init {
        run.stmt(
            &Stmt::Decl(vec![(kernel.header_var, Some(init.clone()))]),
            &mut Flow { active: 1.0, brk: 0.0, cont: 0.0 },
        );
    }
    let mut ys = Vec::new();
    loop {
        let go = match &kernel.header_test {
            Some(t) => run.expr(t, &Flow { active: 1.0, brk: 0.0, cont: 0.0 }) != 0.0,
            None => true,
        };
        if !go {
            break;
        }
        ys.push(run.slots[kernel.header_var]);
        if ys.len() > 1 << 22 {
            return Err("header enumeration runaway".into());
        }
        if let Some(u) = &kernel.header_update {
            let _ = run.expr(u, &Flow { active: 1.0, brk: 0.0, cont: 0.0 });
        }
    }
    Ok(ys)
}

/// 一批（≤8 个外层行值）的 lane 域执行。`ys` 不足 8 的尾批补 NaN——补位
/// lane 的写产出 idx = u32::MAX（组包侧丢弃，对 TypedArray 本就无效）。
/// `args` = plain 实参（按 arg_slots 赋槽，外层变量 per-lane 各自行值）。
pub fn run_row(kernel: &Kernel, ys: &[f64], args: &[f64]) -> Result<Vec<SimWrite>, String> {
    debug_assert!(ys.len() <= 8);
    let mut lanes = [f64::NAN; 8];
    lanes[..ys.len()].copy_from_slice(ys);
    let mut slots = vec![<Lane8 as Domain>::splat(f64::NAN); kernel.n_slots];
    slots[kernel.header_var] = Lane8::from_lanes(lanes);
    for (slot, v) in kernel.arg_slots.iter().zip(args) {
        slots[*slot] = Lane8::splat(*v);
    }
    let mut run = Run::<Lane8> { kernel, slots, writes: Vec::new() };
    run.stmts(
        &kernel.body,
        &mut Flow {
            active: Lane8::splat(1.0),
            brk: Lane8::splat(0.0),
            cont: Lane8::splat(0.0),
        },
    );
    Ok(std::mem::take(&mut run.writes))
}

/// 标量域逐点重跑（`OC_OMP_VERIFY` 对拍面——产出按 (channel, idx, acc,
/// val 位型) 排序后与 lane 产出逐位比对）。
pub fn run_row_verify(kernel: &Kernel, ys: &[f64], args: &[f64]) -> Result<Vec<SimWrite>, String> {
    let mut all = Vec::new();
    for &y in ys {
        let mut slots = vec![f64::NAN; kernel.n_slots];
        slots[kernel.header_var] = y;
        for (slot, v) in kernel.arg_slots.iter().zip(args) {
            slots[*slot] = *v;
        }
        let mut run = Run::<Scalar> { kernel, slots, writes: Vec::new() };
        run.stmts(
            &kernel.body,
            &mut Flow { active: 1.0, brk: 0.0, cont: 0.0 },
        );
        all.extend(run.writes);
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 曼德博形态（直线算术 + 内层迭代 for + if-escape break）——simd 栈
    /// 的目标白名单面。acc = 写产出通道（ir args 位置 2）。
    fn mandelbrot_ir() -> JsonValue {
        serde_json::json!({
            "args": ["w", "h", "acc"],
            "captures": {"acc": "array"},
            "header": "let y = 0; y < h; y += 1",
            "var": "y",
            "body": "let px = (y % w) * 0.01; let py = Math.floor(y / w) * 0.01;\nlet cx = px - 1.5; let cy = py - 1;\nlet zr = 0; let zi = 0; let n = 0;\nfor (let i = 0; i < 96; i += 1) {\n  const t = zr * zr - zi * zi + cx;\n  zi = 2 * zr * zi + cy;\n  zr = t;\n  if (zr * zr + zi * zi > 4) { break; }\n  n += 1;\n}\nacc[y] = n;"
        })
    }

    fn sort_writes(ws: &mut [SimWrite]) {
        ws.sort_by_key(|w| (w.channel, w.idx, w.acc, w.val.to_bits()));
    }

    #[test]
    fn simd_minimal_for() {
        // 最小 i 循环：分离「+= 累加」与「if-break」两个变量。
        let ir = serde_json::json!({
            "args": ["arr"],
            "captures": {"arr": "array"},
            "header": "let y = 0; y < 4; y += 1",
            "var": "y",
            "body": "let s = 0;\nfor (let i = 0; i < 96; i += 1) {\n  s += 1;\n}\narr[y] = s;"
        });
        let k = prepare(&ir).expect("prepare");
        let w = run_row_verify(&k, &[0.0], &[8.0, 4.0]).expect("scalar");
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].val, 96.0, "inner loop trip count");
    }

    #[test]
    fn simd_prepare_and_header_ys() {
        let k = prepare(&mandelbrot_ir()).expect("prepare");
        assert_eq!(k.arg_names, vec!["w", "h"]);
        let ys = header_ys(&k, &[64.0, 16.0]).expect("header");
        assert_eq!(ys.len(), 16);
        assert_eq!((ys[0], ys[15]), (0.0, 15.0));
    }

    #[test]
    fn simd_lane_scalar_bitwise_match() {
        let k = prepare(&mandelbrot_ir()).expect("prepare");
        let ys: Vec<f64> = (0..37).map(|i| i as f64).collect(); // 含尾批 5/8
        let args = [64.0, 64.0];
        let mut lane_all = Vec::new();
        let mut scalar_all = Vec::new();
        for bi in 0..ys.len().div_ceil(8) {
            let slice = &ys[bi * 8..(bi * 8 + 8).min(ys.len())];
            lane_all.extend(run_row(&k, slice, &args).expect("lane"));
            scalar_all.extend(run_row_verify(&k, slice, &args).expect("scalar"));
        }
        // 尾批补位 lane 产出哨兵写，过滤后再对拍。
        lane_all.retain(|w| w.idx != u32::MAX);
        sort_writes(&mut lane_all);
        sort_writes(&mut scalar_all);
        assert_eq!(lane_all.len(), scalar_all.len());
        for (a, b) in lane_all.iter().zip(&scalar_all) {
            assert_eq!((a.channel, a.idx, a.acc), (b.channel, b.idx, b.acc));
            assert_eq!(
                a.val.to_bits(),
                b.val.to_bits(),
                "lane idx={} val={} vs scalar idx={} val={}",
                a.idx,
                a.val,
                b.idx,
                b.val
            );
        }
        // 全 37 点都有产出，channel 0 = acc（ir args 位置 2）。
        assert_eq!(lane_all.len(), 37);
        assert_eq!(lane_all[0].channel, 0);
        assert_eq!(k.array_arg_pos(0), Some(2));
    }

    #[test]
    fn simd_whitelist_rejects() {
        // 页面函数调用（零参 = prng）拒绝。
        let mut ir = mandelbrot_ir();
        ir["body"] = serde_json::json!("acc[y] = rnd();");
        ir["args"] = serde_json::json!(["rnd", "acc"]);
        ir["captures"] = serde_json::json!({"acc": "array"});
        assert!(prepare(&ir).is_err());
        // 数组读拒绝。
        let mut ir = mandelbrot_ir();
        ir["body"] = serde_json::json!("acc[y] = acc[0] + 1;");
        assert!(prepare(&ir).is_err());
        // reduction 拒绝。
        let mut ir = mandelbrot_ir();
        ir["reduction"] = serde_json::json!([["+", ["s"]]]);
        assert!(prepare(&ir).is_err());
    }
}
