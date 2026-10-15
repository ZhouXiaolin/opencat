//! oxc 解析 + 白名单校验 + 类型推断 -> [`Program`]。
//!
//! 原则：lambda 只被编译、从不被执行。JS 源码被转成受限 IR，白名单之外的
//! 结构一律以带源码位置的错误拒绝。允许的语法面：
//! - 参数（pixel 类首参 = `uv`；scan 类首参 = `get`；`rect`；`u`；其余按序
//!   为 child，仅 pixel 类）
//! - 数值/布尔/数组字面量；算术/比较/逻辑/三目；`if/else`；`let/const`
//! - 有界数值 `for (let i = a; i < b; i++)` 循环
//! - 顶层用户函数 `const f = (p) => …`：单态、无捕获、先声明后使用
//! - stdlib 裸名调用与 `Math.*` 映射；向量 swizzle 读；`child.eval(pos)`；
//!   scan 类的 `get(dx, dy)`
//! - 语句级赋值（仅 let 变量）
//!
//! 拒绝：嵌套/块内函数声明、裸块、try/catch、对象/字符串/模板串、位运算、
//! `new`、`this`、`Math.random` 等破坏确定性的来源。

use std::collections::HashMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, BindingPattern, Expression, Statement};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

use super::error::{LambdaError, LambdaResult};
use super::infer::{FnBody, FnDecl, FnSig, infer_signatures};
use super::program::{BinOp, Expr, FnDef, Param, Program, Stmt, Ty, UnOp};
use super::stdlib::{self, BuiltinId as B};
use super::EffectSpec;

struct Ctx<'s> {
    source: &'s str,
    spec: &'s EffectSpec,
    params: HashMap<String, Param>,
    children: Vec<String>,
    local_names: Vec<String>,
    local_tys: Vec<Ty>,
    local_mutable: Vec<bool>,
    return_ty: Option<Ty>,
    uses_cpu_only: bool,

    /// 当前函数体的局部槽起点（主函数体为 0）。所有名字查找都限定在
    /// `[scope_base, local_names.len())`——函数之间、函数与主函数体的局部
    /// 互不可见，所以同一个名字可以在不同函数里重复出现。
    scope_base: usize,
    /// 当前函数已构建的函数下标（主函数体为 `None`）。
    cur_fn: Option<usize>,
    /// 当前函数的返回类型（主函数体为 `None`，那时由 `return_ty` 推断）。
    fn_ret: Option<Ty>,
    /// 函数名 -> 下标（预扫即填满）。
    fn_names: HashMap<String, usize>,
    /// 函数名 -> 签名（推断产出）。
    sigs: Vec<FnSig>,
    /// 已构建完成的函数（用于「先声明后使用」检查）。
    fn_done: Vec<bool>,
    /// 构建中的函数表。
    functions: Vec<FnDef>,
}

type ExprT = (Expr, Ty);

impl Ctx<'_> {
    pub(crate) fn err(&self, msg: impl Into<String>, node: &impl GetSpan) -> LambdaError {
        let span = node.span();
        LambdaError::at(msg, (span.start, span.end), self.source)
    }

    fn local_slot(
        &mut self,
        name: &str,
        ty: Ty,
        mutable: bool,
        span: (u32, u32),
    ) -> LambdaResult<u32> {
        if self.scope().any(|n| n == name) {
            return Err(LambdaError::at(
                format!("变量 `{name}` 重复声明"),
                span,
                self.source,
            ));
        }
        let slot = self.local_names.len() as u32;
        self.local_names.push(name.to_string());
        self.local_tys.push(ty);
        self.local_mutable.push(mutable);
        Ok(slot)
    }

    /// 当前作用域可见的局部名（`[scope_base, len)`）。
    fn scope(&self) -> impl Iterator<Item = &String> {
        self.local_names[self.scope_base..].iter()
    }

    /// 当前作用域内按名查槽。
    fn scope_slot(&self, name: &str) -> Option<u32> {
        self.local_names[self.scope_base..]
            .iter()
            .position(|n| n == name)
            .map(|i| (self.scope_base + i) as u32)
    }

    /// 报错用的函数名（反查 `fn_names`；表很小）。
    fn fn_label(&self, index: usize) -> String {
        self.fn_names
            .iter()
            .find(|(_, i)| **i == index)
            .map(|(n, _)| n.clone())
            .unwrap_or_else(|| format!("#{index}"))
    }
}

