//! oxc 解析 + 白名单校验 + 类型推断 -> [`Program`]。
//!
//! 原则：lambda 只被编译、从不被执行。JS 源码被转成受限 IR，白名单之外的
//! 结构一律以带源码位置的错误拒绝。允许的语法面：
//! - 参数（pixel 类首参 = `uv`；scan 类首参 = `get`；`rect`；`u`；其余按序
//!   为 child，仅 pixel 类）
//! - 数值/布尔/数组字面量；算术/比较/逻辑/三目；`if/else`；`let/const`
//! - stdlib 裸名调用与 `Math.*` 映射；向量 swizzle 读；`child.eval(pos)`；
//!   scan 类的 `get(dx, dy)`
//! - 语句级赋值（仅 let 变量）
//!
//! 拒绝：嵌套函数、循环、裸块、try/catch、对象/字符串/模板串、位运算、
//! `new`、`this`、`Math.random` 等破坏确定性的来源。

use std::collections::HashMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, BindingPattern, Expression, Statement};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

use super::error::{LambdaError, LambdaResult};
use super::program::{BinOp, Expr, Param, Program, Stmt, Ty, UnOp};
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
}

type ExprT = (Expr, Ty);

impl Ctx<'_> {
    fn err(&self, msg: impl Into<String>, node: &impl GetSpan) -> LambdaError {
        let span = node.span();
        LambdaError::at(msg, (span.start, span.end), self.source)
    }

    fn local_slot(
        &mut self,
        name: &str,
        ty: Ty,
        mutable: bool,
        node: &impl GetSpan,
    ) -> LambdaResult<u32> {
        if self.local_names.iter().any(|n| n == name) {
            return Err(self.err(format!("变量 `{name}` 重复声明"), node));
        }
        let slot = self.local_names.len() as u32;
        self.local_names.push(name.to_string());
        self.local_tys.push(ty);
        self.local_mutable.push(mutable);
        Ok(slot)
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

    let stmts = parse_statements(&mut ctx, fn_statements)?;

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
        return_ty,
        uses_cpu_only: ctx.uses_cpu_only,
        backend_override: None,
        kind: spec.kind,
    })
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
                let (expr, ty) = parse_expr(ctx, init)?;
                let mutable =
                    !matches!(decl.kind, oxc_ast::ast::VariableDeclarationKind::Const);
                let slot = ctx.local_slot(&name, ty, mutable, &declarator.id)?;
                out.push(Stmt::Let { slot, value: expr });
            }
            Ok(out)
        }
        Statement::ExpressionStatement(es) => match &es.expression {
            Expression::AssignmentExpression(assign) => {
                if assign.operator != oxc_ast::ast::AssignmentOperator::Assign {
                    return Err(ctx.err("仅支持普通赋值 `=`", stmt));
                }
                let name = assignment_target_name(ctx, &assign.left)?;
                let Some(slot) = ctx
                    .local_names
                    .iter()
                    .position(|n| n == name.as_str())
                    .map(|i| i as u32)
                else {
                    return Err(ctx.err(format!("对未声明变量 `{name}` 赋值"), stmt));
                };
                if !ctx.local_mutable[slot as usize] {
                    return Err(ctx.err(format!("`{name}` 是 const，不可重新赋值"), stmt));
                }
                let want = ctx.local_tys[slot as usize];
                let (expr, ty) = parse_expr(ctx, &assign.right)?;
                if ty != want {
                    return Err(ctx.err(
                        format!("赋值类型不匹配：`{name}` 是 {want:?}，右侧是 {ty:?}"),
                        stmt,
                    ));
                }
                Ok(vec![Stmt::Assign { slot, value: expr }])
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
        Statement::ReturnStatement(ret) => {
            let Some(arg) = &ret.argument else {
                return Err(ctx.err("return 必须带值（float/vec3/vec4）", stmt));
            };
            let (expr, ty) = parse_expr(ctx, arg)?;
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
        | Statement::IfStatement(_)) => parse_statement(ctx, single),
        other => Err(ctx.err("if 分支必须是代码块或单条语句", other)),
    }
}

fn statement_reject_reason(stmt: &Statement) -> String {
    match stmt {
        Statement::FunctionDeclaration(_) => "不支持函数声明（效果 lambda 是单个函数体）".into(),
        Statement::ForStatement(_)
        | Statement::ForInStatement(_)
        | Statement::ForOfStatement(_)
        | Statement::WhileStatement(_)
        | Statement::DoWhileStatement(_) => {
            "不支持循环（噪声请用 h01 等 hash 内建）".into()
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
                    if let Some(slot) = ctx
                        .local_names
                        .iter()
                        .position(|n| n == name)
                        .map(|i| i as u32)
                    {
                        return Ok((Expr::Local(slot), ctx.local_tys[slot as usize]));
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
        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => {
            Err(ctx.err("不支持嵌套函数", expr))
        }
        other => Err(ctx.err("不支持的表达式形式", other)),
    }
}

fn is_numeric(ty: Ty) -> bool {
    ty.is_scalar_num() || ty.vec_len().is_some()
}

fn vec_ty(n: usize) -> Ty {
    match n {
        2 => Ty::Vec2,
        3 => Ty::Vec3,
        _ => Ty::Vec4,
    }
}

fn bin_ty(ctx: &Ctx, op: BinOp, lty: Ty, rty: Ty, node: &impl GetSpan) -> LambdaResult<Ty> {
    match op {
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
            if !is_numeric(lty) || !is_numeric(rty) {
                return Err(ctx.err(format!("算术运算类型不匹配：{lty:?} 与 {rty:?}"), node));
            }
            match (lty.vec_len(), rty.vec_len()) {
                (None, None) => Ok(Ty::Float),
                (None, Some(n)) => Ok(vec_ty(n)),
                (Some(n), None) => Ok(vec_ty(n)),
                (Some(a), Some(b)) if a == b => Ok(vec_ty(a)),
                (Some(a), Some(b)) => {
                    Err(ctx.err(format!("向量维度不一致：{a} vs {b}"), node))
                }
            }
        }
        BinOp::Gt | BinOp::Lt | BinOp::Ge | BinOp::Le => {
            if lty == Ty::Float && rty == Ty::Float {
                Ok(Ty::Bool)
            } else {
                Err(ctx.err("比较运算只支持标量数值", node))
            }
        }
        BinOp::Eq | BinOp::Ne => {
            if lty == rty && (lty == Ty::Float || lty == Ty::Bool) {
                Ok(Ty::Bool)
            } else {
                Err(ctx.err("==/!= 只支持同型标量", node))
            }
        }
        BinOp::And | BinOp::Or => {
            if lty == Ty::Bool && rty == Ty::Bool {
                Ok(Ty::Bool)
            } else {
                Err(ctx.err("&&/|| 需要布尔", node))
            }
        }
    }
}

fn swizzle_comps(prop: &str, n: usize) -> Option<[u8; 4]> {
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