/// 解析并校验一个 lambda（`fn.toString()` 的产物）。uniform spec 在解析期
/// 就参与类型检查（`u.<name>` 必须在 spec 里声明）。
pub fn parse_lambda(source: &str, spec: &EffectSpec) -> LambdaResult<Program> {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, source, SourceType::mjs()).parse();
    if let Some(first) = ret.diagnostics.errors().next() {
        let span = first
            .labels
            .first()
            .map(|l| {
                let s = l.span();
                (s.start, s.end)
            })
            .unwrap_or((0, source.len() as u32));
        return Err(LambdaError::at(
            format!("语法错误：{}", first.message),
            span,
            source,
        ));
    }
    if ret.fatal_error || ret.program.body.is_empty() {
        return Err(LambdaError::at(
            "lambda 必须是单个箭头函数（`(uv, u) => ...`）",
            (0, source.len() as u32),
            source,
        ));
    }

    let body = &ret.program.body;
    if body.len() != 1 {
        return Err(LambdaError::at(
            format!("lambda 必须是单个箭头函数，顶层有 {} 条语句", body.len()),
            (0, source.len() as u32),
            source,
        ));
    }

    // 提取 (参数名列表, 函数体语句)：箭头函数表达式或 function 声明
    let (param_names, fn_statements): (Vec<String>, &[Statement]) = match &body[0] {
        Statement::ExpressionStatement(es) => match &es.expression {
            Expression::ArrowFunctionExpression(arrow) => {
                let names = arrow
                    .params
                    .items
                    .iter()
                    .map(|item| pattern_name(&item.pattern, source))
                    .collect::<LambdaResult<Vec<_>>>()?;
                let fn_body = arrow.body.as_function_body().ok_or_else(|| {
                    LambdaError::at(
                        "箭头函数体必须是 `{...}` 代码块",
                        (arrow.span().start, arrow.span().end),
                        source,
                    )
                })?;
                (names, &fn_body.statements)
            }
            other => {
                let span = other.span();
                return Err(LambdaError::at(
                    "顶层必须是箭头函数（`(uv, u) => ...`）",
                    (span.start, span.end),
                    source,
                ));
            }
        },
        Statement::FunctionDeclaration(func) => {
            let names = func
                .params
                .items
                .iter()
                .map(|item| pattern_name(&item.pattern, source))
                .collect::<LambdaResult<Vec<_>>>()?;
            let stmts: &[Statement] = func
                .body
                .as_ref()
                .map(|b| &b.statements[..])
                .unwrap_or(&[]);
            (names, stmts)
        }
        other => {
            let span = other.span();
            return Err(LambdaError::at(
                "顶层必须是单个箭头函数",
                (span.start, span.end),
                source,
            ));
        }
    };

    // 参数分类：pixel 类首参 uv；scan 类首参 get。其余：`rect`；`u`；
    // child（仅 pixel 类）
    if param_names.is_empty() {
        return Err(LambdaError::at(
            if spec.kind == super::program::ScanKind::Scan {
                "scan lambda 至少需要一个 `get` 参数"
            } else {
                "lambda 至少需要一个 `uv` 参数"
            },
            (0, source.len() as u32),
            source,
        ));
    }
    let mut ctx = Ctx {
        source,
        spec,
        params: HashMap::new(),
        children: Vec::new(),
        local_names: Vec::new(),
        local_tys: Vec::new(),
        local_mutable: Vec::new(),
        return_ty: None,
        uses_cpu_only: false,
        scope_base: 0,
        cur_fn: None,
        fn_ret: None,
        fn_names: HashMap::new(),
        sigs: Vec::new(),
        fn_done: Vec::new(),
        functions: Vec::new(),
    };
    let mut params: Vec<(String, Param)> = Vec::new();
    for (i, name) in param_names.iter().enumerate() {
        let param = if i == 0 {
            if spec.kind == super::program::ScanKind::Scan {
                Param::Get
            } else {
                Param::Uv
            }
        } else if name == "rect" {
            Param::Rect
        } else if name == "u" {
            // 占位：`u` 通过 u.<name> 成员访问解析
            Param::Uniforms
        } else if spec.kind == super::program::ScanKind::Scan {
            return Err(LambdaError::at(
                format!(
                    "scan lambda 不支持 child 参数 `{name}`（采样只能通过 `get(dx, dy)` 读本目标）"
                ),
                (0, source.len() as u32),
                source,
            ));
        } else {
            let index = ctx.children.len();
            ctx.children.push(name.clone());
            Param::Child { index }
        };
        params.push((name.clone(), param.clone()));
        ctx.params.insert(name.clone(), param);
    }

    // 顶层预扫函数声明，再推断签名——两者都必须在构建主函数体之前完成，
    // 因为主函数体可以在任意位置调用它们（只需函数在**调用点之前**声明）。
    let decls = prescan_functions(&ctx, fn_statements)?;
    let param_tys: HashMap<String, Ty> = ctx
        .params
        .iter()
        .filter_map(|(name, p)| match p {
            Param::Uv | Param::Rect | Param::Get => Some((name.clone(), param_ty(p))),
            // `u` 是成员访问器，`child` 只能 `.eval()`；两者都不作为值出现。
            Param::Uniforms | Param::Child { .. } => None,
        })
        .collect();
    ctx.sigs = infer_signatures(&decls, fn_statements, source, &spec.uniforms, &param_tys)?;
    ctx.fn_names = decls
        .iter()
        .enumerate()
        .map(|(i, d)| (d.name.clone(), i))
        .collect();
    ctx.fn_done = vec![false; decls.len()];
    ctx.functions = Vec::with_capacity(decls.len());

    // 主函数体：遇函数声明即构建它（占用一段独立槽区间），随后回到主函数体。
    let stmts = parse_main_body(&mut ctx, fn_statements, &decls)?;

    let return_ty = ctx.return_ty.ok_or_else(|| {
        LambdaError::at("lambda 缺少 return", (0, source.len() as u32), source)
    })?;

    Ok(Program {
        source: source.to_string(),
        params,
        uniforms: spec
            .uniforms
            .iter()
            .map(|(name, ty)| super::program::UniformSpec { name: name.clone(), ty: *ty })
            .collect(),
        children: ctx.children,
        local_tys: ctx.local_tys,
        body: stmts,
        functions: ctx.functions,
        return_ty,
        uses_cpu_only: ctx.uses_cpu_only,
        backend_override: None,
        kind: spec.kind,
    })
}

/// 顶层预扫：收集 `const NAME = (params) => …`，拒绝其它函数形态。
///
/// 只认 `const` + 箭头函数：`let`/`var` 绑定箭头会被后续重赋值，破坏单态
/// 与「先声明后使用」的静态保证；`function` 声明会被提升，同样破坏该保证。
fn prescan_functions<'a>(
    ctx: &Ctx,
    stmts: &'a [Statement<'a>],
) -> LambdaResult<Vec<FnDecl<'a>>> {
    let mut decls: Vec<FnDecl<'a>> = Vec::new();
    for stmt in stmts {
        let Statement::VariableDeclaration(decl) = stmt else {
            continue;
        };
        // 一条语句里混声明（`const f = …, g = 1;`）会让「哪条是函数声明」变得
        // 含混，构建阶段也难分清哪些声明符该发语句。要求函数独占一条声明。
        let has_fn = decl.declarations.iter().any(|d| {
            matches!(
                d.init.as_ref(),
                Some(Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_))
            )
        });
        if has_fn && decl.declarations.len() > 1 {
            return Err(ctx.err(
                "用户函数必须独占一条 `const f = (…) => …;` 声明",
                stmt,
            ));
        }
        for d in &decl.declarations {
            let Some(init) = &d.init else { continue };
            let is_arrow = matches!(init, Expression::ArrowFunctionExpression(_));
            let is_fn_expr = matches!(init, Expression::FunctionExpression(_));
            if !is_arrow && !is_fn_expr {
                continue;
            }
            if !matches!(decl.kind, oxc_ast::ast::VariableDeclarationKind::Const) {
                return Err(ctx.err(
                    "用户函数必须用 `const` 声明（`let` 可被重赋值，破坏单态与调用顺序保证）",
                    d,
                ));
            }
            if is_fn_expr {
                return Err(ctx.err(
                    "请用箭头函数 `const f = (p) => …` 声明用户函数",
                    init,
                ));
            }
            let BindingPattern::BindingIdentifier(id) = &d.id else {
                return Err(ctx.err("函数名必须是普通标识符", &d.id));
            };
            let name = id.name.to_string();
            if ctx.params.contains_key(&name) {
                return Err(ctx.err(
                    format!("函数名 `{name}` 与 lambda 参数冲突"),
                    &d.id,
                ));
            }
            if stdlib::lookup(&name).is_some() || stdlib::math_member_id(&name).is_some() {
                return Err(ctx.err(
                    format!("函数名 `{name}` 与内建函数冲突，请改名"),
                    &d.id,
                ));
            }
            if decls.iter().any(|f| f.name == name) {
                return Err(ctx.err(format!("函数 `{name}` 重复声明"), &d.id));
            }
            let Expression::ArrowFunctionExpression(arrow) = init else {
                unreachable!("已判 is_arrow");
            };
            let params = arrow
                .params
                .items
                .iter()
                .map(|item| pattern_name(&item.pattern, ctx.source))
                .collect::<LambdaResult<Vec<_>>>()?;
            let param_spans = arrow
                .params
                .items
                .iter()
                .map(|item| {
                    let s = item.pattern.span();
                    (s.start, s.end)
                })
                .collect();
            let body = match arrow.body.as_function_body() {
                Some(b) => FnBody::Block(&b.statements),
                None => FnBody::Expr(
                    arrow
                        .body
                        .as_expression()
                        .expect("非代码块体即简洁表达式体"),
                ),
            };
            let span = id.span();
            decls.push(FnDecl {
                name,
                name_span: (span.start, span.end),
                params,
                param_spans,
                body,
            });
        }
    }
    Ok(decls)
}

/// 构建主函数体。遇函数声明（`decls[i]`）就构建该函数——它会占用一段独立槽
/// 区间，构建完再把作用域还给主函数体。函数体内部的嵌套声明由
/// [`parse_fn_body`] 拒绝，不会递归进来。
fn parse_main_body<'a>(
    ctx: &mut Ctx,
    stmts: &'a [Statement<'a>],
    decls: &'a [FnDecl<'a>],
) -> LambdaResult<Vec<Stmt>> {
    let mut out = Vec::new();
    for stmt in stmts {
        if matches!(out.last(), Some(Stmt::Return(_))) {
            return Err(ctx.err("return 之后的语句不可达", stmt));
        }
        if let Statement::VariableDeclaration(decl) = stmt {
            let mut fn_decl: Option<usize> = None;
            for d in &decl.declarations {
                if let Some(init) = &d.init
                    && matches!(
                        init,
                        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
                    )
                {
                    let BindingPattern::BindingIdentifier(id) = &d.id else {
                        continue;
                    };
                    fn_decl = decls.iter().position(|f| f.name == id.name.as_str());
                }
            }
            if let Some(i) = fn_decl {
                build_function(ctx, i, &decls[i])?;
                continue;
            }
        }
        out.extend(parse_statement(ctx, stmt)?);
    }
    Ok(out)
}

/// 构建一个用户函数，推入 `ctx.functions`。
fn build_function<'a>(ctx: &mut Ctx, index: usize, decl: &'a FnDecl<'a>) -> LambdaResult<()> {
    let sig = ctx.sigs[index].clone();
    let saved_base = ctx.scope_base;
    let saved_ret = ctx.return_ty;
    ctx.scope_base = ctx.local_names.len();
    ctx.cur_fn = Some(index);
    ctx.fn_ret = Some(sig.ret);
    ctx.return_ty = None;

    // 形参 = 可变局部槽。参考的 `fbm` 会对参数 `p` 重新赋值，所以参数不能是 const。
    let mut param_slots = Vec::with_capacity(decl.params.len());
    for (pi, (pname, pty)) in decl.params.iter().zip(&sig.params).enumerate() {
        let slot = ctx.local_slot(pname, *pty, true, decl.param_spans[pi])?;
        param_slots.push(slot);
    }

    let body = parse_fn_body(ctx, decl)?;

    ctx.scope_base = saved_base;
    ctx.return_ty = saved_ret;
    ctx.cur_fn = None;
    ctx.fn_ret = None;
    ctx.fn_done[index] = true;
    ctx.functions.push(FnDef {
        name: decl.name.clone(),
        params: decl.params.iter().cloned().zip(sig.params.iter().copied()).collect(),
        param_slots,
        ret: sig.ret,
        body,
    });
    Ok(())
}

fn parse_fn_body<'a>(ctx: &mut Ctx, decl: &'a FnDecl<'a>) -> LambdaResult<Vec<Stmt>> {
    let stmts: &[Statement] = match &decl.body {
        FnBody::Block(s) => s,
        FnBody::Expr(e) => return Ok(vec![Stmt::Return(parse_expr(ctx, e)?.0)]),
    };
    let mut out = Vec::new();
    for stmt in stmts {
        if matches!(out.last(), Some(Stmt::Return(_))) {
            return Err(ctx.err("return 之后的语句不可达", stmt));
        }
        if let Statement::VariableDeclaration(decl) = stmt {
            for d in &decl.declarations {
                if let Some(init) = &d.init
                    && matches!(
                        init,
                        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
                    )
                {
                    return Err(ctx.err("不支持嵌套函数声明（用户函数必须声明在 lambda 顶层）", init));
                }
            }
        }
        out.extend(parse_statement(ctx, stmt)?);
    }
    Ok(out)
}

/// 非 uniform 参数的类型（作为值时）。
fn param_ty(p: &Param) -> Ty {
    match p {
        Param::Uv => Ty::Vec2,
        Param::Rect => Ty::Vec4,
        Param::Get => Ty::Vec4,
        Param::Uniforms | Param::Child { .. } => Ty::Float,
    }
}

fn pattern_name(pattern: &BindingPattern, source: &str) -> LambdaResult<String> {
    match pattern {
        BindingPattern::BindingIdentifier(id) => Ok(id.name.to_string()),
        _ => Err(LambdaError::at(
            "lambda 参数必须是普通标识符（不支持解构/默认值/rest）",
            (pattern.span().start, pattern.span().end),
            source,
        )),
    }
}

fn parse_statements(ctx: &mut Ctx, statements: &[Statement]) -> LambdaResult<Vec<Stmt>> {
    let mut out = Vec::new();
    for stmt in statements {
        if matches!(out.last(), Some(Stmt::Return(_))) {
            return Err(ctx.err("return 之后的语句不可达", stmt));
        }
        out.extend(parse_statement(ctx, stmt)?);
    }
    Ok(out)
}

/// 一条 JS 语句展开为 0..n 条 IR 语句（let a=…, b=… 展开多条）。
fn parse_statement(ctx: &mut Ctx, stmt: &Statement) -> LambdaResult<Vec<Stmt>> {
    match stmt {
        Statement::VariableDeclaration(decl) => {
            let mut out = Vec::new();
            for declarator in &decl.declarations {
                let Some(init) = &declarator.init else {
                    return Err(ctx.err("let/const 声明必须有初始值", declarator));
                };
                let name = match &declarator.id {
                    BindingPattern::BindingIdentifier(id) => id.name.to_string(),
                    _ => {
                        return Err(ctx.err(
                            "声明必须是普通标识符（不支持解构）",
                            &declarator.id,
                        ))
                    }
                };
                if matches!(
                    init,
                    Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
                ) {
                    return Err(ctx.err(
                        "不支持嵌套函数声明（用户函数必须声明在 lambda 顶层）",
                        init,
                    ));
                }
                let (expr, ty) = parse_expr(ctx, init)?;
                let mutable =
                    !matches!(decl.kind, oxc_ast::ast::VariableDeclarationKind::Const);
                let s = declarator.id.span();
                let slot = ctx.local_slot(&name, ty, mutable, (s.start, s.end))?;
                out.push(Stmt::Let { slot, value: expr });
            }
            Ok(out)
        }
        Statement::ExpressionStatement(es) => match &es.expression {
            Expression::AssignmentExpression(assign) => {
                let compound = match assign.operator {
                    oxc_ast::ast::AssignmentOperator::Assign => None,
                    oxc_ast::ast::AssignmentOperator::Addition => Some(BinOp::Add),
                    oxc_ast::ast::AssignmentOperator::Subtraction => Some(BinOp::Sub),
                    oxc_ast::ast::AssignmentOperator::Multiplication => Some(BinOp::Mul),
                    oxc_ast::ast::AssignmentOperator::Division => Some(BinOp::Div),
                    _ => return Err(ctx.err("仅支持 `=` `+=` `-=` `*=` `/=` 赋值", stmt)),
                };
                let name = assignment_target_name(ctx, &assign.left)?;
                let Some(slot) = ctx.scope_slot(&name) else {
                    return Err(ctx.err(format!("对未声明变量 `{name}` 赋值"), stmt));
                };
                if !ctx.local_mutable[slot as usize] {
                    return Err(ctx.err(format!("`{name}` 是 const，不可重新赋值"), stmt));
                }
                let want = ctx.local_tys[slot as usize];
                let (rhs, rty) = parse_expr(ctx, &assign.right)?;
                let value = match compound {
                    None => {
                        if rty != want {
                            return Err(ctx.err(
                                format!("赋值类型不匹配：`{name}` 是 {want:?}，右侧是 {rty:?}"),
                                stmt,
                            ));
                        }
                        rhs
                    }
                    // `a += b` 脱糖为 `a = a + b`（复用 bin_ty 的类型规则）
                    Some(op) => {
                        let ty = bin_ty(ctx, op, want, rty, stmt)?;
                        if ty != want {
                            return Err(ctx.err(
                                format!("复合赋值类型不匹配：`{name}` 是 {want:?}，右侧是 {rty:?}"),
                                stmt,
                            ));
                        }
                        Expr::Bin(op, Box::new(Expr::Local(slot)), Box::new(rhs), ty)
                    }
                };
                Ok(vec![Stmt::Assign { slot, value }])
            }
            other => Err(ctx.err(
                "表达式语句只允许赋值（效果 lambda 无副作用）",
                other,
            )),
        },
        Statement::IfStatement(if_stmt) => {
            let (cond, cond_ty) = parse_expr(ctx, &if_stmt.test)?;
            if cond_ty != Ty::Bool {
                return Err(ctx.err("if 条件必须是布尔表达式", &if_stmt.test));
            }
            let then = parse_branch(ctx, &if_stmt.consequent)?;
            let els = match &if_stmt.alternate {
                Some(alt) => parse_branch(ctx, alt)?,
                None => Vec::new(),
            };
            Ok(vec![Stmt::If { cond, then, els }])
        }
        Statement::ForStatement(for_stmt) => parse_for_loop(ctx, for_stmt),
        Statement::ReturnStatement(ret) => {
            let Some(arg) = &ret.argument else {
                return Err(ctx.err("return 必须带值", stmt));
            };
            let (expr, ty) = parse_expr(ctx, arg)?;
            match ctx.fn_ret {
                // 用户函数：返回类型由推断给定，这里只校验一致。
                Some(want) => {
                    if ty != want {
                        return Err(ctx.err(
                            format!("return 类型与推断出的函数返回类型不符：期望 {want:?}，得到 {ty:?}"),
                            stmt,
                        ));
                    }
                }
                // 主函数体：pixel 类的 SKSL `main` 只接受 float3/float4 与
                // float（隐式转 float4 灰度）。用户函数的 Vec2/Bool 返回不在此列。
                None => {
                    match ty {
                        Ty::Float | Ty::Vec3 | Ty::Vec4 => {}
                        other => {
                            return Err(ctx.err(
                                format!("return 类型必须是 float/vec3/vec4，得到 {other:?}"),
                                stmt,
                            ))
                        }
                    }
                    if let Some(prev) = ctx.return_ty {
                        if prev != ty {
                            return Err(ctx.err(
                                format!("return 类型不一致：先前 {prev:?}，此处 {ty:?}"),
                                stmt,
                            ));
                        }
                    } else {
                        ctx.return_ty = Some(ty);
                    }
                }
            }
            Ok(vec![Stmt::Return(expr)])
        }
        other => {
            let msg = statement_reject_reason(other);
            Err(ctx.err(msg, other))
        }
    }
}

fn parse_branch(ctx: &mut Ctx, stmt: &Statement) -> LambdaResult<Vec<Stmt>> {
    match stmt {
        Statement::BlockStatement(block) => parse_statements(ctx, &block.body),
        single @ (Statement::ReturnStatement(_)
        | Statement::ExpressionStatement(_)
        | Statement::VariableDeclaration(_)
        | Statement::ForStatement(_)
        | Statement::IfStatement(_)) => parse_statement(ctx, single),
        other => Err(ctx.err("if 分支必须是代码块或单条语句", other)),
    }
}

/// 有界数值 `for` 循环。严格形状（唯一支持形态）：
/// `for (let i = <start>; i <|<= <end>; i++ /* 或 i += 1 */) { … }`
/// `start`/`end` 各求值一次；`i` 是可变 f64 槽。body 可含 return。
fn parse_for_loop(ctx: &mut Ctx, for_stmt: &oxc_ast::ast::ForStatement) -> LambdaResult<Vec<Stmt>> {
    // init：`let i = <expr>`（仅单声明、Let、普通标识符）
    let Some(init) = &for_stmt.init else {
        return Err(ctx.err("for 循环需要 `let i = 初值` 初始化", for_stmt));
    };
    let oxc_ast::ast::ForStatementInit::VariableDeclaration(decl) = init else {
        return Err(ctx.err("for 初始化必须是 `let i = 初值`", init));
    };
    let decl: &oxc_ast::ast::VariableDeclaration = decl.as_ref();
    if decl.declarations.len() != 1 {
        return Err(ctx.err("for 初始化只支持单个变量", decl));
    }
    if !matches!(decl.kind, oxc_ast::ast::VariableDeclarationKind::Let) {
        return Err(ctx.err(
            "for 循环变量必须用 `let` 声明（循环体会更新它）",
            decl,
        ));
    }
    let declarator = &decl.declarations[0];
    let BindingPattern::BindingIdentifier(id) = &declarator.id else {
        return Err(ctx.err("for 循环变量必须是普通标识符", &declarator.id));
    };
    let Some(start_node) = &declarator.init else {
        return Err(ctx.err("for 循环变量必须有初值", declarator));
    };
    let (start, sty) = parse_expr(ctx, start_node)?;
    if sty != Ty::Float {
        return Err(ctx.err("for 循环初值必须是数值", start_node));
    }
    let s = declarator.id.span();
    let var = ctx.local_slot(id.name.as_str(), Ty::Float, true, (s.start, s.end))?;

    // test：`i < end` 或 `i <= end`
    let Some(test) = &for_stmt.test else {
        return Err(ctx.err("for 循环必须有终止条件（有界循环）", for_stmt));
    };
    let Expression::BinaryExpression(bin) = test else {
        return Err(ctx.err("for 终止条件必须是 `i < 上界` 或 `i <= 上界`", test));
    };
    let le = match bin.operator {
        oxc_ast::ast::BinaryOperator::LessThan => false,
        oxc_ast::ast::BinaryOperator::LessEqualThan => true,
        _ => return Err(ctx.err("for 条件只支持 `<` 或 `<=`", test)),
    };
    let Expression::Identifier(test_id) = &bin.left else {
        return Err(ctx.err("for 条件左侧必须是循环变量", &bin.left));
    };
    if test_id.name.as_str() != id.name.as_str() {
        return Err(ctx.err("for 条件左侧必须是循环变量", &bin.left));
    }
    let (end, ety) = parse_expr(ctx, &bin.right)?;
    if ety != Ty::Float {
        return Err(ctx.err("for 循环上界必须是数值", &bin.right));
    }

    // update：`i++` / `++i` / `i += 1`
    let Some(update) = &for_stmt.update else {
        return Err(ctx.err("for 循环必须有 `i++` 更新（有界循环）", for_stmt));
    };
    let valid_update = match update {
        Expression::UpdateExpression(u) => {
            matches!(u.argument, oxc_ast::ast::SimpleAssignmentTarget::AssignmentTargetIdentifier(ref a) if a.name.as_str() == id.name.as_str())
                && (u.operator == oxc_ast::ast::UpdateOperator::Increment)
        }
        Expression::AssignmentExpression(a)
            if a.operator == oxc_ast::ast::AssignmentOperator::Addition =>
        {
            let target_ok = assignment_target_name(ctx, &a.left)
                .map(|n| n == id.name.as_str())
                .unwrap_or(false);
            target_ok && matches!(&a.right, Expression::NumericLiteral(l) if l.value == 1.0)
        }
        _ => false,
    };
    if !valid_update {
        return Err(ctx.err("for 更新只支持 `i++` / `++i` / `i += 1`", update));
    }

    // body：代码块或单条语句
    let body = match &for_stmt.body {
        Statement::BlockStatement(b) => parse_statements(ctx, &b.body)?,
        single => parse_statement(ctx, single)?,
    };
    Ok(vec![Stmt::ForLoop { var, start, end, le, body }])
}

fn statement_reject_reason(stmt: &Statement) -> String {
    match stmt {
        Statement::FunctionDeclaration(_) => {
            "不支持 `function` 声明：用户函数请写成顶层 `const f = (p) => …`（先声明后使用）"
                .into()
        }
        Statement::ForInStatement(_)
        | Statement::ForOfStatement(_)
        | Statement::WhileStatement(_)
        | Statement::DoWhileStatement(_) => {
            "只支持有界 `for (let i = a; i < b; i++)` 循环；while/for-in/for-of 不支持".into()
        }
        Statement::TryStatement(_) => "不支持 try/catch".into(),
        Statement::ThrowStatement(_) => "不支持 throw".into(),
        Statement::BreakStatement(_) | Statement::ContinueStatement(_) => {
            "不支持 break/continue".into()
        }
        Statement::SwitchStatement(_) => "不支持 switch（请用 if/else）".into(),
        Statement::BlockStatement(_) => "不支持裸代码块（请直接内联语句）".into(),
        _ => "该语句形式不在白名单内（允许：let/const、赋值、if/else、return）".into(),
    }
}

fn assignment_target_name(ctx: &Ctx, target: &oxc_ast::ast::AssignmentTarget) -> LambdaResult<String> {
    // v1 只支持对简单标识符赋值
    let text = target.span().source_text(ctx.source).trim();
    if text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$') {
        return Ok(text.to_string());
    }
    Err(ctx.err("只能对 let 变量整体赋值", target))
}

fn parse_expr(ctx: &mut Ctx, expr: &Expression) -> LambdaResult<ExprT> {
    match expr {
        Expression::NumericLiteral(lit) => {
            if !lit.value.is_finite() {
                return Err(ctx.err("数值必须是有限数", expr));
            }
            Ok((Expr::LitF(lit.value), Ty::Float))
        }
        Expression::BooleanLiteral(lit) => Ok((Expr::LitB(lit.value), Ty::Bool)),
        Expression::Identifier(id) => {
            let name = id.name.as_str();
            // 函数体不得捕获 lambda 参数。`uv`/`rect` 是参数，会走下面的
            // `Some(Param::Uv/Rect)` 臂直接返回，所以必须在分派前拦一次——
            // 否则 `uv` 会静默漏进函数体，SKSL 里引用 `main` 的局部 `oc_uv`，
            // Skia 编译失败。
            if ctx.cur_fn.is_some()
                && matches!(ctx.params.get(name), Some(Param::Uv | Param::Rect))
            {
                return Err(ctx.err(
                    format!("函数体不能捕获 lambda 参数 `{name}`：数据必须经形参传入"),
                    expr,
                ));
            }
            match ctx.params.get(name) {
                Some(Param::Uv) => Ok((Expr::Uv, Ty::Vec2)),
                Some(Param::Rect) => Ok((Expr::Rect, Ty::Vec4)),
                Some(Param::Uniforms) => Err(ctx.err(
                    "`u` 不能单独使用，请通过 `u.<名字>` 访问 uniform",
                    expr,
                )),
                Some(Param::Child { .. }) => Err(ctx.err(
                    format!("child `{name}` 不能作为值使用，只能 `.eval(pos)`"),
                    expr,
                )),
                Some(Param::Get) => Err(ctx.err(
                    "`get` 不能单独使用，只能 `get(dx, dy)` 采样",
                    expr,
                )),
                None => {
                    if let Some(slot) = ctx.scope_slot(name) {
                        return Ok((Expr::Local(slot), ctx.local_tys[slot as usize]));
                    }
                    // 用户函数名不是值（主函数体与函数体一致）。
                    if ctx.fn_names.contains_key(name) {
                        return Err(ctx.err(
                            format!("函数 `{name}` 不能作为值使用，只能直接调用"),
                            expr,
                        ));
                    }
                    if ctx.cur_fn.is_some() && ctx.params.contains_key(name) {
                        // 函数体读不到 lambda 参数与主函数体的局部——数据只能经形参进入。
                        return Err(ctx.err(
                            format!("函数体不能捕获 lambda 参数 `{name}`：数据必须经形参传入"),
                            expr,
                        ));
                    }
                    if name == "Math" {
                        return Err(ctx.err("`Math` 不能单独使用", expr));
                    }
                    if stdlib::lookup(name).is_some() {
                        return Err(ctx.err(
                            format!("`{name}` 是内建函数，请调用它而不是引用"),
                            expr,
                        ));
                    }
                    Err(ctx.err(format!("未定义变量 `{name}`"), expr))
                }
            }
        }
        Expression::BinaryExpression(bin) => {
            let (lhs, lty) = parse_expr(ctx, &bin.left)?;
            let (rhs, rty) = parse_expr(ctx, &bin.right)?;
            let op = match bin.operator {
                oxc_ast::ast::BinaryOperator::Addition => BinOp::Add,
                oxc_ast::ast::BinaryOperator::Subtraction => BinOp::Sub,
                oxc_ast::ast::BinaryOperator::Multiplication => BinOp::Mul,
                oxc_ast::ast::BinaryOperator::Division => BinOp::Div,
                oxc_ast::ast::BinaryOperator::Remainder => BinOp::Rem,
                oxc_ast::ast::BinaryOperator::GreaterThan => BinOp::Gt,
                oxc_ast::ast::BinaryOperator::LessThan => BinOp::Lt,
                oxc_ast::ast::BinaryOperator::GreaterEqualThan => BinOp::Ge,
                oxc_ast::ast::BinaryOperator::LessEqualThan => BinOp::Le,
                oxc_ast::ast::BinaryOperator::Equality
                | oxc_ast::ast::BinaryOperator::StrictEquality => BinOp::Eq,
                oxc_ast::ast::BinaryOperator::Inequality
                | oxc_ast::ast::BinaryOperator::StrictInequality => BinOp::Ne,
                oxc_ast::ast::BinaryOperator::BitwiseAnd
                | oxc_ast::ast::BinaryOperator::BitwiseOR
                | oxc_ast::ast::BinaryOperator::BitwiseXOR
                | oxc_ast::ast::BinaryOperator::ShiftLeft
                | oxc_ast::ast::BinaryOperator::ShiftRight
                | oxc_ast::ast::BinaryOperator::ShiftRightZeroFill => {
                    return Err(ctx.err(
                        "位运算不支持：整数语义请用 `imul`/`h01`，截断请用 `byte`",
                        expr,
                    ))
                }
                oxc_ast::ast::BinaryOperator::Exponential => {
                    return Err(ctx.err("`**` 不支持，请用 `pow(a, b)`", expr))
                }
                _ => return Err(ctx.err("不支持的二元运算", expr)),
            };
            let ty = bin_ty(ctx, op, lty, rty, expr)?;
            Ok((Expr::Bin(op, Box::new(lhs), Box::new(rhs), ty), ty))
        }
        Expression::LogicalExpression(log) => {
            let op = match log.operator {
                oxc_ast::ast::LogicalOperator::And => BinOp::And,
                oxc_ast::ast::LogicalOperator::Or => BinOp::Or,
                oxc_ast::ast::LogicalOperator::Coalesce => {
                    return Err(ctx.err("?? 不支持", expr))
                }
            };
            let (lhs, lty) = parse_expr(ctx, &log.left)?;
            let (rhs, rty) = parse_expr(ctx, &log.right)?;
            if lty != Ty::Bool || rty != Ty::Bool {
                return Err(ctx.err("&&/|| 两侧必须是布尔表达式", expr));
            }
            Ok((
                Expr::Bin(op, Box::new(lhs), Box::new(rhs), Ty::Bool),
                Ty::Bool,
            ))
        }
        Expression::UnaryExpression(un) => {
            let (arg, ty) = parse_expr(ctx, &un.argument)?;
            match un.operator {
                oxc_ast::ast::UnaryOperator::UnaryNegation => {
                    if !is_numeric(ty) {
                        return Err(ctx.err("取负需要数值类型", expr));
                    }
                    Ok((Expr::Un(UnOp::Neg, Box::new(arg), ty), ty))
                }
                oxc_ast::ast::UnaryOperator::LogicalNot => {
                    if ty != Ty::Bool {
                        return Err(ctx.err("! 需要布尔类型", expr));
                    }
                    Ok((Expr::Un(UnOp::Not, Box::new(arg), Ty::Bool), Ty::Bool))
                }
                _ => Err(ctx.err("不支持的一元运算（仅有 - 和 !）", expr)),
            }
        }
        Expression::ConditionalExpression(cond) => {
            let (c, cty) = parse_expr(ctx, &cond.test)?;
            if cty != Ty::Bool {
                return Err(ctx.err("三目条件必须是布尔表达式", expr));
            }
            let (t, tty) = parse_expr(ctx, &cond.consequent)?;
            let (f, fty) = parse_expr(ctx, &cond.alternate)?;
            if tty != fty || !(is_numeric(tty) || tty == Ty::Bool) {
                return Err(ctx.err("三目两个分支类型必须一致（数值或布尔）", expr));
            }
            Ok((Expr::Cond(Box::new(c), Box::new(t), Box::new(f), tty), tty))
        }
        Expression::StaticMemberExpression(member) => {
            let prop = member.property.name.as_str();
            // u.<name>：uniform 访问
            if let Expression::Identifier(obj) = &member.object
                && obj.name == "u"
            {
                if ctx.cur_fn.is_some() {
                    return Err(ctx.err(
                        "函数体不能捕获 uniform `u.*`：数据必须经形参传入",
                        expr,
                    ));
                }
                let Some(pos) =
                    ctx.spec.uniforms.iter().position(|(n, _)| n == prop)
                else {
                    let available: Vec<&str> =
                        ctx.spec.uniforms.iter().map(|(n, _)| n.as_str()).collect();
                    return Err(ctx.err(
                        format!("未知 uniform `{prop}`（spec 声明：{available:?}）"),
                        expr,
                    ));
                };
                let ty = ctx.spec.uniforms[pos].1;
                return Ok((Expr::Uniform(pos as u32), ty));
            }
            if prop == "eval" {
                return Err(ctx.err("`.eval` 只能作为方法调用：child.eval(pos)", expr));
            }
            // swizzle 读
            let (base, bty) = parse_expr(ctx, &member.object)?;
            let Some(n) = bty.vec_len() else {
                return Err(ctx.err(format!("`.{prop}` 需要向量类型"), expr));
            };
            let comps = swizzle_comps(prop, n).ok_or_else(|| {
                ctx.err(
                    format!("非法 swizzle `.{prop}`（xyzw/rgba，分量需 < {n}）"),
                    expr,
                )
            })?;
            let len = prop.len() as u8;
            let ty = if len == 1 { Ty::Float } else { vec_ty(len as usize) };
            Ok((
                Expr::Swizzle { base: Box::new(base), comps, len, ty },
                ty,
            ))
        }
        Expression::CallExpression(call) => parse_call(ctx, call),
        Expression::ArrayExpression(arr) => {
            if arr.elements.len() < 2 || arr.elements.len() > 4 {
                return Err(ctx.err(
                    "数组字面量必须是 2-4 个标量（构造 float2/3/4）",
                    expr,
                ));
            }
            let mut items = Vec::new();
            for el in &arr.elements {
                let Some(el_expr) = el.as_expression() else {
                    return Err(ctx.err("数组内不允许 spread/空位", el));
                };
                let (e, ty) = parse_expr(ctx, el_expr)?;
                if !is_numeric(ty) {
                    return Err(ctx.err("数组分量必须是标量数值", el));
                }
                items.push(e);
            }
            let ty = vec_ty(items.len());
            Ok((Expr::VecCons(items, ty), ty))
        }
        Expression::ParenthesizedExpression(paren) => parse_expr(ctx, &paren.expression),
        Expression::AssignmentExpression(_) => Err(ctx.err(
            "赋值只能作为独立语句（不支持嵌套赋值）",
            expr,
        )),
        Expression::StringLiteral(_) | Expression::TemplateLiteral(_) => {
            Err(ctx.err("不支持字符串（效果 lambda 是纯数值计算）", expr))
        }
        Expression::ObjectExpression(_) => Err(ctx.err(
            "不支持对象字面量（uniform 请通过 `u.<名字>` 访问）",
            expr,
        )),
        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => Err(
            ctx.err("不支持嵌套函数：用户函数必须声明在 lambda 顶层（`const f = (p) => …`）", expr),
        ),
        other => Err(ctx.err("不支持的表达式形式", other)),
    }
}

fn is_numeric(ty: Ty) -> bool {
    ty.is_scalar_num() || ty.vec_len().is_some()
}

pub(crate) fn vec_ty(n: usize) -> Ty {
    match n {
        2 => Ty::Vec2,
        3 => Ty::Vec3,
        _ => Ty::Vec4,
    }
}

/// 二元运算的类型规则，不含源码位置。`infer.rs` 的掩码推断直接调用它，所以
/// 规则只有这一份——两份规则漂移是这类特性的主要长期风险。
pub(crate) fn bin_ty_pure(op: BinOp, lty: Ty, rty: Ty) -> Option<Ty> {
    match op {
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
            if !is_numeric(lty) || !is_numeric(rty) {
                return None;
            }
            match (lty.vec_len(), rty.vec_len()) {
                (None, None) => Some(Ty::Float),
                (None, Some(n)) => Some(vec_ty(n)),
                (Some(n), None) => Some(vec_ty(n)),
                (Some(a), Some(b)) if a == b => Some(vec_ty(a)),
                _ => None,
            }
        }
        BinOp::Gt | BinOp::Lt | BinOp::Ge | BinOp::Le => {
            (lty == Ty::Float && rty == Ty::Float).then_some(Ty::Bool)
        }
        BinOp::Eq | BinOp::Ne => {
            (lty == rty && (lty == Ty::Float || lty == Ty::Bool)).then_some(Ty::Bool)
        }
        BinOp::And | BinOp::Or => {
            (lty == Ty::Bool && rty == Ty::Bool).then_some(Ty::Bool)
        }
    }
}

fn bin_ty(ctx: &Ctx, op: BinOp, lty: Ty, rty: Ty, node: &impl GetSpan) -> LambdaResult<Ty> {
    if let Some(ty) = bin_ty_pure(op, lty, rty) {
        return Ok(ty);
    }
    Err(ctx.err(
        format!("{}：{lty:?} 与 {rty:?}", bin_op_reason(op)),
        node,
    ))
}

fn bin_op_reason(op: BinOp) -> &'static str {
    match op {
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
            "算术运算类型不匹配（数值，且向量维度须一致）"
        }
        BinOp::Gt | BinOp::Lt | BinOp::Ge | BinOp::Le => "比较运算只支持标量数值",
        BinOp::Eq | BinOp::Ne => "==/!= 只支持同型标量",
        BinOp::And | BinOp::Or => "&&/|| 需要布尔",
    }
}

pub(crate) fn swizzle_comps(prop: &str, n: usize) -> Option<[u8; 4]> {
    let mut comps = [0u8; 4];
    for (i, ch) in prop.chars().enumerate() {
        let idx: u8 = match ch {
            'x' | 'r' => 0,
            'y' | 'g' => 1,
            'z' | 'b' => 2,
            'w' | 'a' => 3,
            _ => return None,
        };
        if idx as usize >= n {
            return None;
        }
        comps[i] = idx;
    }
    Some(comps)
}

fn parse_call(ctx: &mut Ctx, call: &oxc_ast::ast::CallExpression) -> LambdaResult<ExprT> {
    // child.eval(pos)
    if let Expression::StaticMemberExpression(member) = &call.callee {
        if member.property.name == "eval" {
            if ctx.cur_fn.is_some()
                && let Expression::Identifier(id) = &member.object
                && ctx.params.contains_key(id.name.as_str())
            {
                return Err(ctx.err(
                    format!("函数体不能捕获 child `{}`：数据必须经形参传入", id.name),
                    call,
                ));
            }
            if let Expression::Identifier(id) = &member.object
                && let Some(Param::Child { index }) = ctx.params.get(id.name.as_str())
            {
                let index = *index;
                if call.arguments.len() != 1 {
                    return Err(ctx.err("eval 需要 1 个 float2 参数", call));
                }
                let Some(arg) = call.arguments[0].as_expression() else {
                    return Err(ctx.err("eval 参数不允许 spread", call));
                };
                let (pos, pty) = parse_expr(ctx, arg)?;
                if pty != Ty::Vec2 {
                    return Err(ctx.err(
                        format!("eval 参数需要 float2，得到 {pty:?}"),
                        arg,
                    ));
                }
                return Ok((
                    Expr::Eval { child: index, pos: Box::new(pos) },
                    Ty::Vec4,
                ));
            }
            return Err(ctx.err("`.eval` 只能由 child 参数调用", call));
        }
        if let Expression::Identifier(id) = &member.object
            && id.name == "Math"
        {
            let name = member.property.name.as_str();
            let Some(bid) = stdlib::math_member_id(name) else {
                return Err(ctx.err(format!("不支持 Math.{name}"), call));
            };
            return parse_builtin_call(ctx, bid, &call.arguments, call);
        }
        return Err(ctx.err("不支持的成员方法调用", call));
    }
    // 裸名调用
    if let Expression::Identifier(id) = &call.callee {
        let name = id.name.as_str();
        // 用户函数优先于 stdlib（预扫已拒绝与内建同名，这里只是顺序保证）。
        if let Some(&fi) = ctx.fn_names.get(name) {
            return parse_fn_call(ctx, fi, &call.arguments, call);
        }
        if ctx.cur_fn.is_some()
            && let Some(Param::Get) = ctx.params.get(name)
        {
            return Err(ctx.err(
                "函数体不能捕获 `get`：数据必须经形参传入",
                call,
            ));
        }
        if let Some(Param::Get) = ctx.params.get(name) {
            // get(dx, dy)：scan 类的 in-progress 采样
            if call.arguments.len() != 2 {
                return Err(ctx.err("get 需要 2 个标量参数 (dx, dy)", call));
            }
            let mut comps: [Option<Expr>; 2] = [None, None];
            for (i, arg) in call.arguments.iter().enumerate() {
                let Some(expr) = arg.as_expression() else {
                    return Err(ctx.err("get 参数不允许 spread", call));
                };
                let (e, ty) = parse_expr(ctx, expr)?;
                if !ty.is_scalar_num() {
                    return Err(ctx.err(
                        format!("get 参数需要标量数值，得到 {ty:?}"),
                        expr,
                    ));
                }
                comps[i] = Some(e);
            }
            let [dx, dy] = [comps[0].take(), comps[1].take()];
            return Ok((
                Expr::ScanGet {
                    dx: Box::new(dx.unwrap_or(Expr::LitF(0.0))),
                    dy: Box::new(dy.unwrap_or(Expr::LitF(0.0))),
                },
                Ty::Vec4,
            ));
        }
        let Some((bid, cap)) = stdlib::lookup(name) else {
            return Err(ctx.err(format!("未知函数 `{name}`"), call));
        };
        if cap == stdlib::Capability::CpuOnly {
            ctx.uses_cpu_only = true;
        }
        return parse_builtin_call(ctx, bid, &call.arguments, call);
    }
    Err(ctx.err("不支持的调用形式", call))
}

/// 用户函数调用。实参按声明序求值，类型逐个与推断出的形参类型核对。
fn parse_fn_call(
    ctx: &mut Ctx,
    index: usize,
    args: &[Argument],
    call: &impl GetSpan,
) -> LambdaResult<ExprT> {
    let sig = ctx.sigs[index].clone();
    let fname = ctx.fn_label(index);

    // 「先声明后使用」：调用点必须在被调函数的声明之后。互调必然违反此序，
    // 所以它同时排除了互递归（递归由 infer 层的调用图找环报错）。
    if !ctx.fn_done[index] {
        if ctx.cur_fn == Some(index) {
            return Err(ctx.err(
                format!("函数 `{fname}` 不能调用自身（不支持递归）"),
                call,
            ));
        }
        return Err(ctx.err(
            format!(
                "函数 `{fname}` 在调用点之后才声明：用户函数必须先声明后使用"
            ),
            call,
        ));
    }

    if args.len() != sig.params.len() {
        return Err(ctx.err(
            format!(
                "函数 `{fname}` 期望 {} 个参数，得到 {}",
                sig.params.len(),
                args.len()
            ),
            call,
        ));
    }
    let mut parsed = Vec::with_capacity(args.len());
    for (i, arg) in args.iter().enumerate() {
        let Some(expr) = arg.as_expression() else {
            return Err(ctx.err("参数不允许 spread", arg));
        };
        let (e, ty) = parse_expr(ctx, expr)?;
        let want = sig.params[i];
        if ty != want {
            return Err(ctx.err(
                format!(
                    "函数 `{fname}` 的第 {} 个参数期望 {want:?}，得到 {ty:?}",
                    i + 1
                ),
                expr,
            ));
        }
        parsed.push(e);
    }
    Ok((
        Expr::FnCall { func: index as u32, args: parsed, ty: sig.ret },
        sig.ret,
    ))
}

fn parse_builtin_call(
    ctx: &mut Ctx,
    id: B,
    args: &[Argument],
    call: &impl GetSpan,
) -> LambdaResult<ExprT> {
    let mut parsed = Vec::new();
    let mut tys = Vec::new();
    for arg in args {
        let Some(expr) = arg.as_expression() else {
            return Err(ctx.err("参数不允许 spread", arg));
        };
        let (e, ty) = parse_expr(ctx, expr)?;
        tys.push(ty);
        parsed.push(e);
    }
    let mut fail: Option<LambdaError> = None;
    let ret = stdlib::check_signature(id, &tys, &mut |msg| {
        if fail.is_none() {
            fail = Some(ctx.err(msg, call));
        }
    });
    let Some(ret) = ret else {
        return Err(fail.unwrap_or_else(|| ctx.err("参数不匹配", call)));
    };
    Ok((Expr::Call { id, args: parsed, ty: ret }, ret))
}
